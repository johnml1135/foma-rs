//! foma/lexcread.c — literal (bug-for-bug) Wave-2 port per
//! docs/port/rust-conventions.md. Sem rules: docs/spec/port/foma/lexcread.md
//! (per-file ids) and docs/spec/port/foma/lexc.md (lexc.h header ids).
//!
//! The lexc compiler builds a raw `struct states` graph (shared, cyclic,
//! heavily aliased through statelist/trans/merge_with/lexstate pointers),
//! then converts it to a struct fsm. Safe Rust cannot hold the C's raw
//! pointer graph, so — exactly as the fsm line table becomes index walks in
//! types.rs — every C pointer becomes an index into a `Vec` arena, and NULL
//! becomes `Option<usize>` / `None`. All file-static globals plus the arenas
//! live in a single `Lexc` context behind one thread_local `LEXC` (the C
//! non-reentrancy contract is preserved; the C names are kept on the fields).
//! The lexc.h public functions keep their C signatures and borrow LEXC once;
//! the file-static helpers take `&mut Lexc` so the whole compile threads one
//! borrow and never re-borrows the RefCell.

use std::cell::RefCell;

use crate::constructions::{add_fsm_arc, fsm_update_flags};
use crate::define::{G_DEFINES, add_defined};
use crate::determinize::fsm_determinize;
use crate::io::file_to_mem;
use crate::mem::{G_LEXC_ALIGN, G_VERBOSE};
use crate::minimize::fsm_minimize;
use crate::regex::fsm_parse_regex;
use crate::sigma::{
    sigma_add, sigma_add_special, sigma_cleanup, sigma_create, sigma_find, sigma_find_number,
    sigma_max, sigma_sort,
};
use crate::structures::{fsm_create, fsm_empty_set};
use crate::topsort::fsm_topsort;
use crate::types::{DefinedNetworks, EPSILON, Fsm, FsmState, IDENTITY, Sigma, UNK, UNKNOWN};
use crate::utf8::{utf8skip, utf8strlen};

const SIGMA_HASH_TABLESIZE: usize = 3079;

const WORD_ENTRY: i32 = 1;
const REGEX_ENTRY: i32 = 2;

/* ------------------------------------------------------------------ */
/* File-local struct types (declared inside lexcread.c)               */
/* ------------------------------------------------------------------ */

// [spec:foma:def:lexcread.multichar-symbols]
// C: struct multichar_symbols { char *symbol; short int sigma_number;
// struct multichar_symbols *next; }. `next` is an arena index (None ↔ NULL).
#[derive(Debug, Clone)]
struct MulticharSymbols {
    symbol: Option<String>,
    sigma_number: i16,
    next: Option<usize>,
}

// [spec:foma:def:lexcread.lexstates]
// C: struct lexstates — separate list of LEXICON states. `state` is a state
// arena index; `next` a lexstates arena index (None ↔ NULL).
#[derive(Debug, Clone)]
struct Lexstates {
    name: Option<String>,
    state: usize,
    next: Option<usize>,
    targeted: u8,
    has_outgoing: u8,
}

// [spec:foma:def:lexcread.states.trans]
// C: struct trans { short int in; short int out; struct states *target;
// struct trans *next; }. `target` is a state arena index; `next` a trans
// arena index (None ↔ NULL).
#[derive(Debug, Clone)]
struct Trans {
    r#in: i16,
    out: i16,
    target: usize,
    next: Option<usize>,
}

// [spec:foma:def:lexcread.states]
// C: struct states { struct trans *trans; struct lexstates *lexstate;
// int number; unsigned int hashval; unsigned char mergeable;
// unsigned short int distance; struct states *merge_with; }.
// `trans` and `lexstate` are Option<arena index>; `merge_with` a state arena
// index (initialised to self).
#[derive(Debug, Clone)]
struct States {
    trans: Option<usize>,
    lexstate: Option<usize>,
    number: i32,
    hashval: u32,
    mergeable: u8,
    distance: u16,
    merge_with: usize,
}

// [spec:foma:def:lexcread.statelist]
// C: struct statelist { struct states *state; struct statelist *next;
// char start; char final; }. `state` is a state arena index; `next` a
// statelist arena index (None ↔ NULL).
#[derive(Debug, Clone)]
struct Statelist {
    state: usize,
    next: Option<usize>,
    start: i8,
    r#final: i8,
}

// [spec:foma:def:lexcread.lexc-hashtable]
// C: struct lexc_hashtable { char *symbol; struct lexc_hashtable *next;
// int sigma_number; }. Hash for looking up symbols in sigma quickly. The
// 3079 bucket heads live in a Vec; collision chains are owned Box nodes.
#[derive(Debug, Clone)]
struct LexcHashtable {
    symbol: Option<String>,
    next: Option<Box<LexcHashtable>>,
    sigma_number: i32,
}

/* C: static unsigned int primes[26] = { ... }; */
static PRIMES: [u32; 26] = [
    61, 127, 251, 509, 1021, 2039, 4093, 8191, 16381, 32749, 65521, 131071, 262139, 524287,
    1048573, 2097143, 4194301, 8388593, 16777213, 33554393, 67108859, 134217689, 268435399,
    536870909, 1073741789, 2147483647,
];

/* ------------------------------------------------------------------ */
/* File-static globals, grouped into one non-reentrant thread_local    */
/* ------------------------------------------------------------------ */

/// Holds every lexcread.c file-static (the C names are kept on the fields)
/// plus the arenas that back the C pointer graph. Pointer fields are arena
/// indices; NULL is `None` / an empty arena.
struct Lexc {
    /* arenas backing the C `struct *` graph (never reclaimed mid-compile;
    the C frees are no-ops here — see the DEVIATION notes at each free site) */
    state_arena: Vec<States>,
    trans_arena: Vec<Trans>,
    statelist_arena: Vec<Statelist>,
    lexstates_arena: Vec<Lexstates>,
    mc_arena: Vec<MulticharSymbols>,

    /* C: static struct statelist *statelist / *mc / *lexstates — list heads */
    statelist: Option<usize>,
    mc: Option<usize>,
    lexstates: Option<usize>,

    /* C: static struct sigma *lexsigma */
    lexsigma: Option<Box<Sigma>>,
    /* C: static struct lexc_hashtable *hashtable — 3079 calloc'd bucket heads */
    hashtable: Vec<LexcHashtable>,
    /* C: static struct fsm *current_regex_network */
    current_regex_network: Option<Box<Fsm>>,

    /* C: static int cwordin[1000], cwordout[1000], medcwordin[2000],
    medcwordout[2000] — fixed sizes kept; an over-long entry side overflows
    them (DEVIATION: C corrupts adjacent memory, Rust panics on OOB index) */
    cwordin: [i32; 1000],
    cwordout: [i32; 1000],
    medcwordin: [i32; 2000],
    medcwordout: [i32; 2000],
    /* C: static _Bool *mchash — calloc(256*256) two-byte-prefix filter */
    mchash: Vec<bool>,

    /* C scalar file-statics */
    carity: i32,
    lexc_statecount: i32,
    maxlen: i32,
    hasfinal: i32,
    current_entry: i32,
    net_has_unknown: i32,

    /* C: static struct lexstates *clexicon, *ctarget — lexstates indices */
    clexicon: Option<usize>,
    ctarget: Option<usize>,
}

impl Lexc {
    const fn new_empty() -> Lexc {
        Lexc {
            state_arena: Vec::new(),
            trans_arena: Vec::new(),
            statelist_arena: Vec::new(),
            lexstates_arena: Vec::new(),
            mc_arena: Vec::new(),
            statelist: None,
            mc: None,
            lexstates: None,
            lexsigma: None,
            hashtable: Vec::new(),
            current_regex_network: None,
            cwordin: [0; 1000],
            cwordout: [0; 1000],
            medcwordin: [0; 2000],
            medcwordout: [0; 2000],
            mchash: Vec::new(),
            carity: 0,
            lexc_statecount: 0,
            maxlen: 0,
            hasfinal: 0,
            current_entry: 0,
            net_has_unknown: 0,
            clexicon: None,
            ctarget: None,
        }
    }
}

thread_local! {
    static LEXC: RefCell<Lexc> = const { RefCell::new(Lexc::new_empty()) };
}

/* ------------------------------------------------------------------ */
/* C-string helpers over NUL-terminated byte buffers                   */
/* ------------------------------------------------------------------ */

/// strlen: byte offset of the first NUL in `buf` (buf.len() if none — the C
/// callers always supply a NUL-terminated buffer).
fn cstrlen(buf: &[u8]) -> usize {
    buf.iter().position(|&b| b == 0).unwrap_or(buf.len())
}

/// strdup of the NUL-terminated content of `buf`.
/// DEVIATION from C (symbols are stored as String; malformed UTF-8 — only
/// reachable on the invalid-input infinite-loop path — is lossy-decoded).
fn cstrdup(buf: &[u8]) -> String {
    let n = cstrlen(buf);
    String::from_utf8_lossy(&buf[..n]).into_owned()
}

/// The NUL-terminated content bytes of `buf` (up to the first NUL).
fn cstr(buf: &[u8]) -> &[u8] {
    &buf[..cstrlen(buf)]
}

/// strcmp(stored_symbol, buf) == 0 — compares a stored String against the
/// NUL-terminated content of `buf`.
fn sym_eq(sym: &Option<String>, buf: &[u8]) -> bool {
    match sym {
        Some(s) => s.as_bytes() == cstr(buf),
        None => false,
    }
}

/* ------------------------------------------------------------------ */
/* Hashing                                                             */
/* ------------------------------------------------------------------ */

// [spec:foma:def:lexcread.lexc-suffix-hash-fn]
// [spec:foma:sem:lexcread.lexc-suffix-hash-fn]
fn lexc_suffix_hash(lx: &Lexc, offset: i32) -> u32 {
    let mut h: u32 = 0;
    let mut g: u32;
    /* Read suffixes in cwordin[] and cwordout[] and return a hash value */
    let mut p = offset as usize;
    while lx.cwordin[p] != -1 {
        h = (h << 4).wrapping_add((lx.cwordin[p] | (lx.cwordout[p] << 8)) as u32);
        g = h & 0xf000_0000;
        if g != 0 {
            h ^= g >> 24;
            h ^= g;
        }
        p += 1;
    }
    /* No tablemod here, we decide on the table size later */
    h
}

// [spec:foma:def:lexcread.lexc-symbol-hash-fn]
// [spec:foma:sem:lexcread.lexc-symbol-hash-fn]
fn lexc_symbol_hash(s: &[u8]) -> u32 {
    let mut hash: u32 = 5381;
    /* while ((c = *s++)) — signed char sign-extension into int c, per the
    conventions (bytes >= 0x80 add a wrapped large value) */
    for &b in cstr(s) {
        let c = b as i8 as i32;
        hash = ((hash << 5).wrapping_add(hash)).wrapping_add(c as u32);
    }
    hash % SIGMA_HASH_TABLESIZE as u32
}

// [spec:foma:def:lexcread.lexc-find-sigma-hash-fn]
// [spec:foma:sem:lexcread.lexc-find-sigma-hash-fn]
fn lexc_find_sigma_hash(lx: &Lexc, symbol: &[u8]) -> i32 {
    let ptr = lexc_symbol_hash(symbol) as usize;

    if lx.hashtable[ptr].symbol.is_none() {
        return -1;
    }
    /* for (h = head; h != NULL; h = h->next) */
    if sym_eq(&lx.hashtable[ptr].symbol, symbol) {
        return lx.hashtable[ptr].sigma_number;
    }
    let mut h = lx.hashtable[ptr].next.as_deref();
    while let Some(node) = h {
        if sym_eq(&node.symbol, symbol) {
            return node.sigma_number;
        }
        h = node.next.as_deref();
    }
    -1
}

// [spec:foma:def:lexcread.lexc-add-sigma-hash-fn]
// [spec:foma:sem:lexcread.lexc-add-sigma-hash-fn]
fn lexc_add_sigma_hash(lx: &mut Lexc, symbol: &[u8], number: i32) {
    let ptr = lexc_symbol_hash(symbol) as usize;

    if lx.net_has_unknown == 1 {
        lexc_update_unknowns(lx, number);
    }

    if lx.hashtable[ptr].symbol.is_none() {
        lx.hashtable[ptr].symbol = Some(cstrdup(symbol));
        lx.hashtable[ptr].sigma_number = number;
        return;
    }
    /* for (h = head; h->next != NULL; h = h->next) {} — walk to the tail */
    let mut tail_next = &mut lx.hashtable[ptr].next;
    while tail_next.is_some() {
        tail_next = &mut tail_next.as_mut().unwrap().next;
    }
    *tail_next = Some(Box::new(LexcHashtable {
        symbol: Some(cstrdup(symbol)),
        sigma_number: number,
        next: None,
    }));
}

// [spec:foma:def:lexcread.lexc-init-fn]
// [spec:foma:sem:lexcread.lexc-init-fn]
// [spec:foma:def:lexc.lexc-init-fn]
// [spec:foma:sem:lexc.lexc-init-fn]
pub fn lexc_init() {
    LEXC.with_borrow_mut(|lx| {
        lx.lexsigma = Some(sigma_create());
        lx.mc = None;
        lx.lexstates = None;
        lx.clexicon = None;
        lx.ctarget = None;
        lx.statelist = None;
        lx.lexc_statecount = 0;
        lx.net_has_unknown = 0;
        lexc_clear_current_word_impl(lx);
        /* calloc(SIGMA_HASH_TABLESIZE) then the loop below sets each bucket
        head to {symbol=NULL, sigma_number=-1, next=NULL} */
        lx.hashtable = vec![
            LexcHashtable {
                symbol: None,
                next: None,
                sigma_number: -1,
            };
            SIGMA_HASH_TABLESIZE
        ];

        lx.maxlen = 0;

        lx.mchash = vec![false; 256 * 256];
        /* Does not free structures from a previous run (that is
        lexc_cleanup's job); calling lexc_init twice without an intervening
        cleanup leaks the old tables — reproduced by not clearing the arenas
        here (they are cleared in lexc_cleanup). current_regex_network is not
        reset. */
    });
}

// [spec:foma:def:lexcread.lexc-clear-current-word-fn]
// [spec:foma:sem:lexcread.lexc-clear-current-word-fn]
// [spec:foma:def:lexc.lexc-clear-current-word-fn]
// [spec:foma:sem:lexc.lexc-clear-current-word-fn]
pub fn lexc_clear_current_word() {
    LEXC.with_borrow_mut(|lx| lexc_clear_current_word_impl(lx));
}

fn lexc_clear_current_word_impl(lx: &mut Lexc) {
    lx.cwordin[0] = 0;
    lx.cwordout[0] = 0;
    lx.cwordin[1] = -1;
    lx.cwordout[1] = -1;
    lx.current_entry = WORD_ENTRY;
}

// [spec:foma:def:lexcread.lexc-add-state-fn]
// [spec:foma:sem:lexcread.lexc-add-state-fn]
fn lexc_add_state(lx: &mut Lexc, s: usize) {
    /* sl = malloc(struct statelist); sl->state = s; s->number = -1;
    sl->next = statelist; sl->start = 0; sl->final = 0; statelist = sl; */
    let slidx = lx.statelist_arena.len();
    lx.statelist_arena.push(Statelist {
        state: s,
        next: lx.statelist,
        start: 0,
        r#final: 0,
    });
    lx.state_arena[s].number = -1;
    lx.statelist = Some(slidx);
    lx.lexc_statecount += 1;
}

/* Go through the net built so far and add new transitions for @ */
/* to reflect the new symbols we now have in sigma */

// [spec:foma:def:lexcread.lexc-update-unknowns-fn]
// [spec:foma:sem:lexcread.lexc-update-unknowns-fn]
fn lexc_update_unknowns(lx: &mut Lexc, sigma_number: i32) {
    /* for (s = statelist; s != NULL; s = s->next) */
    let mut s = lx.statelist;
    while let Some(sidx) = s {
        let stateidx = lx.statelist_arena[sidx].state;
        if lx.state_arena[stateidx].mergeable != 2 {
            /* for (t = s->state->trans; t != NULL; t = t->next) */
            let mut t = lx.state_arena[stateidx].trans;
            while let Some(tidx) = t {
                if lx.trans_arena[tidx].r#in as i32 == IDENTITY
                    || lx.trans_arena[tidx].out as i32 == IDENTITY
                {
                    let target = lx.trans_arena[tidx].target;
                    let tnext = lx.trans_arena[tidx].next;
                    /* newtrans->next = t->next; t->next = newtrans */
                    let newidx = lx.trans_arena.len();
                    lx.trans_arena.push(Trans {
                        r#in: sigma_number as i16,
                        out: sigma_number as i16,
                        target,
                        next: tnext,
                    });
                    lx.trans_arena[tidx].next = Some(newidx);
                }
                /* t = t->next: after an insertion this visits the new arc next
                (labels are the new symbol, not IDENTITY, so no recursion) */
                t = lx.trans_arena[tidx].next;
            }
        }
        s = lx.statelist_arena[sidx].next;
    }
}

// [spec:foma:def:lexcread.lexc-add-network-fn]
// [spec:foma:sem:lexcread.lexc-add-network-fn]
fn lexc_add_network(lx: &mut Lexc) {
    let mut unknown_symbols = 0;
    let mut first_new_sigma = 0;
    let sourcestate = lx.lexstates_arena[lx.clexicon.unwrap()].state;
    let deststate = lx.lexstates_arena[lx.ctarget.unwrap()].state;

    /* net = current_regex_network; taken out so the &mut lx calls below do not
    conflict with reading net->states / net->sigma. Put back at the end (C
    never frees it and leaves current_regex_network pointing at the mutated
    net). */
    let mut net = lx.current_regex_network.take().unwrap();

    /* sigreplace = calloc(sigma_max(net->sigma)+1, sizeof(int)) */
    let mut sigreplace: Vec<i32> = vec![0; (sigma_max(net.sigma.as_deref()) + 1) as usize];

    /* for (sigma = net->sigma; sigma != NULL && sigma->number != -1; ...) */
    {
        let mut node = net.sigma.as_deref();
        while let Some(s) = node {
            if s.number == -1 {
                break;
            }
            let sym = s.symbol.as_deref().unwrap_or("");
            let symbytes = sym.as_bytes();
            let signumber = lexc_find_sigma_hash(lx, symbytes);
            if signumber == -1 {
                /* Add to existing lexc sigma */
                let signumber = sigma_add(sym, lx.lexsigma.as_deref_mut().unwrap());
                first_new_sigma = if first_new_sigma > 0 {
                    first_new_sigma
                } else {
                    signumber
                };
                lexc_add_sigma_hash(lx, symbytes, signumber);
                sigreplace[s.number as usize] = signumber;
            } else {
                /* We already have it, add to conversion table */
                sigreplace[s.number as usize] = signumber;
            }
            node = s.next.as_deref();
        }
    }

    /* Renum arcs — net->states is mutated in place */
    let mut maxstate = 0i32;
    {
        let fsm = &mut net.states;
        let mut i = 0usize;
        while fsm[i].state_no != -1 {
            if fsm[i].r#in != -1 {
                fsm[i].r#in = sigreplace[fsm[i].r#in as usize] as i16;
            }
            if fsm[i].out != -1 {
                fsm[i].out = sigreplace[fsm[i].out as usize] as i16;
            }
            if fsm[i].state_no > maxstate {
                maxstate = fsm[i].state_no;
            }
            if fsm[i].r#in as i32 == IDENTITY
                || fsm[i].r#in as i32 == UNKNOWN
                || fsm[i].out as i32 == UNKNOWN
            {
                unknown_symbols = 1;
            }
            i += 1;
        }
    }

    /* unk: 0-terminated list of concrete lexsigma symbols absent from net */
    let mut unk: Vec<i32> = Vec::new();
    if unknown_symbols == 1 {
        unk = vec![0; (sigma_max(lx.lexsigma.as_deref()) + 2) as usize];
        let mut i = 0usize;
        let mut node = lx.lexsigma.as_deref();
        while let Some(s) = node {
            if s.number == -1 {
                break;
            }
            if s.number > 2 && sigma_find(s.symbol.as_deref().unwrap_or(""), net.sigma.as_deref()) == -1
            {
                unk[i] = s.number;
                i += 1;
            }
            node = s.next.as_deref();
        }
    }

    /* slist[state_no] -> fresh state index; finals[state_no] -> final flag */
    let mut slist: Vec<usize> = vec![0; (maxstate + 1) as usize];
    let mut finals: Vec<i32> = vec![0; (maxstate + 1) as usize];

    for i in 0..=(maxstate as usize) {
        let newidx = lx.state_arena.len();
        lx.state_arena.push(States {
            trans: None,
            lexstate: None,
            number: -1,
            hashval: u32::MAX, /* C: newstate->hashval = -1 (unsigned int) */
            mergeable: 0,
            distance: 0,
            merge_with: 0, /* set to self below */
        });
        lx.state_arena[newidx].merge_with = newidx;
        slist[i] = newidx;
        /* Prepend a statelist cell directly (NOT via lexc_add_state, so
        lexc_statecount is not bumped — harmless; recomputed later) */
        let slidx = lx.statelist_arena.len();
        lx.statelist_arena.push(Statelist {
            state: newidx,
            next: lx.statelist,
            start: 0,
            r#final: 0,
        });
        lx.statelist = Some(slidx);
    }

    /* Add an EPSILON transition from sourcestate to state 0 */
    {
        let newtrans = lx.trans_arena.len();
        lx.trans_arena.push(Trans {
            r#in: EPSILON as i16,
            out: EPSILON as i16,
            target: slist[0],
            next: lx.state_arena[sourcestate].trans,
        });
        lx.state_arena[sourcestate].trans = Some(newtrans);
    }

    {
        let mut i = 0usize;
        while net.states[i].state_no != -1 {
            if net.states[i].target != -1 {
                let newstate = slist[net.states[i].state_no as usize];
                let newtrans = lx.trans_arena.len();
                lx.trans_arena.push(Trans {
                    r#in: net.states[i].r#in,
                    out: net.states[i].out,
                    target: slist[net.states[i].target as usize],
                    next: lx.state_arena[newstate].trans,
                });
                lx.state_arena[newstate].trans = Some(newtrans);
                /* Add new symbols for @:@ transitions */
                /* TODO: make this work for ?: or :? trans as well */
                if unknown_symbols == 1
                    && (net.states[i].r#in as i32 == IDENTITY
                        || net.states[i].out as i32 == IDENTITY)
                {
                    let mut j = 0usize;
                    while unk[j] != 0 {
                        let nt = lx.trans_arena.len();
                        lx.trans_arena.push(Trans {
                            r#in: unk[j] as i16,
                            out: unk[j] as i16,
                            target: slist[net.states[i].target as usize],
                            next: lx.state_arena[newstate].trans,
                        });
                        lx.state_arena[newstate].trans = Some(nt);
                        j += 1;
                    }
                }
            }
            finals[net.states[i].state_no as usize] = net.states[i].final_state as i32;
            i += 1;
        }
    }

    /* Add an EPSILON transition from all final states to deststate */
    for i in 0..=(maxstate as usize) {
        if finals[i] == 1 {
            let newstate = slist[i];
            let nt = lx.trans_arena.len();
            lx.trans_arena.push(Trans {
                r#in: EPSILON as i16,
                out: EPSILON as i16,
                target: deststate,
                next: lx.state_arena[newstate].trans,
            });
            lx.state_arena[newstate].trans = Some(nt);
        }
    }

    if unknown_symbols == 1 {
        /* free(unk) — no-op (local Vec) */
        lx.net_has_unknown = 1;
    }
    /* free(slist); free(finals) — no-op. sigreplace is never freed in C
    (leak); the local Vecs drop here. */
    lx.current_regex_network = Some(net);
    let _ = first_new_sigma; /* recorded but never read (dead code in C) */
}

// [spec:foma:def:lexcread.lexc-set-network-fn]
// [spec:foma:sem:lexcread.lexc-set-network-fn]
// [spec:foma:def:lexc.lexc-set-network-fn]
// [spec:foma:sem:lexc.lexc-set-network-fn]
pub fn lexc_set_network(net: Box<Fsm>) {
    LEXC.with_borrow_mut(|lx| {
        lx.current_regex_network = Some(net);
        lx.current_entry = REGEX_ENTRY;
    });
}

// [spec:foma:def:lexcread.lexc-set-current-lexicon-fn]
// [spec:foma:sem:lexcread.lexc-set-current-lexicon-fn]
// [spec:foma:def:lexc.lexc-set-current-lexicon-fn]
// [spec:foma:sem:lexc.lexc-set-current-lexicon-fn]
pub fn lexc_set_current_lexicon(name: &[u8], which: i32) {
    /* Sets the global lexicon variable to point to a new lexicon */
    /* which == 0 indicates source, which == 1 indicates target */
    LEXC.with_borrow_mut(|lx| {
        let mut l = lx.lexstates;
        while let Some(lidx) = l {
            if sym_eq(&lx.lexstates_arena[lidx].name, name) {
                if which == 0 {
                    lx.lexstates_arena[lidx].has_outgoing = 1;
                    lx.clexicon = Some(lidx);
                } else {
                    lx.ctarget = Some(lidx);
                }
                return;
            }
            l = lx.lexstates_arena[lidx].next;
        }
        let lidx = lx.lexstates_arena.len();
        lx.lexstates_arena.push(Lexstates {
            name: Some(cstrdup(name)),
            /* state assigned below after lexc_add_state */
            state: 0,
            next: lx.lexstates,
            has_outgoing: 0,
            targeted: 0,
        });
        lx.lexstates = Some(lidx);
        let newidx = lx.state_arena.len();
        lx.state_arena.push(States {
            trans: None,
            lexstate: None,
            number: -1,
            /* C leaves hashval and distance uninitialised — never read while
            mergeable == 0; zeroed here */
            hashval: 0,
            mergeable: 0,
            distance: 0,
            merge_with: 0,
        });
        lexc_add_state(lx, newidx);
        lx.state_arena[newidx].lexstate = Some(lidx);
        lx.state_arena[newidx].trans = None;
        lx.state_arena[newidx].mergeable = 0;
        lx.state_arena[newidx].merge_with = newidx;
        lx.lexstates_arena[lidx].state = newidx;
        if which == 0 {
            lx.clexicon = Some(lidx);
            lx.lexstates_arena[lidx].has_outgoing = 1;
        } else {
            lx.ctarget = Some(lidx);
        }
    });
}

// [spec:foma:def:lexcread.lexc-find-delim-fn]
// [spec:foma:sem:lexcread.lexc-find-delim-fn]
fn lexc_find_delim(name: &[u8], delimiter: u8, escape: u8) -> Option<usize> {
    let mut i = 0usize;
    while name[i] != 0 {
        if name[i] == escape && name[i + 1] != 0 {
            i += 1; /* body i++ */
            i += 1; /* for-loop i++ on continue */
            continue;
        }
        if name[i] == delimiter {
            return Some(i);
        }
        i += 1; /* for-loop i++ */
    }
    None
}

// [spec:foma:def:lexcread.lexc-deescape-string-fn]
// [spec:foma:sem:lexcread.lexc-deescape-string-fn]
fn lexc_deescape_string(name: &mut [u8], escape: u8, mode: i32) {
    let mut i = 0usize;
    let mut j = 0usize;
    while name[i] != 0 {
        name[j] = name[i];
        if name[i] == escape {
            name[j] = name[i + 1];
            j += 1;
            i += 1; /* body i++ */
            i += 1; /* for-loop i++ */
            continue;
        } else if mode == 1 && name[i] == b'0' {
            /* Marks alignment EPSILON */
            name[j] = 0xff;
            j += 1;
            i += 1; /* for-loop i++ */
            continue;
        } else if name[i] != escape && name[i] != b'0' {
            j += 1;
            i += 1; /* for-loop i++ */
            continue;
        }
        /* char == '0' && mode != 1: no branch taken, j not advanced (the '0'
        is silently deleted) */
        i += 1; /* for-loop i++ */
    }
    name[j] = 0;
}

/* Read a string and fill cwordin, cwordout arrays */
/* with the sigma numbers of the current word, -1 terminated */

// [spec:foma:def:lexcread.lexc-set-current-word-fn]
// [spec:foma:sem:lexcread.lexc-set-current-word-fn]
// [spec:foma:def:lexc.lexc-set-current-word-fn]
// [spec:foma:sem:lexc.lexc-set-current-word-fn]
pub fn lexc_set_current_word(name: &mut [u8]) {
    LEXC.with_borrow_mut(|lx| {
        lx.carity = 1;
        /* instring = name; outstring = lexc_find_delim(name, ':', '%') */
        let outstring = lexc_find_delim(name, b':', b'%');
        let mut out_off = 0usize;
        if let Some(colon) = outstring {
            name[colon] = 0;
            out_off = colon + 1;
            lexc_deescape_string(&mut name[out_off..], b'%', 1);
            lx.carity = 2;
        }
        lexc_deescape_string(&mut name[..], b'%', 1);

        /* lexc_string_to_tokens(instring, cwordin) — cwordin copied out so it
        can be a &mut param disjoint from &mut lx */
        let mut intarr = lx.cwordin;
        lexc_string_to_tokens(lx, &name[..], &mut intarr);
        lx.cwordin = intarr;

        if lx.carity == 2 {
            let mut intarr = lx.cwordout;
            lexc_string_to_tokens(lx, &name[out_off..], &mut intarr);
            lx.cwordout = intarr;
            if G_LEXC_ALIGN.with(|v| v.get()) != 0 {
                lexc_medpad(lx);
            } else {
                lexc_pad(lx);
            }
        } else {
            let mut i = 0usize;
            while lx.cwordin[i] != -1 {
                lx.cwordout[i] = lx.cwordin[i];
                i += 1;
            }
            lx.cwordout[i] = -1;
        }
        lx.current_entry = WORD_ENTRY;
    });
}

const LEV_DOWN: i32 = 0;
const LEV_LEFT: i32 = 1;
const LEV_DIAG: i32 = 2;

// [spec:foma:def:lexcread.lexc-medpad-fn]
// [spec:foma:sem:lexcread.lexc-medpad-fn]
fn lexc_medpad(lx: &mut Lexc) {
    if lx.cwordin[0] == -1 && lx.cwordout[0] == -1 {
        lx.cwordin[0] = EPSILON;
        lx.cwordout[0] = EPSILON;
        lx.cwordin[1] = -1;
        lx.cwordout[1] = -1;
        return;
    }

    /* compact cwordin, deleting every EPSILON token */
    {
        let mut i = 0usize;
        let mut j = 0usize;
        while lx.cwordin[i] != -1 {
            if lx.cwordin[i] == EPSILON {
                i += 1;
                continue;
            }
            lx.cwordin[j] = lx.cwordin[i];
            j += 1;
            i += 1;
        }
        lx.cwordin[j] = -1;
    }
    /* compact cwordout */
    {
        let mut i = 0usize;
        let mut j = 0usize;
        while lx.cwordout[i] != -1 {
            if lx.cwordout[i] == EPSILON {
                i += 1;
                continue;
            }
            lx.cwordout[j] = lx.cwordout[i];
            j += 1;
            i += 1;
        }
        lx.cwordout[j] = -1;
    }

    let mut s1len = 0usize;
    while lx.cwordin[s1len] != -1 {
        s1len += 1;
    }
    let mut s2len = 0usize;
    while lx.cwordout[s2len] != -1 {
        s2len += 1;
    }

    /* calloc (s1len+2) x (s2len+2) int matrices */
    let mut matrix: Vec<Vec<i32>> = vec![vec![0i32; s2len + 2]; s1len + 2];
    let mut dirmatrix: Vec<Vec<i32>> = vec![vec![0i32; s2len + 2]; s1len + 2];

    matrix[0][0] = 0;
    dirmatrix[0][0] = 0;
    for x in 1..=s1len {
        matrix[x][0] = matrix[x - 1][0] + 1;
        dirmatrix[x][0] = LEV_LEFT;
    }
    for y in 1..=s2len {
        matrix[0][y] = matrix[0][y - 1] + 1;
        dirmatrix[0][y] = LEV_DOWN;
    }
    for x in 1..=s1len {
        for y in 1..=s2len {
            let diag = matrix[x - 1][y - 1]
                + if lx.cwordin[x - 1] == lx.cwordout[y - 1] {
                    0
                } else {
                    100
                };
            let down = matrix[x][y - 1] + 1;
            let left = matrix[x - 1][y] + 1;
            if diag <= left && diag <= down {
                matrix[x][y] = diag;
                dirmatrix[x][y] = LEV_DIAG;
            } else if left <= diag && left <= down {
                matrix[x][y] = left;
                dirmatrix[x][y] = LEV_LEFT;
            } else {
                matrix[x][y] = down;
                dirmatrix[x][y] = LEV_DOWN;
            }
        }
    }

    let mut x = s1len;
    let mut y = s2len;
    let mut i = 0usize;
    while x > 0 || y > 0 {
        let dir = dirmatrix[x][y];
        if dir == LEV_DIAG {
            lx.medcwordin[i] = lx.cwordin[x - 1];
            lx.medcwordout[i] = lx.cwordout[y - 1];
            x -= 1;
            y -= 1;
        } else if dir == LEV_DOWN {
            lx.medcwordin[i] = EPSILON;
            lx.medcwordout[i] = lx.cwordout[y - 1];
            y -= 1;
        } else {
            lx.medcwordin[i] = lx.cwordin[x - 1];
            lx.medcwordout[i] = EPSILON;
            x -= 1;
        }
        i += 1;
    }
    /* for (j = 0, i -= 1; i >= 0; j++, i--) — copy the reversed scratch back */
    let mut j = 0usize;
    let mut k: isize = i as isize - 1;
    while k >= 0 {
        lx.cwordin[j] = lx.medcwordin[k as usize];
        lx.cwordout[j] = lx.medcwordout[k as usize];
        j += 1;
        k -= 1;
    }
    lx.cwordin[j] = -1;
    lx.cwordout[j] = -1;
    /* free matrices — Vecs drop here */
}

// [spec:foma:def:lexcread.lexc-pad-fn]
// [spec:foma:sem:lexcread.lexc-pad-fn]
fn lexc_pad(lx: &mut Lexc) {
    /* Pad the shorter of current in, out words with EPSILON */
    if lx.cwordin[0] == -1 && lx.cwordout[0] == -1 {
        lx.cwordin[0] = EPSILON;
        lx.cwordout[0] = EPSILON;
        lx.cwordin[1] = -1;
        lx.cwordout[1] = -1;
        return;
    }

    let mut i = 0usize;
    let mut pad = 0;
    loop {
        if pad == 1 && lx.cwordout[i] == -1 {
            lx.cwordin[i] = -1;
            break;
        }
        if pad == 2 && lx.cwordin[i] == -1 {
            lx.cwordout[i] = -1;
            break;
        }
        if lx.cwordin[i] == -1 && lx.cwordout[i] != -1 {
            pad = 1; /* Pad upper */
        } else if lx.cwordin[i] != -1 && lx.cwordout[i] == -1 {
            pad = 2; /* Pad lower */
        }
        if pad == 1 {
            lx.cwordin[i] = EPSILON;
        }
        if pad == 2 {
            lx.cwordout[i] = EPSILON;
        }
        if pad == 0 && lx.cwordin[i] == -1 {
            break;
        }
        i += 1;
    }
}

// [spec:foma:def:lexcread.lexc-string-to-tokens-fn]
// [spec:foma:sem:lexcread.lexc-string-to-tokens-fn]
fn lexc_string_to_tokens(lx: &mut Lexc, string: &[u8], intarr: &mut [i32; 1000]) {
    let len = cstrlen(string) as i32;
    let mut tmpstring = [0u8; 5];
    let mut i = 0i32;
    let mut pos = 0usize;
    while i < len {
        /* EPSILON for alignment is marked as 0xff */
        if string[i as usize] == 0xff {
            /* DEVIATION from C (intarr is the fixed 1000-int cwordin/cwordout;
            an over-long entry side — including the malformed-UTF-8 infinite
            loop below — panics on OOB index where C corrupts memory) */
            intarr[pos] = EPSILON;
            pos += 1;
            i += 1;
            continue;
        }

        let mut multi = 0;
        let mut mcs_idx: Option<usize> = None;
        let b0 = string[i as usize] as usize;
        let b1 = if ((i + 1) as usize) < string.len() {
            string[(i + 1) as usize] as usize
        } else {
            0
        };
        let mchashval = b0 * 256 + b1;
        if i < len - 1 && lx.mchash[mchashval] {
            let mut mcs = lx.mc;
            while let Some(m) = mcs {
                let sym = lx.mc_arena[m].symbol.as_deref().unwrap().as_bytes();
                if string[i as usize..].starts_with(sym) {
                    multi = 1;
                    mcs_idx = Some(m);
                    break;
                }
                mcs = lx.mc_arena[m].next;
            }
        }

        if multi == 1 {
            let m = mcs_idx.unwrap();
            intarr[pos] = lx.mc_arena[m].sigma_number as i32;
            pos += 1;
            i += lx.mc_arena[m].symbol.as_deref().unwrap().len() as i32;
        } else {
            let skip = utf8skip(&string[i as usize..]);
            mystrncpy(&mut tmpstring, &string[i as usize..], skip + 1);
            let signumber = lexc_find_sigma_hash(lx, &tmpstring);
            if signumber != -1 {
                intarr[pos] = signumber;
                pos += 1;
                i = i + skip + 1;
            } else {
                mystrncpy(&mut tmpstring, &string[i as usize..], skip + 1);
                let signumber = sigma_add(&cstrdup(&tmpstring), lx.lexsigma.as_deref_mut().unwrap());
                lexc_add_sigma_hash(lx, &tmpstring, signumber);
                intarr[pos] = signumber;
                pos += 1;
                i = i + skip + 1;
            }
        }
    }
    intarr[pos] = -1;
}

// [spec:foma:def:lexcread.mystrncpy-fn]
// [spec:foma:sem:lexcread.mystrncpy-fn]
fn mystrncpy(dest: &mut [u8], src: &[u8], len: i32) {
    let mut i = 0i32;
    while i < len {
        dest[i as usize] = src[i as usize];
        if src[i as usize] == 0 {
            return;
        }
        i += 1;
    }
    dest[i as usize] = 0;
}

/* Add MC to front of chain */
/* In decreasing order of length */

// [spec:foma:def:lexcread.lexc-add-mc-fn]
// [spec:foma:sem:lexcread.lexc-add-mc-fn]
// [spec:foma:def:lexc.lexc-add-mc-fn]
// [spec:foma:sem:lexc.lexc-add-mc-fn]
pub fn lexc_add_mc(symbol: &mut [u8]) {
    LEXC.with_borrow_mut(|lx| {
        lexc_deescape_string(symbol, b'%', 0);
        if lexc_find_mc_impl(lx, symbol) == 0 {
            let len = utf8strlen(symbol);
            let mut mcprev: Option<usize> = None;
            /* for (mcs = mc; mcs != NULL && utf8strlen(mcs->symbol) > len; ...) */
            let mut mcs = lx.mc;
            while let Some(m) = mcs {
                if !(utf8strlen(lx.mc_arena[m].symbol.as_deref().unwrap().as_bytes()) > len) {
                    break;
                }
                mcprev = Some(m);
                mcs = lx.mc_arena[m].next;
            }
            let mcnew = lx.mc_arena.len();
            lx.mc_arena.push(MulticharSymbols {
                symbol: Some(cstrdup(symbol)),
                sigma_number: 0, /* set below */
                next: mcs,
            });
            if lx.mc.is_none() || (mcs.is_some() && mcprev.is_none()) {
                lx.mc = Some(mcnew);
            }
            if let Some(p) = mcprev {
                lx.mc_arena[p].next = Some(mcnew);
            }

            let s = sigma_add(&cstrdup(symbol), lx.lexsigma.as_deref_mut().unwrap());
            /* mchashval = (unsigned char)symbol[0]*256 + (unsigned char)symbol[1]
            — raw second byte (NUL for a 1-byte symbol) */
            let b0 = symbol[0] as usize;
            let b1 = symbol.get(1).copied().unwrap_or(0) as usize;
            let mchashval = b0 * 256 + b1;
            lexc_add_sigma_hash(lx, symbol, s);
            lx.mchash[mchashval] = true;
            lx.mc_arena[mcnew].sigma_number = s as i16;
        }
    });
}

// [spec:foma:def:lexcread.lexc-find-mc-fn]
// [spec:foma:sem:lexcread.lexc-find-mc-fn]
// [spec:foma:def:lexc.lexc-find-mc-fn]
// [spec:foma:sem:lexc.lexc-find-mc-fn]
pub fn lexc_find_mc(symbol: &[u8]) -> i32 {
    LEXC.with_borrow(|lx| lexc_find_mc_impl(lx, symbol))
}

fn lexc_find_mc_impl(lx: &Lexc, symbol: &[u8]) -> i32 {
    let mut mcs = lx.mc;
    while let Some(m) = mcs {
        if sym_eq(&lx.mc_arena[m].symbol, symbol) {
            return 1;
        }
        mcs = lx.mc_arena[m].next;
    }
    0
}

// [spec:foma:def:lexcread.lexc-find-lex-state-fn]
// [spec:foma:sem:lexcread.lexc-find-lex-state-fn]
// [spec:foma:def:lexc.lexc-find-lex-state-fn]
// [spec:foma:sem:lexc.lexc-find-lex-state-fn]
// Returns the lexicon's state (a private `struct states` — exposed here as its
// arena index, since this is dead API with no callers in the C tree).
pub fn lexc_find_lex_state(name: &[u8]) -> Option<usize> {
    LEXC.with_borrow(|lx| {
        let mut l = lx.lexstates;
        while let Some(lidx) = l {
            if sym_eq(&lx.lexstates_arena[lidx].name, name) {
                return Some(lx.lexstates_arena[lidx].state);
            }
            l = lx.lexstates_arena[lidx].next;
        }
        None
    })
}

// [spec:foma:def:lexcread.lexc-add-word-fn]
// [spec:foma:sem:lexcread.lexc-add-word-fn]
// [spec:foma:def:lexc.lexc-add-word-fn]
// [spec:foma:sem:lexc.lexc-add-word-fn]
pub fn lexc_add_word() {
    /* Add a word from source state to destination state */
    LEXC.with_borrow_mut(|lx| {
        if lx.current_entry == REGEX_ENTRY {
            lexc_add_network(lx);
            return;
        }

        /* find source, dest */
        let mut sourcestate = lx.lexstates_arena[lx.clexicon.unwrap()].state;
        let deststate = lx.lexstates_arena[lx.ctarget.unwrap()].state;

        let mut li = 0usize;
        while lx.cwordin[li] != -1 {
            li += 1;
        }
        let len = li as i32;
        if len > lx.maxlen {
            lx.maxlen = len;
        }

        /* We follow the source state if the symbols are the same (merge prefixes) */
        let mut follow = 1;
        let mut i = 0usize;
        while lx.cwordin[i] != -1 {
            let mut followed = false;
            if follow == 1 {
                let mut trans = lx.state_arena[sourcestate].trans;
                while let Some(tidx) = trans {
                    let t_in = lx.trans_arena[tidx].r#in as i32;
                    let t_out = lx.trans_arena[tidx].out as i32;
                    let t_target = lx.trans_arena[tidx].target;
                    if t_in == lx.cwordin[i]
                        && t_out == lx.cwordout[i]
                        && lx.state_arena[t_target].lexstate.is_none()
                    {
                        /* Can't follow if target needs to be lexstate */
                        if lx.cwordin[i + 1] == -1 && t_target != deststate {
                            trans = lx.trans_arena[tidx].next;
                            continue;
                        }
                        sourcestate = t_target;
                        lx.state_arena[sourcestate].mergeable = 0;
                        followed = true; /* goto breakout */
                        break;
                    }
                    trans = lx.trans_arena[tidx].next;
                }
            }
            if followed {
                i += 1;
                continue;
            }
            follow = 0;

            let target;
            if lx.cwordin[i + 1] == -1 {
                target = deststate;
            } else {
                let newstate = lx.state_arena.len();
                lx.state_arena.push(States {
                    trans: None,
                    lexstate: None,
                    number: -1,
                    hashval: 0,
                    mergeable: 1,
                    distance: 0,
                    merge_with: 0,
                });
                lexc_add_state(lx, newstate);
                lx.state_arena[newstate].trans = None;
                lx.state_arena[newstate].lexstate = None;
                lx.state_arena[newstate].mergeable = 1;
                lx.state_arena[newstate].hashval = lexc_suffix_hash(lx, (i + 1) as i32);
                lx.state_arena[newstate].distance = (len - i as i32 - 1) as u16;
                lx.state_arena[newstate].merge_with = newstate;
                target = newstate;
            }
            let newtrans = lx.trans_arena.len();
            lx.trans_arena.push(Trans {
                r#in: lx.cwordin[i] as i16,
                out: lx.cwordout[i] as i16,
                target,
                next: lx.state_arena[sourcestate].trans,
            });
            lx.state_arena[sourcestate].trans = Some(newtrans);
            sourcestate = target;
            i += 1;
        }
    });
}

// [spec:foma:def:lexcread.lexc-number-states-fn]
// [spec:foma:sem:lexcread.lexc-number-states-fn]
fn lexc_number_states(lx: &mut Lexc) {
    let mut smax = 0i32;
    let mut n = 0i32;
    lx.hasfinal = 0;

    let mut hasroot = 0;
    {
        let mut s = lx.statelist;
        while let Some(sidx) = s {
            smax += 1;
            let state = lx.statelist_arena[sidx].state;
            let is_root = match lx.state_arena[state].lexstate {
                Some(lidx) => lx.lexstates_arena[lidx].name.as_deref() == Some("Root"),
                None => false,
            };
            if is_root {
                lx.state_arena[state].number = 0;
                lx.statelist_arena[sidx].start = 1;
                n += 1;
                hasroot = 1;
                break;
            }
            s = lx.statelist_arena[sidx].next;
        }
    }
    /* If there is no Root lexicon, the first lexicon mentioned is Root */
    if hasroot == 0 {
        let mut s = lx.statelist;
        while let Some(sidx) = s {
            if lx.statelist_arena[sidx].next.is_none() {
                let state = lx.statelist_arena[sidx].state;
                lx.state_arena[state].number = 0;
                if G_VERBOSE.with(|v| v.get()) != 0 {
                    let lidx = lx.state_arena[state].lexstate.unwrap();
                    let name = lx.lexstates_arena[lidx].name.as_deref().unwrap_or("");
                    eprint!("*Warning: no Root lexicon, using '{}' as Root.\n", name);
                }
                lx.statelist_arena[sidx].start = 1;
                n += 1;
            }
            s = lx.statelist_arena[sidx].next;
        }
    }
    /* Mark # as the last state */
    {
        let mut s = lx.statelist;
        while let Some(sidx) = s {
            let state = lx.statelist_arena[sidx].state;
            if let Some(lidx) = lx.state_arena[state].lexstate {
                if lx.lexstates_arena[lidx].name.as_deref() == Some("#") {
                    lx.state_arena[state].number = smax - 1;
                    lx.statelist_arena[sidx].r#final = 1;
                    lx.hasfinal = 1;
                } else if lx.lexstates_arena[lidx].has_outgoing == 0 {
                    /* Also mark uncontinued states as final */
                    lx.statelist_arena[sidx].r#final = 1;
                }
            }
            s = lx.statelist_arena[sidx].next;
        }
    }

    {
        let mut s = lx.statelist;
        while let Some(sidx) = s {
            let state = lx.statelist_arena[sidx].state;
            if lx.state_arena[state].number == -1 {
                lx.state_arena[state].number = n;
                n += 1;
            }
            s = lx.statelist_arena[sidx].next;
        }
    }
    lx.lexc_statecount = n + 1;
    {
        let mut l = lx.lexstates;
        while let Some(lidx) = l {
            let state = lx.lexstates_arena[lidx].state;
            if lx.lexstates_arena[lidx].targeted == 0 && lx.state_arena[state].number != 0 {
                if G_VERBOSE.with(|v| v.get()) != 0 {
                    let name = lx.lexstates_arena[lidx].name.as_deref().unwrap_or("");
                    eprint!("*Warning: lexicon '{}' defined but not used\n", name);
                }
            }
            if lx.lexstates_arena[lidx].has_outgoing == 0
                && lx.lexstates_arena[lidx].name.as_deref() != Some("#")
            {
                if G_VERBOSE.with(|v| v.get()) != 0 {
                    let name = lx.lexstates_arena[lidx].name.as_deref().unwrap_or("");
                    eprint!("***Warning: lexicon '{}' used but never defined\n", name);
                }
            }
            l = lx.lexstates_arena[lidx].next;
        }
    }
}

// [spec:foma:def:lexcread.lexc-eq-paths-fn]
// [spec:foma:sem:lexcread.lexc-eq-paths-fn]
fn lexc_eq_paths(lx: &Lexc, mut one: usize, mut two: usize) -> i32 {
    while lx.state_arena[one].lexstate.is_none() && lx.state_arena[two].lexstate.is_none() {
        /* dereferences trans without a NULL check (unwrap → panic on None,
        the nearest safe behavior to C's crash) */
        let ot = lx.state_arena[one].trans.unwrap();
        let tt = lx.state_arena[two].trans.unwrap();
        if lx.trans_arena[ot].r#in != lx.trans_arena[tt].r#in
            || lx.trans_arena[ot].out != lx.trans_arena[tt].out
        {
            return 0;
        }
        one = lx.trans_arena[ot].target;
        two = lx.trans_arena[tt].target;
    }
    if lx.state_arena[one].lexstate != lx.state_arena[two].lexstate {
        return 0;
    }
    1
}

/* Local chained bucket cell for lexc_merge_states (lenlist / hashstates).
Heads occupy the first maxlen+1 / tablesize entries of their Vec; chained
cells are appended, linked via `next` into the same Vec. */
#[derive(Debug, Clone, Copy)]
struct MergeNode {
    state: Option<usize>,
    next: Option<usize>,
}

// [spec:foma:def:lexcread.lexc-merge-states-fn]
// [spec:foma:sem:lexcread.lexc-merge-states-fn]
fn lexc_merge_states(lx: &mut Lexc) {
    /* Array of ptrs to states depending on string length */
    let mut lenlist: Vec<MergeNode> = vec![
        MergeNode {
            state: None,
            next: None
        };
        (lx.maxlen + 1) as usize
    ];
    let mut numstates = 0i32;
    {
        let mut s = lx.statelist;
        while let Some(sidx) = s {
            if lx.state_arena[lx.statelist_arena[sidx].state].mergeable != 0 {
                numstates += 1;
            }
            s = lx.statelist_arena[sidx].next;
        }
    }

    /* Find a suitable prime proportional to the number of mergeable states */
    let mut pi = 0usize;
    while PRIMES[pi] < (numstates / 4) as u32 {
        pi += 1;
    }
    let tablesize = PRIMES[pi];
    let mut hashstates: Vec<MergeNode> = vec![
        MergeNode {
            state: None,
            next: None
        };
        tablesize as usize
    ];

    {
        let mut s = lx.statelist;
        while let Some(sidx) = s {
            let state = lx.statelist_arena[sidx].state;
            if lx.state_arena[state].mergeable != 0 {
                numstates += 1; /* dead second count, as in C */
                let distance = lx.state_arena[state].distance as usize;
                if lenlist[distance].state.is_none() {
                    lenlist[distance].state = Some(state);
                } else {
                    let newl = lenlist.len();
                    let oldnext = lenlist[distance].next;
                    lenlist.push(MergeNode {
                        state: Some(state),
                        next: oldnext,
                    });
                    lenlist[distance].next = Some(newl);
                }
                lx.state_arena[state].hashval %= tablesize;
                let h = lx.state_arena[state].hashval as usize;
                if hashstates[h].state.is_none() {
                    hashstates[h].state = Some(state);
                } else {
                    let newh = hashstates.len();
                    let oldnext = hashstates[h].next;
                    hashstates.push(MergeNode {
                        state: Some(state),
                        next: oldnext,
                    });
                    hashstates[h].next = Some(newh);
                }
            }
            s = lx.statelist_arena[sidx].next;
        }
    }

    let mut i = lx.maxlen;
    while i >= 1 {
        let mut cl = Some(i as usize);
        while let Some(clidx) = cl {
            match lenlist[clidx].state {
                None => break,
                Some(cstate) => {
                    if lx.state_arena[cstate].mergeable != 1 {
                        cl = lenlist[clidx].next;
                        continue;
                    }
                    let state = cstate;
                    let hash = lx.state_arena[state].hashval as usize;
                    let mut ch = Some(hash);
                    while let Some(chidx) = ch {
                        if let Some(hstate) = hashstates[chidx].state {
                            if hstate != state
                                && lx.state_arena[hstate].mergeable == 1
                                && lx.state_arena[hstate].distance == lx.state_arena[state].distance
                                && lexc_eq_paths(lx, hstate, state) == 1
                            {
                                lx.state_arena[hstate].merge_with = state;
                                let mut purge = hstate;
                                while lx.state_arena[purge].lexstate.is_none() {
                                    lx.state_arena[purge].mergeable = 2;
                                    let t = lx.state_arena[purge].trans.unwrap();
                                    purge = lx.trans_arena[t].target;
                                }
                            }
                        }
                        ch = hashstates[chidx].next;
                    }
                    cl = lenlist[clidx].next;
                }
            }
        }
        i -= 1;
    }

    /* Rewrite pass: redirect targets through merge_with; free merged states'
    trans cells (no-op in the arena); set lexstate->targeted for survivors */
    {
        let mut s = lx.statelist;
        while let Some(sidx) = s {
            let state = lx.statelist_arena[sidx].state;
            let merged = lx.state_arena[state].mergeable == 2;
            let mut t = lx.state_arena[state].trans;
            let mut tprev: Option<usize> = None;
            while let Some(tidx) = t {
                let tgt = lx.trans_arena[tidx].target;
                lx.trans_arena[tidx].target = lx.state_arena[tgt].merge_with;
                if tprev.is_some() && merged {
                    /* free(tprev) — no-op (arena) */
                } else {
                    let newtgt = lx.trans_arena[tidx].target;
                    if let Some(lidx) = lx.state_arena[newtgt].lexstate {
                        lx.lexstates_arena[lidx].targeted = 1;
                    }
                }
                tprev = Some(tidx);
                t = lx.trans_arena[tidx].next;
            }
            /* if (tprev != NULL && merged) free(tprev) — no-op */
            s = lx.statelist_arena[sidx].next;
        }
    }

    /* Removal pass: unlink and free cells (and states) with mergeable == 2 */
    {
        let mut s = lx.statelist;
        let mut sprev: Option<usize> = None;
        while let Some(sidx) = s {
            let state = lx.statelist_arena[sidx].state;
            if lx.state_arena[state].mergeable == 2 {
                match sprev {
                    Some(p) => {
                        lx.statelist_arena[p].next = lx.statelist_arena[sidx].next;
                    }
                    None => {
                        /* C latent bug: statelist = s (the removed cell), not
                        s->next.
                        DEVIATION from C (C free()s s → the head dangles at freed
                        memory but "usually works" because the cell's fields are
                        read before reuse; the arena never reclaims s, so the head
                        deterministically stays at the removed cell with its next
                        intact — the observable "usually works" result) */
                        lx.statelist = Some(sidx);
                    }
                }
                /* free(s->state); free(sf) — no-op (arena) */
                s = lx.statelist_arena[sidx].next;
            } else {
                sprev = Some(sidx);
                s = lx.statelist_arena[sidx].next;
            }
        }
    }

    /* Cleanup of the index chains is memory-only (the local Vecs drop here);
    the C off-by-one leak of lenlist[maxlen]'s chain has no observable effect. */
}

// [spec:foma:def:lexcread.lexc-to-fsm-fn]
// [spec:foma:sem:lexcread.lexc-to-fsm-fn]
// [spec:foma:def:lexc.lexc-to-fsm-fn]
// [spec:foma:sem:lexc.lexc-to-fsm-fn]
pub fn lexc_to_fsm() -> Box<Fsm> {
    LEXC.with_borrow_mut(|lx| {
        if G_VERBOSE.with(|v| v.get()) != 0 {
            eprint!("Building lexicon...\n");
        }
        lexc_merge_states(lx);
        let mut net = fsm_create("");
        /* free(net->sigma); net->sigma = lexsigma (ownership transfer) */
        net.sigma = lx.lexsigma.take();
        lexc_number_states(lx);
        if lx.hasfinal == 0 {
            if G_VERBOSE.with(|v| v.get()) != 0 {
                eprint!("Warning: # is never reached!!!\n");
            }
            /* Leak path: lexc_cleanup is not called; the state graph and hash
            tables persist in the arenas until the next lexc_init. */
            return fsm_empty_set();
        }
        // DEVIATION from C (sa is malloc'd uninitialized and indexed by state
        // number; a numbering gap from the "#" collision bug leaves entries
        // unset — read as (state 0, 0, 0) here instead of C's garbage; an
        // out-of-range collision index panics where C corrupts the heap)
        let statecount = lx.lexc_statecount as usize;
        let mut sa: Vec<(usize, i8, i8)> = vec![(0usize, 0i8, 0i8); statecount];
        {
            let mut s = lx.statelist;
            while let Some(sidx) = s {
                let state = lx.statelist_arena[sidx].state;
                let num = lx.state_arena[state].number as usize;
                sa[num] = (
                    state,
                    lx.statelist_arena[sidx].start,
                    lx.statelist_arena[sidx].r#final,
                );
                s = lx.statelist_arena[sidx].next;
            }
        }
        let mut linecount = 0i32;
        {
            let mut s = lx.statelist;
            while let Some(sidx) = s {
                linecount += 1;
                let state = lx.statelist_arena[sidx].state;
                let mut t = lx.state_arena[state].trans;
                while let Some(tidx) = t {
                    linecount += 1;
                    t = lx.trans_arena[tidx].next;
                }
                s = lx.statelist_arena[sidx].next;
            }
        }
        let default_line = FsmState {
            state_no: 0,
            r#in: 0,
            out: 0,
            target: 0,
            final_state: 0,
            start_state: 0,
        };
        let mut fsm: Vec<FsmState> = vec![default_line; (linecount + 1) as usize];
        let mut i = 0i32;
        for j in 0..statecount {
            /* sa[num] was stored as (state, start, final); C calls
            add_fsm_arc(..., s[j].final, s[j].start), so bind so that
            `sfinal` = the final flag and `sstart` = the start flag. */
            let (state, sstart, sfinal) = sa[j];
            if lx.state_arena[state].trans.is_none() {
                add_fsm_arc(
                    &mut fsm,
                    i,
                    lx.state_arena[state].number,
                    -1,
                    -1,
                    -1,
                    sfinal as i32,
                    sstart as i32,
                );
                i += 1;
            } else {
                let mut t = lx.state_arena[state].trans;
                while let Some(tidx) = t {
                    let tgt = lx.trans_arena[tidx].target;
                    add_fsm_arc(
                        &mut fsm,
                        i,
                        lx.state_arena[state].number,
                        lx.trans_arena[tidx].r#in as i32,
                        lx.trans_arena[tidx].out as i32,
                        lx.state_arena[tgt].number,
                        sfinal as i32,
                        sstart as i32,
                    );
                    i += 1;
                    t = lx.trans_arena[tidx].next;
                }
            }
        }
        add_fsm_arc(&mut fsm, i, -1, -1, -1, -1, -1, -1);
        net.states = fsm;
        net.statecount = lx.lexc_statecount;
        fsm_update_flags(&mut net, UNK, UNK, UNK, UNK, UNK, UNK);
        /* lexsigma is now net.sigma (aliased in C); operate on net.sigma */
        if sigma_find_number(EPSILON, net.sigma.as_deref()) == -1 {
            sigma_add_special(EPSILON, net.sigma.as_deref_mut().unwrap());
        }
        /* free(s): C frees the sa array here (s == sa after the build loop);
        the sa Vec drops at scope end, observably identical */
        lexc_cleanup(lx);
        sigma_cleanup(&mut net, 0);
        sigma_sort(&mut net);

        if G_VERBOSE.with(|v| v.get()) != 0 {
            eprint!("Determinizing...\n");
        }
        let net = fsm_determinize(net);
        if G_VERBOSE.with(|v| v.get()) != 0 {
            eprint!("Minimizing...\n");
        }
        let net = fsm_topsort(fsm_minimize(net));
        if G_VERBOSE.with(|v| v.get()) != 0 {
            eprint!("Done!\n");
        }
        net
    })
}

// [spec:foma:def:lexcread.lexc-cleanup-fn]
// [spec:foma:sem:lexcread.lexc-cleanup-fn]
fn lexc_cleanup(lx: &mut Lexc) {
    /* free(mchash) */
    lx.mchash = Vec::new();
    /* free every hashtable chain node + symbol, then the bucket array */
    lx.hashtable = Vec::new();
    /* free the mc list (symbols + nodes) */
    lx.mc_arena = Vec::new();
    lx.mc = None;
    /* free the lexstates list (names + nodes; states freed via statelist) */
    lx.lexstates_arena = Vec::new();
    lx.lexstates = None;
    /* free each state's trans, then each state, then the statelist cells */
    lx.trans_arena = Vec::new();
    lx.state_arena = Vec::new();
    lx.statelist_arena = Vec::new();
    lx.statelist = None;
    /* lexsigma is not freed here — ownership was transferred to the net. The
    static pointers are left dangling in C; reset to None here (lexc_init must
    still run before any further lexc use). */
    lx.clexicon = None;
    lx.ctarget = None;
}

// [spec:foma:def:lexc.lexc-trim-fn]
// [spec:foma:sem:lexc.lexc-trim-fn]
// Implemented in foma/lexc.l (not lexcread.c); ported here per the concern.
pub fn lexc_trim(s: &mut [u8]) {
    /* Remove trailing ; and = and space and initial space */
    // DEVIATION from C (phase 1 has no lower bound and underruns the buffer on
    // an empty / all-trimmable string — UB; bounded at index 0 here).
    let mut i: isize = cstrlen(s) as isize - 1;
    while i >= 0
        && (s[i as usize] == b';'
            || s[i as usize] == b'='
            || s[i as usize] == b' '
            || s[i as usize] == b'\t')
    {
        s[i as usize] = 0;
        i -= 1;
    }
    let mut i = 0usize;
    while s[i] == b' ' || s[i] == b'\t' || s[i] == b'\n' {
        i += 1;
    }
    let mut j = 0usize;
    while s[i] != 0 {
        s[j] = s[i];
        i += 1;
        j += 1;
    }
    s[j] = s[i];
}

/* ------------------------------------------------------------------ */
/* lexc lexer driver (foma/lexc.l): fsm_lexc_parse_string / _file      */
/* ------------------------------------------------------------------ */

/* lexc.l: #define SOURCE_LEXICON 0 / #define TARGET_LEXICON 1 */
const SOURCE_LEXICON: i32 = 0;
const TARGET_LEXICON: i32 = 1;

/// Build a NUL-terminated byte buffer from a symbol string, exactly as the C
/// lexer's `lexctext` reaches the lexcread API (used for lexicon names and
/// continuations, which the C passes through `lexc_trim` — already trimmed by
/// nfst-lexc — and NOT through `%` de-escaping).
fn to_cbuf(s: &str) -> Vec<u8> {
    let mut v = s.as_bytes().to_vec();
    v.push(0);
    v
}

/// Re-escape a de-escaped nfst-lexc identifier back into the `%`-escaped form
/// the C lexer's `lexctext` would have carried into `lexc_set_current_word` /
/// `lexc_add_mc` (both of which run `lexc_deescape_string` themselves).
///
/// nfst-lexc already stripped `%X`→`X`, and encoded `%0` (foma's escaped
/// literal zero) as the marker `@ZERO@`. Its NAME_CH set excludes `< % ! ; : "`
/// and whitespace, so any of those in the stored string can only have come from
/// an escape — but of those only `%` and `:` are meaningful to the two API
/// functions (`:` is the pair delimiter, `%` the escape char), so only they
/// need re-escaping. `@ZERO@` is mapped back to `%0`. Trailing NUL slack is
/// appended so the in-place `lexc_deescape_string` never reads past the end.
fn foma_reescape(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len() + 4);
    let mut rest = s;
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix("@ZERO@") {
            out.extend_from_slice(b"%0");
            rest = r;
            continue;
        }
        let c = rest.chars().next().unwrap();
        match c {
            '%' => out.extend_from_slice(b"%%"),
            ':' => out.extend_from_slice(b"%:"),
            _ => {
                let mut buf = [0u8; 4];
                out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            }
        }
        rest = &rest[c.len_utf8()..];
    }
    out.extend_from_slice(&[0u8; 4]);
    out
}

// [spec:foma:def:fomalib.fsm-lexc-parse-string-fn]
// [spec:foma:sem:fomalib.fsm-lexc-parse-string-fn]
pub fn fsm_lexc_parse_string(string: &str, verbose: i32) -> Option<Box<Fsm>> {
    use std::io::Write;
    /* The `verbose` parameter is ignored, exactly as in C (the warnings key off
    the global g_verbose, read inside lexc_number_states/lexc_to_fsm). */
    let _ = verbose;

    /* olddefines = g_defines. The C never repoints g_defines, so the save is a
    no-op and any Definitions-section nets persist in g_defines after the call;
    reproduced by taking the box out (so nested fsm_parse_regex sees the same
    registry via the passed parameter, not the thread_local) and restoring it. */
    let mut olddefines: Option<Box<DefinedNetworks>> =
        G_DEFINES.with(|d| d.borrow_mut().take());

    /* lexentries = -1; lexclineno = 1; lexc_init() */
    let mut lexentries: i32 = -1;
    lexc_init();

    /* lexclex(): the flex scanner is replaced by nfst-lexc; its return code is
    modelled as `syntax_error` (the C returns 1 on the <*>(.) error rule). */
    let mut syntax_error = false;

    match nfst_lexc::parse(string) {
        Ok(file) => {
            let f = &file.value;

            /* foma's lexc.l has no NOFLAGS section: a "NOFLAGS" token hits the
            <*>(.) syntax-error rule. nfst-lexc accepts it (an HFST feature), so
            reject here to stay faithful (see report). */
            if !f.noflags.is_empty() {
                eprintln!("***lexc: NOFLAGS section is not supported by foma");
                G_DEFINES.with(|d| *d.borrow_mut() = olddefines.take());
                return None;
            }

            /* <MCS>{NONRESERVED}+ { lexc_add_mc(lexctext); } */
            for m in &f.multichars {
                let mut sym = foma_reescape(&m.value.0);
                lexc_add_mc(&mut sym);
            }

            /* <DEFREGEX>[\073] { if (my_yyparse(...,g_defines,...)==0)
                 add_defined(g_defines, fsm_topsort(fsm_minimize(current_parse)),
                             tempstr); } */
            for d in &f.definitions {
                let body = nfst_xre::pretty_print(&d.value.body);
                if let Some(net) = fsm_parse_regex(&body, olddefines.as_deref_mut(), None) {
                    let net = fsm_topsort(net);
                    if let Some(defs) = olddefines.as_deref_mut() {
                        add_defined(defs, Some(net), &d.value.name);
                    }
                }
            }

            for lex in &f.lexicons {
                /* <*>(LEXICON|Lexicon){SPACE}+{NONRESERVED}+ */
                if lexentries != -1 {
                    print!("{}, ", lexentries);
                }
                print!("{}...", lex.value.name);
                let _ = std::io::stdout().flush();
                lexentries = 0;
                let name = to_cbuf(&lex.value.name);
                lexc_set_current_lexicon(&name, SOURCE_LEXICON);

                for entry in &lex.value.entries {
                    /* The gloss ("info" string) is discarded by the C lexer
                    (EATUPINFO state), so entry.value.gloss is ignored. */
                    match &entry.value.spec {
                        nfst_lexc::EntrySpec::Empty => {
                            /* No word token: current word stays the epsilon left
                            by the preceding lexc_clear_current_word. */
                        }
                        nfst_lexc::EntrySpec::String(s) => {
                            let mut w = foma_reescape(s);
                            lexc_set_current_word(&mut w);
                        }
                        nfst_lexc::EntrySpec::Pair { upper, lower } => {
                            /* Rebuild the raw `upper:lower` token the C matched:
                            re-escape each side, join with a bare `:` so
                            lexc_set_current_word splits it back. */
                            let mut w = foma_reescape(upper);
                            let padlen = w.len() - 4;
                            w.truncate(padlen);
                            w.push(b':');
                            w.extend_from_slice(&foma_reescape(lower));
                            lexc_set_current_word(&mut w);
                        }
                        nfst_lexc::EntrySpec::Regex(xre) => {
                            /* <REGEX>[\076] { if (my_yyparse(...)==0)
                                 lexc_set_network(current_parse); } */
                            let r = nfst_xre::pretty_print(xre);
                            if let Some(net) =
                                fsm_parse_regex(&r, olddefines.as_deref_mut(), None)
                            {
                                lexc_set_network(net);
                            }
                        }
                    }

                    /* The continuation token drives the target lexicon + word:
                    lexc_trim; lexc_set_current_lexicon(TARGET); lexc_add_word();
                    lexc_clear_current_word(); lexentries++. */
                    let cont = to_cbuf(&entry.value.continuation);
                    lexc_set_current_lexicon(&cont, TARGET_LEXICON);
                    lexc_add_word();
                    lexc_clear_current_word();
                    lexentries += 1;
                    if lexentries % 10000 == 0 {
                        print!("{}...", lexentries);
                        let _ = std::io::stdout().flush();
                    }
                }
            }
        }
        Err(e) => {
            /* The C prints "\n***Syntax error on line %i column %i at '%s'\n"
            and returns 1; line/column/text are not recoverable from nfst-lexc,
            so emit its first diagnostic instead (see report). lexc_to_fsm is
            still called below, as in C. */
            syntax_error = true;
            let msg = e
                .diagnostics
                .first()
                .map(|d| d.message.clone())
                .unwrap_or_else(|| "syntax error".to_string());
            eprintln!("\n***Syntax error: {}", msg);
        }
    }

    /* if (lexclex() != 1) { if (lexentries != -1) printf("%i\n", lexentries); } */
    if !syntax_error && lexentries != -1 {
        println!("{}", lexentries);
    }

    /* g_defines = olddefines */
    G_DEFINES.with(|d| *d.borrow_mut() = olddefines.take());

    /* return lexc_to_fsm() */
    Some(lexc_to_fsm())
}

// [spec:foma:def:fomalib.fsm-lexc-parse-file-fn]
// [spec:foma:sem:fomalib.fsm-lexc-parse-file-fn]
pub fn fsm_lexc_parse_file(filename: &str, verbose: i32) -> Option<Box<Fsm>> {
    /* mystring = file_to_mem(filename); return fsm_lexc_parse_string(mystring,
    verbose). The C never frees mystring (documented leak); here the buffer is a
    Vec that drops at scope end — an observable no-op. */
    let mystring = match file_to_mem(filename) {
        Some(v) => v,
        /* C has no NULL check and hands NULL to the scanner (undefined
        behavior); file_to_mem already printed the error. DEVIATION from C: a
        null pointer cannot be reconstructed safely, so return None. */
        None => return None,
    };
    /* file_to_mem appends a terminating NUL; strip it (and any BOM-free tail of
    trailing NULs) before handing the text to the parser. */
    let end = mystring.iter().position(|&b| b == 0).unwrap_or(mystring.len());
    let text = String::from_utf8_lossy(&mystring[..end]);
    fsm_lexc_parse_string(&text, verbose)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apply::{
        apply_down, apply_init, apply_lower_words, apply_up, apply_upper_words, apply_words,
    };

    /* ---- helpers -------------------------------------------------------- */

    /// NUL-terminated buffer for a lexicon/continuation name (no de-escape).
    fn cbuf(s: &str) -> Vec<u8> {
        let mut v = s.as_bytes().to_vec();
        v.push(0);
        v
    }

    /// NUL-terminated word buffer with trailing slack so the in-place
    /// de-escapers (which may read name[i+1] past a trailing escape) stay in
    /// bounds.
    fn wbuf(s: &str) -> Vec<u8> {
        let mut v = s.as_bytes().to_vec();
        v.extend_from_slice(&[0u8; 8]);
        v
    }

    /// Compile a lexc source string (verbose off is irrelevant — g_verbose is a
    /// separate global; the parse string ignores its own `verbose` arg).
    fn compile(src: &str) -> Box<Fsm> {
        fsm_lexc_parse_string(src, 0).expect("compile produced a net")
    }

    fn lower_all(net: &Fsm) -> Vec<String> {
        let mut h = apply_init(net);
        let mut v = Vec::new();
        let mut r = apply_lower_words(&mut h);
        while let Some(s) = r {
            v.push(s);
            r = apply_lower_words(&mut h);
        }
        v.sort();
        v.dedup();
        v
    }

    fn upper_all(net: &Fsm) -> Vec<String> {
        let mut h = apply_init(net);
        let mut v = Vec::new();
        let mut r = apply_upper_words(&mut h);
        while let Some(s) = r {
            v.push(s);
            r = apply_upper_words(&mut h);
        }
        v.sort();
        v.dedup();
        v
    }

    fn words_all(net: &Fsm) -> Vec<String> {
        let mut h = apply_init(net);
        let mut v = Vec::new();
        let mut r = apply_words(&mut h);
        while let Some(s) = r {
            v.push(s);
            r = apply_words(&mut h);
        }
        v.sort();
        v.dedup();
        v
    }

    fn down_one(net: &Fsm, w: &str) -> Option<String> {
        let mut h = apply_init(net);
        apply_down(&mut h, Some(w))
    }

    fn up_one(net: &Fsm, w: &str) -> Option<String> {
        let mut h = apply_init(net);
        apply_up(&mut h, Some(w))
    }

    /* Drive one word entry Root -> "#" through the public trie API. */
    fn add_word_entry(word: &str) {
        let mut buf = wbuf(word);
        lexc_set_current_word(&mut buf);
        lexc_set_current_lexicon(&cbuf("#"), TARGET_LEXICON);
        lexc_add_word();
        lexc_clear_current_word();
    }

    /* ---- end-to-end (fsm_lexc_parse_string + apply) --------------------- */

    // Root + `#` termination, identity words, continuation-class LIFO trie:
    // exercises the whole driver (init -> set_current_lexicon/word -> add_word
    // -> clear -> to_fsm -> number_states -> merge_states -> cleanup).
    // [spec:foma:sem:fomalib.fsm-lexc-parse-string-fn/test]
    // [spec:foma:sem:lexcread.lexc-to-fsm-fn/test]
    // [spec:foma:sem:lexc.lexc-to-fsm-fn/test]
    #[test]
    fn e2e_basic_root_and_hash() {
        let net = compile("LEXICON Root\ncat # ;\ndog # ;\n");
        assert_eq!(lower_all(&net), vec!["cat", "dog"]);
        assert_eq!(upper_all(&net), vec!["cat", "dog"]);
    }

    // A cross-lexicon continuation (Root -> N) concatenates prefixes.
    // [spec:foma:sem:lexcread.lexc-set-current-lexicon-fn/test]
    // [spec:foma:sem:lexc.lexc-set-current-lexicon-fn/test]
    #[test]
    fn e2e_continuation_class() {
        let net = compile("LEXICON Root\nbig N ;\nLEXICON N\ncat # ;\ndog # ;\n");
        assert_eq!(lower_all(&net), vec!["bigcat", "bigdog"]);
    }

    // Pair entry `cat:dog`: upper and lower projections differ.
    // [spec:foma:sem:lexcread.lexc-set-current-word-fn/test]
    // [spec:foma:sem:lexc.lexc-set-current-word-fn/test]
    #[test]
    fn e2e_pair_entry() {
        let net = compile("LEXICON Root\ncat:dog # ;\n");
        assert_eq!(upper_all(&net), vec!["cat"]);
        assert_eq!(lower_all(&net), vec!["dog"]);
        assert_eq!(down_one(&net, "cat").as_deref(), Some("dog"));
        assert_eq!(up_one(&net, "dog").as_deref(), Some("cat"));
    }

    // Multichar symbols declared + longest-first tokenization: `+PlPoss` must
    // win over its prefix `+Pl` (mc list is kept in decreasing length order).
    // [spec:foma:sem:lexcread.lexc-add-mc-fn/test]
    // [spec:foma:sem:lexc.lexc-add-mc-fn/test]
    #[test]
    fn e2e_multichar_longest_first() {
        let net = compile(
            "Multichar_Symbols +Pl +PlPoss\nLEXICON Root\nx+Pl # ;\ny+PlPoss # ;\n",
        );
        assert_eq!(upper_all(&net), vec!["x+Pl", "y+PlPoss"]);
    }

    // `%`-escape: `a%:b` is the literal three-symbol string a : b (identity),
    // not a pair split at the colon.
    // [spec:foma:sem:lexcread.lexc-find-delim-fn/test]
    // [spec:foma:sem:lexcread.lexc-deescape-string-fn/test]
    #[test]
    fn e2e_percent_escape_colon() {
        let net = compile("LEXICON Root\na%:b # ;\n");
        assert_eq!(upper_all(&net), vec!["a:b"]);
        assert_eq!(lower_all(&net), vec!["a:b"]);
    }

    // `0` as epsilon: `a:0` = a:eps, `0:b` = eps:b — projections drop epsilon.
    // [spec:foma:sem:lexcread.lexc-string-to-tokens-fn/test]
    #[test]
    fn e2e_zero_is_epsilon() {
        let net = compile("LEXICON Root\na:0 # ;\n0:b # ;\n");
        assert_eq!(upper_all(&net), vec!["", "a"]);
        assert_eq!(lower_all(&net), vec!["", "b"]);
    }

    // `< regex >` entry: the parsed net is spliced between source and target.
    // [spec:foma:sem:lexcread.lexc-set-network-fn/test]
    // [spec:foma:sem:lexc.lexc-set-network-fn/test]
    // [spec:foma:sem:lexcread.lexc-add-network-fn/test]
    #[test]
    fn e2e_regex_entry() {
        let net = compile("LEXICON Root\n< a (b) c > # ;\n");
        assert_eq!(words_all(&net), vec!["abc", "ac"]);
    }

    // `< a ? >` (identity/unknown) plus a later word `b` introducing a new
    // symbol: lexc_update_unknowns patches the @-arc so `ab` is also matched.
    // [spec:foma:sem:lexcread.lexc-update-unknowns-fn/test]
    #[test]
    fn e2e_regex_unknown_symbols() {
        let net = compile("LEXICON Root\n< a ? > # ;\nb # ;\n");
        let low = lower_all(&net);
        // C `read lexc` gives 4 paths: b, aa, ab, a@ (@ = other/unknown).
        assert_eq!(low.len(), 4);
        assert!(low.contains(&"aa".to_string()));
        assert!(low.contains(&"ab".to_string()));
        assert!(low.contains(&"b".to_string()));
    }

    // Definitions section + use inside a `< regex >` entry. As in the REPL,
    // g_defines is an initialized (dummy-head) registry before parsing.
    // [spec:foma:sem:fomalib.fsm-lexc-parse-string-fn/test]
    #[test]
    fn e2e_definitions_section() {
        crate::define::G_DEFINES
            .with(|d| *d.borrow_mut() = Some(crate::define::defined_networks_init()));
        let net = compile("Definitions\nV = a | e | i | o | u ;\nLEXICON Root\n< V V > # ;\n");
        assert_eq!(words_all(&net).len(), 25);
    }

    // Info ("gloss") string is discarded by the lexer (EATUPINFO).
    // [spec:foma:sem:lexc.lexc-add-word-fn/test]
    #[test]
    fn e2e_infostring_discarded() {
        let net = compile("LEXICON Root\ncat # \"a feline\" ;\n");
        assert_eq!(lower_all(&net), vec!["cat"]);
    }

    // Undefined continuation class: the net is still built (the dead-end
    // lexicon has has_outgoing==0 so number_states makes it final).
    // [spec:foma:sem:lexcread.lexc-number-states-fn/test]
    #[test]
    fn e2e_undefined_continuation() {
        let net = compile("LEXICON Root\ncat Nonexistent ;\ndog # ;\n");
        assert_eq!(lower_all(&net), vec!["cat", "dog"]);
    }

    // No Root lexicon: the first-mentioned lexicon becomes the start state.
    // [spec:foma:sem:lexcread.lexc-number-states-fn/test]
    #[test]
    fn e2e_no_root_lexicon() {
        let net = compile("LEXICON First\nhi # ;\n");
        assert_eq!(lower_all(&net), vec!["hi"]);
    }

    // `#` never reached -> fsm_empty_set() (0 paths).
    // [spec:foma:sem:lexcread.lexc-to-fsm-fn/test]
    // [spec:foma:sem:lexc.lexc-to-fsm-fn/test]
    #[test]
    fn e2e_hash_never_reached_empty() {
        let net = compile("LEXICON Root\ncat Root ;\n");
        assert_eq!(net.finalcount, 0);
        assert_eq!(lower_all(&net), Vec::<String>::new());
    }

    /* ---- fsm_lexc_parse_file -------------------------------------------- */

    // Parse via a temp file; a nonexistent path returns None (DEVIATION: C
    // hands NULL to the scanner).
    // [spec:foma:sem:fomalib.fsm-lexc-parse-file-fn/test]
    #[test]
    fn parse_file_roundtrip_and_missing() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("foma_lexc_test_{}.lexc", std::process::id()));
        std::fs::write(&path, "LEXICON Root\ncat # ;\n").unwrap();
        let net = fsm_lexc_parse_file(path.to_str().unwrap(), 0).expect("file parsed");
        assert_eq!(lower_all(&net), vec!["cat"]);
        std::fs::remove_file(&path).ok();

        assert!(fsm_lexc_parse_file("/no/such/foma/lexc/file.xyz", 0).is_none());
    }

    /* ---- direct API: hashing -------------------------------------------- */

    // djb2 with signed-char sign extension, % 3079. Values derived offline.
    // [spec:foma:sem:lexcread.lexc-symbol-hash-fn/test]
    #[test]
    fn symbol_hash_signed_char() {
        assert_eq!(lexc_symbol_hash(b"a\0"), 2167);
        assert_eq!(lexc_symbol_hash(b"cat\0"), 686);
        assert_eq!(lexc_symbol_hash(b"+Noun\0"), 211);
        assert_eq!(lexc_symbol_hash(b"\0"), 2302);
        // High byte (0xC3 0xA9 = "é"): signed-char extension gives 1551, not
        // the unsigned-byte value 1018 — pins the sign-extension quirk.
        assert_eq!(lexc_symbol_hash(&[0xC3, 0xA9, 0]), 1551);
    }

    // PJW-style suffix fold over cwordin|cwordout<<8, no table modulus.
    // [spec:foma:sem:lexcread.lexc-suffix-hash-fn/test]
    #[test]
    fn suffix_hash_values() {
        LEXC.with_borrow_mut(|lx| {
            lx.cwordin[0] = 3;
            lx.cwordin[1] = 4;
            lx.cwordin[2] = -1;
            lx.cwordout[0] = 3;
            lx.cwordout[1] = 4;
            lx.cwordout[2] = -1;
            assert_eq!(lexc_suffix_hash(lx, 0), 13364);
            assert_eq!(lexc_suffix_hash(lx, 1), 1028);
            lx.cwordin[0] = 5;
            lx.cwordin[1] = -1;
            lx.cwordout[0] = 7;
            lx.cwordout[1] = -1;
            assert_eq!(lexc_suffix_hash(lx, 0), 1797);
        });
    }

    // Add/find in the sigma hashtable: head fill, chain append (no dedup ->
    // shadowed entry), find returns the head match, miss -> -1.
    // [spec:foma:sem:lexcread.lexc-add-sigma-hash-fn/test]
    // [spec:foma:sem:lexcread.lexc-find-sigma-hash-fn/test]
    #[test]
    fn sigma_hash_add_find_shadow() {
        lexc_init();
        LEXC.with_borrow_mut(|lx| {
            assert_eq!(lexc_find_sigma_hash(lx, b"a\0"), -1);
            lexc_add_sigma_hash(lx, b"a\0", 7);
            lexc_add_sigma_hash(lx, b"a\0", 99); // shadow appended to tail
            let bucket = lexc_symbol_hash(b"a\0") as usize;
            assert_eq!(lx.hashtable[bucket].symbol.as_deref(), Some("a"));
            assert_eq!(lx.hashtable[bucket].sigma_number, 7);
            assert!(lx.hashtable[bucket].next.is_some());
            assert_eq!(lexc_find_sigma_hash(lx, b"a\0"), 7); // head wins
            assert_eq!(lexc_find_sigma_hash(lx, b"zzz\0"), -1);
        });
    }

    /* ---- direct API: string helpers ------------------------------------- */

    // First unescaped delimiter; escape protects the next byte; a lone
    // trailing escape does not skip.
    // [spec:foma:sem:lexcread.lexc-find-delim-fn/test]
    #[test]
    fn find_delim_cases() {
        assert_eq!(lexc_find_delim(b"a:b\0", b':', b'%'), Some(1));
        assert_eq!(lexc_find_delim(b"a%:b\0\0", b':', b'%'), None);
        assert_eq!(lexc_find_delim(b"ab\0", b':', b'%'), None);
        assert_eq!(lexc_find_delim(b"a%\0", b':', b'%'), None);
    }

    // De-escape quirks: escaped char kept literally; mode-1 '0' -> 0xff marker;
    // mode-0 unescaped '0' silently deleted; trailing escape truncates.
    // [spec:foma:sem:lexcread.lexc-deescape-string-fn/test]
    #[test]
    fn deescape_string_quirks() {
        let mut a = wbuf("a%:b");
        lexc_deescape_string(&mut a, b'%', 1);
        assert_eq!(cstr(&a), b"a:b");

        let mut b = wbuf("a0b");
        lexc_deescape_string(&mut b, b'%', 1); // mode 1: 0 -> 0xff
        assert_eq!(cstr(&b), &[b'a', 0xff, b'b']);

        let mut c = wbuf("a0b");
        lexc_deescape_string(&mut c, b'%', 0); // mode 0: 0 deleted
        assert_eq!(cstr(&c), b"ab");

        let mut d = wbuf("ab%"); // trailing escape swallows the NUL
        lexc_deescape_string(&mut d, b'%', 1);
        assert_eq!(cstr(&d), b"ab");
    }

    // Always NUL-terminates; stops early on a copied NUL; writes len+1 bytes.
    // [spec:foma:sem:lexcread.mystrncpy-fn/test]
    #[test]
    fn mystrncpy_terminates() {
        let mut dst = [0xAAu8; 8];
        mystrncpy(&mut dst, b"cat", 3);
        assert_eq!(&dst[..4], b"cat\0");
        let mut dst2 = [0xAAu8; 8];
        mystrncpy(&mut dst2, b"hi\0zz", 5);
        assert_eq!(&dst2[..3], b"hi\0");
    }

    // lexc_trim: strip trailing ;/=/space/tab, then leading space/tab/nl;
    // empty / all-trimmable input must not underrun (bounded at 0).
    // [spec:foma:sem:lexc.lexc-trim-fn/test]
    #[test]
    fn trim_cases() {
        let mut a = wbuf("  cat ;;  ");
        lexc_trim(&mut a);
        assert_eq!(cstr(&a), b"cat");
        let mut e = wbuf("");
        lexc_trim(&mut e); // no underrun
        assert_eq!(cstr(&e), b"");
        let mut f = wbuf("===");
        lexc_trim(&mut f);
        assert_eq!(cstr(&f), b"");
    }

    /* ---- direct API: tokenization --------------------------------------- */

    // Multichar longest-first, %-marked epsilon (0xff), and fresh-sigma
    // numbering (first regular symbol = 3).
    // [spec:foma:sem:lexcread.lexc-string-to-tokens-fn/test]
    // [spec:foma:sem:lexcread.lexc-find-mc-fn/test]
    // [spec:foma:sem:lexc.lexc-find-mc-fn/test]
    #[test]
    fn string_to_tokens_multichar_and_epsilon() {
        lexc_init();
        // register +Pl (len 3) then +PlPoss (len 7): mc list -> longest first
        lexc_add_mc(&mut wbuf("+Pl")); // sigma 3
        lexc_add_mc(&mut wbuf("+PlPoss")); // sigma 4
        assert_eq!(lexc_find_mc(b"+Pl\0"), 1);
        assert_eq!(lexc_find_mc(b"+Nope\0"), 0);
        LEXC.with_borrow_mut(|lx| {
            // mc list head is the longest symbol
            let head = lx.mc.unwrap();
            assert_eq!(lx.mc_arena[head].symbol.as_deref(), Some("+PlPoss"));

            let mut arr = [0i32; 1000];
            lexc_string_to_tokens(lx, b"+PlPoss", &mut arr);
            assert_eq!(&arr[..2], &[4, -1]); // longest match wins

            let mut arr2 = [0i32; 1000];
            lexc_string_to_tokens(lx, b"+Plx", &mut arr2); // +Pl then new 'x'=5
            assert_eq!(&arr2[..3], &[3, 5, -1]);

            // 0xff marker -> EPSILON(0)
            let mut arr3 = [0i32; 1000];
            lexc_string_to_tokens(lx, &[b'a', 0xff, b'a'], &mut arr3);
            assert_eq!(arr3[1], EPSILON);
        });
    }

    // Malformed UTF-8 (leading 0x80) -> utf8skip == -1 -> i never advances ->
    // intarr overflow (DEVIATION: C corrupts memory, Rust panics on OOB).
    // [spec:foma:sem:lexcread.lexc-string-to-tokens-fn/test]
    #[test]
    #[should_panic]
    fn string_to_tokens_infinite_loop_panics() {
        lexc_init();
        LEXC.with_borrow_mut(|lx| {
            let mut arr = [0i32; 1000];
            lexc_string_to_tokens(lx, &[0x80u8, 0], &mut arr);
        });
    }

    /* ---- direct API: alignment ------------------------------------------ */

    // Tail-pad the shorter side with EPSILON; empty:empty -> single eps pair.
    // [spec:foma:sem:lexcread.lexc-pad-fn/test]
    #[test]
    fn pad_shapes() {
        LEXC.with_borrow_mut(|lx| {
            lx.cwordin[..3].copy_from_slice(&[3, 4, -1]);
            lx.cwordout[..2].copy_from_slice(&[5, -1]);
            lexc_pad(lx);
            assert_eq!(&lx.cwordin[..3], &[3, 4, -1]);
            assert_eq!(&lx.cwordout[..3], &[5, EPSILON, -1]);

            lx.cwordin[0] = -1;
            lx.cwordout[0] = -1;
            lexc_pad(lx);
            assert_eq!(&lx.cwordin[..2], &[EPSILON, -1]);
            assert_eq!(&lx.cwordout[..2], &[EPSILON, -1]);
        });
    }

    // Min-edit alignment: unequal symbols never substitute (cost 100), so
    // a->b becomes eps:b, a:eps; equal symbols align on the diagonal.
    // [spec:foma:sem:lexcread.lexc-medpad-fn/test]
    #[test]
    fn medpad_shapes() {
        LEXC.with_borrow_mut(|lx| {
            lx.cwordin[..2].copy_from_slice(&[3, -1]);
            lx.cwordout[..2].copy_from_slice(&[4, -1]);
            lexc_medpad(lx);
            assert_eq!(&lx.cwordin[..3], &[EPSILON, 3, -1]);
            assert_eq!(&lx.cwordout[..3], &[4, EPSILON, -1]);

            lx.cwordin[..3].copy_from_slice(&[3, 4, -1]);
            lx.cwordout[..3].copy_from_slice(&[3, 4, -1]);
            lexc_medpad(lx);
            assert_eq!(&lx.cwordin[..3], &[3, 4, -1]);
            assert_eq!(&lx.cwordout[..3], &[3, 4, -1]);
        });
    }

    /* ---- direct API: set_current_word ----------------------------------- */

    // Pair split at unescaped ':', carity=2, identity copy for carity=1.
    // [spec:foma:sem:lexcread.lexc-set-current-word-fn/test]
    // [spec:foma:sem:lexc.lexc-set-current-word-fn/test]
    // [spec:foma:sem:lexcread.lexc-clear-current-word-fn/test]
    // [spec:foma:sem:lexc.lexc-clear-current-word-fn/test]
    #[test]
    fn set_current_word_pair_and_identity() {
        lexc_init();
        lexc_set_current_word(&mut wbuf("cat:dog"));
        LEXC.with_borrow(|lx| {
            assert_eq!(lx.carity, 2);
            assert_eq!(&lx.cwordin[..4], &[3, 4, 5, -1]); // c a t
            assert_eq!(&lx.cwordout[..4], &[6, 7, 8, -1]); // d o g
        });
        lexc_clear_current_word();
        LEXC.with_borrow(|lx| {
            assert_eq!(&lx.cwordin[..2], &[EPSILON, -1]);
            assert_eq!(lx.current_entry, WORD_ENTRY);
        });
        lexc_set_current_word(&mut wbuf("cat"));
        LEXC.with_borrow(|lx| {
            assert_eq!(lx.carity, 1);
            assert_eq!(&lx.cwordin[..4], &[3, 4, 5, -1]);
            assert_eq!(&lx.cwordout[..4], &[3, 4, 5, -1]); // identity copy
        });
    }

    /* ---- direct API: lexicons & states ---------------------------------- */

    // init resets the tables; create/reuse a lexicon; source vs target; a dead
    // lexicon lookup.
    // [spec:foma:sem:lexcread.lexc-init-fn/test]
    // [spec:foma:sem:lexc.lexc-init-fn/test]
    // [spec:foma:sem:lexcread.lexc-set-current-lexicon-fn/test]
    // [spec:foma:sem:lexc.lexc-set-current-lexicon-fn/test]
    // [spec:foma:sem:lexcread.lexc-add-state-fn/test]
    // [spec:foma:sem:lexcread.lexc-find-lex-state-fn/test]
    // [spec:foma:sem:lexc.lexc-find-lex-state-fn/test]
    #[test]
    fn init_and_set_current_lexicon() {
        lexc_init();
        LEXC.with_borrow(|lx| {
            assert_eq!(lx.hashtable.len(), SIGMA_HASH_TABLESIZE);
            assert_eq!(lx.mchash.len(), 256 * 256);
            assert!(lx.lexsigma.is_some());
            assert_eq!(lx.lexc_statecount, 0);
            assert_eq!(lx.hashtable[0].sigma_number, -1);
        });
        assert!(lexc_find_lex_state(b"Root\0").is_none());
        lexc_set_current_lexicon(&cbuf("Root"), SOURCE_LEXICON);
        LEXC.with_borrow(|lx| {
            // one lexstate + one state registered; clexicon set, has_outgoing=1
            assert_eq!(lx.lexstates_arena.len(), 1);
            assert_eq!(lx.lexc_statecount, 1);
            let l = lx.clexicon.unwrap();
            assert_eq!(lx.lexstates_arena[l].has_outgoing, 1);
            assert_eq!(lx.lexstates_arena[l].name.as_deref(), Some("Root"));
        });
        assert!(lexc_find_lex_state(b"Root\0").is_some());
        // target reuse of a fresh name creates a second lexicon
        lexc_set_current_lexicon(&cbuf("N"), TARGET_LEXICON);
        LEXC.with_borrow(|lx| {
            assert_eq!(lx.lexstates_arena.len(), 2);
            assert_eq!(lx.lexstates_arena[lx.ctarget.unwrap()].has_outgoing, 0);
        });
        // re-selecting Root as source reuses the same lexstate (no growth)
        lexc_set_current_lexicon(&cbuf("Root"), SOURCE_LEXICON);
        LEXC.with_borrow(|lx| assert_eq!(lx.lexstates_arena.len(), 2));
    }

    // Prefix sharing (trie): a second word sharing a prefix follows existing
    // transitions and adds no new state.
    // [spec:foma:sem:lexcread.lexc-add-word-fn/test]
    // [spec:foma:sem:lexc.lexc-add-word-fn/test]
    #[test]
    fn add_word_prefix_sharing() {
        lexc_init();
        lexc_set_current_lexicon(&cbuf("Root"), SOURCE_LEXICON);
        add_word_entry("cat");
        let after_cat = LEXC.with_borrow(|lx| lx.state_arena.len());
        add_word_entry("car"); // shares "ca" prefix -> no new state
        let after_car = LEXC.with_borrow(|lx| lx.state_arena.len());
        assert_eq!(after_cat, after_car);
        // followed prefix states were demoted to mergeable == 0
        LEXC.with_borrow(|lx| {
            let root = lx.lexstates_arena[lx.clexicon.unwrap()].state;
            let t = lx.state_arena[root].trans.unwrap();
            let s1 = lx.trans_arena[t].target; // 'c' target
            assert_eq!(lx.state_arena[s1].mergeable, 0);
        });
    }

    /* ---- direct API: eq_paths ------------------------------------------- */

    // Identical single-arc suffix chains ending in the same lexstate compare
    // equal; a label mismatch compares unequal.
    // [spec:foma:sem:lexcread.lexc-eq-paths-fn/test]
    #[test]
    fn eq_paths_match_and_mismatch() {
        LEXC.with_borrow_mut(|lx| {
            // lexicon terminal state (index 0)
            lx.lexstates_arena.push(Lexstates {
                name: Some("#".into()),
                state: 0,
                next: None,
                targeted: 0,
                has_outgoing: 0,
            });
            let dest = push_state(lx, Some(0));
            let a = push_chain(lx, 5, 5, dest); // (5:5) -> dest
            let b = push_chain(lx, 5, 5, dest);
            let c = push_chain(lx, 6, 6, dest); // different label
            assert_eq!(lexc_eq_paths(lx, a, b), 1);
            assert_eq!(lexc_eq_paths(lx, a, c), 0);
        });
    }

    // Null trans deref: eq_paths on two non-lexicon states with no transition
    // panics (DEVIATION: C dereferences NULL).
    // [spec:foma:sem:lexcread.lexc-eq-paths-fn/test]
    #[test]
    #[should_panic]
    fn eq_paths_null_trans_panics() {
        LEXC.with_borrow_mut(|lx| {
            let a = push_state(lx, None);
            let b = push_state(lx, None);
            lexc_eq_paths(lx, a, b);
        });
    }

    /* ---- direct API: merge_states --------------------------------------- */

    // Suffix merge shrinks the trie: cat + bat share tail "at", so two suffix
    // states are marked deleted (mergeable == 2); language preserved.
    // [spec:foma:sem:lexcread.lexc-merge-states-fn/test]
    #[test]
    fn merge_states_shared_suffix() {
        lexc_init();
        lexc_set_current_lexicon(&cbuf("Root"), SOURCE_LEXICON);
        add_word_entry("cat");
        add_word_entry("bat");
        LEXC.with_borrow_mut(|lx| {
            lexc_merge_states(lx);
            let deleted = lx
                .state_arena
                .iter()
                .filter(|s| s.mergeable == 2)
                .count();
            assert_eq!(deleted, 2); // the "at" and "t" states of one branch
        });
        // whole compile still yields both words
        let net = compile("LEXICON Root\ncat # ;\nbat # ;\n");
        assert_eq!(lower_all(&net), vec!["bat", "cat"]);
    }

    // DEVIATION repro: when the deleted cell is the statelist head, C assigns
    // `statelist = s` (the freed cell) not `s->next`; the arena keeps that head
    // deterministically with its `next` intact ("usually works").
    // [spec:foma:sem:lexcread.lexc-merge-states-fn/test]
    #[test]
    fn merge_states_head_use_after_free_repro() {
        LEXC.with_borrow_mut(|lx| {
            // lexicon terminal DEST = state 0
            lx.lexstates_arena.push(Lexstates {
                name: Some("#".into()),
                state: 0,
                next: None,
                targeted: 0,
                has_outgoing: 0,
            });
            let dest = push_state(lx, Some(0)); // 0
            // survivor chain X0 -(a)-> X1 -(b)-> DEST
            let x1 = push_suffix(lx, 20, 1, dest, 2, 2); // state 2, dist 1
            let x0 = push_suffix(lx, 10, 2, x1, 1, 1); // state 1, dist 2
            // loser chain Y0 -(a)-> Y1 -(b)-> DEST (same labels/hashes)
            let y1 = push_suffix(lx, 20, 1, dest, 2, 2); // dist 1
            let y0 = push_suffix(lx, 10, 2, y1, 1, 1); // dist 2

            // statelist head = Y1 (a loser-chain interior node)
            let head = push_sl(lx, y1); // cell 0
            let c_x0 = push_sl(lx, x0); // cell 1
            let c_x1 = push_sl(lx, x1); // cell 2
            let c_y0 = push_sl(lx, y0); // cell 3
            let c_dest = push_sl(lx, dest); // cell 4
            lx.statelist_arena[head].next = Some(c_x0);
            lx.statelist_arena[c_x0].next = Some(c_x1);
            lx.statelist_arena[c_x1].next = Some(c_y0);
            lx.statelist_arena[c_y0].next = Some(c_dest);
            lx.statelist = Some(head);
            lx.maxlen = 2;

            lexc_merge_states(lx);

            // loser chain deleted, survivor kept, redirect recorded
            assert_eq!(lx.state_arena[y0].mergeable, 2);
            assert_eq!(lx.state_arena[y1].mergeable, 2);
            assert_eq!(lx.state_arena[x0].mergeable, 1);
            assert_eq!(lx.state_arena[y0].merge_with, x0);
            // the head still points at the "freed" cell, next intact
            assert_eq!(lx.statelist, Some(head));
            assert_eq!(lx.statelist_arena[head].next, Some(c_x0));
            // the interior loser cell was unlinked (X1 -> DEST)
            assert_eq!(lx.statelist_arena[c_x1].next, Some(c_dest));
        });
    }

    /* ---- direct API: number_states -------------------------------------- */

    // "#"-collision quirk: smax is Root's list position, not the state count,
    // so when Root is not the first-created state, "#" gets a number that
    // collides with one assigned sequentially.
    // [spec:foma:sem:lexcread.lexc-number-states-fn/test]
    #[test]
    fn number_states_hash_collision_bug() {
        LEXC.with_borrow_mut(|lx| {
            // three lexicon states: First(0), Root(1), #(2)
            for (i, nm) in ["First", "Root", "#"].iter().enumerate() {
                lx.lexstates_arena.push(Lexstates {
                    name: Some((*nm).into()),
                    state: i,
                    next: None,
                    targeted: 1,
                    has_outgoing: 1,
                });
                let s = push_state(lx, Some(i));
                assert_eq!(s, i);
            }
            lx.lexstates_arena[2].has_outgoing = 0; // '#' is a target-only leaf
            // statelist order [#, Root, First] (reverse creation)
            let c_h = push_sl(lx, 2);
            let c_r = push_sl(lx, 1);
            let c_f = push_sl(lx, 0);
            lx.statelist_arena[c_h].next = Some(c_r);
            lx.statelist_arena[c_r].next = Some(c_f);
            lx.statelist = Some(c_h);
            // link lexstates list so the warning loop has something to walk
            lx.lexstates = Some(0);

            lexc_number_states(lx);

            assert_eq!(lx.hasfinal, 1);
            assert_eq!(lx.state_arena[1].number, 0); // Root
            // smax = Root's position = 2, so '#' = smax-1 = 1 ...
            assert_eq!(lx.state_arena[2].number, 1);
            // ... and First is numbered sequentially to the SAME value (bug)
            assert_eq!(lx.state_arena[0].number, 1);
        });
    }

    /* ---- direct API: cleanup -------------------------------------------- */

    // Frees every compile table (arenas emptied), lexsigma left for the net.
    // [spec:foma:sem:lexcread.lexc-cleanup-fn/test]
    #[test]
    fn cleanup_empties_arenas() {
        lexc_init();
        lexc_set_current_lexicon(&cbuf("Root"), SOURCE_LEXICON);
        add_word_entry("cat");
        LEXC.with_borrow_mut(|lx| {
            assert!(!lx.state_arena.is_empty());
            lexc_cleanup(lx);
            assert!(lx.state_arena.is_empty());
            assert!(lx.trans_arena.is_empty());
            assert!(lx.statelist_arena.is_empty());
            assert!(lx.lexstates_arena.is_empty());
            assert!(lx.mc_arena.is_empty());
            assert!(lx.hashtable.is_empty());
            assert!(lx.mchash.is_empty());
            assert_eq!(lx.statelist, None);
        });
    }

    /* ---- arena construction helpers (tests only) ------------------------ */

    fn push_state(lx: &mut Lexc, lexstate: Option<usize>) -> usize {
        let idx = lx.state_arena.len();
        lx.state_arena.push(States {
            trans: None,
            lexstate,
            number: -1,
            hashval: 0,
            mergeable: 0,
            distance: 0,
            merge_with: idx,
        });
        idx
    }

    /* One mergeable suffix state carrying a single (in:out) arc to `target`. */
    fn push_suffix(
        lx: &mut Lexc,
        hashval: u32,
        distance: u16,
        target: usize,
        r#in: i16,
        out: i16,
    ) -> usize {
        let sidx = lx.state_arena.len();
        lx.state_arena.push(States {
            trans: None,
            lexstate: None,
            number: -1,
            hashval,
            mergeable: 1,
            distance,
            merge_with: sidx,
        });
        let tidx = lx.trans_arena.len();
        lx.trans_arena.push(Trans {
            r#in,
            out,
            target,
            next: None,
        });
        lx.state_arena[sidx].trans = Some(tidx);
        sidx
    }

    /* A one-arc chain state (mergeable, distance 1) to a lexicon `target`. */
    fn push_chain(lx: &mut Lexc, r#in: i16, out: i16, target: usize) -> usize {
        push_suffix(lx, 0, 1, target, r#in, out)
    }

    fn push_sl(lx: &mut Lexc, state: usize) -> usize {
        let idx = lx.statelist_arena.len();
        lx.statelist_arena.push(Statelist {
            state,
            next: None,
            start: 0,
            r#final: 0,
        });
        idx
    }
}

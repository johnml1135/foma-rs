//! foma/apply.c — literal (bug-for-bug) Wave-2 port per
//! docs/port/rust-conventions.md. Sem rules: docs/spec/port/foma/apply.md
//! (per-file `apply.*` ids) plus the fomalib.h prototype ids
//! (`fomalib.apply-*`) carried at the single Rust site.
//!
//! Representation notes (all observably equivalent to C):
//!  - The `char *` results of apply_net / apply_updown / apply_enumerate /
//!    apply_return_string etc. point into `h.outstring` in C and are valid
//!    only until the next call on the handle. The Rust twins return
//!    `Option<String>` (an owned copy of the NUL-terminated content). This
//!    matches the convention `char * -> String/Option<String>`.
//!  - `h.gstates` is the interior pointer `net->states`; represented as a
//!    base index (always 0) into `last_net`'s line table, so `gstates+ptr`
//!    becomes `last_net.states[gstates + ptr as usize]`.
//!  - The 256-way byte sigma trie: C uses one calloc'd 256-cell array per
//!    level, chained by `sigma_trie.next`. The given `SigmaTrie` node has a
//!    single `next: Option<Box<SigmaTrie>>`, which cannot hold a 256-cell
//!    child array. DEVIATION from C: `h.sigma_trie` is used as a flat arena
//!    — the root level is cells 0..256, each new level is 256 cells appended
//!    at the arena tail, and a cell's child-level base index is stored in a
//!    synthetic `next` node's `signum`. Tokenization behaviour is identical.
//!  - The per-state arc index (index_in/index_out): C stores one
//!    sigma_size-cell array per state, indexed by symbol with EPSILON
//!    fall-through. The given `ApplyStateIndex` has a single `next` chain,
//!    which cannot encode a 2-D (symbol x overflow) array. DEVIATION from C:
//!    `index_in[state]` holds a chain of ALL of the state's real arcs
//!    (fsmptr = arc line). apply_set_iptr hands the whole chain to `h.iptr`;
//!    correctness is preserved because apply_follow_next_arc re-matches each
//!    arc via apply_match_length/apply_match_str (the index is only a
//!    pruning optimisation). The densest-first / mem-limit decision of which
//!    states get an index is reproduced.

use crate::dynarray::{rand, srand};
use crate::flags::{flag_check, flag_get_name, flag_get_type, flag_get_value};
use crate::mem::round_up_to_power_of_two;
use crate::sigma::sigma_max;
use crate::types::{
    APPLY_BINSEARCH_THRESHOLD, APPLY_INDEX_INPUT, ApplyHandle, ApplyStateIndex,
    DEFAULT_OUTSTRING_SIZE, DEFAULT_STACK_SIZE, DOWN, ENUMERATE, EPSILON, FAIL, FLAG_CLEAR,
    FLAG_DISALLOW, FLAG_EQUAL, FLAG_NEGATIVE, FLAG_POSITIVE, FLAG_REQUIRE, FLAG_UNIFY, FlagList,
    FlagLookup, Fsm, IDENTITY, LOWER, RANDOM, SUCCEED, Searchstack, SigmaTrie, SigmaTrieArrays,
    SigmatchArray, Sigs, UNKNOWN, UP, UPPER,
};
use crate::utf8::{utf8iscombining, utf8skip};

/* ------------------------------------------------------------------ */
/* Small helpers reproducing the C pointer/bit macros                 */
/* ------------------------------------------------------------------ */

/* (h->gstates + off) line accessors. gstates is the base index of
last_net->states (always 0); off is a line offset (h->ptr / h->curr_ptr). */
fn l_state_no(h: &ApplyHandle, off: i32) -> i32 {
    h.last_net.as_ref().unwrap().states[h.gstates + off as usize].state_no
}
fn l_in(h: &ApplyHandle, off: i32) -> i32 {
    h.last_net.as_ref().unwrap().states[h.gstates + off as usize].r#in as i32
}
fn l_out(h: &ApplyHandle, off: i32) -> i32 {
    h.last_net.as_ref().unwrap().states[h.gstates + off as usize].out as i32
}
fn l_target(h: &ApplyHandle, off: i32) -> i32 {
    h.last_net.as_ref().unwrap().states[h.gstates + off as usize].target
}
fn l_final(h: &ApplyHandle, off: i32) -> i32 {
    h.last_net.as_ref().unwrap().states[h.gstates + off as usize].final_state as i32
}

/* BITSLOT/BITMASK/BITTEST/BITSET/BITNSLOTS from apply.c */
fn bittest(a: &[u8], b: i32) -> bool {
    (a[(b >> 3) as usize] & (1u8 << (b & 7))) != 0
}
fn bitset(a: &mut [u8], b: i32) {
    a[(b >> 3) as usize] |= 1u8 << (b & 7);
}
fn bitnslots(nb: i32) -> usize {
    ((nb + 8 - 1) / 8) as usize
}

/* strcpy(buf+off, src): copy src bytes then a NUL terminator. */
fn buf_strcpy(buf: &mut Vec<u8>, off: usize, src: &[u8]) {
    let end = off + src.len();
    // DEVIATION from C (C writes into a fixed malloc'd buffer and can overflow
    // on the IDENTITY/space latent bugs; the Vec is grown to stay memory-safe)
    if buf.len() < end + 1 {
        buf.resize(end + 1, 0);
    }
    buf[off..end].copy_from_slice(src);
    buf[end] = 0;
}

/* memcpy(buf+off, src, len): copy len bytes, no terminator. */
fn buf_memcpy(buf: &mut Vec<u8>, off: usize, src: &[u8], len: usize) {
    let end = off + len;
    // DEVIATION from C (see buf_strcpy)
    if buf.len() < end {
        buf.resize(end, 0);
    }
    buf[off..end].copy_from_slice(&src[..len]);
}

/* Read h->outstring up to its NUL terminator as an owned String.
DEVIATION from C (from_utf8_lossy; C hands back the raw byte pointer). */
fn outstring_result(h: &ApplyHandle) -> String {
    let end = h
        .outstring
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(h.outstring.len());
    String::from_utf8_lossy(&h.outstring[..end]).into_owned()
}

fn new_searchstack_frame() -> Searchstack {
    Searchstack {
        offset: 0,
        iptr: None,
        state_has_index: 0,
        opos: 0,
        ipos: 0,
        visitmark: 0,
        flagname: None,
        flagvalue: None,
        flagneg: 0,
    }
}

/* Walk the flag_list to a node matching `name` (mutable). */
fn flag_find_mut<'a>(
    mut cur: &'a mut Option<Box<FlagList>>,
    name: &str,
) -> Option<&'a mut FlagList> {
    loop {
        let is_match = match cur {
            Some(n) => n.name.as_deref() == Some(name),
            None => return None,
        };
        if is_match {
            return cur.as_deref_mut();
        }
        cur = &mut cur.as_mut().unwrap().next;
    }
}

/* ------------------------------------------------------------------ */
/* Setters                                                            */
/* ------------------------------------------------------------------ */

// [spec:foma:def:apply.apply-set-obey-flags-fn]
// [spec:foma:sem:apply.apply-set-obey-flags-fn]
// [spec:foma:def:fomalib.apply-set-obey-flags-fn]
// [spec:foma:sem:fomalib.apply-set-obey-flags-fn]
pub fn apply_set_obey_flags(h: &mut ApplyHandle, value: i32) {
    h.obey_flags = value;
}

// [spec:foma:def:apply.apply-set-show-flags-fn]
// [spec:foma:sem:apply.apply-set-show-flags-fn]
// [spec:foma:def:fomalib.apply-set-show-flags-fn]
// [spec:foma:sem:fomalib.apply-set-show-flags-fn]
pub fn apply_set_show_flags(h: &mut ApplyHandle, value: i32) {
    h.show_flags = value;
}

// [spec:foma:def:apply.apply-set-print-space-fn]
// [spec:foma:sem:apply.apply-set-print-space-fn]
// [spec:foma:def:fomalib.apply-set-print-space-fn]
// [spec:foma:sem:fomalib.apply-set-print-space-fn]
pub fn apply_set_print_space(h: &mut ApplyHandle, value: i32) {
    h.print_space = value;
    h.space_symbol = Some(" ".to_string()); // C: strdup(" ")
}

// [spec:foma:def:apply.apply-set-separator-fn]
// [spec:foma:sem:apply.apply-set-separator-fn]
// [spec:foma:def:fomalib.apply-set-separator-fn]
// [spec:foma:sem:fomalib.apply-set-separator-fn]
pub fn apply_set_separator(h: &mut ApplyHandle, symbol: &str) {
    h.separator = Some(symbol.to_string());
}

// [spec:foma:def:apply.apply-set-epsilon-fn]
// [spec:foma:sem:apply.apply-set-epsilon-fn]
// [spec:foma:def:fomalib.apply-set-epsilon-fn]
// [spec:foma:sem:fomalib.apply-set-epsilon-fn]
pub fn apply_set_epsilon(h: &mut ApplyHandle, symbol: &str) {
    // free(h->epsilon_symbol); strdup(symbol)
    h.epsilon_symbol = Some(symbol.to_string());
    let s = h.epsilon_symbol.clone();
    let len = s.as_deref().unwrap().len() as i32;
    h.sigs[EPSILON as usize].symbol = s;
    h.sigs[EPSILON as usize].length = len;
}

// [spec:foma:def:apply.apply-set-space-symbol-fn]
// [spec:foma:sem:apply.apply-set-space-symbol-fn]
// [spec:foma:def:fomalib.apply-set-space-symbol-fn]
// [spec:foma:sem:fomalib.apply-set-space-symbol-fn]
pub fn apply_set_space_symbol(h: &mut ApplyHandle, space: &str) {
    h.space_symbol = Some(space.to_string());
    h.print_space = 1;
}

// [spec:foma:def:apply.apply-set-print-pairs-fn]
// [spec:foma:sem:apply.apply-set-print-pairs-fn]
// [spec:foma:def:fomalib.apply-set-print-pairs-fn]
// [spec:foma:sem:fomalib.apply-set-print-pairs-fn]
pub fn apply_set_print_pairs(h: &mut ApplyHandle, value: i32) {
    h.print_pairs = value;
}

/* ------------------------------------------------------------------ */
/* Stack management                                                   */
/* ------------------------------------------------------------------ */

// [spec:foma:def:apply.apply-force-clear-stack-fn]
// [spec:foma:sem:apply.apply-force-clear-stack-fn]
pub(crate) fn apply_force_clear_stack(h: &mut ApplyHandle) {
    /* Make sure stack is empty and marks reset */
    if apply_stack_isempty(h) == 0 {
        let sn = l_state_no(h, h.ptr);
        h.marks[sn as usize] = 0;
        while apply_stack_isempty(h) == 0 {
            apply_stack_pop(h);
            let sn = l_state_no(h, h.ptr);
            h.marks[sn as usize] = 0;
        }
        h.iterator = 0;
        h.iterate_old = 0;
        apply_stack_clear(h);
    }
}

// [spec:foma:def:apply.apply-stack-isempty-fn]
// [spec:foma:sem:apply.apply-stack-isempty-fn]
pub(crate) fn apply_stack_isempty(h: &ApplyHandle) -> i32 {
    if h.apply_stack_ptr == 0 {
        return 1;
    }
    0
}

// [spec:foma:def:apply.apply-stack-clear-fn]
// [spec:foma:sem:apply.apply-stack-clear-fn]
pub(crate) fn apply_stack_clear(h: &mut ApplyHandle) {
    h.apply_stack_ptr = 0;
}

// [spec:foma:def:apply.apply-stack-pop-fn]
// [spec:foma:sem:apply.apply-stack-pop-fn]
pub(crate) fn apply_stack_pop(h: &mut ApplyHandle) {
    h.apply_stack_ptr -= 1;
    let ss = h.searchstack[h.apply_stack_ptr as usize].clone();

    h.iptr = ss.iptr.clone();
    h.ptr = ss.offset;
    h.ipos = ss.ipos;
    h.opos = ss.opos;
    h.state_has_index = ss.state_has_index;
    /* Restore mark */
    let sn = l_state_no(h, h.ptr);
    h.marks[sn as usize] = ss.visitmark;

    if h.has_flags != 0 && ss.flagname.is_some() {
        /* Restore flag */
        let name = ss.flagname.as_deref().unwrap();
        match flag_find_mut(&mut h.flag_list, name) {
            Some(flist) => {
                flist.value = ss.flagvalue.clone();
                flist.neg = ss.flagneg as i16;
            }
            None => {
                // C: perror("***Nothing to pop") then dereferences NULL (crash).
                // DEVIATION from C (NULL deref after "Nothing to pop"; unreachable
                // in practice — every feature name is pre-registered).
                eprintln!("***Nothing to pop");
            }
        }
    }
}

// [spec:foma:def:apply.apply-stack-push-fn]
// [spec:foma:sem:apply.apply-stack-push-fn]
pub(crate) fn apply_stack_push(
    h: &mut ApplyHandle,
    vmark: i32,
    sflagname: Option<String>,
    sflagvalue: Option<String>,
    sflagneg: i32,
) {
    if h.apply_stack_ptr == h.apply_stack_top {
        // C: realloc to double; failure perror+exit(0). Vec growth aborts on OOM.
        let newtop = (h.apply_stack_top * 2) as usize;
        h.searchstack.resize(newtop, new_searchstack_frame());
        h.apply_stack_top *= 2;
    }
    let curr_ptr = h.curr_ptr;
    let ipos = h.ipos;
    let opos = h.opos;
    let iptr = h.iptr.clone();
    let state_has_index = h.state_has_index;
    let has_flags = h.has_flags;
    let ss = &mut h.searchstack[h.apply_stack_ptr as usize];
    ss.offset = curr_ptr;
    ss.ipos = ipos;
    ss.opos = opos;
    ss.visitmark = vmark;
    ss.iptr = iptr;
    ss.state_has_index = state_has_index;
    if has_flags != 0 {
        ss.flagname = sflagname;
        ss.flagvalue = sflagvalue;
        ss.flagneg = sflagneg;
    }
    h.apply_stack_ptr += 1;
}

/* ------------------------------------------------------------------ */
/* Entry points                                                       */
/* ------------------------------------------------------------------ */

// [spec:foma:def:apply.apply-enumerate-fn]
// [spec:foma:sem:apply.apply-enumerate-fn]
pub fn apply_enumerate(h: &mut ApplyHandle) -> Option<String> {
    let result: Option<String>;

    if h.last_net.is_none() || h.last_net.as_ref().unwrap().finalcount == 0 {
        return None;
    }
    h.binsearch = 0;
    if h.iterator == 0 {
        h.iterate_old = 0;
        apply_force_clear_stack(h);
        result = apply_net(h);
        if (h.mode & RANDOM) != RANDOM {
            h.iterator += 1;
        }
    } else {
        h.iterate_old = 1;
        result = apply_net(h);
    }
    result
}

// [spec:foma:def:apply.apply-words-fn]
// [spec:foma:sem:apply.apply-words-fn]
// [spec:foma:def:fomalib.apply-words-fn]
// [spec:foma:sem:fomalib.apply-words-fn]
pub fn apply_words(h: &mut ApplyHandle) -> Option<String> {
    h.mode = DOWN + ENUMERATE + LOWER + UPPER;
    apply_enumerate(h)
}

// [spec:foma:def:apply.apply-upper-words-fn]
// [spec:foma:sem:apply.apply-upper-words-fn]
// [spec:foma:def:fomalib.apply-upper-words-fn]
// [spec:foma:sem:fomalib.apply-upper-words-fn]
pub fn apply_upper_words(h: &mut ApplyHandle) -> Option<String> {
    h.mode = DOWN + ENUMERATE + UPPER;
    apply_enumerate(h)
}

// [spec:foma:def:apply.apply-lower-words-fn]
// [spec:foma:sem:apply.apply-lower-words-fn]
// [spec:foma:def:fomalib.apply-lower-words-fn]
// [spec:foma:sem:fomalib.apply-lower-words-fn]
pub fn apply_lower_words(h: &mut ApplyHandle) -> Option<String> {
    h.mode = DOWN + ENUMERATE + LOWER;
    apply_enumerate(h)
}

// [spec:foma:def:apply.apply-random-words-fn]
// [spec:foma:sem:apply.apply-random-words-fn]
// [spec:foma:def:fomalib.apply-random-words-fn]
// [spec:foma:sem:fomalib.apply-random-words-fn]
pub fn apply_random_words(h: &mut ApplyHandle) -> Option<String> {
    apply_clear_flags(h);
    h.mode = DOWN + ENUMERATE + LOWER + UPPER + RANDOM;
    apply_enumerate(h)
}

// [spec:foma:def:apply.apply-random-lower-fn]
// [spec:foma:sem:apply.apply-random-lower-fn]
// [spec:foma:def:fomalib.apply-random-lower-fn]
// [spec:foma:sem:fomalib.apply-random-lower-fn]
pub fn apply_random_lower(h: &mut ApplyHandle) -> Option<String> {
    apply_clear_flags(h);
    h.mode = DOWN + ENUMERATE + LOWER + RANDOM;
    apply_enumerate(h)
}

// [spec:foma:def:apply.apply-random-upper-fn]
// [spec:foma:sem:apply.apply-random-upper-fn]
// [spec:foma:def:fomalib.apply-random-upper-fn]
// [spec:foma:sem:fomalib.apply-random-upper-fn]
pub fn apply_random_upper(h: &mut ApplyHandle) -> Option<String> {
    apply_clear_flags(h);
    h.mode = DOWN + ENUMERATE + UPPER + RANDOM;
    apply_enumerate(h)
}

/* Frees memory associated with applies */
// [spec:foma:def:apply.apply-clear-fn]
// [spec:foma:sem:apply.apply-clear-fn]
// [spec:foma:def:fomalib.apply-clear-fn]
// [spec:foma:sem:fomalib.apply-clear-fn]
pub fn apply_clear(mut h: Box<ApplyHandle>) {
    // C walks and frees each sigma_trie level array. Here the trie arena lives
    // in h.sigma_trie (freed on drop); clear the bookkeeping list + arena.
    h.sigma_trie_arrays = None;
    h.sigma_trie = Vec::new();
    h.statemap = Vec::new();
    h.numlines = Vec::new();
    h.marks = Vec::new();
    h.searchstack = Vec::new();
    h.sigs = Vec::new();
    h.flag_lookup = Vec::new();
    h.sigmatch_array = Vec::new();
    h.flagstates = Vec::new();
    apply_clear_index(&mut h);
    h.last_net = None;
    h.iterator = 0;
    h.outstring = Vec::new();
    h.separator = None;
    h.epsilon_symbol = None;
    drop(h);
}

// [spec:foma:def:apply.apply-updown-fn]
// [spec:foma:sem:apply.apply-updown-fn]
pub fn apply_updown(h: &mut ApplyHandle, word: Option<&str>) -> Option<String> {
    let result: Option<String>;

    if h.last_net.is_none() || h.last_net.as_ref().unwrap().finalcount == 0 {
        return None;
    }

    if word.is_none() {
        h.iterate_old = 1;
        result = apply_net(h);
    } else {
        h.iterate_old = 0;
        // C borrows the caller's word pointer; owned copy of the bytes here.
        h.instring = word.unwrap().as_bytes().to_vec();
        apply_create_sigmatch(h);

        /* Remove old marks if necessary */
        apply_force_clear_stack(h);
        result = apply_net(h);
    }
    result
}

// [spec:foma:def:apply.apply-down-fn]
// [spec:foma:sem:apply.apply-down-fn]
// [spec:foma:def:fomalib.apply-down-fn]
// [spec:foma:sem:fomalib.apply-down-fn]
pub fn apply_down(h: &mut ApplyHandle, word: Option<&str>) -> Option<String> {
    h.mode = DOWN;
    if !h.index_in.is_empty() {
        h.indexed = 1;
    } else {
        h.indexed = 0;
    }
    // C dereferences last_net before apply_updown's NULL guard.
    h.binsearch = if h.last_net.as_ref().unwrap().arcs_sorted_in == 1 {
        1
    } else {
        0
    };
    apply_updown(h, word)
}

// [spec:foma:def:apply.apply-up-fn]
// [spec:foma:sem:apply.apply-up-fn]
// [spec:foma:def:fomalib.apply-up-fn]
// [spec:foma:sem:fomalib.apply-up-fn]
pub fn apply_up(h: &mut ApplyHandle, word: Option<&str>) -> Option<String> {
    h.mode = UP;
    if !h.index_out.is_empty() {
        h.indexed = 1;
    } else {
        h.indexed = 0;
    }
    h.binsearch = if h.last_net.as_ref().unwrap().arcs_sorted_out == 1 {
        1
    } else {
        0
    };
    apply_updown(h, word)
}

// [spec:foma:def:apply.apply-init-fn]
// [spec:foma:sem:apply.apply-init-fn]
// [spec:foma:def:fomalib.apply-init-fn]
// [spec:foma:sem:fomalib.apply-init-fn]
pub fn apply_init(net: &Fsm) -> Box<ApplyHandle> {
    // C: srand((unsigned int) time(NULL)); reseeds the shared LCG.
    // DEVIATION from C (SystemTime seconds stand in for time(NULL)).
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as u32;
    srand(seed);

    // calloc(1, sizeof(struct apply_handle)) — zeroed handle.
    let mut h = Box::new(ApplyHandle {
        ptr: 0,
        curr_ptr: 0,
        ipos: 0,
        opos: 0,
        mode: 0,
        printcount: 0,
        numlines: Vec::new(),
        statemap: Vec::new(),
        marks: Vec::new(),
        sigma_trie: Vec::new(),
        sigmatch_array: Vec::new(),
        sigma_trie_arrays: None,
        binsearch: 0,
        indexed: 0,
        state_has_index: 0,
        sigma_size: 0,
        sigmatch_array_size: 0,
        current_instring_length: 0,
        has_flags: 0,
        obey_flags: 0,
        show_flags: 0,
        print_space: 0,
        space_symbol: None,
        separator: None,
        epsilon_symbol: None,
        print_pairs: 0,
        apply_stack_ptr: 0,
        apply_stack_top: 0,
        oldflagneg: 0,
        outstringtop: 0,
        iterate_old: 0,
        iterator: 0,
        flagstates: Vec::new(),
        outstring: Vec::new(),
        instring: Vec::new(),
        sigs: Vec::new(),
        oldflagvalue: None,
        last_net: None,
        gstates: 0,
        gsigma: None,
        index_in: Vec::new(),
        index_out: Vec::new(),
        iptr: None,
        flag_list: None,
        flag_lookup: Vec::new(),
        searchstack: Vec::new(),
    });

    /* Init */
    h.iterate_old = 0;
    h.iterator = 0;
    h.instring = Vec::new();
    h.flag_list = None;
    h.flag_lookup = Vec::new();
    h.obey_flags = 1;
    h.show_flags = 0;
    h.print_space = 0;
    h.print_pairs = 0;
    h.separator = Some(":".to_string());
    h.epsilon_symbol = Some("0".to_string());
    // C: h->last_net = net (borrowed). DEVIATION from C (owns a clone; the
    // handle never mutates it, so observably equivalent for application).
    h.last_net = Some(Box::new(net.clone()));
    h.outstring = vec![0u8; DEFAULT_OUTSTRING_SIZE];
    h.outstringtop = DEFAULT_OUTSTRING_SIZE as i32;
    // *(h->outstring) = '\0' — already 0.
    h.gstates = 0; // net->states base
    h.gsigma = net.sigma.clone();
    h.printcount = 1;
    apply_create_statemap(&mut h, net);
    h.searchstack = vec![new_searchstack_frame(); DEFAULT_STACK_SIZE];
    h.apply_stack_top = DEFAULT_STACK_SIZE as i32;
    apply_stack_clear(&mut h);
    apply_create_sigarray(&mut h, net);
    h
}

// [spec:foma:def:apply.apply-reset-enumerator-fn]
// [spec:foma:sem:apply.apply-reset-enumerator-fn]
// [spec:foma:def:fomalib.apply-reset-enumerator-fn]
// [spec:foma:sem:fomalib.apply-reset-enumerator-fn]
pub fn apply_reset_enumerator(h: &mut ApplyHandle) {
    let statecount = h.last_net.as_ref().unwrap().statecount;
    for i in 0..statecount {
        h.marks[i as usize] = 0;
    }
    h.iterator = 0;
    h.iterate_old = 0;
}

/* ------------------------------------------------------------------ */
/* Idiomatic iterator front-ends (Wave 4 — additive sugar)            */
/* ------------------------------------------------------------------ */

/* Which C-shaped entry point an ApplyIter drives. */
#[derive(Clone, Copy)]
enum ApplyDir {
    Down,
    Up,
    Words,
    UpperWords,
    LowerWords,
    Enumerate,
}

/// Lazy iterator over the results of applying a word (or enumerating the
/// relation) through an [`ApplyHandle`]. Each `next()` yields one result
/// `String` by driving the existing C-shaped `apply_*` / NULL-resume
/// protocol: the first call seeds the search with the word, subsequent
/// calls resume it (`apply_down(h, None)`) until it is exhausted. This is
/// pure sugar over the resume protocol — it adds no new search behaviour.
pub struct ApplyIter<'a> {
    h: &'a mut ApplyHandle,
    dir: ApplyDir,
    word: Option<String>,
    started: bool,
    done: bool,
}

impl Iterator for ApplyIter<'_> {
    type Item = String;
    fn next(&mut self) -> Option<String> {
        if self.done {
            return None;
        }
        // down/up seed with the word on the first pull, then resume with None;
        // the enumerate-family entry points ignore the word entirely.
        let seed = if self.started {
            None
        } else {
            self.word.as_deref()
        };
        let result = match self.dir {
            ApplyDir::Down => apply_down(self.h, seed),
            ApplyDir::Up => apply_up(self.h, seed),
            ApplyDir::Words => apply_words(self.h),
            ApplyDir::UpperWords => apply_upper_words(self.h),
            ApplyDir::LowerWords => apply_lower_words(self.h),
            ApplyDir::Enumerate => apply_enumerate(self.h),
        };
        self.started = true;
        if result.is_none() {
            self.done = true;
        }
        result
    }
}

impl ApplyHandle {
    fn apply_iter(&mut self, dir: ApplyDir, word: Option<&str>) -> ApplyIter<'_> {
        ApplyIter {
            h: self,
            dir,
            word: word.map(str::to_string),
            started: false,
            done: false,
        }
    }

    /// Apply `word` downward (input → output side), yielding every output.
    pub fn down(&mut self, word: &str) -> ApplyIter<'_> {
        self.apply_iter(ApplyDir::Down, Some(word))
    }

    /// Apply `word` upward (output → input side), yielding every input.
    pub fn up(&mut self, word: &str) -> ApplyIter<'_> {
        self.apply_iter(ApplyDir::Up, Some(word))
    }

    /// Enumerate the language/relation, yielding both sides per path.
    pub fn words(&mut self) -> ApplyIter<'_> {
        self.apply_iter(ApplyDir::Words, None)
    }

    /// Enumerate the upper (input) side of the relation.
    pub fn upper_words(&mut self) -> ApplyIter<'_> {
        self.apply_iter(ApplyDir::UpperWords, None)
    }

    /// Enumerate the lower (output) side of the relation.
    pub fn lower_words(&mut self) -> ApplyIter<'_> {
        self.apply_iter(ApplyDir::LowerWords, None)
    }

    /// Drive `apply_enumerate` in whatever mode the handle is already set to.
    pub fn enumerate(&mut self) -> ApplyIter<'_> {
        self.apply_iter(ApplyDir::Enumerate, None)
    }
}

/* ------------------------------------------------------------------ */
/* Index (built externally by flookup/cgflookup)                      */
/* ------------------------------------------------------------------ */

// [spec:foma:def:apply.apply-clear-index-list-fn]
// [spec:foma:sem:apply.apply-clear-index-list-fn]
pub fn apply_clear_index_list(h: &ApplyHandle, index: &mut [Option<Box<ApplyStateIndex>>]) {
    // C walks each state's sigma_size cell array freeing overflow nodes while
    // avoiding the shared EPSILON tail. Here each state's chain is a plain
    // owned list; dropping it frees everything. Kept for id fidelity.
    let statecount = h.last_net.as_ref().unwrap().statecount;
    for i in 0..statecount as usize {
        if i < index.len() {
            index[i] = None;
        }
    }
}

// [spec:foma:def:apply.apply-clear-index-fn]
// [spec:foma:sem:apply.apply-clear-index-fn]
pub fn apply_clear_index(h: &mut ApplyHandle) {
    if !h.index_in.is_empty() {
        let mut idx = std::mem::take(&mut h.index_in);
        apply_clear_index_list(h, &mut idx);
        h.index_in = Vec::new();
    }
    if !h.index_out.is_empty() {
        let mut idx = std::mem::take(&mut h.index_out);
        apply_clear_index_list(h, &mut idx);
        h.index_out = Vec::new();
    }
}

// [spec:foma:def:apply.apply-index-fn]
// [spec:foma:sem:apply.apply-index-fn]
// [spec:foma:def:fomalib.apply-index-fn]
// [spec:foma:sem:fomalib.apply-index-fn]
pub fn apply_index(
    h: &mut ApplyHandle,
    inout: i32,
    densitycutoff: i32,
    mem_limit: i32,
    flags_only: i32,
) {
    if flags_only != 0 && h.has_flags == 0 {
        return;
    }
    let net = h.last_net.as_ref().unwrap();
    let statecount = net.statecount;
    let states = net.states.clone();

    /* Pass 1: get maxtrans (largest per-state count of real arcs). Both passes
    only close a state when the next line's state_no differs, so the final
    block before the -1 sentinel is never registered (latent bug, harmless). */
    let mut laststate = 0i32;
    let mut maxtrans = 0i32;
    let mut numtrans = 0i32;
    let mut i = 0usize;
    while states[i].state_no != -1 {
        if states[i].state_no != laststate {
            maxtrans = if numtrans > maxtrans {
                numtrans
            } else {
                maxtrans
            };
            numtrans = 0;
        }
        if states[i].target != -1 {
            numtrans += 1;
        }
        laststate = states[i].state_no;
        i += 1;
    }

    /* Pass 2: bucket states by their real-arc count. pre_index[count] holds
    the state numbers with that count (in encounter order). */
    let mut pre_index: Vec<Vec<i32>> = vec![Vec::new(); (maxtrans + 1) as usize];
    laststate = 0;
    maxtrans = 0;
    numtrans = 0;
    i = 0;
    while states[i].state_no != -1 {
        if states[i].state_no != laststate {
            pre_index[numtrans as usize].push(laststate);
            maxtrans = if numtrans > maxtrans {
                numtrans
            } else {
                maxtrans
            };
            numtrans = 0;
        }
        if states[i].target != -1 {
            numtrans += 1;
        }
        laststate = states[i].state_no;
        i += 1;
    }

    let mut cnt: u32 = round_up_to_power_of_two(
        (statecount as u32).wrapping_mul(std::mem::size_of::<usize>() as u32),
    );

    if cnt as i32 > mem_limit {
        // cnt -= ...; goto memlimitnoindex — no index built.
        if inout == APPLY_INDEX_INPUT {
            h.index_in = Vec::new();
        } else {
            h.index_out = Vec::new();
        }
        return;
    }

    // calloc(statecount) per-state chain heads.
    let mut indexed: Vec<Option<Box<ApplyStateIndex>>> = vec![None; statecount as usize];

    if h.has_flags != 0 && flags_only != 0 && h.flagstates.is_empty() {
        apply_mark_flagstates(h);
    }

    /* Decide which states get an index (densest first, mem-limited). */
    let mut allow: Vec<bool> = vec![false; statecount as usize];
    let mut stop = false;
    let cell_bytes = round_up_to_power_of_two(
        (h.sigma_size as u32).wrapping_mul(std::mem::size_of::<i32>() as u32 * 2),
    );
    let mut ii = maxtrans;
    while ii >= 0 && !stop {
        for &stateno in &pre_index[ii as usize] {
            if stateno < 0 {
                continue;
            }
            if ii < densitycutoff
                && !(h.has_flags != 0
                    && flags_only != 0
                    && !h.flagstates.is_empty()
                    && bittest(&h.flagstates, stateno))
            {
                continue;
            }
            cnt = cnt.wrapping_add(cell_bytes);
            if cnt as i32 > mem_limit {
                cnt = cnt.wrapping_sub(cell_bytes);
                stop = true;
                break;
            }
            allow[stateno as usize] = true;
        }
        ii -= 1;
    }

    /* Fill: build each allowed state's all-arcs chain (fsmptr = line index).
    Overflow/EPSILON-fallthrough of C is replaced by re-matching in
    apply_follow_next_arc (DEVIATION documented in the module header). The
    chain preserves arc-line order. */
    i = 0;
    // Collect each allowed state's arc line indices in line order.
    let mut per_state: Vec<Vec<i32>> = vec![Vec::new(); statecount as usize];
    while states[i].state_no != -1 {
        let sno = states[i].state_no;
        if allow[sno as usize] && states[i].target != -1 {
            per_state[sno as usize].push(i as i32);
        }
        i += 1;
    }
    for s in 0..statecount as usize {
        if !allow[s] {
            continue;
        }
        let mut chain: Option<Box<ApplyStateIndex>> = None;
        // build in reverse so the resulting chain is in ascending line order
        for &fsmptr in per_state[s].iter().rev() {
            chain = Some(Box::new(ApplyStateIndex {
                fsmptr,
                next: chain.take(),
            }));
        }
        indexed[s] = chain;
    }

    if inout == APPLY_INDEX_INPUT {
        h.index_in = indexed;
    } else {
        h.index_out = indexed;
    }
}

/* ------------------------------------------------------------------ */
/* Search / matching                                                  */
/* ------------------------------------------------------------------ */

// [spec:foma:def:apply.apply-binarysearch-fn]
// [spec:foma:sem:apply.apply-binarysearch-fn]
pub fn apply_binarysearch(h: &mut ApplyHandle) -> i32 {
    let mut thisptr: i32;
    let mut lastptr: i32;
    let mut midptr: i32;
    let mut nextsym: i32;
    let seeksym: i32;
    let thisstate: i32;

    h.curr_ptr = h.ptr;
    thisptr = h.curr_ptr;
    nextsym = if (h.mode & DOWN) == DOWN {
        l_in(h, h.curr_ptr)
    } else {
        l_out(h, h.curr_ptr)
    };
    if nextsym == EPSILON {
        return 1;
    }
    if nextsym == -1 {
        return 0;
    }
    if h.ipos >= h.current_instring_length {
        return 0;
    }
    seeksym = h.sigmatch_array[h.ipos as usize].signumber;
    if seeksym == nextsym || (nextsym == UNKNOWN && seeksym == IDENTITY) {
        return 1;
    }

    thisstate = l_state_no(h, thisptr);
    lastptr = h.statemap[thisstate as usize] + h.numlines[thisstate as usize] - 1;
    thisptr += 1;

    if seeksym == IDENTITY || lastptr - thisptr < APPLY_BINSEARCH_THRESHOLD {
        while thisptr <= lastptr {
            nextsym = if (h.mode & DOWN) == DOWN {
                l_in(h, thisptr)
            } else {
                l_out(h, thisptr)
            };
            if (nextsym == seeksym) || (nextsym == UNKNOWN && seeksym == IDENTITY) {
                h.curr_ptr = thisptr;
                return 1;
            }
            if nextsym > seeksym || nextsym == -1 {
                return 0;
            }
            thisptr += 1;
        }
        return 0;
    }

    loop {
        if thisptr > lastptr {
            return 0;
        }
        midptr = (thisptr + lastptr) / 2;
        nextsym = if (h.mode & DOWN) == DOWN {
            l_in(h, midptr)
        } else {
            l_out(h, midptr)
        };
        if seeksym < nextsym {
            lastptr = midptr - 1;
            continue;
        } else if seeksym > nextsym {
            thisptr = midptr + 1;
            continue;
        } else {
            while (if (h.mode & DOWN) == DOWN {
                l_in(h, midptr - 1)
            } else {
                l_out(h, midptr - 1)
            }) == seeksym
            {
                midptr -= 1; /* Find first match in case of ties */
            }
            h.curr_ptr = midptr;
            return 1;
        }
    }
}

// [spec:foma:def:apply.apply-follow-next-arc-fn]
// [spec:foma:sem:apply.apply-follow-next-arc-fn]
pub fn apply_follow_next_arc(h: &mut ApplyHandle) -> i32 {
    let fname: Option<String>;
    let fvalue: Option<String>;
    let mut eatupi: i32;
    let eatupo: i32;
    let mut symin: i32;
    let mut symout: i32;
    let fneg: i32;
    let mut vcount: i32;
    let mut marksource: i32;
    let mut marktarget: i32;

    if h.state_has_index != 0 {
        while h.iptr.is_some() && h.iptr.as_ref().unwrap().fsmptr != -1 {
            let fsmptr = h.iptr.as_ref().unwrap().fsmptr;
            h.ptr = fsmptr;
            h.curr_ptr = fsmptr;
            if (h.mode & DOWN) == DOWN {
                symin = l_in(h, h.curr_ptr);
                symout = l_out(h, h.curr_ptr);
            } else {
                symin = l_out(h, h.curr_ptr);
                symout = l_in(h, h.curr_ptr);
            }

            let src_sn = l_state_no(h, h.ptr);
            marksource = h.marks[src_sn as usize];
            let tgt = l_target(h, h.curr_ptr);
            let tgt_line = h.statemap[tgt as usize];
            let tgt_sn = l_state_no(h, tgt_line);
            marktarget = h.marks[tgt_sn as usize];
            eatupi = apply_match_length(h, symin);
            if !(eatupi == -1 || -1 - h.ipos - eatupi == marktarget) {
                eatupi = apply_match_str(h, symin, h.ipos);
                if eatupi != -1 {
                    eatupo = apply_append(h, h.curr_ptr, symout);
                    if h.obey_flags != 0
                        && h.has_flags != 0
                        && (h.flag_lookup[symin as usize].r#type
                            & (FLAG_UNIFY | FLAG_CLEAR | FLAG_POSITIVE | FLAG_NEGATIVE))
                            != 0
                    {
                        fname = h.flag_lookup[symin as usize].name.clone();
                        fvalue = h.oldflagvalue.clone();
                        fneg = h.oldflagneg;
                    } else {
                        fname = None;
                        fvalue = None;
                        fneg = 0;
                    }
                    apply_stack_push(h, marksource, fname, fvalue, fneg);
                    let tgt2 = l_target(h, h.curr_ptr);
                    h.ptr = h.statemap[tgt2 as usize];
                    h.ipos += eatupi;
                    h.opos += eatupo;
                    apply_set_iptr(h);
                    return 1;
                }
            }
            h.iptr = h.iptr.as_ref().unwrap().next.clone();
        }
        return 0;
    } else if (h.binsearch != 0 && h.has_flags == 0)
        || (h.binsearch != 0 && {
            let sn = l_state_no(h, h.ptr);
            !bittest(&h.flagstates, sn)
        })
    {
        loop {
            if apply_binarysearch(h) != 0 {
                if (h.mode & DOWN) == DOWN {
                    symin = l_in(h, h.curr_ptr);
                    symout = l_out(h, h.curr_ptr);
                } else {
                    symin = l_out(h, h.curr_ptr);
                    symout = l_in(h, h.curr_ptr);
                }

                let src_sn = l_state_no(h, h.ptr);
                marksource = h.marks[src_sn as usize];
                let tgt = l_target(h, h.curr_ptr);
                let tgt_line = h.statemap[tgt as usize];
                let tgt_sn = l_state_no(h, tgt_line);
                marktarget = h.marks[tgt_sn as usize];

                eatupi = apply_match_length(h, symin);
                if eatupi != -1 && -1 - h.ipos - eatupi != marktarget {
                    eatupi = apply_match_str(h, symin, h.ipos);
                    if eatupi != -1 {
                        eatupo = apply_append(h, h.curr_ptr, symout);

                        apply_stack_push(h, marksource, None, None, 0);

                        let tgt2 = l_target(h, h.curr_ptr);
                        h.ptr = h.statemap[tgt2 as usize];
                        h.ipos += eatupi;
                        h.opos += eatupo;
                        apply_set_iptr(h);
                        return 1;
                    }
                }
                let a = l_state_no(h, h.curr_ptr);
                let b = l_state_no(h, h.curr_ptr + 1);
                if a == b {
                    h.curr_ptr += 1;
                    h.ptr = h.curr_ptr;
                    if l_target(h, h.curr_ptr) == -1 {
                        return 0;
                    }
                    continue;
                }
            }
            return 0;
        }
    } else {
        h.curr_ptr = h.ptr;
        while l_state_no(h, h.curr_ptr) == l_state_no(h, h.ptr) && l_in(h, h.curr_ptr) != -1 {
            /* Select one random arc to follow out of all outgoing arcs */
            if (h.mode & RANDOM) == RANDOM {
                let mut vc = 0;
                h.curr_ptr = h.ptr;
                while l_state_no(h, h.curr_ptr) == l_state_no(h, h.ptr) && l_in(h, h.curr_ptr) != -1
                {
                    vc += 1;
                    h.curr_ptr += 1;
                }
                vcount = vc;
                if vcount > 0 {
                    h.curr_ptr = h.ptr + (rand() % vcount);
                } else {
                    h.curr_ptr = h.ptr;
                }
            }

            if (h.mode & DOWN) == DOWN {
                symin = l_in(h, h.curr_ptr);
                symout = l_out(h, h.curr_ptr);
            } else {
                symin = l_out(h, h.curr_ptr);
                symout = l_in(h, h.curr_ptr);
            }

            let src_sn = l_state_no(h, h.ptr);
            marksource = h.marks[src_sn as usize];
            let tgt = l_target(h, h.curr_ptr);
            let tgt_line = h.statemap[tgt as usize];
            let tgt_sn = l_state_no(h, tgt_line);
            marktarget = h.marks[tgt_sn as usize];

            eatupi = apply_match_length(h, symin);

            if eatupi == -1 || -1 - h.ipos - eatupi == marktarget {
                h.curr_ptr += 1;
                continue;
            }
            eatupi = apply_match_str(h, symin, h.ipos);
            if eatupi != -1 {
                eatupo = apply_append(h, h.curr_ptr, symout);
                if h.obey_flags != 0
                    && h.has_flags != 0
                    && (h.flag_lookup[symin as usize].r#type
                        & (FLAG_UNIFY | FLAG_CLEAR | FLAG_POSITIVE | FLAG_NEGATIVE))
                        != 0
                {
                    fname = h.flag_lookup[symin as usize].name.clone();
                    fvalue = h.oldflagvalue.clone();
                    fneg = h.oldflagneg;
                } else {
                    fname = None;
                    fvalue = None;
                    fneg = 0;
                }

                apply_stack_push(h, marksource, fname, fvalue, fneg);

                let tgt2 = l_target(h, h.curr_ptr);
                h.ptr = h.statemap[tgt2 as usize];
                h.ipos += eatupi;
                h.opos += eatupo;
                apply_set_iptr(h);
                return 1;
            }
            h.curr_ptr += 1;
        }
        return 0;
    }
}

// [spec:foma:def:apply.apply-return-string-fn]
// [spec:foma:sem:apply.apply-return-string-fn]
pub fn apply_return_string(h: &mut ApplyHandle) -> Option<String> {
    /* Stick a 0 to endpos to avoid getting old accumulated gunk strings */
    let opos = h.opos as usize;
    if h.outstring.len() <= opos {
        h.outstring.resize(opos + 1, 0);
    }
    h.outstring[opos] = 0;
    if (h.mode & RANDOM) == RANDOM {
        /* To end or not to end */
        if rand() % 2 == 0 {
            apply_stack_clear(h);
            h.iterator = 0;
            h.iterate_old = 0;
            return Some(outstring_result(h));
        }
    } else {
        return Some(outstring_result(h));
    }
    None
}

// [spec:foma:def:apply.apply-mark-state-fn]
// [spec:foma:sem:apply.apply-mark-state-fn]
pub fn apply_mark_state(h: &mut ApplyHandle) {
    if (h.mode & RANDOM) != RANDOM {
        let sn = l_state_no(h, h.ptr) as usize;
        if h.marks[sn] == h.ipos + 1 {
            h.marks[sn] = -(h.ipos + 1);
        } else {
            h.marks[sn] = h.ipos + 1;
        }
    }
}

// [spec:foma:def:apply.apply-skip-this-arc-fn]
// [spec:foma:sem:apply.apply-skip-this-arc-fn]
pub fn apply_skip_this_arc(h: &mut ApplyHandle) {
    if h.iptr.is_some() {
        h.ptr = h.iptr.as_ref().unwrap().fsmptr;
        h.iptr = h.iptr.as_ref().unwrap().next.clone();
    } else {
        h.ptr += 1;
    }
}

// [spec:foma:def:apply.apply-at-last-arc-fn]
// [spec:foma:sem:apply.apply-at-last-arc-fn]
pub fn apply_at_last_arc(h: &ApplyHandle) -> i32 {
    let seeksym: i32;
    let nextsym: i32;
    if h.state_has_index != 0 {
        let iptr = h.iptr.as_ref().unwrap();
        if iptr.next.is_none() || iptr.next.as_ref().unwrap().fsmptr == -1 {
            return 1;
        }
    } else if (h.binsearch != 0 && h.has_flags == 0)
        || (h.binsearch != 0 && !bittest(&h.flagstates, l_state_no(h, h.ptr)))
    {
        if l_state_no(h, h.ptr) != l_state_no(h, h.ptr + 1) {
            return 1;
        }
        // C reads sigmatch_array[ipos] without bounds check; at end-of-input
        // this is an OOB/stale read. DEVIATION from C (guard; use a sentinel
        // so `seeksym < nextsym` cannot early-terminate).
        seeksym = if (h.ipos as usize) < h.sigmatch_array.len() {
            h.sigmatch_array[h.ipos as usize].signumber
        } else {
            i32::MAX
        };
        nextsym = if (h.mode & DOWN) == DOWN {
            l_in(h, h.ptr)
        } else {
            l_out(h, h.ptr)
        };
        if nextsym == -1 || seeksym < nextsym {
            return 1;
        }
    } else if l_state_no(h, h.ptr) != l_state_no(h, h.ptr + 1) {
        return 1;
    }
    0
}

/* map h->ptr (line pointer) to h->iptr (index pointer) */
// [spec:foma:def:apply.apply-set-iptr-fn]
// [spec:foma:sem:apply.apply-set-iptr-fn]
pub fn apply_set_iptr(h: &mut ApplyHandle) {
    // Select index for the direction; if absent, leave iptr/state_has_index.
    let is_down = (h.mode & DOWN) == DOWN;
    let idx_empty = if is_down {
        h.index_in.is_empty()
    } else {
        h.index_out.is_empty()
    };
    if idx_empty {
        return;
    }

    h.iptr = None;
    h.state_has_index = 0;
    let stateno = l_state_no(h, h.ptr);
    if stateno < 0 {
        return;
    }

    // DEVIATION from C: index[state] is the state's full arc chain; seeksym
    // slot-selection is not used (apply_follow_next_arc re-matches). The C
    // unguarded sigmatch_array[ipos] read is therefore avoided entirely.
    let chain = if is_down {
        h.index_in[stateno as usize].clone()
    } else {
        h.index_out[stateno as usize].clone()
    };
    if chain.is_none() {
        return;
    }
    h.state_has_index = 1;
    let c = chain.unwrap();
    if c.fsmptr == -1 {
        // A state that is indexed but has no candidate arcs.
        h.iptr = None;
    } else {
        h.iptr = Some(c);
    }
    h.state_has_index = 1;
}

// [spec:foma:def:apply.apply-net-fn]
// [spec:foma:sem:apply.apply-net-fn]
pub fn apply_net(h: &mut ApplyHandle) -> Option<String> {
    // Program counter reproducing the C goto structure (L1/L2/resume + loop).
    enum Pc {
        Loop,
        L1,
        L2,
        Resume,
    }

    let mut pc: Pc;

    if h.iterate_old == 1 {
        // goto resume
        pc = Pc::Resume;
    } else {
        h.iptr = None;
        h.ptr = 0;
        h.ipos = 0;
        h.opos = 0;
        apply_set_iptr(h);

        apply_stack_clear(h);

        if h.has_flags != 0 {
            apply_clear_flags(h);
        }
        // goto L2
        pc = Pc::L2;
    }

    loop {
        match pc {
            Pc::Loop => {
                if apply_stack_isempty(h) != 0 {
                    break;
                }
                apply_stack_pop(h);
                /* If last line was popped */
                if apply_at_last_arc(h) != 0 {
                    let sn = l_state_no(h, h.ptr);
                    h.marks[sn as usize] = 0; /* Unmark */
                    pc = Pc::Loop; /* pop next */
                    continue;
                }
                apply_skip_this_arc(h); /* skip old pushed arc */
                pc = Pc::L1;
                continue;
            }
            Pc::L1 => {
                if apply_follow_next_arc(h) == 0 {
                    let sn = l_state_no(h, h.ptr);
                    h.marks[sn as usize] = 0; /* Unmark */
                    pc = Pc::Loop; /* pop next */
                    continue;
                }
                pc = Pc::L2;
                continue;
            }
            Pc::L2 => {
                /* Print accumulated string upon entry to state */
                if l_final(h, h.ptr) == 1
                    && (h.ipos == h.current_instring_length || (h.mode & ENUMERATE) == ENUMERATE)
                {
                    if let Some(returnstring) = apply_return_string(h) {
                        return Some(returnstring);
                    }
                }
                pc = Pc::Resume;
                continue;
            }
            Pc::Resume => {
                apply_mark_state(h); /* Mark upon arrival to new state */
                pc = Pc::L1;
                continue;
            }
        }
    }

    if (h.mode & RANDOM) == RANDOM {
        apply_stack_clear(h);
        h.iterator = 0;
        h.iterate_old = 0;
        // RANDOM-mode stale-terminated buffer: outstring is whatever the last
        // return-string wrote (empty on a fresh run). Literal behaviour.
        return Some(outstring_result(h));
    }
    apply_stack_clear(h);
    None
}

// [spec:foma:def:apply.apply-append-fn]
// [spec:foma:sem:apply.apply-append-fn+1]
pub fn apply_append(h: &mut ApplyHandle, cptr: i32, sym: i32) -> i32 {
    let symin = l_in(h, cptr);
    let symout = l_out(h, cptr);
    let mut astring: Vec<u8> = h.sigs[symin as usize]
        .symbol
        .as_deref()
        .unwrap_or("")
        .as_bytes()
        .to_vec();
    let mut alen = h.sigs[symin as usize].length;
    let mut bstring: Vec<u8> = h.sigs[symout as usize]
        .symbol
        .as_deref()
        .unwrap_or("")
        .as_bytes()
        .to_vec();
    let mut blen = h.sigs[symout as usize].length;
    let sep: Vec<u8> = h.separator.as_deref().unwrap_or("").as_bytes().to_vec();
    let seplen = sep.len() as i32; /* strlen(h->separator) */
    let mut len: i32;

    while alen + blen + h.opos + 2 + seplen >= h.outstringtop {
        let newtop = (h.outstringtop * 2) as usize;
        h.outstring.resize(newtop, 0);
        h.outstringtop *= 2;
    }

    // Track pointer-equality of astring/bstring (see module notes): true when
    // both were suppressed to the "" literal, or when neither was and
    // symin == symout (same sigs slot).
    let mut a_empty_lit = false;
    let mut b_empty_lit = false;

    if h.has_flags != 0 && h.show_flags == 0 && h.flag_lookup[symin as usize].r#type != 0 {
        astring = Vec::new();
        alen = 0;
        a_empty_lit = true;
    }
    if h.has_flags != 0 && h.show_flags == 0 && h.flag_lookup[symout as usize].r#type != 0 {
        bstring = Vec::new();
        blen = 0;
        b_empty_lit = true;
    }
    let astring_eq_bstring =
        (a_empty_lit && b_empty_lit) || (!a_empty_lit && !b_empty_lit && symin == symout);

    len = 0;

    if (h.mode & ENUMERATE) == ENUMERATE {
        /* Print both sides separated by colon (if printing "words") */
        if (h.mode & (UPPER | LOWER)) == (UPPER | LOWER) {
            if astring_eq_bstring {
                buf_strcpy(&mut h.outstring, h.opos as usize, &astring);
                len = alen;
            } else {
                buf_strcpy(&mut h.outstring, h.opos as usize, &astring);
                buf_strcpy(&mut h.outstring, (h.opos + alen) as usize, &sep);
                buf_strcpy(
                    &mut h.outstring,
                    (h.opos + alen + seplen) as usize,
                    &bstring,
                );
                len = alen + blen + seplen;
            }
        }

        /* Print one side only */
        if (h.mode & (UPPER | LOWER)) != (UPPER | LOWER) {
            if symin == EPSILON {
                astring = Vec::new();
                alen = 0;
            }
            if symout == EPSILON {
                bstring = Vec::new();
                blen = 0;
            }
            let pstring: &[u8];
            if (h.mode & (UPPER | LOWER)) == UPPER {
                pstring = &astring;
                len = alen;
            } else {
                pstring = &bstring;
                len = blen;
            }
            buf_memcpy(&mut h.outstring, h.opos as usize, pstring, len as usize);
        }
    }
    if (h.mode & ENUMERATE) != ENUMERATE {
        /* Print pairs is ON and symbols are different */
        if h.print_pairs != 0 && (symin != symout) {
            if symin == UNKNOWN && (h.mode & DOWN) == DOWN {
                // C: strncpy(astring, instring+ipos, 1) — writes into the shared
                // "?" literal (UB). DEVIATION from C: set the local display to
                // the single input byte; the shared sigs entry is not mutated.
                if (h.ipos as usize) < h.instring.len() {
                    astring = vec![h.instring[h.ipos as usize]];
                }
            }
            if symout == UNKNOWN && (h.mode & UP) == UP {
                if (h.ipos as usize) < h.instring.len() {
                    bstring = vec![h.instring[h.ipos as usize]];
                }
            }
            buf_strcpy(&mut h.outstring, h.opos as usize, b"<");
            buf_strcpy(&mut h.outstring, (h.opos + 1) as usize, &astring);
            buf_strcpy(&mut h.outstring, (h.opos + alen + 1) as usize, &sep);
            buf_strcpy(
                &mut h.outstring,
                (h.opos + alen + 1 + seplen) as usize,
                &bstring,
            );
            buf_strcpy(
                &mut h.outstring,
                (h.opos + alen + blen + 1 + seplen) as usize,
                b">",
            );
            len = alen + blen + 2 + seplen;
        } else if sym == IDENTITY {
            /* Apply up/down */
            let idlen = h.sigmatch_array[h.ipos as usize].consumes;
            // C: strncpy(outstring+opos, instring+ipos, idlen) then a NUL.
            // Budget only covered alen+blen; buf_strcpy grows to stay safe.
            let ip = h.ipos as usize;
            let end = (ip + idlen as usize).min(h.instring.len());
            let src: Vec<u8> = h.instring[ip..end].to_vec();
            // reproduce copying exactly idlen bytes (zero-pad if input short)
            let mut chunk = src;
            chunk.resize(idlen as usize, 0);
            buf_memcpy(&mut h.outstring, h.opos as usize, &chunk, idlen as usize);
            let nul_at = (h.opos + idlen) as usize;
            if h.outstring.len() <= nul_at {
                h.outstring.resize(nul_at + 1, 0);
            }
            h.outstring[nul_at] = 0;
            len = idlen;
        } else if sym == EPSILON {
            return 0;
        } else {
            let pstring: &[u8];
            if (h.mode & DOWN) == DOWN {
                pstring = &bstring;
                len = blen;
            } else {
                pstring = &astring;
                len = alen;
            }
            buf_memcpy(&mut h.outstring, h.opos as usize, pstring, len as usize);
        }
    }
    if h.print_space != 0 && len > 0 {
        let space: Vec<u8> = h.space_symbol.as_deref().unwrap_or("").as_bytes().to_vec();
        buf_strcpy(&mut h.outstring, (h.opos + len) as usize, &space);
        // [spec:foma:sem:apply.apply-append-fn+1] advance by the separator's full
        // byte length (C did `len++`, so a multi-byte space symbol had all but its
        // first byte overwritten by the next append).
        len += space.len() as i32;
    }
    len
}

// [spec:foma:def:apply.apply-match-length-fn]
// [spec:foma:sem:apply.apply-match-length-fn]
pub fn apply_match_length(h: &ApplyHandle, symbol: i32) -> i32 {
    if symbol == EPSILON {
        return 0;
    }
    if h.has_flags != 0 && h.flag_lookup[symbol as usize].r#type != 0 {
        return 0;
    }
    if (h.mode & ENUMERATE) == ENUMERATE {
        return 0;
    }
    if h.ipos >= h.current_instring_length {
        return -1;
    }
    if h.sigmatch_array[h.ipos as usize].signumber == symbol {
        return h.sigmatch_array[h.ipos as usize].consumes;
    }
    if (symbol == IDENTITY) || (symbol == UNKNOWN) {
        if h.sigmatch_array[h.ipos as usize].signumber == IDENTITY {
            return h.sigmatch_array[h.ipos as usize].consumes;
        }
    }
    -1
}

// [spec:foma:def:apply.apply-match-str-fn]
// [spec:foma:sem:apply.apply-match-str-fn]
pub fn apply_match_str(h: &mut ApplyHandle, symbol: i32, position: i32) -> i32 {
    if (h.mode & ENUMERATE) == ENUMERATE {
        if h.has_flags != 0 && h.flag_lookup[symbol as usize].r#type != 0 {
            if h.obey_flags == 0 {
                return 0;
            }
            let ftype = h.flag_lookup[symbol as usize].r#type;
            let fname = h.flag_lookup[symbol as usize].name.clone();
            let fvalue = h.flag_lookup[symbol as usize].value.clone();
            if apply_check_flag(h, ftype, fname.as_deref(), fvalue.as_deref()) == SUCCEED {
                return 0;
            } else {
                return -1;
            }
        }
        return 0;
    }

    if symbol == EPSILON {
        return 0;
    }

    /* If symbol is a flag, we need to check consistency */
    if h.has_flags != 0 && h.flag_lookup[symbol as usize].r#type != 0 {
        if h.obey_flags == 0 {
            return 0;
        }
        let ftype = h.flag_lookup[symbol as usize].r#type;
        let fname = h.flag_lookup[symbol as usize].name.clone();
        let fvalue = h.flag_lookup[symbol as usize].value.clone();
        if apply_check_flag(h, ftype, fname.as_deref(), fvalue.as_deref()) == SUCCEED {
            return 0;
        } else {
            return -1;
        }
    }

    if position >= h.current_instring_length {
        return -1;
    }
    if h.sigmatch_array[position as usize].signumber == symbol {
        return h.sigmatch_array[position as usize].consumes;
    }
    if (symbol == IDENTITY) || (symbol == UNKNOWN) {
        if h.sigmatch_array[position as usize].signumber == IDENTITY {
            return h.sigmatch_array[position as usize].consumes;
        }
    }
    -1
}

// [spec:foma:def:apply.apply-create-statemap-fn]
// [spec:foma:sem:apply.apply-create-statemap-fn]
pub fn apply_create_statemap(h: &mut ApplyHandle, net: &Fsm) {
    let statecount = net.statecount;
    h.statemap = vec![0i32; statecount as usize];
    h.marks = vec![0i32; statecount as usize];
    h.numlines = vec![0i32; statecount as usize];

    for i in 0..statecount as usize {
        h.numlines[i] = 0; /* Only needed in binary search */
        h.statemap[i] = -1;
        h.marks[i] = 0;
    }
    let mut i = 0usize;
    while net.states[i].state_no != -1 {
        let sn = net.states[i].state_no as usize;
        h.numlines[sn] += 1;
        if h.statemap[sn] == -1 {
            h.statemap[sn] = i as i32;
        }
        i += 1;
    }
}

// [spec:foma:def:apply.apply-add-sigma-trie-fn]
// [spec:foma:sem:apply.apply-add-sigma-trie-fn]
pub fn apply_add_sigma_trie(h: &mut ApplyHandle, number: i32, symbol: &str, len: i32) {
    // See module notes: the trie is a flat arena in h.sigma_trie. A cell's
    // child-level base index is stored in a synthetic `next` node's signum.
    let bytes = symbol.as_bytes();
    let mut base = 0usize; /* root level */
    for i in 0..len as usize {
        let cell = base + bytes[i] as usize;
        if i == (len as usize - 1) {
            h.sigma_trie[cell].signum = number;
        } else if h.sigma_trie[cell].next.is_none() {
            let child_base = h.sigma_trie.len();
            h.sigma_trie.resize(
                child_base + 256,
                SigmaTrie {
                    signum: 0,
                    next: None,
                },
            );
            h.sigma_trie[cell].next = Some(Box::new(SigmaTrie {
                signum: child_base as i32,
                next: None,
            }));
            base = child_base;
        } else {
            base = h.sigma_trie[cell].next.as_ref().unwrap().signum as usize;
        }
    }
}

// [spec:foma:def:apply.apply-mark-flagstates-fn]
// [spec:foma:sem:apply.apply-mark-flagstates-fn]
pub fn apply_mark_flagstates(h: &mut ApplyHandle) {
    if h.has_flags == 0 || h.flag_lookup.is_empty() {
        return;
    }
    // free previous
    h.flagstates = Vec::new();
    let statecount = h.last_net.as_ref().unwrap().statecount;
    let mut fs = vec![0u8; bitnslots(statecount)];
    let net = h.last_net.as_ref().unwrap();
    let mut i = 0usize;
    while net.states[i].state_no != -1 {
        let ln = &net.states[i];
        if ln.target != -1 {
            if h.flag_lookup[ln.r#in as usize].r#type != 0 {
                bitset(&mut fs, ln.state_no);
            }
            if h.flag_lookup[ln.out as usize].r#type != 0 {
                bitset(&mut fs, ln.state_no);
            }
        }
        i += 1;
    }
    h.flagstates = fs;
}

// [spec:foma:def:apply.apply-create-sigarray-fn]
// [spec:foma:sem:apply.apply-create-sigarray-fn]
pub fn apply_create_sigarray(h: &mut ApplyHandle, net: &Fsm) {
    let maxsigma = sigma_max(net.sigma.as_deref());
    h.sigma_size = maxsigma + 1;
    // Default size created at init, resized later if necessary.
    h.sigmatch_array = vec![
        SigmatchArray {
            signumber: 0,
            consumes: 0
        };
        1024
    ];
    h.sigmatch_array_size = 1024;

    h.sigs = vec![
        Sigs {
            symbol: None,
            length: 0
        };
        (maxsigma + 1) as usize
    ];
    h.has_flags = 0;
    h.flag_list = None;

    // Root level of the trie arena (256 cells) + bookkeeping node.
    h.sigma_trie = vec![
        SigmaTrie {
            signum: 0,
            next: None
        };
        256
    ];
    h.sigma_trie_arrays = Some(Box::new(SigmaTrieArrays {
        arr: Vec::new(),
        next: None,
    }));

    // Snapshot sigma (number, symbol) so we can mutate h while iterating.
    let mut sig_entries: Vec<(i32, String)> = Vec::new();
    {
        let mut sig = h.gsigma.as_deref();
        while let Some(s) = sig {
            if s.number == -1 {
                break;
            }
            sig_entries.push((s.number, s.symbol.as_deref().unwrap_or("").to_string()));
            sig = s.next.as_deref();
        }
    }

    for (number, symbol) in &sig_entries {
        if flag_check(symbol) != 0 {
            h.has_flags = 1;
            apply_add_flag(h, flag_get_name(symbol));
        }
        h.sigs[*number as usize].symbol = Some(symbol.clone());
        h.sigs[*number as usize].length = symbol.len() as i32;
        /* Add sigma entry to trie */
        if *number > IDENTITY {
            let len = h.sigs[*number as usize].length;
            apply_add_sigma_trie(h, *number, symbol, len);
        }
    }
    if maxsigma >= IDENTITY {
        let eps = h.epsilon_symbol.clone();
        let epslen = eps.as_deref().unwrap_or("").len() as i32;
        h.sigs[EPSILON as usize].symbol = eps;
        h.sigs[EPSILON as usize].length = epslen;
        h.sigs[UNKNOWN as usize].symbol = Some("?".to_string());
        h.sigs[UNKNOWN as usize].length = 1;
        h.sigs[IDENTITY as usize].symbol = Some("@".to_string());
        h.sigs[IDENTITY as usize].length = 1;
    }
    if h.has_flags != 0 {
        h.flag_lookup = vec![
            FlagLookup {
                r#type: 0,
                name: None,
                value: None,
            };
            (maxsigma + 1) as usize
        ];
        // C's second walk lacks the number != -1 guard; snapshot everything
        // including a possible terminator (number == -1 would be OOB in C —
        // in practice the sigma list has no such flag entry).
        let mut sig = h.gsigma.as_deref();
        let mut entries2: Vec<(i32, String)> = Vec::new();
        while let Some(s) = sig {
            entries2.push((s.number, s.symbol.as_deref().unwrap_or("").to_string()));
            sig = s.next.as_deref();
        }
        for (number, symbol) in &entries2 {
            if flag_check(symbol) != 0 {
                h.flag_lookup[*number as usize].r#type = flag_get_type(symbol);
                h.flag_lookup[*number as usize].name = flag_get_name(symbol);
                h.flag_lookup[*number as usize].value = flag_get_value(symbol);
            }
        }
        apply_mark_flagstates(h);
    }
}

// [spec:foma:def:apply.apply-create-sigmatch-fn]
// [spec:foma:sem:apply.apply-create-sigmatch-fn]
pub fn apply_create_sigmatch(h: &mut ApplyHandle) {
    /* We create a sigmatch array only in case we match against a real string */
    if (h.mode & ENUMERATE) == ENUMERATE {
        return;
    }
    let symbol: Vec<u8> = h.instring.clone();
    let inlen = symbol.len();
    h.current_instring_length = inlen as i32;
    if inlen as i32 >= h.sigmatch_array_size {
        h.sigmatch_array = vec![
            SigmatchArray {
                signumber: 0,
                consumes: 0
            };
            inlen
        ];
        h.sigmatch_array_size = inlen as i32;
    }
    /* Find longest match in alphabet at current position */
    let mut i = 0usize;
    while i < inlen {
        let mut base = 0usize; /* root level of trie arena */
        let mut lastmatch = 0i32;
        let mut j = 0usize;
        loop {
            if i + j >= symbol.len() {
                break;
            }
            let cell = base + symbol[i + j] as usize;
            let signum = h.sigma_trie[cell].signum;
            let child = h.sigma_trie[cell].next.as_ref().map(|n| n.signum as usize);
            if signum != 0 {
                lastmatch = signum;
                match child {
                    None => break,
                    Some(cb) => base = cb,
                }
            } else if let Some(cb) = child {
                base = cb;
            } else {
                break;
            }
            j += 1;
        }
        let mut consumes: i32;
        if lastmatch != 0 {
            h.sigmatch_array[i].signumber = lastmatch;
            consumes = h.sigs[lastmatch as usize].length;
        } else {
            /* Not found in trie */
            h.sigmatch_array[i].signumber = IDENTITY;
            consumes = utf8skip(&symbol[i..]) + 1;
        }

        /* Merge trailing Unicode combining characters into one ? (IDENTITY). */
        loop {
            let pos = i + consumes as usize;
            let slice: &[u8] = if pos < symbol.len() {
                &symbol[pos..]
            } else {
                &[]
            };
            let cons = utf8iscombining(slice);
            if cons == 0 {
                break;
            }
            h.sigmatch_array[i].signumber = IDENTITY;
            consumes += cons;
        }
        h.sigmatch_array[i].consumes = consumes;

        i += consumes as usize;
    }
}

// [spec:foma:def:apply.apply-add-flag-fn]
// [spec:foma:sem:apply.apply-add-flag-fn]
pub fn apply_add_flag(h: &mut ApplyHandle, name: Option<String>) {
    if h.flag_list.is_none() {
        h.flag_list = Some(Box::new(FlagList {
            name,
            value: None,
            neg: 0,
            next: None,
        }));
        return;
    }
    // Walk to the tail; if a node's name matches, return unchanged.
    let nm = name.as_deref();
    let mut cur = h.flag_list.as_deref_mut();
    loop {
        let node = cur.unwrap();
        if node.name.as_deref() == nm {
            return;
        }
        if node.next.is_none() {
            node.next = Some(Box::new(FlagList {
                name,
                value: None,
                neg: 0,
                next: None,
            }));
            return;
        }
        cur = node.next.as_deref_mut();
    }
}

// [spec:foma:def:apply.apply-clear-flags-fn]
// [spec:foma:sem:apply.apply-clear-flags-fn]
pub fn apply_clear_flags(h: &mut ApplyHandle) {
    let mut cur = h.flag_list.as_deref_mut();
    while let Some(node) = cur {
        node.value = None;
        node.neg = 0;
        cur = node.next.as_deref_mut();
    }
}

/* Check for flag consistency by looking at the current states of flags */
// [spec:foma:def:apply.apply-check-flag-fn]
// [spec:foma:sem:apply.apply-check-flag-fn]
pub fn apply_check_flag(
    h: &mut ApplyHandle,
    r#type: i32,
    name: Option<&str>,
    value: Option<&str>,
) -> i32 {
    // Find flist by name. C dereferences NULL if not found (unreachable).
    let name = name.unwrap_or("");
    // Save current value/neg into oldflagvalue/oldflagneg.
    {
        let flist = flag_find_mut(&mut h.flag_list, name);
        let flist = flist.expect("flag not registered"); // DEVIATION from C (NULL deref; unreachable)
        h.oldflagvalue = flist.value.clone();
        h.oldflagneg = flist.neg as i32;
    }

    if r#type == FLAG_UNIFY {
        let flist = flag_find_mut(&mut h.flag_list, name).unwrap();
        if flist.value.is_none() {
            flist.value = value.map(|s| s.to_string());
            return SUCCEED;
        } else if value == flist.value.as_deref() && flist.neg == 0 {
            return SUCCEED;
        } else if value != flist.value.as_deref() && flist.neg == 1 {
            flist.value = value.map(|s| s.to_string());
            flist.neg = 0;
            return SUCCEED;
        }
        return FAIL;
    }

    if r#type == FLAG_CLEAR {
        let flist = flag_find_mut(&mut h.flag_list, name).unwrap();
        flist.value = None;
        flist.neg = 0;
        return SUCCEED;
    }

    if r#type == FLAG_DISALLOW {
        let flist = flag_find_mut(&mut h.flag_list, name).unwrap();
        if flist.value.is_none() {
            return SUCCEED;
        }
        if value.is_none() && flist.value.is_some() {
            return FAIL;
        }
        if value != flist.value.as_deref() {
            if flist.neg == 1 {
                return FAIL;
            }
            return SUCCEED;
        }
        if value == flist.value.as_deref() && flist.neg == 1 {
            return SUCCEED;
        }
        return FAIL;
    }

    if r#type == FLAG_NEGATIVE {
        let flist = flag_find_mut(&mut h.flag_list, name).unwrap();
        flist.value = value.map(|s| s.to_string());
        flist.neg = 1;
        return SUCCEED;
    }

    if r#type == FLAG_POSITIVE {
        let flist = flag_find_mut(&mut h.flag_list, name).unwrap();
        flist.value = value.map(|s| s.to_string());
        flist.neg = 0;
        return SUCCEED;
    }

    if r#type == FLAG_REQUIRE {
        let flist = flag_find_mut(&mut h.flag_list, name).unwrap();
        if value.is_none() {
            if flist.value.is_none() {
                return FAIL;
            } else {
                return SUCCEED;
            }
        } else {
            if flist.value.is_none() {
                return FAIL;
            }
            if value != flist.value.as_deref() {
                return FAIL;
            } else {
                if flist.neg == 1 {
                    return FAIL;
                }
                return SUCCEED;
            }
        }
    }

    if r#type == FLAG_EQUAL {
        // value names another feature; find flist2.
        let (f2_present, f2_value, f2_neg) = {
            let f2 = flag_find_immut(&h.flag_list, value.unwrap_or(""));
            match f2 {
                Some(n) => (true, n.value.clone(), n.neg),
                None => (false, None, 0),
            }
        };
        let flist = flag_find_immut(&h.flag_list, name).unwrap();
        let f1_value = flist.value.clone();
        let f1_neg = flist.neg;

        if !f2_present && f1_value.is_some() {
            return FAIL;
        }
        if !f2_present && f1_value.is_none() {
            return SUCCEED;
        }
        if f2_value.is_none() || f1_value.is_none() {
            if f2_value.is_none() && f1_value.is_none() && f1_neg == f2_neg {
                return SUCCEED;
            } else {
                return FAIL;
            }
        } else if f2_value == f1_value && f1_neg == f2_neg {
            return SUCCEED;
        }
        return FAIL;
    }

    eprintln!(
        "***Don't know what do with flag [{}][{}][{}]",
        r#type,
        name,
        value.unwrap_or("")
    );
    FAIL
}

/* Immutable flag_list lookup (for FLAG_EQUAL). */
fn flag_find_immut<'a>(list: &'a Option<Box<FlagList>>, name: &str) -> Option<&'a FlagList> {
    let mut cur = list.as_deref();
    while let Some(node) = cur {
        if node.name.as_deref() == Some(name) {
            return Some(node);
        }
        cur = node.next.as_deref();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::regex::fsm_parse_regex;
    use crate::structures::fsm_sort_arcs;
    use crate::types::{APPLY_INDEX_INPUT, APPLY_INDEX_OUTPUT};

    /* Build a fresh, minimized net from a regex (the Wave-2 pipeline). */
    fn parse(rx: &str) -> Box<Fsm> {
        fsm_parse_regex(rx, None, None).expect("regex should compile")
    }

    /* Full result-set enumeration via the iterator protocol: first call passes
    the word, subsequent NULL-resume calls drain the remaining paths. */
    fn drain_down(net: &Fsm, word: &str) -> Vec<String> {
        let mut h = apply_init(net);
        let mut out = Vec::new();
        let mut r = apply_down(&mut h, Some(word));
        while let Some(s) = r {
            out.push(s);
            r = apply_down(&mut h, None);
        }
        out
    }
    fn drain_up(net: &Fsm, word: &str) -> Vec<String> {
        let mut h = apply_init(net);
        let mut out = Vec::new();
        let mut r = apply_up(&mut h, Some(word));
        while let Some(s) = r {
            out.push(s);
            r = apply_up(&mut h, None);
        }
        out
    }

    fn signum(net: &Fsm, symbol: &str) -> i32 {
        let mut s = net.sigma.as_deref();
        while let Some(x) = s {
            if x.symbol.as_deref() == Some(symbol) {
                return x.number;
            }
            s = x.next.as_deref();
        }
        -1
    }

    // [spec:foma:sem:apply.apply-init-fn/test]
    // [spec:foma:sem:fomalib.apply-init-fn/test]
    #[test]
    fn apply_init_sets_defaults() {
        let net = parse("a:b");
        let h = apply_init(&net);
        assert_eq!(h.obey_flags, 1);
        assert_eq!(h.show_flags, 0);
        assert_eq!(h.print_space, 0);
        assert_eq!(h.print_pairs, 0);
        assert_eq!(h.separator.as_deref(), Some(":"));
        assert_eq!(h.epsilon_symbol.as_deref(), Some("0"));
        assert_eq!(h.outstringtop, DEFAULT_OUTSTRING_SIZE as i32);
        assert_eq!(h.outstring.len(), DEFAULT_OUTSTRING_SIZE);
        assert_eq!(h.printcount, 1);
        assert_eq!(h.iterator, 0);
        assert_eq!(h.apply_stack_top, DEFAULT_STACK_SIZE as i32);
        assert_eq!(apply_stack_isempty(&h), 1);
    }

    // [spec:foma:sem:apply.apply-create-statemap-fn/test]
    #[test]
    fn create_statemap_builds_line_tables() {
        // a:b => state 0 --a:b--> 1 (final); one dummy line for state 1.
        let net = parse("a:b");
        let mut h = apply_init(&net);
        // rebuild directly to pin the function under test
        apply_create_statemap(&mut h, &net);
        let sc = net.statecount as usize;
        assert_eq!(h.statemap.len(), sc);
        assert_eq!(h.numlines.len(), sc);
        assert_eq!(h.marks.len(), sc);
        // state 0 is the start state; its first line is line 0.
        assert_eq!(h.statemap[0], 0);
        // every state contributes at least one line (arcless states a dummy).
        for s in 0..sc {
            assert!(h.numlines[s] >= 1);
        }
    }

    // [spec:foma:sem:apply.apply-create-sigarray-fn/test]
    #[test]
    fn create_sigarray_builds_sigs_and_reserved() {
        let net = parse(r#""abc" | a"#);
        let h = apply_init(&net);
        // multichar symbol "abc" and single "a" are installed by number.
        let abc = signum(&net, "abc");
        let a = signum(&net, "a");
        assert_eq!(h.sigs[abc as usize].symbol.as_deref(), Some("abc"));
        assert_eq!(h.sigs[abc as usize].length, 3);
        assert_eq!(h.sigs[a as usize].symbol.as_deref(), Some("a"));
        assert_eq!(h.sigs[a as usize].length, 1);
        // reserved displays.
        assert_eq!(h.sigs[EPSILON as usize].symbol.as_deref(), Some("0"));
        assert_eq!(h.sigs[UNKNOWN as usize].symbol.as_deref(), Some("?"));
        assert_eq!(h.sigs[IDENTITY as usize].symbol.as_deref(), Some("@"));
        // no flag diacritics in this net.
        assert_eq!(h.has_flags, 0);
    }

    // [spec:foma:sem:apply.apply-create-sigmatch-fn/test]
    // [spec:foma:sem:apply.apply-add-sigma-trie-fn/test]
    #[test]
    fn sigmatch_longest_leftmost_multichar() {
        // "abc" is a genuine multichar sigma symbol; "a" is a prefix of its bytes.
        let net = parse(r#""abc" | a"#);
        let abc = signum(&net, "abc");
        let a = signum(&net, "a");
        let mut h = apply_init(&net);
        h.mode = DOWN;
        h.instring = b"aabc".to_vec();
        apply_create_sigmatch(&mut h);
        assert_eq!(h.current_instring_length, 4);
        // position 0: only "a" matches (1 byte)
        assert_eq!(h.sigmatch_array[0].signumber, a);
        assert_eq!(h.sigmatch_array[0].consumes, 1);
        // position 1: longest-leftmost picks "abc" (3 bytes) over "a"
        assert_eq!(h.sigmatch_array[1].signumber, abc);
        assert_eq!(h.sigmatch_array[1].consumes, 3);
    }

    // [spec:foma:sem:apply.apply-match-length-fn/test]
    // [spec:foma:sem:apply.apply-match-str-fn/test]
    #[test]
    fn match_length_and_str() {
        let net = parse(r#""abc" | a"#);
        let abc = signum(&net, "abc");
        let a = signum(&net, "a");
        let mut h = apply_init(&net);
        h.mode = DOWN;
        h.instring = b"abc".to_vec();
        apply_create_sigmatch(&mut h);
        h.ipos = 0;
        // token at 0 is "abc": matches abc (consumes 3), not a, epsilon consumes 0.
        assert_eq!(apply_match_length(&h, abc), 3);
        assert_eq!(apply_match_length(&h, a), -1);
        assert_eq!(apply_match_length(&h, EPSILON), 0);
        assert_eq!(apply_match_str(&mut h, abc, 0), 3);
        assert_eq!(apply_match_str(&mut h, a, 0), -1);
        assert_eq!(apply_match_str(&mut h, EPSILON, 0), 0);
        // input exhausted
        assert_eq!(apply_match_length(&h, abc), 3);
        h.ipos = 3;
        assert_eq!(apply_match_length(&h, abc), -1);
    }

    // [spec:foma:sem:apply.apply-down-fn/test]
    // [spec:foma:sem:fomalib.apply-down-fn/test]
    // [spec:foma:sem:apply.apply-up-fn/test]
    // [spec:foma:sem:fomalib.apply-up-fn/test]
    // [spec:foma:sem:apply.apply-updown-fn/test]
    // [spec:foma:sem:apply.apply-net-fn/test]
    // [spec:foma:sem:apply.apply-follow-next-arc-fn/test]
    // [spec:foma:sem:apply.apply-append-fn/test]
    // [spec:foma:sem:apply.apply-return-string-fn/test]
    // [spec:foma:sem:apply.apply-mark-state-fn/test]
    // [spec:foma:sem:apply.apply-at-last-arc-fn/test]
    // [spec:foma:sem:apply.apply-set-iptr-fn/test]
    #[test]
    fn apply_down_up_transducer() {
        let net = parse("a:b");
        assert_eq!(drain_down(&net, "a"), vec!["b".to_string()]);
        assert_eq!(drain_up(&net, "b"), vec!["a".to_string()]);
        // input not in the relation yields nothing.
        assert!(drain_down(&net, "x").is_empty());
        assert!(drain_up(&net, "a").is_empty());
    }

    // Iterator front-ends are additive Wave-4 sugar over the C-shaped resume
    // protocol; these tests pin the sugar's equivalence and carry no /test
    // facet (the underlying rules are pinned by the manual-protocol tests).
    #[test]
    fn iterator_down_up_match_manual_protocol() {
        // Backtracking net: two outputs for one input, drained via the iterator.
        let net = parse("{cat}:{dog} | {cat}:{cot}");
        let mut h = apply_init(&net);
        let mut got: Vec<String> = h.down("cat").collect();
        got.sort();
        let mut manual = drain_down(&net, "cat");
        manual.sort();
        assert_eq!(got, manual);
        assert_eq!(got, vec!["cot".to_string(), "dog".to_string()]);

        // Up direction.
        let tnet = parse("a:b");
        let mut hu = apply_init(&tnet);
        assert_eq!(hu.up("b").collect::<Vec<_>>(), vec!["a".to_string()]);

        // A non-matching input yields an empty (fused) iterator.
        let mut hx = apply_init(&tnet);
        let mut it = hx.down("z");
        assert!(it.next().is_none());
        assert!(it.next().is_none());
    }

    #[test]
    fn iterator_words_enumeration() {
        let net = parse("{ab}|{cd}|a");
        let mut h = apply_init(&net);
        assert_eq!(
            h.words().collect::<Vec<_>>(),
            vec!["a".to_string(), "ab".to_string(), "cd".to_string()]
        );
        let tnet = parse("{ab}:{xy}|{cd}");
        let mut hu = apply_init(&tnet);
        assert_eq!(
            hu.upper_words().collect::<Vec<_>>(),
            vec!["ab".to_string(), "cd".to_string()]
        );
        let mut hl = apply_init(&tnet);
        assert_eq!(
            hl.lower_words().collect::<Vec<_>>(),
            vec!["xy".to_string(), "cd".to_string()]
        );
    }

    // [spec:foma:sem:apply.apply-skip-this-arc-fn/test]
    // [spec:foma:sem:apply.apply-stack-pop-fn/test]
    #[test]
    fn cascade_multiple_results_backtrack() {
        // Two paths on the same input exercise backtracking (pop + skip-arc).
        let net = parse("{cat}:{dog} | {cat}:{cot}");
        let mut got = drain_down(&net, "cat");
        got.sort();
        assert_eq!(got, vec!["cot".to_string(), "dog".to_string()]);
    }

    // [spec:foma:sem:apply.apply-append-fn/test]
    #[test]
    fn epsilon_output_path() {
        // a:0 -> lower side is epsilon (empty output).
        let net = parse("a:0");
        assert_eq!(drain_down(&net, "a"), vec!["".to_string()]);
    }

    // [spec:foma:sem:apply.apply-follow-next-arc-fn/test]
    #[test]
    fn unknown_identity_application() {
        // a:b | ? : an out-of-alphabet input matches the ? (IDENTITY) arc.
        let net = parse("a:b | ?");
        assert_eq!(drain_down(&net, "z"), vec!["z".to_string()]);
        // 'a' matches both the a:b arc and the ? (as IDENTITY) arc.
        let mut got = drain_down(&net, "a");
        got.sort();
        assert_eq!(got, vec!["a".to_string(), "b".to_string()]);
    }

    // [spec:foma:sem:apply.apply-words-fn/test]
    // [spec:foma:sem:fomalib.apply-words-fn/test]
    // [spec:foma:sem:apply.apply-upper-words-fn/test]
    // [spec:foma:sem:fomalib.apply-upper-words-fn/test]
    // [spec:foma:sem:apply.apply-lower-words-fn/test]
    // [spec:foma:sem:fomalib.apply-lower-words-fn/test]
    // [spec:foma:sem:apply.apply-enumerate-fn/test]
    #[test]
    fn words_enumeration_order() {
        let net = parse("{ab}|{cd}|a");
        let mut h = apply_init(&net);
        let mut words = Vec::new();
        let mut r = apply_words(&mut h);
        while let Some(s) = r {
            words.push(s);
            r = apply_words(&mut h);
        }
        // C foma yields this exact order on a small acyclic net.
        assert_eq!(
            words,
            vec!["a".to_string(), "ab".to_string(), "cd".to_string()]
        );

        // upper/lower words on a transducer: both sides differ.
        let tnet = parse("{ab}:{xy}|{cd}");
        let mut hu = apply_init(&tnet);
        let mut upper = Vec::new();
        let mut r = apply_upper_words(&mut hu);
        while let Some(s) = r {
            upper.push(s);
            r = apply_upper_words(&mut hu);
        }
        assert_eq!(upper, vec!["ab".to_string(), "cd".to_string()]);
        let mut hl = apply_init(&tnet);
        let mut lower = Vec::new();
        let mut r = apply_lower_words(&mut hl);
        while let Some(s) = r {
            lower.push(s);
            r = apply_lower_words(&mut hl);
        }
        // C foma yields the lower side in this order (xy before cd).
        assert_eq!(lower, vec!["xy".to_string(), "cd".to_string()]);
    }

    // [spec:foma:sem:apply.apply-reset-enumerator-fn/test]
    // [spec:foma:sem:fomalib.apply-reset-enumerator-fn/test]
    #[test]
    fn reset_enumerator_restarts() {
        let net = parse("{ab}|{cd}|a");
        let mut h = apply_init(&net);
        let collect = |h: &mut ApplyHandle| {
            let mut v = Vec::new();
            let mut r = apply_words(h);
            while let Some(s) = r {
                v.push(s);
                r = apply_words(h);
            }
            v
        };
        let first = collect(&mut h);
        assert!(!first.is_empty());
        // reset zeroes the iterator so enumeration restarts from scratch...
        apply_reset_enumerator(&mut h);
        assert_eq!(h.iterator, 0);
        // ...yielding the same list again (without reset the second pass would
        // resume the exhausted search and yield nothing).
        let second = collect(&mut h);
        assert_eq!(first, second);
    }

    // [spec:foma:sem:apply.apply-random-words-fn/test]
    // [spec:foma:sem:fomalib.apply-random-words-fn/test]
    // [spec:foma:sem:apply.apply-random-lower-fn/test]
    // [spec:foma:sem:fomalib.apply-random-lower-fn/test]
    // [spec:foma:sem:apply.apply-random-upper-fn/test]
    // [spec:foma:sem:fomalib.apply-random-upper-fn/test]
    #[test]
    fn random_words_are_wellformed() {
        // srand reseeds from time; assert only well-formedness — a word from the
        // language, never the "???" no-result marker.
        let net = parse("{cat}|{dog}");
        let mut h = apply_init(&net);
        for _ in 0..16 {
            let w = apply_random_words(&mut h).expect("random word");
            assert!(w == "cat" || w == "dog", "unexpected random word {:?}", w);
            assert_ne!(w, "???");
        }
        let mut hl = apply_init(&net);
        for _ in 0..16 {
            let w = apply_random_lower(&mut hl).expect("random lower");
            assert!(w == "cat" || w == "dog");
        }
        let mut hu = apply_init(&net);
        for _ in 0..16 {
            let w = apply_random_upper(&mut hu).expect("random upper");
            assert!(w == "cat" || w == "dog");
        }
    }

    // [spec:foma:sem:apply.apply-set-print-pairs-fn/test]
    // [spec:foma:sem:fomalib.apply-set-print-pairs-fn/test]
    // [spec:foma:sem:apply.apply-set-separator-fn/test]
    // [spec:foma:sem:fomalib.apply-set-separator-fn/test]
    #[test]
    fn set_print_pairs_and_separator() {
        let net = parse("a:b");
        let mut h = apply_init(&net);
        apply_set_print_pairs(&mut h, 1);
        assert_eq!(h.print_pairs, 1);
        let mut r = apply_down(&mut h, Some("a"));
        let mut got = Vec::new();
        while let Some(s) = r {
            got.push(s);
            r = apply_down(&mut h, None);
        }
        assert_eq!(got, vec!["<a:b>".to_string()]);

        // custom separator changes the pair rendering.
        let mut h2 = apply_init(&net);
        apply_set_print_pairs(&mut h2, 1);
        apply_set_separator(&mut h2, "/");
        assert_eq!(h2.separator.as_deref(), Some("/"));
        let mut r = apply_down(&mut h2, Some("a"));
        let mut got = Vec::new();
        while let Some(s) = r {
            got.push(s);
            r = apply_down(&mut h2, None);
        }
        assert_eq!(got, vec!["<a/b>".to_string()]);
    }

    // [spec:foma:sem:apply.apply-set-print-space-fn/test]
    // [spec:foma:sem:fomalib.apply-set-print-space-fn/test]
    // [spec:foma:sem:apply.apply-set-space-symbol-fn/test]
    // [spec:foma:sem:fomalib.apply-set-space-symbol-fn/test]
    #[test]
    fn set_print_space_and_space_symbol() {
        let net = parse("{ab}");
        let mut h = apply_init(&net);
        apply_set_print_space(&mut h, 1);
        assert_eq!(h.print_space, 1);
        assert_eq!(h.space_symbol.as_deref(), Some(" "));
        let mut r = apply_down(&mut h, Some("ab"));
        let mut got = Vec::new();
        while let Some(s) = r {
            got.push(s);
            r = apply_down(&mut h, None);
        }
        // one space appended after every emitted symbol.
        assert_eq!(got, vec!["a b ".to_string()]);

        // space_symbol setter also turns print_space on.
        let mut h2 = apply_init(&net);
        apply_set_space_symbol(&mut h2, "_");
        assert_eq!(h2.print_space, 1);
        assert_eq!(h2.space_symbol.as_deref(), Some("_"));
        let mut r = apply_down(&mut h2, Some("ab"));
        let mut got = Vec::new();
        while let Some(s) = r {
            got.push(s);
            r = apply_down(&mut h2, None);
        }
        assert_eq!(got, vec!["a_b_".to_string()]);
    }

    // [spec:foma:sem:apply.apply-append-fn+1/test]
    #[test]
    fn apply_append_multibyte_space_symbol_survives() {
        // A multi-byte separator ("»" = 0xC2 0xBB) must be emitted whole between
        // symbols. C advanced the output cursor by 1 regardless of the symbol's
        // byte length, so the next append overwrote all but its first byte.
        let net = parse("{ab}");
        let mut h = apply_init(&net);
        apply_set_space_symbol(&mut h, "»");
        let r = apply_down(&mut h, Some("ab")).unwrap();
        assert_eq!(r, "a»b»");
    }

    // [spec:foma:sem:apply.apply-set-epsilon-fn/test]
    // [spec:foma:sem:fomalib.apply-set-epsilon-fn/test]
    #[test]
    fn set_epsilon_symbol() {
        // a:0 word rendering shows the epsilon display on the lower side.
        let net = parse("a:0");
        let mut h = apply_init(&net);
        apply_set_epsilon(&mut h, "[]");
        assert_eq!(h.epsilon_symbol.as_deref(), Some("[]"));
        assert_eq!(h.sigs[EPSILON as usize].symbol.as_deref(), Some("[]"));
        let mut words = Vec::new();
        let mut r = apply_words(&mut h);
        while let Some(s) = r {
            words.push(s);
            r = apply_words(&mut h);
        }
        assert_eq!(words, vec!["a:[]".to_string()]);
    }

    // [spec:foma:sem:apply.apply-set-obey-flags-fn/test]
    // [spec:foma:sem:fomalib.apply-set-obey-flags-fn/test]
    // [spec:foma:sem:apply.apply-set-show-flags-fn/test]
    // [spec:foma:sem:fomalib.apply-set-show-flags-fn/test]
    // [spec:foma:sem:apply.apply-mark-flagstates-fn/test]
    #[test]
    fn flag_diacritics_end_to_end() {
        let net = parse(r#"[a "@U.F.1@" | b "@U.F.2@"] [c "@R.F.1@" | d "@R.F.2@"]"#);
        let h = apply_init(&net);
        assert_eq!(h.has_flags, 1);
        // states with flag arcs are recorded in flagstates.
        assert!(!h.flagstates.is_empty());
        assert!(h.flagstates.iter().any(|&b| b != 0));

        // obey on (default), show off: consistent path "ac" survives with flags
        // rendered as empty; "ad" (U.F.1 then R.F.2) is inconsistent -> nothing.
        assert_eq!(drain_down(&net, "ac"), vec!["ac".to_string()]);
        assert!(drain_down(&net, "ad").is_empty());

        // show-flags on renders the diacritics literally.
        let net2 = parse(r#"[a "@U.F.1@"] [c "@R.F.1@"]"#);
        let mut hs = apply_init(&net2);
        apply_set_show_flags(&mut hs, 1);
        let mut r = apply_down(&mut hs, Some("ac"));
        let mut got = Vec::new();
        while let Some(s) = r {
            got.push(s);
            r = apply_down(&mut hs, None);
        }
        assert_eq!(got, vec!["a@U.F.1@c@R.F.1@".to_string()]);

        // obey off makes flag arcs freely traversable, so "ad" now passes.
        let mut ho = apply_init(&net);
        apply_set_obey_flags(&mut ho, 0);
        let mut r = apply_down(&mut ho, Some("ad"));
        let mut got = Vec::new();
        while let Some(s) = r {
            got.push(s);
            r = apply_down(&mut ho, None);
        }
        assert_eq!(got, vec!["ad".to_string()]);
    }

    // [spec:foma:sem:apply.apply-check-flag-fn/test]
    // [spec:foma:sem:apply.apply-clear-flags-fn/test]
    #[test]
    fn check_flag_dispatch() {
        let net = parse(r#"[a "@U.F.1@"] [c "@R.F.1@"]"#);
        let mut h = apply_init(&net);
        // U.F.1 on an unset feature stores the value and succeeds.
        assert_eq!(
            apply_check_flag(&mut h, FLAG_UNIFY, Some("F"), Some("1")),
            SUCCEED
        );
        // same value unifies; different value fails.
        assert_eq!(
            apply_check_flag(&mut h, FLAG_UNIFY, Some("F"), Some("1")),
            SUCCEED
        );
        assert_eq!(
            apply_check_flag(&mut h, FLAG_UNIFY, Some("F"), Some("2")),
            FAIL
        );
        // R.F requires a value present -> succeed while set.
        assert_eq!(
            apply_check_flag(&mut h, FLAG_REQUIRE, Some("F"), None),
            SUCCEED
        );
        // D.F.1 disallows the currently-set value -> fail; a different value ok.
        assert_eq!(
            apply_check_flag(&mut h, FLAG_DISALLOW, Some("F"), Some("1")),
            FAIL
        );
        assert_eq!(
            apply_check_flag(&mut h, FLAG_DISALLOW, Some("F"), Some("9")),
            SUCCEED
        );
        // C.F clears the value -> R.F now fails (nothing set).
        assert_eq!(
            apply_check_flag(&mut h, FLAG_CLEAR, Some("F"), None),
            SUCCEED
        );
        assert_eq!(
            apply_check_flag(&mut h, FLAG_REQUIRE, Some("F"), None),
            FAIL
        );
        // P.F.5 sets; clear_flags resets, so REQUIRE fails again.
        assert_eq!(
            apply_check_flag(&mut h, FLAG_POSITIVE, Some("F"), Some("5")),
            SUCCEED
        );
        assert_eq!(
            apply_check_flag(&mut h, FLAG_REQUIRE, Some("F"), None),
            SUCCEED
        );
        apply_clear_flags(&mut h);
        assert_eq!(
            apply_check_flag(&mut h, FLAG_REQUIRE, Some("F"), None),
            FAIL
        );
    }

    // [spec:foma:sem:apply.apply-check-flag-fn/test]
    #[test]
    #[should_panic]
    fn check_flag_unregistered_name_panics() {
        // DEVIATION: C dereferences NULL for an unregistered feature; the port
        // panics via .expect (unreachable in practice).
        let net = parse(r#"[a "@U.F.1@"]"#);
        let mut h = apply_init(&net);
        apply_check_flag(&mut h, FLAG_UNIFY, Some("NOPE"), Some("1"));
    }

    // [spec:foma:sem:apply.apply-add-flag-fn/test]
    #[test]
    fn add_flag_dedups_and_appends() {
        let net = parse(r#"[a "@U.F.1@"]"#);
        let mut h = apply_init(&net);
        let count = |h: &ApplyHandle, name: &str| -> usize {
            let mut n = 0;
            let mut cur = h.flag_list.as_deref();
            while let Some(node) = cur {
                if node.name.as_deref() == Some(name) {
                    n += 1;
                }
                cur = node.next.as_deref();
            }
            n
        };
        // "F" already registered by create_sigarray; adding again is a no-op.
        assert_eq!(count(&h, "F"), 1);
        apply_add_flag(&mut h, Some("F".to_string()));
        assert_eq!(count(&h, "F"), 1);
        // a fresh feature is appended.
        apply_add_flag(&mut h, Some("G".to_string()));
        assert_eq!(count(&h, "G"), 1);
    }

    // [spec:foma:sem:apply.apply-stack-isempty-fn/test]
    // [spec:foma:sem:apply.apply-stack-clear-fn/test]
    // [spec:foma:sem:apply.apply-stack-push-fn/test]
    // [spec:foma:sem:apply.apply-stack-pop-fn/test]
    #[test]
    fn stack_push_pop_roundtrip() {
        let net = parse("a:b");
        let mut h = apply_init(&net);
        apply_stack_clear(&mut h);
        assert_eq!(apply_stack_isempty(&h), 1);
        // push records curr_ptr/ipos/opos; pop restores them.
        h.curr_ptr = 0;
        h.ipos = 5;
        h.opos = 7;
        h.iptr = None;
        h.state_has_index = 0;
        apply_stack_push(&mut h, 0, None, None, 0);
        assert_eq!(apply_stack_isempty(&h), 0);
        assert_eq!(h.apply_stack_ptr, 1);
        h.ipos = 99;
        h.opos = 99;
        apply_stack_pop(&mut h);
        assert_eq!(h.ptr, 0);
        assert_eq!(h.ipos, 5);
        assert_eq!(h.opos, 7);
        assert_eq!(apply_stack_isempty(&h), 1);
    }

    // [spec:foma:sem:apply.apply-force-clear-stack-fn/test]
    #[test]
    fn force_clear_stack_empties() {
        let net = parse("a:b");
        let mut h = apply_init(&net);
        h.curr_ptr = 0;
        h.ipos = 0;
        h.opos = 0;
        apply_stack_push(&mut h, 0, None, None, 0);
        assert_eq!(apply_stack_isempty(&h), 0);
        apply_force_clear_stack(&mut h);
        assert_eq!(apply_stack_isempty(&h), 1);
        assert_eq!(h.iterator, 0);
        assert_eq!(h.iterate_old, 0);
    }

    // [spec:foma:sem:apply.apply-binarysearch-fn/test]
    // [spec:foma:sem:apply.apply-at-last-arc-fn/test]
    #[test]
    fn binary_search_on_sorted_arcs() {
        // Shared-prefix net; sorting the input side enables the binsearch path.
        let mut net = parse("{cat}|{car}|{can}");
        fsm_sort_arcs(&mut net, 1);
        assert_eq!(net.arcs_sorted_in, 1);
        // apply_down sets h.binsearch = 1 from arcs_sorted_in.
        let mut h = apply_init(&net);
        let mut r = apply_down(&mut h, Some("cat"));
        assert_eq!(h.binsearch, 1);
        let mut got = Vec::new();
        while let Some(s) = r {
            got.push(s);
            r = apply_down(&mut h, None);
        }
        assert_eq!(got, vec!["cat".to_string()]);
        assert_eq!(drain_down(&net, "car"), vec!["car".to_string()]);
        assert_eq!(drain_down(&net, "can"), vec!["can".to_string()]);
        assert!(drain_down(&net, "cax").is_empty());
    }

    // [spec:foma:sem:apply.apply-index-fn/test]
    // [spec:foma:sem:fomalib.apply-index-fn/test]
    // [spec:foma:sem:apply.apply-set-iptr-fn/test]
    // [spec:foma:sem:apply.apply-clear-index-fn/test]
    // [spec:foma:sem:apply.apply-clear-index-list-fn/test]
    #[test]
    fn indexed_application_matches_unindexed() {
        let net = parse("a:b | c:d");
        let base_down = drain_down(&net, "a");
        assert_eq!(base_down, vec!["b".to_string()]);

        // Build an input index; indexed application returns identical results.
        let mut h = apply_init(&net);
        apply_index(&mut h, APPLY_INDEX_INPUT, 0, 1 << 30, 0);
        assert!(!h.index_in.is_empty());
        let mut r = apply_down(&mut h, Some("a"));
        assert_eq!(h.indexed, 1);
        let mut got = Vec::new();
        while let Some(s) = r {
            got.push(s);
            r = apply_down(&mut h, None);
        }
        assert_eq!(got, base_down);

        // apply_clear_index releases both indexes.
        apply_index(&mut h, APPLY_INDEX_OUTPUT, 0, 1 << 30, 0);
        assert!(!h.index_out.is_empty());
        apply_clear_index(&mut h);
        assert!(h.index_in.is_empty());
        assert!(h.index_out.is_empty());

        // A too-small memory limit builds no index at all.
        let mut h2 = apply_init(&net);
        apply_index(&mut h2, APPLY_INDEX_INPUT, 0, 0, 0);
        assert!(h2.index_in.is_empty());
    }

    // [spec:foma:sem:apply.apply-clear-fn/test]
    // [spec:foma:sem:fomalib.apply-clear-fn/test]
    #[test]
    fn apply_clear_consumes_handle() {
        let net = parse("a:b");
        let h = apply_init(&net);
        // Destroys the handle and everything it owns without panicking.
        apply_clear(h);
    }
}

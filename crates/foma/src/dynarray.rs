//! foma/dynarray.c — literal (bug-for-bug) Wave-2 port per
//! docs/port/rust-conventions.md. Sem rules: docs/spec/port/foma/dynarray.md
//! (per-file ids) plus the fomalib.h / fomalibconf.h prototype ids.
//!
//! Two facilities live here:
//! - the fsm_state_* dynamic line-array builder, which operates on module
//!   statics (thread_local per the conventions — non-reentrancy is part of
//!   the contract, exactly as in C);
//! - the fsm_construct_* / fsm_read_* handle families for building and
//!   iterating networks.
//!
//! Interior pointers of the C (arcs_cursor, states_head entries, the
//! finals/initials cursors) are represented as indices per the conventions.
//! The fsm_get_next_state protocol parks arcs_cursor one line *before* the
//! state's first line (C: a pointer one element before the array position —
//! UB but works); here that park position is `index.wrapping_sub(1)` and
//! fsm_get_next_state_arc's pre-increment wraps it back.

use std::cell::{Cell, RefCell};

use crate::mem::next_power_of_two;
use crate::sigma::{sigma_max, sigma_sort, sigma_to_list};
use crate::structures::{fsm_create, fsm_destroy, fsm_empty_set};
use crate::types::{
    Fsm, FsmConstructHandle, FsmReadHandle, FsmSigmaHash, FsmSigmaList, FsmState, FsmStateList,
    FsmTransList, Sigma, EPSILON, FSM_NAME_LEN, IDENTITY, PATHCOUNT_UNKNOWN, UNK, UNKNOWN,
};

/* C: #define INITIAL_SIZE 16384 */
pub const INITIAL_SIZE: usize = 16384;
/* C: #define SIGMA_HASH_SIZE 1021 */
pub const SIGMA_HASH_SIZE: u32 = 1021;
/* C: #define MINSIGMA 3 */
pub const MINSIGMA: i32 = 3;

// [spec:foma:def:dynarray.foma-reserved-symbols]
pub struct FomaReservedSymbols {
    pub symbol: Option<&'static str>,
    pub number: i32,
    pub prints_as: Option<&'static str>,
}

/* C: the table is NULL-terminated; symbol == None is the terminator entry */
pub static FOMA_RESERVED_SYMBOLS: [FomaReservedSymbols; 4] = [
    FomaReservedSymbols {
        symbol: Some("@_EPSILON_SYMBOL_@"),
        number: EPSILON,
        prints_as: Some("0"),
    },
    FomaReservedSymbols {
        symbol: Some("@_UNKNOWN_SYMBOL_@"),
        number: UNKNOWN,
        prints_as: Some("?"),
    },
    FomaReservedSymbols {
        symbol: Some("@_IDENTITY_SYMBOL_@"),
        number: IDENTITY,
        prints_as: Some("@"),
    },
    FomaReservedSymbols {
        symbol: None,
        number: 0,
        prints_as: None,
    },
];

// [spec:foma:def:dynarray.sigma-lookup]
#[derive(Debug, Clone)]
pub struct SigmaLookup {
    pub target: i32,
    pub mainloop: u32,
}

thread_local! {
    // C: static size_t current_fsm_size;
    static CURRENT_FSM_SIZE: Cell<usize> = const { Cell::new(0) };
    // C: static unsigned int current_fsm_linecount, current_state_no,
    //    current_final, current_start, current_trans, num_finals,
    //    num_initials, arity, statecount;
    static CURRENT_FSM_LINECOUNT: Cell<u32> = const { Cell::new(0) };
    static CURRENT_STATE_NO: Cell<u32> = const { Cell::new(0) };
    static CURRENT_FINAL: Cell<u32> = const { Cell::new(0) };
    static CURRENT_START: Cell<u32> = const { Cell::new(0) };
    static CURRENT_TRANS: Cell<u32> = const { Cell::new(0) };
    static NUM_FINALS: Cell<u32> = const { Cell::new(0) };
    static NUM_INITIALS: Cell<u32> = const { Cell::new(0) };
    static ARITY: Cell<u32> = const { Cell::new(0) };
    static STATECOUNT: Cell<u32> = const { Cell::new(0) };
    // C: static _Bool is_deterministic, is_epsilon_free;
    static IS_DETERMINISTIC: Cell<bool> = const { Cell::new(false) };
    static IS_EPSILON_FREE: Cell<bool> = const { Cell::new(false) };
    // C: static struct fsm_state *current_fsm_head;
    static CURRENT_FSM_HEAD: RefCell<Vec<FsmState>> = const { RefCell::new(Vec::new()) };
    // C: static unsigned int mainloop, ssize, arccount;
    static MAINLOOP: Cell<u32> = const { Cell::new(0) };
    static SSIZE: Cell<u32> = const { Cell::new(0) };
    static ARCCOUNT: Cell<u32> = const { Cell::new(0) };
    // C: static struct sigma_lookup *slookup;
    static SLOOKUP: RefCell<Vec<SigmaLookup>> = const { RefCell::new(Vec::new()) };

    // libc rand() state stand-in: the C calls the C library's rand()
    // (seeded elsewhere with srand(time(NULL))); the port has no libc
    // dependency, so the ISO C sample LCG is used. Only affects the
    // random hex names given to constructed nets.
    static RAND_NEXT: Cell<u64> = const { Cell::new(1) };
}

/* C library rand() twin (ISO C sample implementation; see RAND_NEXT) */
pub(crate) fn rand() -> i32 {
    RAND_NEXT.with(|n| {
        let next = n
            .get()
            .wrapping_mul(1103515245)
            .wrapping_add(12345);
        n.set(next);
        ((next / 65536) % 32768) as i32
    })
}

/* Functions for directly building a fsm_state structure */
/* dynamically. */

/* fsm_state_init() is called when a new machine is constructed */

/* fsm_state_add_arc() adds an arc and possibly reallocs the array */

/* fsm_state_close() adds the sentinel entry and clears values */

// [spec:foma:def:dynarray.fsm-state-init-fn]
// [spec:foma:sem:dynarray.fsm-state-init-fn]
// [spec:foma:def:fomalibconf.fsm-state-init-fn]
// [spec:foma:sem:fomalibconf.fsm-state-init-fn]
pub fn fsm_state_init(sigma_size: i32) {
    // C: current_fsm_head = malloc(INITIAL_SIZE * sizeof(struct fsm_state));
    // and returns that pointer (also retained in the static). Every foma
    // caller ignores the return value, so the Rust twin returns ().
    CURRENT_FSM_HEAD.with(|h| *h.borrow_mut() = Vec::with_capacity(INITIAL_SIZE));
    CURRENT_FSM_SIZE.set(INITIAL_SIZE);
    CURRENT_FSM_LINECOUNT.set(0);
    SSIZE.set((sigma_size + 1) as u32);
    let ssize = SSIZE.get() as usize;
    SLOOKUP.with(|s| {
        *s.borrow_mut() = vec![
            SigmaLookup {
                target: 0,
                mainloop: 0,
            };
            ssize * ssize
        ]
    });
    MAINLOOP.set(1);
    IS_DETERMINISTIC.set(true);
    IS_EPSILON_FREE.set(true);
    ARCCOUNT.set(0);
    NUM_FINALS.set(0);
    NUM_INITIALS.set(0);
    STATECOUNT.set(0);
    ARITY.set(1);
    CURRENT_TRANS.set(1);
}

// [spec:foma:def:dynarray.fsm-state-set-current-state-fn]
// [spec:foma:sem:dynarray.fsm-state-set-current-state-fn]
// [spec:foma:def:fomalibconf.fsm-state-set-current-state-fn]
// [spec:foma:sem:fomalibconf.fsm-state-set-current-state-fn]
pub fn fsm_state_set_current_state(state_no: i32, final_state: i32, start_state: i32) {
    /* the statics are unsigned int; C's int→unsigned conversion wraps */
    CURRENT_STATE_NO.set(state_no as u32);
    CURRENT_FINAL.set(final_state as u32);
    CURRENT_START.set(start_state as u32);
    CURRENT_TRANS.set(0);
    /* counts only the exact value 1 — other nonzero flags are stored
    but not counted */
    if CURRENT_FINAL.get() == 1 {
        NUM_FINALS.set(NUM_FINALS.get() + 1);
    }
    if CURRENT_START.get() == 1 {
        NUM_INITIALS.set(NUM_INITIALS.get() + 1);
    }
}

/* Add sentinel if needed */
// [spec:foma:def:dynarray.fsm-state-end-state-fn]
// [spec:foma:sem:dynarray.fsm-state-end-state-fn]
// [spec:foma:def:fomalibconf.fsm-state-end-state-fn]
// [spec:foma:sem:fomalibconf.fsm-state-end-state-fn]
pub fn fsm_state_end_state() {
    if CURRENT_TRANS.get() == 0 {
        fsm_state_add_arc(
            CURRENT_STATE_NO.get() as i32,
            -1,
            -1,
            -1,
            CURRENT_FINAL.get() as i32,
            CURRENT_START.get() as i32,
        );
    }
    STATECOUNT.set(STATECOUNT.get() + 1);
    /* invalidates all slookup duplicate-detection stamps for the next state */
    MAINLOOP.set(MAINLOOP.get() + 1);
}

// [spec:foma:def:dynarray.fsm-state-add-arc-fn]
// [spec:foma:sem:dynarray.fsm-state-add-arc-fn]
// [spec:foma:def:fomalibconf.fsm-state-add-arc-fn]
// [spec:foma:sem:fomalibconf.fsm-state-add-arc-fn]
pub fn fsm_state_add_arc(
    state_no: i32,
    r#in: i32,
    out: i32,
    target: i32,
    final_state: i32,
    start_state: i32,
) {
    if r#in != out {
        ARITY.set(2);
    }
    /* Check epsilon moves */
    if r#in == EPSILON && out == EPSILON {
        if state_no == target {
            return;
        } else {
            IS_DETERMINISTIC.set(false);
            IS_EPSILON_FREE.set(false);
        }
    }

    /* Check if we already added this particular arc and skip */
    /* Also check if net becomes non-det */
    if r#in != -1 && out != -1 {
        /* slookup cell at ssize*in + out. Duplicate detection is stamped
        per state via mainloop. Quirk (kept): on a same-label arc with a
        *different* target the cell's target is overwritten below, so a
        third same-label arc that repeats the FIRST target is no longer
        seen as a duplicate and gets emitted twice. */
        let ssize = SSIZE.get() as usize;
        let idx = ssize * (r#in as usize) + (out as usize);
        let skip = SLOOKUP.with(|s| {
            let mut slookup = s.borrow_mut();
            if slookup[idx].mainloop == MAINLOOP.get() {
                if slookup[idx].target == target {
                    /* exact duplicate (in,out,target): silently dropped */
                    return true;
                } else {
                    IS_DETERMINISTIC.set(false);
                }
            }
            ARCCOUNT.set(ARCCOUNT.get() + 1);
            slookup[idx].mainloop = MAINLOOP.get();
            slookup[idx].target = target;
            false
        });
        if skip {
            return;
        }
    }

    CURRENT_TRANS.set(1);
    if CURRENT_FSM_LINECOUNT.get() as usize >= CURRENT_FSM_SIZE.get() {
        /* C: doubling realloc; realloc failure perror()s and exit(1)s —
        in Rust Vec growth aborts on OOM, an unrepresentable branch */
        CURRENT_FSM_SIZE.set(CURRENT_FSM_SIZE.get() * 2);
    }
    /* write the line at index current_fsm_linecount (== the Vec length);
    in/out truncate int→short, final/start truncate int→char as in C */
    CURRENT_FSM_HEAD.with(|h| {
        h.borrow_mut().push(FsmState {
            state_no,
            r#in: r#in as i16,
            out: out as i16,
            target,
            final_state: final_state as i8,
            start_state: start_state as i8,
        })
    });
    CURRENT_FSM_LINECOUNT.set(CURRENT_FSM_LINECOUNT.get() + 1);
}

// [spec:foma:def:dynarray.fsm-state-close-fn]
// [spec:foma:sem:dynarray.fsm-state-close-fn]
// [spec:foma:def:fomalibconf.fsm-state-close-fn]
// [spec:foma:sem:fomalibconf.fsm-state-close-fn]
pub fn fsm_state_close(net: &mut Fsm) {
    /* array terminator line */
    fsm_state_add_arc(-1, -1, -1, -1, -1, -1);
    /* C: realloc down to exactly current_fsm_linecount lines */
    let mut states = CURRENT_FSM_HEAD.with(|h| std::mem::take(&mut *h.borrow_mut()));
    states.shrink_to_fit();
    net.arity = ARITY.get() as i32;
    net.arccount = ARCCOUNT.get() as i32;
    net.statecount = STATECOUNT.get() as i32;
    net.linecount = CURRENT_FSM_LINECOUNT.get() as i32;
    net.finalcount = NUM_FINALS.get() as i32;
    net.pathcount = PATHCOUNT_UNKNOWN;
    if NUM_INITIALS.get() > 1 {
        IS_DETERMINISTIC.set(false);
    }
    net.is_deterministic = IS_DETERMINISTIC.get() as i32;
    net.is_pruned = UNK;
    net.is_minimized = UNK;
    net.is_epsilon_free = IS_EPSILON_FREE.get() as i32;
    net.is_loop_free = UNK;
    net.is_completed = UNK;
    net.arcs_sorted_in = 0;
    net.arcs_sorted_out = 0;

    net.states = states;
    /* free(slookup) */
    SLOOKUP.with(|s| *s.borrow_mut() = Vec::new());
}

/* Construction functions */

// [spec:foma:def:dynarray.fsm-construct-init-fn]
// [spec:foma:sem:dynarray.fsm-construct-init-fn]
// [spec:foma:def:fomalib.fsm-construct-init-fn]
// [spec:foma:sem:fomalib.fsm-construct-init-fn]
pub fn fsm_construct_init(name: &str) -> Box<FsmConstructHandle> {
    Box::new(FsmConstructHandle {
        /* calloc(1024, ...) — zeroed entries */
        fsm_state_list: vec![
            FsmStateList {
                used: false,
                is_final: false,
                is_initial: false,
                num_trans: 0,
                state_number: 0,
                fsm_trans_list: None,
            };
            1024
        ],
        fsm_state_list_size: 1024,
        fsm_sigma_list: vec![FsmSigmaList { symbol: None }; 1024],
        fsm_sigma_list_size: 1024,
        /* calloc(SIGMA_HASH_SIZE, ...) — symbol == None marks an empty bucket */
        fsm_sigma_hash: vec![
            FsmSigmaHash {
                symbol: None,
                sym: 0,
                next: None,
            };
            SIGMA_HASH_SIZE as usize
        ],
        /* C never initializes this field (malloc'd handle → garbage; the
        field is never read anywhere) */
        fsm_sigma_hash_size: 0,
        maxstate: -1,
        maxsigma: -1,
        numfinals: 0,
        /* C: name == NULL → handle->name = NULL; a &str cannot be NULL and
        no in-tree caller passes NULL */
        name: Some(name.to_string()),
        hasinitial: 0,
    })
}

// [spec:foma:def:dynarray.fsm-construct-check-size-fn]
// [spec:foma:sem:dynarray.fsm-construct-check-size-fn]
pub fn fsm_construct_check_size(handle: &mut FsmConstructHandle, state_no: i32) {
    let oldsize = handle.fsm_state_list_size;
    if oldsize <= state_no {
        let newsize = next_power_of_two(state_no);
        /* C: realloc leaves the grown region uninitialized; the loop below
        then initializes exactly oldsize..newsize (Vec::resize fills the
        same defaults first — observably identical) */
        handle.fsm_state_list.resize(
            newsize as usize,
            FsmStateList {
                used: false,
                is_final: false,
                is_initial: false,
                num_trans: 0,
                state_number: 0,
                fsm_trans_list: None,
            },
        );
        handle.fsm_state_list_size = newsize;
        for i in oldsize..newsize {
            let sl = &mut handle.fsm_state_list[i as usize];
            sl.is_final = false;
            sl.is_initial = false;
            sl.used = false;
            sl.num_trans = 0;
            sl.fsm_trans_list = None;
        }
    }
}

// [spec:foma:def:dynarray.fsm-construct-set-final-fn]
// [spec:foma:sem:dynarray.fsm-construct-set-final-fn]
// [spec:foma:def:fomalib.fsm-construct-set-final-fn]
// [spec:foma:sem:fomalib.fsm-construct-set-final-fn]
pub fn fsm_construct_set_final(handle: &mut FsmConstructHandle, state_no: i32) {
    fsm_construct_check_size(handle, state_no);

    if state_no > handle.maxstate {
        handle.maxstate = state_no;
    }

    if !handle.fsm_state_list[state_no as usize].is_final {
        handle.fsm_state_list[state_no as usize].is_final = true;
        handle.numfinals += 1;
    }
}

// [spec:foma:def:dynarray.fsm-construct-set-initial-fn]
// [spec:foma:sem:dynarray.fsm-construct-set-initial-fn]
// [spec:foma:def:fomalib.fsm-construct-set-initial-fn]
// [spec:foma:sem:fomalib.fsm-construct-set-initial-fn]
pub fn fsm_construct_set_initial(handle: &mut FsmConstructHandle, state_no: i32) {
    fsm_construct_check_size(handle, state_no);

    if state_no > handle.maxstate {
        handle.maxstate = state_no;
    }

    handle.fsm_state_list[state_no as usize].is_initial = true;
    handle.hasinitial = 1;
}

// [spec:foma:def:dynarray.fsm-construct-add-arc-fn]
// [spec:foma:sem:dynarray.fsm-construct-add-arc-fn]
// [spec:foma:def:fomalib.fsm-construct-add-arc-fn]
// [spec:foma:sem:fomalib.fsm-construct-add-arc-fn]
pub fn fsm_construct_add_arc(
    handle: &mut FsmConstructHandle,
    source: i32,
    target: i32,
    r#in: &str,
    out: &str,
) {
    fsm_construct_check_size(handle, source);
    fsm_construct_check_size(handle, target);

    if source > handle.maxstate {
        handle.maxstate = source;
    }
    if target > handle.maxstate {
        handle.maxstate = target;
    }

    handle.fsm_state_list[target as usize].used = true;
    handle.fsm_state_list[source as usize].used = true;
    /* C mallocs the node and prepends it to source's list *before*
    resolving the labels, filling the fields afterwards; the labels are
    resolved first here (check/add only touch the sigma list/hash —
    observably equivalent). num_trans is not updated, as in C. */
    let mut symin = fsm_construct_check_symbol(handle, r#in);
    if symin == -1 {
        symin = fsm_construct_add_symbol(handle, r#in);
    }
    let mut symout = fsm_construct_check_symbol(handle, out);
    if symout == -1 {
        symout = fsm_construct_add_symbol(handle, out);
    }
    let sl = &mut handle.fsm_state_list[source as usize];
    let tl = Box::new(FsmTransList {
        /* int→short truncation as in C */
        r#in: symin as i16,
        out: symout as i16,
        target,
        next: sl.fsm_trans_list.take(),
    });
    sl.fsm_trans_list = Some(tl);
}

// [spec:foma:def:dynarray.fsm-construct-hash-sym-fn]
// [spec:foma:sem:dynarray.fsm-construct-hash-sym-fn]
pub fn fsm_construct_hash_sym(symbol: &str) -> u32 {
    let mut hash: u32 = 0;

    /* C sums plain `char` values: on signed-char platforms bytes >= 0x80
    add sign-extended negative values, wrapping the unsigned sum */
    for b in symbol.as_bytes() {
        hash = hash.wrapping_add((*b as i8 as i32) as u32);
    }
    hash % SIGMA_HASH_SIZE
}

// [spec:foma:def:dynarray.fsm-construct-add-arc-nums-fn]
// [spec:foma:sem:dynarray.fsm-construct-add-arc-nums-fn]
// [spec:foma:def:fomalib.fsm-construct-add-arc-nums-fn]
// [spec:foma:sem:fomalib.fsm-construct-add-arc-nums-fn]
pub fn fsm_construct_add_arc_nums(
    handle: &mut FsmConstructHandle,
    source: i32,
    target: i32,
    r#in: i32,
    out: i32,
) {
    fsm_construct_check_size(handle, source);
    fsm_construct_check_size(handle, target);

    if source > handle.maxstate {
        handle.maxstate = source;
    }
    if target > handle.maxstate {
        handle.maxstate = target;
    }

    handle.fsm_state_list[target as usize].used = true;
    let sl = &mut handle.fsm_state_list[source as usize];
    sl.used = true;
    /* no sigma lookup or insertion: the caller must guarantee the numbers
    have symbol entries. num_trans is not updated, as in C. */
    let tl = Box::new(FsmTransList {
        /* int→short truncation as in C */
        r#in: r#in as i16,
        out: out as i16,
        target,
        next: sl.fsm_trans_list.take(),
    });
    sl.fsm_trans_list = Some(tl);
}

/* Copies entire alphabet from existing network */

// [spec:foma:def:dynarray.fsm-construct-copy-sigma-fn]
// [spec:foma:sem:dynarray.fsm-construct-copy-sigma-fn]
// [spec:foma:def:fomalib.fsm-construct-copy-sigma-fn]
// [spec:foma:sem:fomalib.fsm-construct-copy-sigma-fn]
pub fn fsm_construct_copy_sigma(handle: &mut FsmConstructHandle, sigma: Option<&Sigma>) {
    let mut sigma = sigma;
    while let Some(s) = sigma {
        /* a node numbered -1 terminates the walk */
        if s.number == -1 {
            break;
        }
        let symnum = s.number;
        if symnum > handle.maxsigma {
            handle.maxsigma = symnum;
        }
        /* C derefs sigma->symbol unconditionally (strdup(NULL) crashes) */
        let symbol = s.symbol.as_deref().unwrap();
        if symnum >= handle.fsm_sigma_list_size {
            /* single growth step keyed on the current size, not on symnum:
            a number >= twice the current size is still out of range.
            New slots are not zero-initialized in C (None here). */
            handle.fsm_sigma_list_size = next_power_of_two(handle.fsm_sigma_list_size);
            // DEVIATION from C (OOB write when symnum >= the doubled size; Rust panics on the index below)
            handle
                .fsm_sigma_list
                .resize(handle.fsm_sigma_list_size as usize, FsmSigmaList { symbol: None });
        }
        /* Insert into list */
        /* C shares one strdup between the list slot and the hash node;
        owned copies here (observably equivalent) */
        let symdup = symbol.to_string();
        handle.fsm_sigma_list[symnum as usize].symbol = Some(symdup.clone());

        /* Insert into hashtable */
        let hash = fsm_construct_hash_sym(symbol);
        let fh = &mut handle.fsm_sigma_hash[hash as usize];
        if fh.symbol.is_none() {
            fh.symbol = Some(symdup);
            fh.sym = symnum as i16;
        } else {
            /* calloc'd chain node spliced directly after the head */
            let newfh = Box::new(FsmSigmaHash {
                symbol: Some(symdup),
                sym: symnum as i16,
                next: fh.next.take(),
            });
            fh.next = Some(newfh);
        }
        sigma = s.next.as_deref();
    }
}

// [spec:foma:def:dynarray.fsm-construct-add-symbol-fn]
// [spec:foma:sem:dynarray.fsm-construct-add-symbol-fn]
// [spec:foma:def:fomalib.fsm-construct-add-symbol-fn]
// [spec:foma:sem:fomalib.fsm-construct-add-symbol-fn]
pub fn fsm_construct_add_symbol(handle: &mut FsmConstructHandle, symbol: &str) -> i32 {
    /* no duplicate check: adding an existing symbol allocates a fresh
    number — callers probe with fsm_construct_check_symbol first */
    let mut symnum = 0;
    let mut reserved = 0;

    /* Is symbol reserved? */
    let mut i = 0;
    while FOMA_RESERVED_SYMBOLS[i].symbol.is_some() {
        if symbol == FOMA_RESERVED_SYMBOLS[i].symbol.unwrap() {
            symnum = FOMA_RESERVED_SYMBOLS[i].number;
            reserved = 1;
            if handle.maxsigma < symnum {
                handle.maxsigma = symnum;
            }
            break;
        }
        i += 1;
    }

    if reserved == 0 {
        symnum = handle.maxsigma + 1;
        if symnum < MINSIGMA {
            symnum = MINSIGMA;
        }
        handle.maxsigma = symnum;
    }

    if symnum >= handle.fsm_sigma_list_size {
        /* single growth step keyed on the current size (doubles a
        power-of-two size); new slots are not zero-initialized in C */
        handle.fsm_sigma_list_size = next_power_of_two(handle.fsm_sigma_list_size);
        // DEVIATION from C (OOB write when symnum >= the doubled size; Rust panics on the index below)
        handle
            .fsm_sigma_list
            .resize(handle.fsm_sigma_list_size as usize, FsmSigmaList { symbol: None });
    }
    /* Insert into list */
    /* C shares one strdup between the list slot and the hash node;
    owned copies here (observably equivalent) */
    let symdup = symbol.to_string();
    handle.fsm_sigma_list[symnum as usize].symbol = Some(symdup.clone());

    /* Insert into hashtable */
    let hash = fsm_construct_hash_sym(symbol);
    let fh = &mut handle.fsm_sigma_hash[hash as usize];
    if fh.symbol.is_none() {
        fh.symbol = Some(symdup);
        fh.sym = symnum as i16;
    } else {
        /* calloc'd chain node spliced directly after the head */
        let newfh = Box::new(FsmSigmaHash {
            symbol: Some(symdup),
            sym: symnum as i16,
            next: fh.next.take(),
        });
        fh.next = Some(newfh);
    }
    symnum
}

// [spec:foma:def:dynarray.fsm-construct-check-symbol-fn]
// [spec:foma:sem:dynarray.fsm-construct-check-symbol-fn]
// [spec:foma:def:fomalib.fsm-construct-check-symbol-fn]
// [spec:foma:sem:fomalib.fsm-construct-check-symbol-fn]
pub fn fsm_construct_check_symbol(handle: &FsmConstructHandle, symbol: &str) -> i32 {
    /* C: int hash (the unsigned return converted to int) */
    let hash = fsm_construct_hash_sym(symbol) as i32;
    if handle.fsm_sigma_hash[hash as usize].symbol.is_none() {
        return -1;
    }
    let mut fh = Some(&handle.fsm_sigma_hash[hash as usize]);
    while let Some(node) = fh {
        if node.symbol.as_deref() == Some(symbol) {
            /* short→int promotion */
            return node.sym as i32;
        }
        fh = node.next.as_deref();
    }
    -1
}

// [spec:foma:def:dynarray.fsm-construct-convert-sigma-fn]
// [spec:foma:sem:dynarray.fsm-construct-convert-sigma-fn]
pub fn fsm_construct_convert_sigma(handle: &FsmConstructHandle) -> Option<Box<Sigma>> {
    /* builds the list in ascending symbol-number order, appending at the
    tail; NULL-symbol slots are skipped */
    let mut sigma: Option<Box<Sigma>> = None;
    let mut tail: &mut Option<Box<Sigma>> = &mut sigma;
    for i in 0..=handle.maxsigma {
        if handle.fsm_sigma_list[i as usize].symbol.is_some() {
            /* C moves the char* out of fsm_sigma_list (no strdup) —
            ownership transfers to the sigma; cloned here since the handle
            is not mutable (observably equivalent: the handle's list is
            freed without freeing the strings) */
            let newsigma = Box::new(Sigma {
                number: i,
                symbol: handle.fsm_sigma_list[i as usize].symbol.clone(),
                next: None,
            });
            *tail = Some(newsigma);
            tail = &mut tail.as_mut().unwrap().next;
        }
    }
    sigma
}

// [spec:foma:def:dynarray.fsm-construct-done-fn]
// [spec:foma:sem:dynarray.fsm-construct-done-fn]
// [spec:foma:def:fomalib.fsm-construct-done-fn]
// [spec:foma:sem:fomalib.fsm-construct-done-fn]
pub fn fsm_construct_done(handle: Box<FsmConstructHandle>) -> Box<Fsm> {
    let mut handle = handle;
    if handle.maxstate == -1 || handle.numfinals == 0 || handle.hasinitial == 0 {
        // DEVIATION from C (the handle and its contents are leaked on this
        // path in C; Rust drops them)
        return fsm_empty_set();
    }
    fsm_state_init(handle.maxsigma + 1);

    /* emptyfsm tracks whether the FSM has (a) something outgoing from an
    initial state, or (b) an initial state that is final */
    let mut emptyfsm = 1;
    for i in 0..=handle.maxstate {
        let sl = &handle.fsm_state_list[i as usize];
        fsm_state_set_current_state(i, sl.is_final as i32, sl.is_initial as i32);
        if sl.is_initial && sl.is_final {
            emptyfsm = 0;
        }
        /* transition list is walked in reverse insertion order (LIFO) */
        let mut trans = sl.fsm_trans_list.as_deref();
        while let Some(t) = trans {
            if sl.is_initial {
                emptyfsm = 0;
            }
            /* short→int promotion on in/out */
            fsm_state_add_arc(
                i,
                t.r#in as i32,
                t.out as i32,
                t.target,
                sl.is_final as i32,
                sl.is_initial as i32,
            );
            trans = t.next.as_deref();
        }
        fsm_state_end_state();
    }
    let mut net = fsm_create("");
    net.name = format!("{:X}", rand());
    /* free(net->sigma) */
    net.sigma = None;
    fsm_state_close(&mut net);

    net.sigma = fsm_construct_convert_sigma(&handle);
    if let Some(name) = handle.name.take() {
        /* strncpy(net->name, handle->name, 40): at most 40 bytes are
        copied, with no NUL terminator when the name is >= 40 bytes —
        reproduced as truncation to 40 bytes per the conventions.
        DEVIATION from C (a cut inside a UTF-8 codepoint is lossy-decoded;
        C would keep the raw byte prefix) */
        let bytes = name.as_bytes();
        if bytes.len() > FSM_NAME_LEN {
            net.name = String::from_utf8_lossy(&bytes[..FSM_NAME_LEN]).into_owned();
        } else {
            net.name = name;
        }
        /* free(handle->name) — dropped with the take() above */
    } else {
        net.name = format!("{:X}", rand());
    }

    /* Free transitions (all fsm_state_list_size slots), the sigma-hash
    chain nodes, fsm_sigma_list, fsm_sigma_hash, fsm_state_list, and the
    handle itself — all dropped with `handle` here. The symbol strings
    now belong to net->sigma. */
    drop(handle);
    sigma_sort(&mut net);
    if emptyfsm != 0 {
        fsm_destroy(net);
        return fsm_empty_set();
    }
    net
}

/* Reading functions */

// [spec:foma:def:dynarray.fsm-read-is-final-fn]
// [spec:foma:sem:dynarray.fsm-read-is-final-fn]
// [spec:foma:def:fomalib.fsm-read-is-final-fn]
// [spec:foma:sem:fomalib.fsm-read-is-final-fn]
pub fn fsm_read_is_final(h: &FsmReadHandle, state: i32) -> i32 {
    /* no bounds check on state in C (OOB read); Rust panics */
    (h.lookuptable[state as usize] & 2) as i32
}

// [spec:foma:def:dynarray.fsm-read-is-initial-fn]
// [spec:foma:sem:dynarray.fsm-read-is-initial-fn]
// [spec:foma:def:fomalib.fsm-read-is-initial-fn]
// [spec:foma:sem:fomalib.fsm-read-is-initial-fn]
pub fn fsm_read_is_initial(h: &FsmReadHandle, state: i32) -> i32 {
    /* no bounds check on state in C (OOB read); Rust panics */
    (h.lookuptable[state as usize] & 1) as i32
}

// [spec:foma:def:dynarray.fsm-read-init-fn]
// [spec:foma:sem:dynarray.fsm-read-init-fn]
// [spec:foma:def:fomalib.fsm-read-init-fn]
// [spec:foma:sem:fomalib.fsm-read-init-fn]
pub fn fsm_read_init(net: Option<Box<Fsm>>) -> Option<Box<FsmReadHandle>> {
    if net.is_none() {
        return None;
    }
    // DEVIATION from C (the C handle borrows the caller's net pointer; the
    // Rust handle owns the net for its lifetime and fsm_read_done returns it)
    let net = net.unwrap();

    let num_states = net.statecount;
    let mut lookuptable: Vec<u8> = vec![0; num_states as usize];

    let mut num_initials = 0;
    let mut num_finals = 0;

    /* calloc(num_states+1, sizeof(struct fsm **)) */
    let mut states_head: Vec<Option<usize>> = vec![None; (num_states + 1) as usize];
    let mut has_unknowns = false;

    let mut laststate = -1;
    let fsm = &net.states;
    let mut i = 0usize;
    while fsm[i].state_no != -1 {
        let sno = fsm[i].state_no;
        if fsm[i].start_state != 0 {
            /* lookuptable and states_head are sized by statecount but
            indexed by state_no: sparse state numbering writes OOB in C.
            DEVIATION from C (OOB write; Rust panics) */
            if lookuptable[sno as usize] & 1 == 0 {
                lookuptable[sno as usize] |= 1;
                num_initials += 1;
            }
        }
        if fsm[i].final_state != 0 {
            if lookuptable[sno as usize] & 2 == 0 {
                lookuptable[sno as usize] |= 2;
                num_finals += 1;
            }
        }
        if fsm[i].r#in as i32 == UNKNOWN
            || fsm[i].out as i32 == UNKNOWN
            || fsm[i].r#in as i32 == IDENTITY
            || fsm[i].out as i32 == IDENTITY
        {
            has_unknowns = true;
        }
        if fsm[i].state_no != laststate {
            /* pointer to the state's first line → index */
            states_head[fsm[i].state_no as usize] = Some(i);
        }
        laststate = fsm[i].state_no;
        i += 1;
    }

    let mut finals_head: Vec<i32> = vec![0; (num_finals + 1) as usize];
    let mut initials_head: Vec<i32> = vec![0; (num_initials + 1) as usize];

    let mut j = 0usize;
    let mut k = 0usize;
    for i in 0..num_states {
        if lookuptable[i as usize] & 1 != 0 {
            initials_head[j] = i;
            j += 1;
        }
        if lookuptable[i as usize] & 2 != 0 {
            finals_head[k] = i;
            k += 1;
        }
    }
    initials_head[j] = -1;
    finals_head[k] = -1;

    let fsm_sigma_list = sigma_to_list(net.sigma.as_deref());
    let sigma_list_size = sigma_max(net.sigma.as_deref()) + 1;

    /* handle = calloc(1, ...): all cursors NULL, current_state 0 */
    Some(Box::new(FsmReadHandle {
        finals_head,
        initials_head,
        states_head,
        fsm_sigma_list,
        sigma_list_size,
        /* arcs_head = fsm (base of net->states) → index 0 */
        arcs_head: 0,
        arcs_cursor: None,
        finals_cursor: None,
        states_cursor: None,
        initials_cursor: None,
        current_state: 0,
        lookuptable,
        has_unknowns,
        net: Some(net),
    }))
}

// [spec:foma:def:dynarray.fsm-read-reset-fn]
// [spec:foma:sem:dynarray.fsm-read-reset-fn]
// [spec:foma:def:fomalib.fsm-read-reset-fn]
// [spec:foma:sem:fomalib.fsm-read-reset-fn]
pub fn fsm_read_reset(handle: Option<&mut FsmReadHandle>) {
    if handle.is_none() {
        return;
    }
    let handle = handle.unwrap();
    handle.arcs_cursor = None;
    handle.initials_cursor = None;
    handle.finals_cursor = None;
    handle.states_cursor = None;
}

// [spec:foma:def:dynarray.fsm-get-next-state-arc-fn]
// [spec:foma:sem:dynarray.fsm-get-next-state-arc-fn]
// [spec:foma:def:fomalib.fsm-get-next-state-arc-fn]
// [spec:foma:sem:fomalib.fsm-get-next-state-arc-fn]
pub fn fsm_get_next_state_arc(handle: &mut FsmReadHandle) -> i32 {
    /* pre-increment: fsm_get_next_state parked the cursor one line before
    the state's first line (wrapping_sub(1); see module docs). Calling
    this with a NULL cursor is a crash in C — unwrap panics. */
    let cursor = handle.arcs_cursor.unwrap().wrapping_add(1);
    handle.arcs_cursor = Some(cursor);
    let states = &handle.net.as_ref().unwrap().states;
    if states[cursor].state_no != handle.current_state || states[cursor].target == -1 {
        handle.arcs_cursor = Some(cursor.wrapping_sub(1));
        return 0;
    }
    1
}

// [spec:foma:def:dynarray.fsm-get-next-arc-fn]
// [spec:foma:sem:dynarray.fsm-get-next-arc-fn]
// [spec:foma:def:fomalib.fsm-get-next-arc-fn]
// [spec:foma:sem:fomalib.fsm-get-next-arc-fn]
pub fn fsm_get_next_arc(handle: &mut FsmReadHandle) -> i32 {
    let states = &handle.net.as_ref().unwrap().states;
    if handle.arcs_cursor.is_none() {
        let mut cursor = handle.arcs_head;
        /* skip sentinel lines (target == -1) */
        while states[cursor].state_no != -1 && states[cursor].target == -1 {
            cursor += 1;
        }
        handle.arcs_cursor = Some(cursor);
        if states[cursor].state_no == -1 {
            return 0;
        }
    } else {
        let mut cursor = handle.arcs_cursor.unwrap();
        /* sticky terminator: once on the state_no == -1 line, keep
        returning 0 without moving */
        if states[cursor].state_no == -1 {
            return 0;
        }
        loop {
            cursor += 1;
            if !(states[cursor].state_no != -1 && states[cursor].target == -1) {
                break;
            }
        }
        handle.arcs_cursor = Some(cursor);
        if states[cursor].state_no == -1 {
            return 0;
        }
    }
    1
}

// [spec:foma:def:dynarray.fsm-get-arc-source-fn]
// [spec:foma:sem:dynarray.fsm-get-arc-source-fn]
// [spec:foma:def:fomalib.fsm-get-arc-source-fn]
// [spec:foma:sem:fomalib.fsm-get-arc-source-fn]
pub fn fsm_get_arc_source(handle: &FsmReadHandle) -> i32 {
    if handle.arcs_cursor.is_none() {
        return -1;
    }
    handle.net.as_ref().unwrap().states[handle.arcs_cursor.unwrap()].state_no
}

// [spec:foma:def:dynarray.fsm-get-arc-target-fn]
// [spec:foma:sem:dynarray.fsm-get-arc-target-fn]
// [spec:foma:def:fomalib.fsm-get-arc-target-fn]
// [spec:foma:sem:fomalib.fsm-get-arc-target-fn]
pub fn fsm_get_arc_target(handle: &FsmReadHandle) -> i32 {
    if handle.arcs_cursor.is_none() {
        return -1;
    }
    handle.net.as_ref().unwrap().states[handle.arcs_cursor.unwrap()].target
}

// [spec:foma:def:dynarray.fsm-get-symbol-number-fn]
// [spec:foma:sem:dynarray.fsm-get-symbol-number-fn]
// [spec:foma:def:fomalib.fsm-get-symbol-number-fn]
// [spec:foma:sem:fomalib.fsm-get-symbol-number-fn]
pub fn fsm_get_symbol_number(handle: &FsmReadHandle, symbol: &str) -> i32 {
    for i in 0..handle.sigma_list_size {
        if handle.fsm_sigma_list[i as usize].symbol.is_none() {
            continue;
        }
        if handle.fsm_sigma_list[i as usize].symbol.as_deref() == Some(symbol) {
            return i;
        }
    }
    -1
}

// [spec:foma:def:dynarray.fsm-get-arc-in-fn]
// [spec:foma:sem:dynarray.fsm-get-arc-in-fn]
// [spec:foma:def:fomalib.fsm-get-arc-in-fn]
// [spec:foma:sem:fomalib.fsm-get-arc-in-fn]
pub fn fsm_get_arc_in(handle: &FsmReadHandle) -> Option<&str> {
    /* C returns a borrowed char* into the handle's sigma list, or NULL
    when the cursor is NULL */
    if handle.arcs_cursor.is_none() {
        return None;
    }
    let index = handle.net.as_ref().unwrap().states[handle.arcs_cursor.unwrap()].r#in;
    /* no sentinel check: in == -1 indexes out of bounds in C.
    DEVIATION from C (OOB read; Rust panics) */
    handle.fsm_sigma_list[index as usize].symbol.as_deref()
}

// [spec:foma:def:dynarray.fsm-get-arc-num-in-fn]
// [spec:foma:sem:dynarray.fsm-get-arc-num-in-fn]
// [spec:foma:def:fomalib.fsm-get-arc-num-in-fn]
// [spec:foma:sem:fomalib.fsm-get-arc-num-in-fn]
pub fn fsm_get_arc_num_in(handle: &FsmReadHandle) -> i32 {
    if handle.arcs_cursor.is_none() {
        return -1;
    }
    /* short→int promotion; a sentinel line's stored -1 returns as-is */
    handle.net.as_ref().unwrap().states[handle.arcs_cursor.unwrap()].r#in as i32
}

// [spec:foma:def:dynarray.fsm-get-arc-num-out-fn]
// [spec:foma:sem:dynarray.fsm-get-arc-num-out-fn]
// [spec:foma:def:fomalib.fsm-get-arc-num-out-fn]
// [spec:foma:sem:fomalib.fsm-get-arc-num-out-fn]
pub fn fsm_get_arc_num_out(handle: &FsmReadHandle) -> i32 {
    if handle.arcs_cursor.is_none() {
        return -1;
    }
    /* short→int promotion; a sentinel line's stored -1 returns as-is */
    handle.net.as_ref().unwrap().states[handle.arcs_cursor.unwrap()].out as i32
}

// [spec:foma:def:dynarray.fsm-get-arc-out-fn]
// [spec:foma:sem:dynarray.fsm-get-arc-out-fn]
// [spec:foma:def:fomalib.fsm-get-arc-out-fn]
// [spec:foma:sem:fomalib.fsm-get-arc-out-fn]
pub fn fsm_get_arc_out(handle: &FsmReadHandle) -> Option<&str> {
    /* C returns a borrowed char* into the handle's sigma list, or NULL
    when the cursor is NULL */
    if handle.arcs_cursor.is_none() {
        return None;
    }
    let index = handle.net.as_ref().unwrap().states[handle.arcs_cursor.unwrap()].out;
    /* no sentinel check: out == -1 indexes out of bounds in C.
    DEVIATION from C (OOB read; Rust panics) */
    handle.fsm_sigma_list[index as usize].symbol.as_deref()
}

// [spec:foma:def:dynarray.fsm-get-next-initial-fn]
// [spec:foma:sem:dynarray.fsm-get-next-initial-fn]
// [spec:foma:def:fomalib.fsm-get-next-initial-fn]
// [spec:foma:sem:fomalib.fsm-get-next-initial-fn]
pub fn fsm_get_next_initial(handle: &mut FsmReadHandle) -> i32 {
    if handle.initials_cursor.is_none() {
        handle.initials_cursor = Some(0);
    } else {
        /* sticky -1 terminator: the end returns -1 without advancing */
        if handle.initials_head[handle.initials_cursor.unwrap()] == -1 {
            return -1;
        }
        handle.initials_cursor = Some(handle.initials_cursor.unwrap() + 1);
    }
    handle.initials_head[handle.initials_cursor.unwrap()]
}

// [spec:foma:def:dynarray.fsm-get-next-final-fn]
// [spec:foma:sem:dynarray.fsm-get-next-final-fn]
// [spec:foma:def:fomalib.fsm-get-next-final-fn]
// [spec:foma:sem:fomalib.fsm-get-next-final-fn]
pub fn fsm_get_next_final(handle: &mut FsmReadHandle) -> i32 {
    if handle.finals_cursor.is_none() {
        handle.finals_cursor = Some(0);
    } else {
        /* sticky -1 terminator: the end returns -1 without advancing */
        if handle.finals_head[handle.finals_cursor.unwrap()] == -1 {
            return -1;
        }
        handle.finals_cursor = Some(handle.finals_cursor.unwrap() + 1);
    }
    handle.finals_head[handle.finals_cursor.unwrap()]
}

// [spec:foma:def:dynarray.fsm-get-num-states-fn]
// [spec:foma:sem:dynarray.fsm-get-num-states-fn]
// [spec:foma:def:fomalib.fsm-get-num-states-fn]
// [spec:foma:sem:fomalib.fsm-get-num-states-fn]
pub fn fsm_get_num_states(handle: &FsmReadHandle) -> i32 {
    handle.net.as_ref().unwrap().statecount
}

// [spec:foma:def:dynarray.fsm-get-has-unknowns-fn]
// [spec:foma:sem:dynarray.fsm-get-has-unknowns-fn]
// [spec:foma:def:fomalib.fsm-get-has-unknowns-fn]
// [spec:foma:sem:fomalib.fsm-get-has-unknowns-fn]
pub fn fsm_get_has_unknowns(handle: &FsmReadHandle) -> i32 {
    handle.has_unknowns as i32
}

// [spec:foma:def:dynarray.fsm-get-next-state-fn]
// [spec:foma:sem:dynarray.fsm-get-next-state-fn]
// [spec:foma:def:fomalib.fsm-get-next-state-fn]
// [spec:foma:sem:fomalib.fsm-get-next-state-fn]
pub fn fsm_get_next_state(handle: &mut FsmReadHandle) -> i32 {
    if handle.states_cursor.is_none() {
        handle.states_cursor = Some(0);
    } else {
        handle.states_cursor = Some(handle.states_cursor.unwrap() + 1);
    }
    /* C: states_cursor - states_head >= fsm_get_num_states(handle) —
    ptrdiff vs int comparison, done in i64 here */
    if handle.states_cursor.unwrap() as i64 >= fsm_get_num_states(handle) as i64 {
        return -1;
    }
    /* the state's first line; a NULL entry (state number gap) is a crash
    in C — unwrap panics */
    let first = handle.states_head[handle.states_cursor.unwrap()].unwrap();
    let stateno = handle.net.as_ref().unwrap().states[first].state_no;
    /* park arcs_cursor one line before the state's first line so that
    fsm_get_next_state_arc's pre-increment lands on it (C decrements the
    pointer below the array base for first == 0 — UB; wrapping index here) */
    handle.arcs_cursor = Some(first.wrapping_sub(1));
    handle.current_state = stateno;
    stateno
}

// [spec:foma:def:dynarray.fsm-read-done-fn]
// [spec:foma:sem:dynarray.fsm-read-done-fn]
// [spec:foma:def:fomalib.fsm-read-done-fn]
// [spec:foma:sem:fomalib.fsm-read-done-fn]
pub fn fsm_read_done(handle: Box<FsmReadHandle>) -> Box<Fsm> {
    /* frees lookuptable, fsm_sigma_list (array only — the symbol strings
    are copies here where C borrows net->sigma's), finals_head,
    initials_head, states_head, and the handle — all dropped here.
    DEVIATION from C (C leaves the caller's net pointer untouched; the
    Rust handle owns the net, so it is returned to the caller) */
    let mut handle = handle;
    handle.net.take().unwrap()
}

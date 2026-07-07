//! foma/structures.c — Wave-4 idiomatization of the Wave-2 port per
//! docs/port/rust-conventions.md. Sem rules: docs/spec/port/foma/structures.md
//! (per-file ids) plus the fomalib.h / foma.h / fomalibconf.h prototype ids.
//!
//! The fsm line table is the sentinel-terminated `Vec<FsmState>` of types.rs;
//! C pointer walks become index walks with identical stop conditions.
//! Consuming (`Box<Fsm>`) vs borrowing (`&Fsm`/`&mut Fsm`) conventions follow
//! each function's sem rule. Wave 4 fixed four documented bugs (fsm_isuniversal,
//! purge_quantifier, union_quantifiers' linecount, fsm_copy's stale counts —
//! each `+1`-bumped) and pruned obsolete memory-hazard DEVIATION notes.

use std::cell::RefCell;

use crate::constructions::{
    add_fsm_arc, fsm_compact, fsm_complement, fsm_compose, fsm_concat, fsm_contains, fsm_count,
    fsm_ignore, fsm_invert, fsm_kleene_star, fsm_symbol, fsm_term_negation, fsm_union,
    fsm_universal, fsm_update_flags,
};
use crate::dynarray::{
    fsm_state_add_arc, fsm_state_close, fsm_state_end_state, fsm_state_init,
    fsm_state_set_current_state,
};
use crate::extract::fsm_upper;
use crate::int_stack::{ptr_stack_clear, ptr_stack_isempty, ptr_stack_pop, ptr_stack_push};
use crate::minimize::fsm_minimize;
use crate::sigma::{
    sigma_add, sigma_cleanup, sigma_copy, sigma_create, sigma_max, sigma_remove, sigma_size,
    sigma_sort,
};
use crate::topsort::fsm_topsort;
use crate::types::{
    BUILD_VERSION, DefinedQuantifiers, EPSILON, Fsm, FsmOptions, FsmState, IDENTITY, MAJOR_VERSION,
    MINOR_VERSION, NO, OP_IGNORE_ALL, STATUS_VERSION, Sigma, StateArray, UNKNOWN, YES,
};

thread_local! {
    // C: static struct defined_quantifiers *quantifiers;
    static QUANTIFIERS: RefCell<Option<Box<DefinedQuantifiers>>> = const { RefCell::new(None) };
    // C: struct _fsm_options fsm_options; — non-static zero-initialized
    // global (extern'd via foma.h); no spec id of its own (the type carries
    // foma.fsm-options in types.rs)
    pub static FSM_OPTIONS: RefCell<FsmOptions> =
        const { RefCell::new(FsmOptions { skip_word_boundary_marker: false }) };
}

// [spec:foma:def:structures.fsm-get-library-version-string-fn]
// [spec:foma:sem:structures.fsm-get-library-version-string-fn]
// [spec:foma:def:fomalib.fsm-get-library-version-string-fn]
// [spec:foma:sem:fomalib.fsm-get-library-version-string-fn]
pub fn fsm_get_library_version_string() -> String {
    /* C: sprintf's into a function-local static char s[20] (rewritten on
    every call, not thread-safe) and returns that pointer; an owned String
    here (observably the same "0.10.0alpha" text) */
    format!(
        "{}.{}.{}{}",
        MAJOR_VERSION, MINOR_VERSION, BUILD_VERSION, STATUS_VERSION
    )
}

// [spec:foma:def:structures.fsm-set-option-fn]
// [spec:foma:sem:structures.fsm-set-option-fn]
// [spec:foma:def:fomalib.fsm-set-option-fn]
// [spec:foma:sem:fomalib.fsm-set-option-fn]
pub fn fsm_set_option(option: u64, value: &bool) -> bool {
    /* C: switch (option) — value is a void * dereferenced as _Bool * for
    the matching option (never NULL-checked; &bool here) */
    if option == crate::types::FSM_OPTIONS::FSMO_SKIP_WORD_BOUNDARY_MARKER as u64 {
        FSM_OPTIONS.with(|o| o.borrow_mut().skip_word_boundary_marker = *value);
        return true;
    }
    false
}

// [spec:foma:def:structures.fsm-get-option-fn]
// [spec:foma:sem:structures.fsm-get-option-fn]
// [spec:foma:def:fomalib.fsm-get-option-fn]
// [spec:foma:sem:fomalib.fsm-get-option-fn]
// DEVIATION from C (returns a void * aliasing the live global option field;
// safe Rust returns the current value instead — None ↔ NULL for unknown
// options)
pub fn fsm_get_option(option: u64) -> Option<bool> {
    if option == crate::types::FSM_OPTIONS::FSMO_SKIP_WORD_BOUNDARY_MARKER as u64 {
        return Some(FSM_OPTIONS.with(|o| o.borrow().skip_word_boundary_marker));
    }
    None
}

// [spec:foma:def:structures.linesortcompin-fn]
// [spec:foma:sem:structures.linesortcompin-fn]
pub fn linesortcompin(a: &FsmState, b: &FsmState) -> i32 {
    /* C: qsort comparator over struct fsm_state; int subtraction of the
    short `in` fields */
    a.r#in as i32 - b.r#in as i32
}

// [spec:foma:def:structures.linesortcompout-fn]
// [spec:foma:sem:structures.linesortcompout-fn]
pub fn linesortcompout(a: &FsmState, b: &FsmState) -> i32 {
    a.out as i32 - b.out as i32
}

// [spec:foma:def:structures.fsm-sort-arcs-fn]
// [spec:foma:sem:structures.fsm-sort-arcs-fn]
// [spec:foma:def:fomalib.fsm-sort-arcs-fn]
// [spec:foma:sem:fomalib.fsm-sort-arcs-fn]
pub fn fsm_sort_arcs(net: &mut Fsm, direction: i32) {
    /* direction 1 = in, direction = 2, out */
    let scin: fn(&FsmState, &FsmState) -> i32 = linesortcompin;
    let scout: fn(&FsmState, &FsmState) -> i32 = linesortcompout;
    let fsm = &mut net.states;
    let mut numlines: i32 = 0;
    let mut lasthead: usize = 0;
    let mut i: usize = 0;
    while fsm[i].state_no != -1 {
        if fsm[i].state_no != fsm[i + 1].state_no || fsm[i].target == -1 {
            numlines += 1;
            if fsm[i].target == -1 {
                numlines -= 1;
            }
            if numlines > 1 {
                /* Sort, set numlines = 0 */
                /* C: qsort (unstable); a stable slice sort is an admissible
                qsort behavior */
                if direction == 1 {
                    fsm[lasthead..lasthead + numlines as usize].sort_by(|a, b| scin(a, b).cmp(&0));
                } else {
                    fsm[lasthead..lasthead + numlines as usize].sort_by(|a, b| scout(a, b).cmp(&0));
                }
            }
            numlines = 0;
            lasthead = i + 1;
            i += 1;
            continue;
        }
        numlines += 1;
        i += 1;
    }
    if net.arity == 1 {
        net.arcs_sorted_in = 1;
        net.arcs_sorted_out = 1;
        return;
    }
    if direction == 1 {
        net.arcs_sorted_in = 1;
        net.arcs_sorted_out = 0;
    }
    if direction == 2 {
        net.arcs_sorted_out = 1;
        net.arcs_sorted_in = 0;
    }
}

// [spec:foma:def:structures.map-firstlines-fn]
// [spec:foma:sem:structures.map-firstlines-fn]
// [spec:foma:def:fomalibconf.map-firstlines-fn]
// [spec:foma:sem:fomalibconf.map-firstlines-fn]
pub fn map_firstlines(net: &Fsm) -> Vec<StateArray> {
    let mut sold: i32 = -1;
    /* C malloc'd (statecount+1) uninitialized entries; zeroed here, so a
    state number that never appears reads as index 0 rather than garbage. */
    let mut sa: Vec<StateArray> =
        vec![StateArray { transitions: 0 }; (net.statecount + 1) as usize];
    let fsm = &net.states;
    let mut i: usize = 0;
    while fsm[i].state_no != -1 {
        if fsm[i].state_no != sold {
            /* pointer to the state's first line → index */
            sa[fsm[i].state_no as usize].transitions = i;
            sold = fsm[i].state_no;
        }
        i += 1;
    }
    sa
}

// [spec:foma:def:structures.fsm-boolean-fn]
// [spec:foma:sem:structures.fsm-boolean-fn]
// [spec:foma:def:fomalib.fsm-boolean-fn]
// [spec:foma:sem:fomalib.fsm-boolean-fn]
pub fn fsm_boolean(value: i32) -> Box<Fsm> {
    if value == 0 {
        fsm_empty_set()
    } else {
        fsm_empty_string()
    }
}

// [spec:foma:def:structures.fsm-sigma-net-fn]
// [spec:foma:sem:structures.fsm-sigma-net-fn]
// [spec:foma:def:fomalib.fsm-sigma-net-fn]
// [spec:foma:sem:fomalib.fsm-sigma-net-fn]
pub fn fsm_sigma_net(net: Box<Fsm>) -> Box<Fsm> {
    /* Extract sigma and create net with one arc            */
    /* from state 0 to state 1 with each (state 1 is final) */
    let mut net = net;

    if sigma_size(net.sigma.as_deref()) == 0 {
        fsm_destroy(net);
        return fsm_empty_set();
    }

    fsm_state_init(sigma_max(net.sigma.as_deref()));
    fsm_state_set_current_state(0, 0, 1);
    let mut pathcount: i32 = 0;
    let mut sig = net.sigma.as_deref();
    while let Some(s) = sig {
        if s.number >= 3 || s.number == IDENTITY {
            pathcount += 1;
            fsm_state_add_arc(0, s.number, s.number, 1, 0, 1);
        }
        sig = s.next.as_deref();
    }
    fsm_state_end_state();
    fsm_state_set_current_state(1, 1, 0);
    fsm_state_end_state();
    /* free(net->states) */
    net.states = Vec::new();
    fsm_state_close(&mut net);
    net.is_minimized = YES;
    net.is_loop_free = YES;
    net.pathcount = pathcount as i64;
    sigma_cleanup(&mut net, 1);
    net
}

// [spec:foma:def:structures.fsm-sigma-pairs-net-fn]
// [spec:foma:sem:structures.fsm-sigma-pairs-net-fn]
// [spec:foma:def:fomalib.fsm-sigma-pairs-net-fn]
// [spec:foma:sem:fomalib.fsm-sigma-pairs-net-fn]
pub fn fsm_sigma_pairs_net(net: Box<Fsm>) -> Box<Fsm> {
    /* Create FSM of attested pairs */
    let mut net = net;

    let smax: i32 = sigma_max(net.sigma.as_deref()) + 1;
    /* calloc(smax*smax, sizeof(char)) */
    let mut pairs: Vec<i8> = vec![0; (smax * smax) as usize];

    fsm_state_init(sigma_max(net.sigma.as_deref()));
    fsm_state_set_current_state(0, 0, 1);
    let mut pathcount: i32 = 0;
    let mut i: usize = 0;
    while net.states[i].state_no != -1 {
        if net.states[i].target == -1 {
            i += 1;
            continue;
        }
        let r#in: i16 = net.states[i].r#in;
        let out: i16 = net.states[i].out;
        if pairs[(smax * r#in as i32 + out as i32) as usize] == 0 {
            fsm_state_add_arc(0, r#in as i32, out as i32, 1, 0, 1);
            pairs[(smax * r#in as i32 + out as i32) as usize] = 1;
            pathcount += 1;
        }
        i += 1;
    }
    fsm_state_end_state();
    fsm_state_set_current_state(1, 1, 0);
    fsm_state_end_state();

    /* free(pairs); free(net->states) */
    drop(pairs);
    net.states = Vec::new();

    fsm_state_close(&mut net);
    if pathcount == 0 {
        fsm_destroy(net);
        return fsm_empty_set();
    }
    net.is_minimized = YES;
    net.is_loop_free = YES;
    net.pathcount = pathcount as i64;
    sigma_cleanup(&mut net, 1);
    net
}

// [spec:foma:def:structures.fsm-sigma-destroy-fn]
// [spec:foma:sem:structures.fsm-sigma-destroy-fn]
// [spec:foma:def:fomalib.fsm-sigma-destroy-fn]
// [spec:foma:sem:fomalib.fsm-sigma-destroy-fn]
pub fn fsm_sigma_destroy(sigma: Option<Box<Sigma>>) -> i32 {
    /* per node: save next, free(symbol), free(node) — iterative drop (also
    avoids recursive-drop stack depth on long lists) */
    let mut sig = sigma;
    while let Some(mut node) = sig {
        let sigp = node.next.take();
        drop(node);
        sig = sigp;
    }
    1
}

// [spec:foma:def:structures.fsm-destroy-fn]
// [spec:foma:sem:structures.fsm-destroy-fn]
// [spec:foma:def:fomalib.fsm-destroy-fn]
// [spec:foma:sem:fomalib.fsm-destroy-fn]
pub fn fsm_destroy(net: Box<Fsm>) -> i32 {
    /* C: returns 0 without doing anything when net == NULL; a Box argument
    is never NULL — NULL-able callers keep the check at the call site */
    let mut net = net;
    if net.medlookup.is_some() {
        /* free(net->medlookup->confusion_matrix); free(net->medlookup) */
        net.medlookup = None;
    }
    fsm_sigma_destroy(net.sigma.take());
    if !net.states.is_empty() {
        /* free(net->states) */
        net.states = Vec::new();
    }
    /* free(net) — drop */
    1
}

// [spec:foma:def:structures.fsm-create-fn]
// [spec:foma:sem:structures.fsm-create-fn+1]
// [spec:foma:def:fomalib.fsm-create-fn]
// [spec:foma:sem:fomalib.fsm-create-fn+1]
pub fn fsm_create(name: &str) -> Box<Fsm> {
    // [spec:foma:sem:structures.fsm-create-fn+1] the in-memory net name is stored
    // in full. C used a fixed char[40] field (strncpy without a NUL terminator for
    // names >= 40 bytes), truncating longer names and printing a warning. (The
    // binary file format still caps names at 40 bytes on read/write.)
    let name = name.to_string();
    Box::new(Fsm {
        name,
        arity: 1,
        arccount: 0,
        /* C left statecount, linecount, finalcount, pathcount and
        is_completed as malloc garbage; zero-initialized here. */
        statecount: 0,
        linecount: 0,
        finalcount: 0,
        pathcount: 0,
        is_deterministic: NO,
        is_pruned: NO,
        is_minimized: NO,
        is_epsilon_free: NO,
        is_loop_free: NO,
        is_completed: 0,
        arcs_sorted_in: NO,
        arcs_sorted_out: NO,
        sigma: Some(sigma_create()),
        states: Vec::new(),
        medlookup: None,
    })
}

// [spec:foma:def:structures.fsm-empty-string-fn]
// [spec:foma:sem:structures.fsm-empty-string-fn]
// [spec:foma:def:fomalib.fsm-empty-string-fn]
// [spec:foma:sem:fomalib.fsm-empty-string-fn]
pub fn fsm_empty_string() -> Box<Fsm> {
    let mut net = fsm_create("");
    /* C: malloc(2 lines), uninitialized; every line is written by the
    add_fsm_arc calls below */
    net.states = vec![
        FsmState {
            state_no: 0,
            r#in: 0,
            out: 0,
            target: 0,
            final_state: 0,
            start_state: 0,
        };
        2
    ];
    add_fsm_arc(&mut net.states, 0, 0, -1, -1, -1, 1, 1);
    add_fsm_arc(&mut net.states, 1, -1, -1, -1, -1, -1, -1);
    fsm_update_flags(&mut net, YES, YES, YES, YES, YES, NO);
    net.statecount = 1;
    net.finalcount = 1;
    net.arccount = 0;
    net.linecount = 2;
    net.pathcount = 1;
    net
}

// [spec:foma:def:structures.fsm-identity-fn]
// [spec:foma:sem:structures.fsm-identity-fn]
// [spec:foma:def:fomalib.fsm-identity-fn]
// [spec:foma:sem:fomalib.fsm-identity-fn]
pub fn fsm_identity() -> Box<Fsm> {
    let mut net = fsm_create("");
    /* free(net->sigma) — the single empty sigma node fsm_create made */
    net.sigma = None;
    /* C: malloc(3 lines), uninitialized; written by add_fsm_arc below */
    net.states = vec![
        FsmState {
            state_no: 0,
            r#in: 0,
            out: 0,
            target: 0,
            final_state: 0,
            start_state: 0,
        };
        3
    ];
    add_fsm_arc(&mut net.states, 0, 0, 2, 2, 1, 0, 1);
    add_fsm_arc(&mut net.states, 1, 1, -1, -1, -1, 1, 0);
    add_fsm_arc(&mut net.states, 2, -1, -1, -1, -1, -1, -1);
    let sigma = Box::new(Sigma {
        number: IDENTITY,
        symbol: Some("@_IDENTITY_SYMBOL_@".to_string()),
        next: None,
    });
    net.sigma = Some(sigma);
    fsm_update_flags(&mut net, YES, YES, YES, YES, YES, NO);
    net.statecount = 2;
    net.finalcount = 1;
    net.arccount = 1;
    net.linecount = 3;
    net.pathcount = 1;
    net
}

// [spec:foma:def:structures.fsm-empty-set-fn]
// [spec:foma:sem:structures.fsm-empty-set-fn]
// [spec:foma:def:fomalib.fsm-empty-set-fn]
// [spec:foma:sem:fomalib.fsm-empty-set-fn]
pub fn fsm_empty_set() -> Box<Fsm> {
    let mut net = fsm_create("");
    net.states = fsm_empty();
    fsm_update_flags(&mut net, YES, YES, YES, YES, YES, NO);
    net.statecount = 1;
    net.finalcount = 0;
    net.arccount = 0;
    net.linecount = 2;
    net.pathcount = 0;
    net
}

// [spec:foma:def:structures.fsm-empty-fn]
// [spec:foma:sem:structures.fsm-empty-fn]
// [spec:foma:def:fomalib.fsm-empty-fn]
// [spec:foma:sem:fomalib.fsm-empty-fn]
pub fn fsm_empty() -> Vec<FsmState> {
    /* C: malloc(2 lines), uninitialized; written by add_fsm_arc below */
    let mut new_fsm: Vec<FsmState> = vec![
        FsmState {
            state_no: 0,
            r#in: 0,
            out: 0,
            target: 0,
            final_state: 0,
            start_state: 0,
        };
        2
    ];
    add_fsm_arc(&mut new_fsm, 0, 0, -1, -1, -1, 0, 1);
    add_fsm_arc(&mut new_fsm, 1, -1, -1, -1, -1, -1, -1);
    new_fsm
}

// [spec:foma:def:structures.fsm-isuniversal-fn]
// [spec:foma:sem:structures.fsm-isuniversal-fn+1]
// [spec:foma:def:fomalib.fsm-isuniversal-fn]
// [spec:foma:sem:fomalib.fsm-isuniversal-fn+1]
pub fn fsm_isuniversal(net: Box<Fsm>) -> i32 {
    /* destructive: consumes/replaces the argument; the minimized+compacted
    net is dropped (C leaked it, neither returning nor destroying it).

    Wave 4 fix: the C condition ANDed `line1.state_no == 0` with
    `line1.state_no == -1` (mutually exclusive → always 0). Implement the
    evident universality test instead: after minimize+compact, the universal
    language ?* is exactly the single state 0 that is final with an
    IDENTITY:IDENTITY self-loop (target 0), followed immediately by the -1
    sentinel, over an alphabet of only reserved symbols (sigma_max < 3). */
    let mut net = fsm_minimize(net);
    fsm_compact(&mut net);
    let fsm = &net.states;
    if fsm[0].target == 0
        && fsm[0].final_state == 1
        && fsm[0].r#in as i32 == IDENTITY
        && fsm[0].out as i32 == IDENTITY
        && fsm[1].state_no == -1
        && sigma_max(net.sigma.as_deref()) < 3
    {
        1
    } else {
        0
    }
}

// [spec:foma:def:structures.fsm-isempty-fn]
// [spec:foma:sem:structures.fsm-isempty-fn]
// [spec:foma:def:fomalib.fsm-isempty-fn]
// [spec:foma:sem:fomalib.fsm-isempty-fn]
pub fn fsm_isempty(net: &mut Fsm) -> i32 {
    /* &mut: fsm_copy refreshes the source's counts via fsm_count */
    let minimal = fsm_minimize(fsm_copy(net));
    let fsm = &minimal.states;
    let result = if fsm[0].target == -1 && fsm[0].final_state == 0 && fsm[1].state_no == -1 {
        1
    } else {
        0
    };
    fsm_destroy(minimal);
    result
}

// [spec:foma:def:structures.fsm-issequential-fn]
// [spec:foma:sem:structures.fsm-issequential-fn]
// [spec:foma:def:fomalib.fsm-issequential-fn]
// [spec:foma:sem:fomalib.fsm-issequential-fn]
pub fn fsm_issequential(net: &Fsm) -> i32 {
    /* calloc(sigma_max+1, sizeof(int)) followed by the explicit -2 fill */
    let mut sigtable: Vec<i32> = vec![0; (sigma_max(net.sigma.as_deref()) + 1) as usize];
    let mut i: i32 = 0;
    while i < sigma_max(net.sigma.as_deref()) + 1 {
        sigtable[i as usize] = -2;
        i += 1;
    }
    let fsm = &net.states;
    let mut seentrans = 0;
    let mut epstrans = 0;
    let mut laststate: i32 = -1;
    let mut sequential: i32 = 1;
    let mut i: usize = 0;
    while fsm[i].state_no != -1 {
        let insym = fsm[i].r#in as i32;
        if insym < 0 {
            i += 1;
            continue;
        }
        if fsm[i].state_no != laststate {
            laststate = fsm[i].state_no;
            epstrans = 0;
            seentrans = 0;
        }
        if sigtable[insym as usize] == laststate || epstrans == 1 {
            sequential = 0;
            break;
        }
        if insym == EPSILON {
            if epstrans == 1 || seentrans == 1 {
                sequential = 0;
                break;
            }
            epstrans = 1;
        }
        sigtable[insym as usize] = laststate;
        seentrans = 1;
        i += 1;
    }
    /* free(sigtable) */
    drop(sigtable);
    if sequential == 0 {
        print!("fails at state {}\n", fsm[i].state_no);
    }
    sequential
}

// [spec:foma:def:structures.fsm-isfunctional-fn]
// [spec:foma:sem:structures.fsm-isfunctional-fn]
// [spec:foma:def:fomalib.fsm-isfunctional-fn]
// [spec:foma:sem:fomalib.fsm-isfunctional-fn]
pub fn fsm_isfunctional(net: &mut Fsm) -> i32 {
    let mut tmp = fsm_minimize(fsm_compose(fsm_invert(fsm_copy(net)), fsm_copy(net)));
    let result = fsm_isidentity(&mut tmp);
    fsm_destroy(tmp);
    result
}

// [spec:foma:def:structures.fsm-isunambiguous-fn]
// [spec:foma:sem:structures.fsm-isunambiguous-fn]
// [spec:foma:def:fomalib.fsm-isunambiguous-fn]
// [spec:foma:sem:fomalib.fsm-isunambiguous-fn]
pub fn fsm_isunambiguous(net: &mut Fsm) -> i32 {
    let mut loweruniqnet = fsm_lowerdet(fsm_copy(net));
    let mut testnet = fsm_minimize(fsm_compose(
        fsm_invert(fsm_copy(&mut loweruniqnet)),
        fsm_copy(&mut loweruniqnet),
    ));
    let ret = fsm_isidentity(&mut testnet);
    fsm_destroy(loweruniqnet);
    fsm_destroy(testnet);
    ret
}

// [spec:foma:def:structures.fsm-extract-ambiguous-domain-fn]
// [spec:foma:sem:structures.fsm-extract-ambiguous-domain-fn]
// [spec:foma:def:fomalib.fsm-extract-ambiguous-domain-fn]
// [spec:foma:sem:fomalib.fsm-extract-ambiguous-domain-fn]
pub fn fsm_extract_ambiguous_domain(net: Box<Fsm>) -> Box<Fsm> {
    // define AmbiguousDom(T) [_loweruniq(T) .o. _notid(_loweruniq(T).i .o. _loweruniq(T))].u;
    let mut loweruniqnet = fsm_lowerdet(net);
    let mut result = fsm_topsort(fsm_minimize(fsm_upper(fsm_compose(
        fsm_copy(&mut loweruniqnet),
        fsm_extract_nonidentity(fsm_compose(
            fsm_invert(fsm_copy(&mut loweruniqnet)),
            fsm_copy(&mut loweruniqnet),
        )),
    ))));
    fsm_destroy(loweruniqnet);
    sigma_cleanup(&mut result, 1);
    fsm_compact(&mut result);
    sigma_sort(&mut result);
    result
}

// [spec:foma:def:structures.fsm-extract-ambiguous-fn]
// [spec:foma:sem:structures.fsm-extract-ambiguous-fn]
// [spec:foma:def:fomalib.fsm-extract-ambiguous-fn]
// [spec:foma:sem:fomalib.fsm-extract-ambiguous-fn]
pub fn fsm_extract_ambiguous(net: Box<Fsm>) -> Box<Fsm> {
    /* the ambiguous domain is computed from a copy; net itself is consumed
    as the second compose operand */
    let mut net = net;
    fsm_topsort(fsm_minimize(fsm_compose(
        fsm_extract_ambiguous_domain(fsm_copy(&mut net)),
        net,
    )))
}

// [spec:foma:def:structures.fsm-extract-unambiguous-fn]
// [spec:foma:sem:structures.fsm-extract-unambiguous-fn]
// [spec:foma:def:fomalib.fsm-extract-unambiguous-fn]
// [spec:foma:sem:fomalib.fsm-extract-unambiguous-fn]
pub fn fsm_extract_unambiguous(net: Box<Fsm>) -> Box<Fsm> {
    let mut net = net;
    fsm_topsort(fsm_minimize(fsm_compose(
        fsm_complement(fsm_extract_ambiguous_domain(fsm_copy(&mut net))),
        net,
    )))
}

// [spec:foma:def:structures.fsm-isidentity-fn]
// [spec:foma:sem:structures.fsm-isidentity-fn]
// [spec:foma:def:fomalib.fsm-isidentity-fn]
// [spec:foma:sem:fomalib.fsm-isidentity-fn]
pub fn fsm_isidentity(net: &mut Fsm) -> i32 {
    /* We check whether a given transducer only produces identity relations     */
    /* By doing a DFS on the graph, and storing, for each state a "discrepancy" */
    /* string, showing the current "debt" on the upper or lower side.           */
    /* We immediately fail if: */
    /* a) we encounter an already seen state with a different current           */
    /*    discrepancy than what is stored in the state.                         */
    /* b) when traversing an arc, we encounter a mismatch between the arc and   */
    /*    the current discrepancy.                                              */
    /* c) we encounter a final state and have a non-null current discrepancy.   */
    /* d) we encounter @ with a non-null discrepancy anywhere.                  */
    /* e) we encounter ? anywhere.                                              */

    /* C: struct discrepancy { short int *string; short int length;
    _Bool visited; } — string is an owned Vec here (an empty Vec stands in
    for NULL). Because each record owns its string, the C free-before-realloc
    dance and its resulting use-after-free are simply absent. */
    #[derive(Clone)]
    struct Discrepancy {
        string: Vec<i16>,
        length: i16,
        visited: bool,
    }

    let mut tmp = fsm_minimize(fsm_copy(net));
    fsm_count(&mut tmp);

    let num_states = tmp.statecount;
    /* calloc — zeroed records */
    let mut discrepancy: Vec<Discrepancy> = vec![
        Discrepancy {
            string: Vec::new(),
            length: 0,
            visited: false,
        };
        num_states as usize
    ];
    let state_array = map_firstlines(&tmp);
    ptr_stack_clear();
    ptr_stack_push(state_array[0].transitions);

    /* C function-scope locals (factor/newlength keep their values across
    iterations; startfrom is always assigned before use) */
    let mut factor: i32 = 0;
    let mut newlength: i32 = 1;
    let mut startfrom: i32 = 0;
    let mut failed = false;

    'stack_loop: while ptr_stack_isempty() == 0 {
        let mut curr_ptr = ptr_stack_pop();

        'nopop: loop {
            let v = tmp.states[curr_ptr].state_no; /* source state number */
            let vp = tmp.states[curr_ptr].target; /* target state number */
            /* C computes currd = discrepancy+v here (pointer arithmetic
            only; not dereferenced before the v/vp checks) */
            if v != -1 {
                discrepancy[v as usize].visited = true;
            }
            if v == -1 || vp == -1 {
                break 'nopop; /* continue the pop loop */
            }
            let r#in = tmp.states[curr_ptr].r#in;
            let out = tmp.states[curr_ptr].out;

            /* Check arc and conditions e) d) b) */
            /* e) */
            if r#in as i32 == UNKNOWN || out as i32 == UNKNOWN {
                failed = true;
                break 'stack_loop;
            }
            /* d) */
            if r#in as i32 == IDENTITY && discrepancy[v as usize].length != 0 {
                failed = true;
                break 'stack_loop;
            }
            /* b) */
            if discrepancy[v as usize].length != 0 {
                if discrepancy[v as usize].length > 0
                    && out as i32 != EPSILON
                    && out != discrepancy[v as usize].string[0]
                {
                    failed = true;
                    break 'stack_loop;
                }
                if discrepancy[v as usize].length < 0
                    && r#in as i32 != EPSILON
                    && r#in != discrepancy[v as usize].string[0]
                {
                    failed = true;
                    break 'stack_loop;
                }
            }
            if discrepancy[v as usize].length == 0
                && r#in != out
                && r#in as i32 != EPSILON
                && out as i32 != EPSILON
            {
                failed = true;
                break 'stack_loop;
            }

            /* Calculate new discrepancy */
            let currd_length = discrepancy[v as usize].length as i32;
            if currd_length != 0 {
                if r#in as i32 != EPSILON && out as i32 != EPSILON {
                    factor = 0;
                } else if r#in as i32 == EPSILON {
                    factor = -1;
                } else if out as i32 == EPSILON {
                    factor = 1;
                }

                newlength = currd_length + factor;
                startfrom = if newlength.abs() <= currd_length.abs() {
                    1
                } else {
                    0
                };
            } else {
                if r#in as i32 != EPSILON && out as i32 != EPSILON {
                    newlength = 0;
                } else {
                    newlength = if out as i32 == EPSILON { 1 } else { -1 };
                }
                startfrom = 0;
            }

            /* C freed the previous newstring buffer here before this calloc;
            when the previous iteration had descended into state v, that buffer
            WAS currd->string, so C's copy loop below read freed memory. The
            discrepancy records own their strings here (no shared buffer), so
            there is nothing to free and nothing to alias.
            calloc(abs(newlength), sizeof(int)) — int-width slots used as shorts */
            let mut newstring_v: Vec<i16> = vec![0; newlength.abs() as usize];

            let mut i: i32 = startfrom;
            let mut j: i32 = 0;
            while i < currd_length.abs() {
                newstring_v[j as usize] = discrepancy[v as usize].string[i as usize];
                i += 1;
                j += 1;
            }
            if newlength != 0 {
                if currd_length > 0 && newlength >= currd_length {
                    newstring_v[j as usize] = r#in;
                }
                if currd_length < 0 && newlength <= currd_length {
                    newstring_v[j as usize] = out;
                }
                if currd_length == 0 && newlength < currd_length {
                    newstring_v[j as usize] = out;
                }
                if currd_length == 0 && newlength > currd_length {
                    newstring_v[j as usize] = r#in;
                }
            }

            /* Check target conditions a) c) */
            /* a) */
            if tmp.states[state_array[vp as usize].transitions].final_state != 0 && newlength != 0 {
                failed = true;
                break 'stack_loop;
            }
            if tmp.states[curr_ptr].state_no == tmp.states[curr_ptr + 1].state_no {
                ptr_stack_push(curr_ptr + 1);
            }
            if discrepancy[vp as usize].visited {
                /* C: //free(newstring); (commented out upstream) */
                if discrepancy[vp as usize].length as i32 != newlength {
                    failed = true;
                    break 'stack_loop;
                }
                let mut i: i32 = 0;
                while i < newlength.abs() {
                    if discrepancy[vp as usize].string[i as usize] != newstring_v[i as usize] {
                        failed = true;
                        break 'stack_loop;
                    }
                    i += 1;
                }
                break 'nopop;
            } else {
                /* Add discrepancy to target state */
                discrepancy[vp as usize].length = newlength as i16;
                /* C aliased targetd->string = newstring; the owned buffer is
                moved into the record here (no clone, no shared pointer). */
                discrepancy[vp as usize].string = newstring_v;
                curr_ptr = state_array[vp as usize].transitions;
                continue 'nopop; /* goto nopop */
            }
        }
    }
    /* success/fail epilogues: free(state_array); free(discrepancy);
    fsm_destroy(tmp); (C also freed the last newstring) — all drops here */
    if failed {
        ptr_stack_clear();
        fsm_destroy(tmp);
        return 0;
    }
    fsm_destroy(tmp);
    1
}

// [spec:foma:def:structures.fsm-markallfinal-fn]
// [spec:foma:sem:structures.fsm-markallfinal-fn]
// [spec:foma:def:fomalib.fsm-markallfinal-fn]
// [spec:foma:sem:fomalib.fsm-markallfinal-fn]
pub fn fsm_markallfinal(net: Box<Fsm>) -> Box<Fsm> {
    let mut net = net;
    let mut i: usize = 0;
    while net.states[i].state_no != -1 {
        net.states[i].final_state = YES as i8;
        i += 1;
    }
    net
}

// [spec:foma:def:structures.fsm-lowerdet-fn]
// [spec:foma:sem:structures.fsm-lowerdet-fn]
// [spec:foma:def:fomalib.fsm-lowerdet-fn]
// [spec:foma:sem:fomalib.fsm-lowerdet-fn]
pub fn fsm_lowerdet(net: Box<Fsm>) -> Box<Fsm> {
    let mut newsym: u32; /* Running number for new syms */
    let mut net = fsm_minimize(net);
    fsm_count(&mut net);
    newsym = 8723643;
    let mut maxarc: i32 = 0;
    let maxsigma = sigma_max(net.sigma.as_deref());

    let mut i: usize = 0;
    let mut j: i32 = 0;
    while net.states[i].state_no != -1 {
        if net.states[i].target != -1 {
            j += 1;
        }
        if net.states[i + 1].state_no != net.states[i].state_no {
            maxarc = if maxarc > j { maxarc } else { j };
            j = 0;
        }
        i += 1;
    }
    if maxarc > (maxsigma - 2) {
        let mut i = maxarc;
        while i > (maxsigma - 2) {
            /* sprintf(repstr, "%012X", newsym++) */
            let repstr = format!("{:012X}", newsym);
            newsym += 1;
            sigma_add(&repstr, net.sigma.as_deref_mut().unwrap());
            i -= 1;
        }
        sigma_sort(&mut net);
    }
    let mut i: usize = 0;
    let mut j: i32 = 3;
    while net.states[i].state_no != -1 {
        if net.states[i].target != -1 {
            /* int→short truncation as in C */
            net.states[i].out = j as i16;
            j += 1;
            net.states[i].r#in = if net.states[i].r#in as i32 == IDENTITY {
                UNKNOWN as i16
            } else {
                net.states[i].r#in
            };
        }
        if net.states[i + 1].state_no != net.states[i].state_no {
            j = 3;
        }
        i += 1;
    }
    net
}

// [spec:foma:def:structures.fsm-lowerdeteps-fn]
// [spec:foma:sem:structures.fsm-lowerdeteps-fn]
// [spec:foma:def:fomalib.fsm-lowerdeteps-fn]
// [spec:foma:sem:fomalib.fsm-lowerdeteps-fn]
pub fn fsm_lowerdeteps(net: Box<Fsm>) -> Box<Fsm> {
    let mut newsym: u32; /* Running number for new syms */
    let mut net = fsm_minimize(net);
    fsm_count(&mut net);
    newsym = 8723643;
    let mut maxarc: i32 = 0;
    let maxsigma = sigma_max(net.sigma.as_deref());

    let mut i: usize = 0;
    let mut j: i32 = 0;
    while net.states[i].state_no != -1 {
        if net.states[i].target != -1 {
            j += 1;
        }
        if net.states[i + 1].state_no != net.states[i].state_no {
            maxarc = if maxarc > j { maxarc } else { j };
            j = 0;
        }
        i += 1;
    }
    if maxarc > (maxsigma - 2) {
        let mut i = maxarc;
        while i > (maxsigma - 2) {
            /* sprintf(repstr, "%012X", newsym++) */
            let repstr = format!("{:012X}", newsym);
            newsym += 1;
            sigma_add(&repstr, net.sigma.as_deref_mut().unwrap());
            i -= 1;
        }
        sigma_sort(&mut net);
    }
    let mut i: usize = 0;
    let mut j: i32 = 3;
    while net.states[i].state_no != -1 {
        if net.states[i].target != -1 && net.states[i].out as i32 != EPSILON {
            /* int→short truncation as in C */
            net.states[i].out = j as i16;
            j += 1;
            net.states[i].r#in = if net.states[i].r#in as i32 == IDENTITY {
                UNKNOWN as i16
            } else {
                net.states[i].r#in
            };
        }
        if net.states[i + 1].state_no != net.states[i].state_no {
            j = 3;
        }
        i += 1;
    }
    net
}

// [spec:foma:def:structures.fsm-extract-nonidentity-fn]
// [spec:foma:sem:structures.fsm-extract-nonidentity-fn]
// [spec:foma:def:fomalib.fsm-extract-nonidentity-fn]
// [spec:foma:sem:fomalib.fsm-extract-nonidentity-fn]
pub fn fsm_extract_nonidentity(net: Box<Fsm>) -> Box<Fsm> {
    /* Same algorithm as for test identity, except we mark the arcs that cause nonidentity */
    /* Experimental. */

    /* C: struct discrepancy { short int *string; short int length;
    _Bool visited; } — string is an owned Vec here (an empty Vec ↔ NULL);
    unlike fsm_isidentity there is no free-before-realloc in C, only leaks */
    #[derive(Clone)]
    struct Discrepancy {
        string: Vec<i16>,
        length: i16,
        visited: bool,
    }

    /* C: fsm_minimize(net); — the return value is DISCARDED and net keeps
    being used (relies on in-place minimization); with the consuming
    convention the rebinding below is the safe equivalent */
    let mut net = fsm_minimize(net);
    fsm_count(&mut net);
    let killnum = sigma_add("@KILL@", net.sigma.as_deref_mut().unwrap());

    let num_states = net.statecount;
    /* calloc — zeroed records */
    let mut discrepancy: Vec<Discrepancy> = vec![
        Discrepancy {
            string: Vec::new(),
            length: 0,
            visited: false,
        };
        num_states as usize
    ];
    let state_array = map_firstlines(&net);
    /* no ptr_stack_clear() beforehand, unlike fsm_isidentity */
    ptr_stack_push(state_array[0].transitions);

    /* C function-scope locals (factor/newlength keep their values across
    iterations; startfrom is always assigned before use) */
    let mut factor: i32 = 0;
    let mut newlength: i32 = 1;
    let mut startfrom: i32 = 0;

    while ptr_stack_isempty() == 0 {
        let mut curr_ptr = ptr_stack_pop();

        'nopop: loop {
            let failed = 'body: {
                let v = net.states[curr_ptr].state_no; /* source state number */
                let vp = net.states[curr_ptr].target; /* target state number */
                if v != -1 {
                    discrepancy[v as usize].visited = true;
                }
                if v == -1 || vp == -1 {
                    break 'nopop; /* continue the pop loop */
                }
                let r#in = net.states[curr_ptr].r#in;
                let out = net.states[curr_ptr].out;

                /* Check arc and conditions e) d) b) */
                /* e) */
                if r#in as i32 == UNKNOWN || out as i32 == UNKNOWN {
                    break 'body true;
                }
                /* d) */
                if r#in as i32 == IDENTITY && discrepancy[v as usize].length != 0 {
                    break 'body true;
                }
                /* b) */
                if discrepancy[v as usize].length != 0 {
                    if discrepancy[v as usize].length > 0
                        && out as i32 != EPSILON
                        && out != discrepancy[v as usize].string[0]
                    {
                        break 'body true;
                    }
                    if discrepancy[v as usize].length < 0
                        && r#in as i32 != EPSILON
                        && r#in != discrepancy[v as usize].string[0]
                    {
                        break 'body true;
                    }
                }
                if discrepancy[v as usize].length == 0
                    && r#in != out
                    && r#in as i32 != EPSILON
                    && out as i32 != EPSILON
                {
                    break 'body true;
                }

                /* Calculate new discrepancy */
                let currd_length = discrepancy[v as usize].length as i32;
                if currd_length != 0 {
                    if r#in as i32 != EPSILON && out as i32 != EPSILON {
                        factor = 0;
                    } else if r#in as i32 == EPSILON {
                        factor = -1;
                    } else if out as i32 == EPSILON {
                        factor = 1;
                    }

                    newlength = currd_length + factor;
                    startfrom = if newlength.abs() <= currd_length.abs() {
                        1
                    } else {
                        0
                    };
                } else {
                    if r#in as i32 != EPSILON && out as i32 != EPSILON {
                        newlength = 0;
                    } else {
                        newlength = if out as i32 == EPSILON { 1 } else { -1 };
                    }
                    startfrom = 0;
                }

                /* calloc(abs(newlength), sizeof(int)) — never freed in C
                (leak, no aliasing hazard) */
                let mut newstring: Vec<i16> = vec![0; newlength.abs() as usize];

                let mut i: i32 = startfrom;
                let mut j: i32 = 0;
                while i < currd_length.abs() {
                    newstring[j as usize] = discrepancy[v as usize].string[i as usize];
                    i += 1;
                    j += 1;
                }
                if newlength != 0 {
                    if currd_length > 0 && newlength >= currd_length {
                        newstring[j as usize] = r#in;
                    }
                    if currd_length < 0 && newlength <= currd_length {
                        newstring[j as usize] = out;
                    }
                    if currd_length == 0 && newlength < currd_length {
                        newstring[j as usize] = out;
                    }
                    if currd_length == 0 && newlength > currd_length {
                        newstring[j as usize] = r#in;
                    }
                }

                /* Check target conditions a) c) */
                /* a) */
                if net.states[state_array[vp as usize].transitions].final_state != 0
                    && newlength != 0
                {
                    break 'body true;
                }
                if net.states[curr_ptr].state_no == net.states[curr_ptr + 1].state_no {
                    ptr_stack_push(curr_ptr + 1);
                }

                if discrepancy[vp as usize].visited {
                    /* C: //free(newstring); (commented out upstream) */
                    if discrepancy[vp as usize].length as i32 != newlength {
                        break 'body true;
                    }
                    let mut i: i32 = 0;
                    while i < newlength.abs() {
                        if discrepancy[vp as usize].string[i as usize] != newstring[i as usize] {
                            break 'body true;
                        }
                        i += 1;
                    }
                    break 'body false; /* falls through to C's `continue;` */
                } else {
                    /* Add discrepancy to target state */
                    discrepancy[vp as usize].length = newlength as i16;
                    /* C: targetd->string = newstring (aliased); owned copy
                    here (the C buffer is leaked, never freed) */
                    discrepancy[vp as usize].string = newstring;
                    curr_ptr = state_array[vp as usize].transitions;
                    continue 'nopop; /* goto nopop */
                }
            };
            if failed {
                /* fail: relabel the arc's output to @KILL@ and re-push the
                sibling line (when failure occurs at the revisit-comparison
                stage the sibling was already pushed once — the second push
                is a redundant re-traversal, as in C) */
                net.states[curr_ptr].out = killnum as i16;
                if net.states[curr_ptr].state_no == net.states[curr_ptr + 1].state_no {
                    ptr_stack_push(curr_ptr + 1);
                }
            }
            break 'nopop;
        }
    }
    ptr_stack_clear();
    sigma_sort(&mut net);
    let mut net2 = fsm_upper(fsm_compose(net, fsm_contains(fsm_symbol("@KILL@"))));
    /* C: sigma_remove("@KILL@", net2->sigma) — the returned new head is
    discarded (fine unless @KILL@ were the head node); the owned list here
    must be reassigned */
    net2.sigma = sigma_remove("@KILL@", net2.sigma.take());
    sigma_sort(&mut net2);
    /* free(state_array); free(discrepancy) — drops */
    drop(state_array);
    drop(discrepancy);
    net2
}

// [spec:foma:def:structures.fsm-copy-fn]
// [spec:foma:sem:structures.fsm-copy-fn+1]
// [spec:foma:def:fomalib.fsm-copy-fn]
// [spec:foma:sem:fomalib.fsm-copy-fn+1]
pub fn fsm_copy(net: &mut Fsm) -> Box<Fsm> {
    /* Borrows (does not consume) but mutates the SOURCE: fsm_count refreshes
    its counts. A &mut borrow is never NULL — NULL-able callers keep the
    check at the call site.

    Wave 4 fix: the C memcpy'd the whole struct BEFORE calling fsm_count(net),
    so the copy captured stale statecount/linecount/arccount/finalcount. Here
    fsm_count runs first, so the copy gets the same fresh counts as the source. */
    fsm_count(net);
    let mut net_copy = Box::new(Fsm {
        name: net.name.clone(),
        arity: net.arity,
        arccount: net.arccount,
        statecount: net.statecount,
        linecount: net.linecount,
        finalcount: net.finalcount,
        pathcount: net.pathcount,
        is_deterministic: net.is_deterministic,
        is_pruned: net.is_pruned,
        is_minimized: net.is_minimized,
        is_epsilon_free: net.is_epsilon_free,
        is_loop_free: net.is_loop_free,
        is_completed: net.is_completed,
        arcs_sorted_in: net.arcs_sorted_in,
        arcs_sorted_out: net.arcs_sorted_out,
        states: Vec::new(),
        sigma: None,
        // The C memcpy left medlookup SHARED between source and copy (a
        // double-free hazard); a deep clone here keeps them independent, as
        // recorded in types.rs.
        medlookup: net.medlookup.clone(),
    });

    net_copy.sigma = sigma_copy(net.sigma.as_deref());
    net_copy.states = fsm_state_copy(&net.states, net.linecount);
    net_copy
}

// [spec:foma:def:structures.fsm-state-copy-fn]
// [spec:foma:sem:structures.fsm-state-copy-fn]
// [spec:foma:def:fomalibconf.fsm-state-copy-fn]
// [spec:foma:sem:fomalibconf.fsm-state-copy-fn]
pub fn fsm_state_copy(fsm_state: &[FsmState], linecount: i32) -> Vec<FsmState> {
    /* malloc + memcpy of exactly linecount lines (the caller's linecount
    must include the -1 sentinel line for a complete table; no validation) */
    let new_fsm_state: Vec<FsmState> = fsm_state[..linecount as usize].to_vec();
    new_fsm_state
}

/* TODO: separate linecount and arccount */
// [spec:foma:def:structures.find-arccount-fn]
// [spec:foma:sem:structures.find-arccount-fn]
// [spec:foma:def:fomalibconf.find-arccount-fn]
// [spec:foma:sem:fomalibconf.find-arccount-fn]
pub fn find_arccount(fsm: &[FsmState]) -> i32 {
    let mut i: i32 = 0;
    while fsm[i as usize].state_no != -1 {
        i += 1;
    }
    i
}

// [spec:foma:def:structures.clear-quantifiers-fn]
// [spec:foma:sem:structures.clear-quantifiers-fn]
// [spec:foma:def:foma.clear-quantifiers-fn]
// [spec:foma:sem:foma.clear-quantifiers-fn]
pub fn clear_quantifiers() {
    /* C sets the head to NULL without freeing the nodes (deliberate leak);
    the owned list here is dropped — observably equivalent */
    QUANTIFIERS.with(|qs| *qs.borrow_mut() = None);
}

// [spec:foma:def:structures.count-quantifiers-fn]
// [spec:foma:sem:structures.count-quantifiers-fn]
// [spec:foma:def:foma.count-quantifiers-fn]
// [spec:foma:sem:foma.count-quantifiers-fn]
pub fn count_quantifiers() -> i32 {
    QUANTIFIERS.with(|qs| {
        let qs = qs.borrow();
        let mut i: i32 = 0;
        let mut q = qs.as_deref();
        while let Some(node) = q {
            i += 1;
            q = node.next.as_deref();
        }
        i
    })
}

// [spec:foma:def:structures.add-quantifier-fn]
// [spec:foma:sem:structures.add-quantifier-fn]
// [spec:foma:def:foma.add-quantifier-fn]
// [spec:foma:sem:foma.add-quantifier-fn]
pub fn add_quantifier(string: &str) {
    /* no duplicate check: adding the same name twice creates two nodes */
    QUANTIFIERS.with(|qs| {
        let mut qs = qs.borrow_mut();
        if qs.is_none() {
            *qs = Some(Box::new(DefinedQuantifiers {
                name: Some(string.to_string()),
                next: None,
            }));
        } else {
            /* walk to the tail node (next == NULL) */
            let mut q = qs.as_deref_mut().unwrap();
            while q.next.is_some() {
                q = q.next.as_deref_mut().unwrap();
            }
            q.next = Some(Box::new(DefinedQuantifiers {
                name: Some(string.to_string()),
                next: None,
            }));
        }
    });
}

// [spec:foma:def:structures.union-quantifiers-fn]
// [spec:foma:sem:structures.union-quantifiers-fn+1]
// [spec:foma:def:foma.union-quantifiers-fn]
// [spec:foma:sem:foma.union-quantifiers-fn+1]
pub fn union_quantifiers() -> Box<Fsm> {
    /*     We create a FSM that simply accepts the union of all */
    /*     quantifier symbols */

    let mut net = fsm_create("");
    fsm_update_flags(&mut net, YES, YES, YES, YES, NO, NO);

    let mut syms: i32 = 0;
    let mut symlo: i32 = 0;
    QUANTIFIERS.with(|qs| {
        let qs = qs.borrow();
        let mut q = qs.as_deref();
        while let Some(node) = q {
            let s = sigma_add(
                node.name.as_deref().unwrap(),
                net.sigma.as_deref_mut().unwrap(),
            );
            if symlo == 0 {
                symlo = s;
            }
            syms += 1;
            q = node.next.as_deref();
        }
    });
    /* C: malloc((syms+1) lines), uninitialized; written below */
    net.states = vec![
        FsmState {
            state_no: 0,
            r#in: 0,
            out: 0,
            target: 0,
            final_state: 0,
            start_state: 0,
        };
        (syms + 1) as usize
    ];
    let mut i: i32 = 0;
    while i < syms {
        add_fsm_arc(&mut net.states, i, 0, symlo + i, symlo + i, 0, 1, 1);
        i += 1;
    }
    add_fsm_arc(&mut net.states, i, -1, -1, -1, -1, -1, -1);
    net.arccount = syms;
    net.statecount = 1;
    net.finalcount = 1;
    /* Wave 4 fix: include the sentinel line, matching fsm_count's linecount
    convention (was: syms, excluding it). Every caller recounts via fsm_count
    before reading linecount, so no downstream value changed. pathcount is
    left as fsm_create initialized it. */
    net.linecount = syms + 1;
    net
}

// [spec:foma:def:structures.find-quantifier-fn]
// [spec:foma:sem:structures.find-quantifier-fn]
// [spec:foma:def:foma.find-quantifier-fn]
// [spec:foma:sem:foma.find-quantifier-fn]
pub fn find_quantifier(string: &str) -> Option<String> {
    QUANTIFIERS.with(|qs| {
        let qs = qs.borrow();
        let mut q = qs.as_deref();
        while let Some(node) = q {
            if string == node.name.as_deref().unwrap() {
                /* C returns the node's own name pointer (the caller must
                not free or mutate it); the thread_local list cannot be
                borrowed out of the closure, so an owned clone is returned
                (observably equivalent) */
                return node.name.clone();
            }
            q = node.next.as_deref();
        }
        None
    })
}

// [spec:foma:def:structures.purge-quantifier-fn]
// [spec:foma:sem:structures.purge-quantifier-fn+1]
// [spec:foma:def:foma.purge-quantifier-fn]
// [spec:foma:sem:foma.purge-quantifier-fn+1]
pub fn purge_quantifier(string: &str) {
    /* Wave 4 fix: the C walked with a trailing q_prev pointer that advanced
    onto the node it had just unlinked, so of two CONSECUTIVE matching nodes
    only the first left the live list (the second unlink wrote into the
    already-removed node). This removes EVERY matching node — the evident
    intent. (C leaked the removed nodes and their names; dropped here.) */
    QUANTIFIERS.with(|qs| {
        let mut qs = qs.borrow_mut();
        let mut q: &mut Option<Box<DefinedQuantifiers>> = &mut qs;
        loop {
            let matched = match q.as_deref() {
                None => break,
                Some(node) => string == node.name.as_deref().unwrap(),
            };
            if matched {
                let node = q.take().unwrap();
                *q = node.next;
            } else {
                q = &mut q.as_deref_mut().unwrap().next;
            }
        }
    });
}

// [spec:foma:def:structures.fsm-quantifier-fn]
// [spec:foma:sem:structures.fsm-quantifier-fn]
// [spec:foma:def:fomalib.fsm-quantifier-fn]
// [spec:foma:sem:fomalib.fsm-quantifier-fn]
pub fn fsm_quantifier(string: &str) -> Box<Fsm> {
    /* \x* x \x* x \x* */
    fsm_concat(
        fsm_kleene_star(fsm_term_negation(fsm_symbol(string))),
        fsm_concat(
            fsm_symbol(string),
            fsm_concat(
                fsm_kleene_star(fsm_term_negation(fsm_symbol(string))),
                fsm_concat(
                    fsm_symbol(string),
                    fsm_kleene_star(fsm_term_negation(fsm_symbol(string))),
                ),
            ),
        ),
    )
}

// [spec:foma:def:structures.fsm-logical-precedence-fn]
// [spec:foma:sem:structures.fsm-logical-precedence-fn]
// [spec:foma:def:fomalib.fsm-logical-precedence-fn]
// [spec:foma:sem:fomalib.fsm-logical-precedence-fn]
pub fn fsm_logical_precedence(string1: &str, string2: &str) -> Box<Fsm> {
    /* x < y = \y* x \y* [x | y Q* x] ?* */
    /*          1  2  3        4           5 */

    fsm_concat(
        fsm_kleene_star(fsm_term_negation(fsm_symbol(string2))),
        fsm_concat(
            fsm_symbol(string1),
            fsm_concat(
                fsm_kleene_star(fsm_term_negation(fsm_symbol(string2))),
                fsm_concat(
                    fsm_union(
                        fsm_symbol(string1),
                        fsm_concat(
                            fsm_symbol(string2),
                            fsm_concat(union_quantifiers(), fsm_symbol(string1)),
                        ),
                    ),
                    fsm_universal(),
                ),
            ),
        ),
    )

    /*    1,3   fsm_kleene_star(fsm_term_negation(fsm_symbol(string2))) */
    /*        2 = fsm_symbol(string1) */
    /*        5 = fsm_universal() */
    /* 4 =    fsm_union(fsm_symbol(string1),fsm_concat(fsm_symbol(string2),fsm_concat(union_quantifiers(),fsm_symbol(string1)))) */
}

/** Logical equivalence, i.e. where two variables span the same substring */
/** x = y = ?* [x y|y x]/Q ?* [x y|y x]/Q ?* */
// [spec:foma:def:structures.fsm-logical-eq-fn]
// [spec:foma:sem:structures.fsm-logical-eq-fn]
// [spec:foma:def:fomalib.fsm-logical-eq-fn]
// [spec:foma:sem:fomalib.fsm-logical-eq-fn]
pub fn fsm_logical_eq(string1: &str, string2: &str) -> Box<Fsm> {
    fsm_concat(
        fsm_universal(),
        fsm_concat(
            fsm_ignore(
                fsm_union(
                    fsm_concat(fsm_symbol(string1), fsm_symbol(string2)),
                    fsm_concat(fsm_symbol(string2), fsm_symbol(string1)),
                ),
                union_quantifiers(),
                OP_IGNORE_ALL,
            ),
            fsm_concat(
                fsm_universal(),
                fsm_concat(
                    fsm_ignore(
                        fsm_union(
                            fsm_concat(fsm_symbol(string1), fsm_symbol(string2)),
                            fsm_concat(fsm_symbol(string2), fsm_symbol(string1)),
                        ),
                        union_quantifiers(),
                        OP_IGNORE_ALL,
                    ),
                    fsm_universal(),
                ),
            ),
        ),
    )
}

/* Dead prototypes: declared in fomalib.h but never defined in any C source.
Calling them in C is a link error. DEVIATION from C (Rust has no
declaration-without-definition; these panic to preserve the never-callable
contract). */

// [spec:foma:def:fomalib.fsm-find-ambiguous-fn]
// [spec:foma:sem:fomalib.fsm-find-ambiguous-fn]
pub fn fsm_find_ambiguous(_net: Box<Fsm>) -> Box<Fsm> {
    panic!("fsm_find_ambiguous: dead prototype in C foma (declared, never defined; link error)");
}

// [spec:foma:def:fomalib.fsm-mark-ambiguous-fn]
// [spec:foma:sem:fomalib.fsm-mark-ambiguous-fn]
pub fn fsm_mark_ambiguous(_net: Box<Fsm>) -> Box<Fsm> {
    panic!("fsm_mark_ambiguous: dead prototype in C foma (declared, never defined; link error)");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constructions::fsm_count;

    /* Build a fresh, minimized net from a regex (the Wave-2 pipeline). */
    fn parse(rx: &str) -> Box<Fsm> {
        crate::regex::fsm_parse_regex(rx, None, None).expect("regex should compile")
    }

    fn st(state_no: i32, i: i16, o: i16, target: i32, fin: i8, start: i8) -> FsmState {
        FsmState {
            state_no,
            r#in: i,
            out: o,
            target,
            final_state: fin,
            start_state: start,
        }
    }

    fn raw_fsm(states: Vec<FsmState>, arity: i32) -> Box<Fsm> {
        let mut net = fsm_create("");
        net.states = states;
        net.arity = arity;
        net
    }

    // [spec:foma:sem:structures.fsm-get-library-version-string-fn/test]
    // [spec:foma:sem:fomalib.fsm-get-library-version-string-fn/test]
    #[test]
    fn library_version_string() {
        assert_eq!(fsm_get_library_version_string(), "0.10.0alpha");
    }

    // [spec:foma:sem:structures.fsm-set-option-fn/test]
    // [spec:foma:sem:fomalib.fsm-set-option-fn/test]
    // [spec:foma:sem:structures.fsm-get-option-fn/test]
    // [spec:foma:sem:fomalib.fsm-get-option-fn/test]
    #[test]
    fn set_and_get_option() {
        let skip = crate::types::FSM_OPTIONS::FSMO_SKIP_WORD_BOUNDARY_MARKER as u64;
        let unknown = crate::types::FSM_OPTIONS::FSMO_NUM_OPTIONS as u64;

        // set known option -> true, then get reflects the stored value
        assert!(fsm_set_option(skip, &true));
        assert_eq!(fsm_get_option(skip), Some(true));
        assert!(fsm_set_option(skip, &false));
        assert_eq!(fsm_get_option(skip), Some(false));

        // any other option: set does nothing and returns false; get is None
        assert!(!fsm_set_option(unknown, &true));
        assert_eq!(fsm_get_option(unknown), None);
        assert_eq!(fsm_get_option(9999), None);
    }

    // [spec:foma:sem:structures.linesortcompin-fn/test]
    // [spec:foma:sem:structures.linesortcompout-fn/test]
    #[test]
    fn line_comparators() {
        let a = st(0, 5, 7, 1, 0, 1);
        let b = st(0, 2, 1, 1, 0, 1);
        // int subtraction of the short fields
        assert_eq!(linesortcompin(&a, &b), 3);
        assert_eq!(linesortcompin(&b, &a), -3);
        assert_eq!(linesortcompin(&a, &a), 0);
        assert_eq!(linesortcompout(&a, &b), 6);
        assert_eq!(linesortcompout(&b, &a), -6);
    }

    // [spec:foma:sem:structures.fsm-sort-arcs-fn/test]
    // [spec:foma:sem:fomalib.fsm-sort-arcs-fn/test]
    #[test]
    fn sort_arcs_by_in_direction_1() {
        let mut net = raw_fsm(
            vec![
                st(0, 5, 0, 1, 0, 1),
                st(0, 2, 0, 1, 0, 1),
                st(0, 8, 0, 1, 0, 1),
                st(1, -1, -1, -1, 1, 0),
                st(-1, -1, -1, -1, -1, -1),
            ],
            2,
        );
        fsm_sort_arcs(&mut net, 1);
        assert_eq!(net.states[0].r#in, 2);
        assert_eq!(net.states[1].r#in, 5);
        assert_eq!(net.states[2].r#in, 8);
        // arity != 1, direction 1: in sorted, out flag cleared
        assert_eq!(net.arcs_sorted_in, 1);
        assert_eq!(net.arcs_sorted_out, 0);
    }

    // [spec:foma:sem:structures.fsm-sort-arcs-fn/test]
    // [spec:foma:sem:fomalib.fsm-sort-arcs-fn/test]
    #[test]
    fn sort_arcs_by_out_direction_2() {
        let mut net = raw_fsm(
            vec![
                st(0, 0, 7, 1, 0, 1),
                st(0, 0, 1, 1, 0, 1),
                st(0, 0, 4, 1, 0, 1),
                st(1, -1, -1, -1, 1, 0),
                st(-1, -1, -1, -1, -1, -1),
            ],
            2,
        );
        fsm_sort_arcs(&mut net, 2);
        assert_eq!(net.states[0].out, 1);
        assert_eq!(net.states[1].out, 4);
        assert_eq!(net.states[2].out, 7);
        assert_eq!(net.arcs_sorted_out, 1);
        assert_eq!(net.arcs_sorted_in, 0);
    }

    // [spec:foma:sem:structures.fsm-sort-arcs-fn/test]
    // [spec:foma:sem:fomalib.fsm-sort-arcs-fn/test]
    #[test]
    fn sort_arcs_arity_1_sets_both_flags() {
        let mut net = raw_fsm(
            vec![
                st(0, 5, 0, 1, 0, 1),
                st(0, 2, 0, 1, 0, 1),
                st(1, -1, -1, -1, 1, 0),
                st(-1, -1, -1, -1, -1, -1),
            ],
            1,
        );
        fsm_sort_arcs(&mut net, 1);
        assert_eq!(net.arcs_sorted_in, 1);
        assert_eq!(net.arcs_sorted_out, 1);
    }

    // [spec:foma:sem:structures.map-firstlines-fn/test]
    // [spec:foma:sem:fomalibconf.map-firstlines-fn/test]
    #[test]
    fn map_firstlines_indexes_first_line_per_state() {
        let net = fsm_identity(); // state 0 at line 0, state 1 at line 1, statecount 2
        let sa = map_firstlines(&net);
        assert_eq!(sa.len(), (net.statecount + 1) as usize);
        assert_eq!(sa[0].transitions, 0);
        assert_eq!(sa[1].transitions, 1);
    }

    // [spec:foma:sem:structures.fsm-boolean-fn/test]
    // [spec:foma:sem:fomalib.fsm-boolean-fn/test]
    #[test]
    fn boolean_maps_to_empty_set_or_string() {
        // value 0 -> empty set (accepts nothing)
        let zero = fsm_boolean(0);
        assert_eq!(zero.finalcount, 0);
        assert_eq!(zero.pathcount, 0);
        // any nonzero -> empty string (accepts only "")
        for v in [1, 5, -3] {
            let net = fsm_boolean(v);
            assert_eq!(net.finalcount, 1);
            assert_eq!(net.pathcount, 1);
        }
    }

    // [spec:foma:sem:structures.fsm-empty-set-fn/test]
    // [spec:foma:sem:fomalib.fsm-empty-set-fn/test]
    #[test]
    fn empty_set_shape() {
        let net = fsm_empty_set();
        assert_eq!(net.states.len(), 2);
        // lone non-final arcless start state
        assert_eq!(net.states[0].state_no, 0);
        assert_eq!(net.states[0].target, -1);
        assert_eq!(net.states[0].final_state, 0);
        assert_eq!(net.states[0].start_state, 1);
        assert_eq!(net.states[0].r#in, -1);
        assert_eq!(net.states[0].out, -1);
        assert_eq!(net.states[1].state_no, -1); // sentinel
        assert_eq!(net.statecount, 1);
        assert_eq!(net.finalcount, 0);
        assert_eq!(net.arccount, 0);
        assert_eq!(net.linecount, 2);
        assert_eq!(net.pathcount, 0);
        // flags: det/pru/min/eps/loop YES, completed NO, sort flags cleared
        assert_eq!(net.is_deterministic, YES);
        assert_eq!(net.is_minimized, YES);
        assert_eq!(net.is_loop_free, YES);
        assert_eq!(net.is_completed, NO);
        assert_eq!(net.arcs_sorted_in, NO);
    }

    // [spec:foma:sem:structures.fsm-empty-string-fn/test]
    // [spec:foma:sem:fomalib.fsm-empty-string-fn/test]
    #[test]
    fn empty_string_shape() {
        let net = fsm_empty_string();
        assert_eq!(net.states.len(), 2);
        assert_eq!(net.states[0].state_no, 0);
        assert_eq!(net.states[0].target, -1);
        assert_eq!(net.states[0].final_state, 1); // final start state
        assert_eq!(net.states[0].start_state, 1);
        assert_eq!(net.states[1].state_no, -1); // sentinel
        assert_eq!(net.statecount, 1);
        assert_eq!(net.finalcount, 1);
        assert_eq!(net.arccount, 0);
        assert_eq!(net.linecount, 2);
        assert_eq!(net.pathcount, 1);
    }

    // [spec:foma:sem:structures.fsm-identity-fn/test]
    // [spec:foma:sem:fomalib.fsm-identity-fn/test]
    #[test]
    fn identity_shape() {
        let net = fsm_identity();
        assert_eq!(net.states.len(), 3);
        // line 0: IDENTITY:IDENTITY arc 0 -> 1
        assert_eq!(net.states[0].state_no, 0);
        assert_eq!(net.states[0].r#in as i32, IDENTITY);
        assert_eq!(net.states[0].out as i32, IDENTITY);
        assert_eq!(net.states[0].target, 1);
        assert_eq!(net.states[0].final_state, 0);
        assert_eq!(net.states[0].start_state, 1);
        // line 1: final non-start
        assert_eq!(net.states[1].state_no, 1);
        assert_eq!(net.states[1].target, -1);
        assert_eq!(net.states[1].final_state, 1);
        assert_eq!(net.states[2].state_no, -1); // sentinel
        // single sigma node = IDENTITY symbol
        let sig = net.sigma.as_deref().unwrap();
        assert_eq!(sig.number, IDENTITY);
        assert_eq!(sig.symbol.as_deref(), Some("@_IDENTITY_SYMBOL_@"));
        assert!(sig.next.is_none());
        assert_eq!(net.statecount, 2);
        assert_eq!(net.finalcount, 1);
        assert_eq!(net.arccount, 1);
        assert_eq!(net.linecount, 3);
        assert_eq!(net.pathcount, 1);
    }

    // [spec:foma:sem:structures.fsm-empty-fn/test]
    // [spec:foma:sem:fomalib.fsm-empty-fn/test]
    #[test]
    fn empty_state_table() {
        let t = fsm_empty();
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].state_no, 0);
        assert_eq!(t[0].r#in, -1);
        assert_eq!(t[0].out, -1);
        assert_eq!(t[0].target, -1);
        assert_eq!(t[0].final_state, 0);
        assert_eq!(t[0].start_state, 1);
        assert_eq!(t[1].state_no, -1); // sentinel
    }

    // [spec:foma:sem:structures.fsm-create-fn+1/test]
    // [spec:foma:sem:fomalib.fsm-create-fn+1/test]
    #[test]
    fn create_defaults_and_full_name() {
        let net = fsm_create("mynet");
        assert_eq!(net.name, "mynet");
        assert_eq!(net.arity, 1);
        assert_eq!(net.arccount, 0);
        assert_eq!(net.is_deterministic, NO);
        assert_eq!(net.is_minimized, NO);
        assert_eq!(net.arcs_sorted_in, NO);
        // sigma = single empty node
        let sig = net.sigma.as_deref().unwrap();
        assert_eq!(sig.number, -1);
        assert!(sig.symbol.is_none());
        assert!(sig.next.is_none());
        assert!(net.states.is_empty());

        // in-memory names are stored in full (C truncated to a fixed 40-byte field)
        let long: String = "a".repeat(45);
        let net2 = fsm_create(&long);
        assert_eq!(net2.name, long);
    }

    // [spec:foma:sem:structures.fsm-sigma-destroy-fn/test]
    // [spec:foma:sem:fomalib.fsm-sigma-destroy-fn/test]
    #[test]
    fn sigma_destroy_always_returns_1() {
        assert_eq!(fsm_sigma_destroy(None), 1);
        let list = Some(Box::new(Sigma {
            number: 3,
            symbol: Some("a".to_string()),
            next: Some(Box::new(Sigma {
                number: 4,
                symbol: Some("b".to_string()),
                next: None,
            })),
        }));
        assert_eq!(fsm_sigma_destroy(list), 1);
    }

    // [spec:foma:sem:structures.fsm-destroy-fn/test]
    // [spec:foma:sem:fomalib.fsm-destroy-fn/test]
    #[test]
    fn destroy_returns_1() {
        assert_eq!(fsm_destroy(fsm_empty_set()), 1);
        let mut net = fsm_empty_set();
        net.medlookup = Some(Box::new(crate::types::Medlookup {
            confusion_matrix: vec![1, 2, 3],
        }));
        assert_eq!(fsm_destroy(net), 1);
    }

    // [spec:foma:sem:structures.fsm-isuniversal-fn+1/test]
    // [spec:foma:sem:fomalib.fsm-isuniversal-fn+1/test]
    #[test]
    fn isuniversal_detects_universal_language() {
        // Wave 4 fix: the evident universality test — ?* IS universal -> 1
        assert_eq!(fsm_isuniversal(parse("?*")), 1);
        // non-universal languages -> 0
        assert_eq!(fsm_isuniversal(fsm_empty_set()), 0);
        assert_eq!(fsm_isuniversal(parse("a")), 0);
        // a single identity symbol (not its closure) is not universal
        assert_eq!(fsm_isuniversal(parse("?")), 0);
    }

    // [spec:foma:sem:structures.fsm-isempty-fn/test]
    // [spec:foma:sem:fomalib.fsm-isempty-fn/test]
    #[test]
    fn isempty_predicate() {
        assert_eq!(fsm_isempty(&mut fsm_empty_set()), 1);
        assert_eq!(fsm_isempty(&mut fsm_empty_string()), 0); // {""} is not empty
        assert_eq!(fsm_isempty(&mut parse("a")), 0);
    }

    // [spec:foma:sem:structures.fsm-issequential-fn/test]
    // [spec:foma:sem:fomalib.fsm-issequential-fn/test]
    #[test]
    fn issequential_predicate() {
        assert_eq!(fsm_issequential(&parse("a b c")), 1);
        assert_eq!(fsm_issequential(&parse("a")), 1);
        // two arcs with the same input symbol at one state -> not sequential
        assert_eq!(fsm_issequential(&parse("a:b | a:c")), 0);
    }

    // [spec:foma:sem:structures.fsm-isfunctional-fn/test]
    // [spec:foma:sem:fomalib.fsm-isfunctional-fn/test]
    #[test]
    fn isfunctional_predicate() {
        assert_eq!(fsm_isfunctional(&mut parse("a:b")), 1);
        assert_eq!(fsm_isfunctional(&mut parse("a:b | a:c")), 0);
    }

    // [spec:foma:sem:structures.fsm-isunambiguous-fn/test]
    // [spec:foma:sem:fomalib.fsm-isunambiguous-fn/test]
    #[test]
    fn isunambiguous_predicate() {
        assert_eq!(fsm_isunambiguous(&mut parse("a:b")), 1);
        assert_eq!(fsm_isunambiguous(&mut parse("a:b | a:c")), 0);
    }

    // [spec:foma:sem:structures.fsm-isidentity-fn/test]
    // [spec:foma:sem:fomalib.fsm-isidentity-fn/test]
    #[test]
    fn isidentity_predicate() {
        assert_eq!(fsm_isidentity(&mut fsm_identity()), 1); // ? maps x->x
        assert_eq!(fsm_isidentity(&mut parse("a")), 1); // a:a is identity
        assert_eq!(fsm_isidentity(&mut parse("a:b")), 0);
    }

    // [spec:foma:sem:structures.fsm-extract-ambiguous-domain-fn/test]
    // [spec:foma:sem:fomalib.fsm-extract-ambiguous-domain-fn/test]
    #[test]
    fn extract_ambiguous_domain_predicate() {
        // ambiguously-mapped inputs of a:b|a:c = {a} -> non-empty
        let mut d = fsm_extract_ambiguous_domain(parse("a:b | a:c"));
        assert_eq!(fsm_isempty(&mut d), 0);
        // functional net -> no ambiguous domain
        let mut d2 = fsm_extract_ambiguous_domain(parse("a:b"));
        assert_eq!(fsm_isempty(&mut d2), 1);
    }

    // [spec:foma:sem:structures.fsm-extract-ambiguous-fn/test]
    // [spec:foma:sem:fomalib.fsm-extract-ambiguous-fn/test]
    #[test]
    fn extract_ambiguous_predicate() {
        let mut a = fsm_extract_ambiguous(parse("a:b | a:c"));
        assert_eq!(fsm_isempty(&mut a), 0);
        let mut a2 = fsm_extract_ambiguous(parse("a:b"));
        assert_eq!(fsm_isempty(&mut a2), 1);
    }

    // [spec:foma:sem:structures.fsm-extract-unambiguous-fn/test]
    // [spec:foma:sem:fomalib.fsm-extract-unambiguous-fn/test]
    #[test]
    fn extract_unambiguous_predicate() {
        // only input "a" and it is ambiguous -> unambiguous part empty
        let mut u = fsm_extract_unambiguous(parse("a:b | a:c"));
        assert_eq!(fsm_isempty(&mut u), 1);
        // functional net -> whole thing is unambiguous
        let mut u2 = fsm_extract_unambiguous(parse("a:b"));
        assert_eq!(fsm_isempty(&mut u2), 0);
    }

    // [spec:foma:sem:structures.fsm-extract-nonidentity-fn/test]
    // [spec:foma:sem:fomalib.fsm-extract-nonidentity-fn/test]
    #[test]
    fn extract_nonidentity_predicate() {
        // a:b violates identity -> upper side {a} non-empty
        let mut n = fsm_extract_nonidentity(parse("a:b"));
        assert_eq!(fsm_isempty(&mut n), 0);
        // a:a is an identity relation -> no violating paths
        let mut n2 = fsm_extract_nonidentity(parse("a"));
        assert_eq!(fsm_isempty(&mut n2), 1);
    }

    // [spec:foma:sem:structures.fsm-markallfinal-fn/test]
    // [spec:foma:sem:fomalib.fsm-markallfinal-fn/test]
    #[test]
    fn markallfinal_sets_every_line_final() {
        let net = fsm_markallfinal(fsm_identity());
        assert_eq!(net.states[0].final_state, YES as i8); // was 0
        assert_eq!(net.states[1].final_state, YES as i8);
        assert_eq!(net.states[2].state_no, -1); // sentinel untouched
    }

    // [spec:foma:sem:structures.fsm-lowerdet-fn/test]
    // [spec:foma:sem:fomalib.fsm-lowerdet-fn/test]
    #[test]
    fn lowerdet_relabels_outputs_uniquely() {
        // a:b|a:c: state 0's two arcs get distinct outputs 3,4 (3+k, k=arc index)
        let net = fsm_lowerdet(parse("a:b | a:c"));
        let mut outs: Vec<i16> = net
            .states
            .iter()
            .filter(|s| s.state_no == 0 && s.target != -1)
            .map(|s| s.out)
            .collect();
        outs.sort();
        assert_eq!(outs, vec![3, 4]);

        // IDENTITY input is rewritten to UNKNOWN, output relabeled to 3
        let idnet = fsm_lowerdet(fsm_identity());
        let arc = idnet
            .states
            .iter()
            .find(|s| s.state_no == 0 && s.target != -1)
            .unwrap();
        assert_eq!(arc.out, 3);
        assert_eq!(arc.r#in as i32, UNKNOWN);
    }

    // [spec:foma:sem:structures.fsm-lowerdeteps-fn/test]
    // [spec:foma:sem:fomalib.fsm-lowerdeteps-fn/test]
    #[test]
    fn lowerdeteps_leaves_epsilon_output_untouched() {
        // a:0 has an epsilon-output arc: lowerdet -> out 3, lowerdeteps -> out 0
        let det = fsm_lowerdet(parse("a:0"));
        let a1 = det
            .states
            .iter()
            .find(|s| s.state_no == 0 && s.target != -1)
            .unwrap();
        assert_eq!(a1.out, 3);

        let eps = fsm_lowerdeteps(parse("a:0"));
        let a2 = eps
            .states
            .iter()
            .find(|s| s.state_no == 0 && s.target != -1)
            .unwrap();
        assert_eq!(a2.out as i32, EPSILON); // untouched
    }

    // [spec:foma:sem:structures.fsm-copy-fn+1/test]
    // [spec:foma:sem:fomalib.fsm-copy-fn+1/test]
    #[test]
    fn copy_is_deep_and_refreshes_source_counts() {
        let mut net = fsm_identity();
        // deliberately staled scalar counts on the source
        net.statecount = 999;
        net.finalcount = 888;
        let mut copy = fsm_copy(&mut net);
        // Wave 4 fix: fsm_count runs BEFORE the copy captures the scalars, so
        // the copy gets the FRESH counts (was: the stale 999/888)
        assert_eq!(copy.statecount, 2);
        assert_eq!(copy.finalcount, 1);
        // the SOURCE was refreshed by fsm_count(net)
        assert_eq!(net.statecount, 2);
        assert_eq!(net.finalcount, 1);
        // full table duplicated (linecount includes the sentinel)
        assert_eq!(copy.states.len(), net.linecount as usize);
        // deep copy: mutating the copy does not touch the source buffer
        let src0 = net.states[0].r#in;
        copy.states[0].r#in = 77;
        assert_eq!(net.states[0].r#in, src0);
    }

    // [spec:foma:sem:structures.fsm-state-copy-fn/test]
    // [spec:foma:sem:fomalibconf.fsm-state-copy-fn/test]
    #[test]
    fn state_copy_duplicates_n_lines() {
        let net = fsm_identity(); // 3 lines incl. sentinel
        let full = fsm_state_copy(&net.states, 3);
        assert_eq!(full.len(), 3);
        assert_eq!(full[0].state_no, net.states[0].state_no);
        assert_eq!(full[2].state_no, -1);
        // partial copy of just 2 lines
        let partial = fsm_state_copy(&net.states, 2);
        assert_eq!(partial.len(), 2);
    }

    // [spec:foma:sem:structures.find-arccount-fn/test]
    // [spec:foma:sem:fomalibconf.find-arccount-fn/test]
    #[test]
    fn find_arccount_counts_lines_before_sentinel() {
        // includes arcless marker lines, excludes the sentinel
        assert_eq!(find_arccount(&fsm_identity().states), 2);
        assert_eq!(find_arccount(&fsm_empty()), 1);
    }

    // [spec:foma:sem:structures.clear-quantifiers-fn/test]
    // [spec:foma:sem:foma.clear-quantifiers-fn/test]
    // [spec:foma:sem:structures.count-quantifiers-fn/test]
    // [spec:foma:sem:foma.count-quantifiers-fn/test]
    // [spec:foma:sem:structures.add-quantifier-fn/test]
    // [spec:foma:sem:foma.add-quantifier-fn/test]
    // [spec:foma:sem:structures.find-quantifier-fn/test]
    // [spec:foma:sem:foma.find-quantifier-fn/test]
    #[test]
    fn quantifier_list_add_count_find_clear() {
        clear_quantifiers();
        assert_eq!(count_quantifiers(), 0);
        add_quantifier("x");
        add_quantifier("y");
        assert_eq!(count_quantifiers(), 2);
        assert_eq!(find_quantifier("x").as_deref(), Some("x"));
        assert_eq!(find_quantifier("y").as_deref(), Some("y"));
        assert_eq!(find_quantifier("z"), None);
        // no duplicate check: adding "x" again makes a second node
        add_quantifier("x");
        assert_eq!(count_quantifiers(), 3);
        // clear drops the whole list
        clear_quantifiers();
        assert_eq!(count_quantifiers(), 0);
        assert_eq!(find_quantifier("x"), None);
    }

    // [spec:foma:sem:structures.purge-quantifier-fn+1/test]
    // [spec:foma:sem:foma.purge-quantifier-fn+1/test]
    #[test]
    fn purge_quantifier_removes_all_matches() {
        clear_quantifiers();
        // Wave 4 fix: two CONSECUTIVE matches then a non-match — BOTH matches
        // are now unlinked (the C left the second linked)
        add_quantifier("a");
        add_quantifier("a");
        add_quantifier("b");
        purge_quantifier("a");
        assert_eq!(count_quantifiers(), 1); // only "b" remains
        assert_eq!(find_quantifier("a"), None);
        assert_eq!(find_quantifier("b").as_deref(), Some("b"));

        // non-consecutive matches are also both removed
        clear_quantifiers();
        add_quantifier("a");
        add_quantifier("b");
        add_quantifier("a");
        purge_quantifier("a");
        assert_eq!(count_quantifiers(), 1);
        assert_eq!(find_quantifier("a"), None);
        assert_eq!(find_quantifier("b").as_deref(), Some("b"));
        clear_quantifiers();
    }

    // [spec:foma:sem:structures.union-quantifiers-fn+1/test]
    // [spec:foma:sem:foma.union-quantifiers-fn+1/test]
    #[test]
    fn union_quantifiers_shape_and_linecount() {
        clear_quantifiers();
        add_quantifier("x");
        add_quantifier("y");
        let net = union_quantifiers();
        // syms == 2: table has syms+1 = 3 lines; Wave 4 linecount INCLUDES sentinel
        assert_eq!(net.states.len(), 3);
        assert_eq!(net.linecount, 3);
        assert_eq!(net.arccount, 2);
        assert_eq!(net.statecount, 1);
        assert_eq!(net.finalcount, 1);
        // each arc is a self-loop, final+start, consecutive symbol numbers
        assert_eq!(net.states[0].state_no, 0);
        assert_eq!(net.states[0].target, 0);
        assert_eq!(net.states[0].final_state, 1);
        assert_eq!(net.states[0].start_state, 1);
        assert_eq!(net.states[0].r#in, net.states[0].out);
        assert_eq!(net.states[1].r#in, net.states[0].r#in + 1);
        assert_eq!(net.states[2].state_no, -1); // sentinel

        // empty list: table is just the sentinel (no state 0); linecount 1
        // (the sentinel), per the Wave 4 convention fix
        clear_quantifiers();
        let empty = union_quantifiers();
        assert_eq!(empty.states.len(), 1);
        assert_eq!(empty.states[0].state_no, -1);
        assert_eq!(empty.linecount, 1);
        assert_eq!(empty.arccount, 0);
        clear_quantifiers();
    }

    // [spec:foma:sem:structures.fsm-sigma-net-fn/test]
    // [spec:foma:sem:fomalib.fsm-sigma-net-fn/test]
    #[test]
    fn sigma_net_shape() {
        // one arc per alphabet symbol (a,b,c) from state 0 -> final state 1
        let net = fsm_sigma_net(parse("a | b | c"));
        assert_eq!(net.pathcount, 3);
        assert_eq!(net.statecount, 2);
        assert_eq!(net.is_minimized, YES);
        assert_eq!(net.is_loop_free, YES);

        // sigma_size == 0 (sigma == NULL) -> destroy and return empty set
        let mut bare = fsm_create("");
        bare.sigma = None;
        let res = fsm_sigma_net(bare);
        assert_eq!(res.statecount, 1);
        assert_eq!(res.finalcount, 0);
        assert_eq!(res.pathcount, 0);
    }

    // [spec:foma:sem:structures.fsm-sigma-pairs-net-fn/test]
    // [spec:foma:sem:fomalib.fsm-sigma-pairs-net-fn/test]
    #[test]
    fn sigma_pairs_net_shape() {
        // distinct (in,out) pairs of a:b|a:c = {(a,b),(a,c)} -> 2 arcs
        let net = fsm_sigma_pairs_net(parse("a:b | a:c"));
        assert_eq!(net.pathcount, 2);
        assert_eq!(net.statecount, 2);
        assert_eq!(net.is_minimized, YES);

        // no arcs in source -> pathcount 0 -> destroy and return empty set
        let res = fsm_sigma_pairs_net(fsm_empty_string());
        assert_eq!(res.statecount, 1);
        assert_eq!(res.finalcount, 0);
        assert_eq!(res.pathcount, 0);
    }

    // [spec:foma:sem:structures.fsm-quantifier-fn/test]
    // [spec:foma:sem:fomalib.fsm-quantifier-fn/test]
    #[test]
    fn quantifier_builds_nonempty_net() {
        // \x* x \x* x \x*  (strings with exactly two x's) -> non-empty language
        let mut net = fsm_quantifier("x");
        assert_eq!(fsm_isempty(&mut net), 0);
    }

    // [spec:foma:sem:structures.fsm-logical-precedence-fn/test]
    // [spec:foma:sem:fomalib.fsm-logical-precedence-fn/test]
    #[test]
    fn logical_precedence_builds_net() {
        clear_quantifiers();
        add_quantifier("Q");
        let mut net = fsm_logical_precedence("x", "y");
        fsm_count(&mut net);
        assert!(net.statecount >= 1);
        assert!(!net.states.is_empty());
        clear_quantifiers();
    }

    // [spec:foma:sem:structures.fsm-logical-eq-fn/test]
    // [spec:foma:sem:fomalib.fsm-logical-eq-fn/test]
    #[test]
    fn logical_eq_builds_net() {
        clear_quantifiers();
        add_quantifier("Q");
        let mut net = fsm_logical_eq("x", "y");
        fsm_count(&mut net);
        assert!(net.statecount >= 1);
        assert!(!net.states.is_empty());
        clear_quantifiers();
    }

    // [spec:foma:sem:fomalib.fsm-find-ambiguous-fn/test]
    #[test]
    #[should_panic]
    fn find_ambiguous_dead_prototype_panics() {
        let _ = fsm_find_ambiguous(fsm_empty_set());
    }

    // [spec:foma:sem:fomalib.fsm-mark-ambiguous-fn/test]
    #[test]
    #[should_panic]
    fn mark_ambiguous_dead_prototype_panics() {
        let _ = fsm_mark_ambiguous(fsm_empty_set());
    }
}

//! foma/constructions.c — literal Wave-2 (bug-for-bug) port per
//! docs/port/rust-conventions.md. Sem rules:
//! docs/spec/port/foma/constructions.md (per-file ids) plus the fomalib.h /
//! fomalibconf.h prototype ids.
//!
//! Slice 1: infrastructure (merge-sigma, state pointers, triplet hash)
//! and the product/regular constructions. Slice 2 (from fsm_escape down):
//! the elementary machines, derived regex operators, substitutions and the
//! remaining constructions.
//!
//! Interior pointers of the C (state_arr.transitions, the outarray/index
//! tails in fsm_compose) are represented as indices per the conventions.
//! The worklist is the global int stack; state numbering comes from the
//! triplet hash (keys are consecutive ints in insertion order).

use crate::coaccessible::fsm_coaccessible;
use crate::determinize::fsm_determinize;
use crate::dynarray::{
    fsm_construct_add_arc, fsm_construct_add_arc_nums, fsm_construct_copy_sigma,
    fsm_construct_done, fsm_construct_init, fsm_construct_set_final, fsm_construct_set_initial,
    fsm_get_arc_in, fsm_get_arc_num_in, fsm_get_arc_num_out, fsm_get_arc_out, fsm_get_arc_source,
    fsm_get_arc_target, fsm_get_next_arc, fsm_get_next_final, fsm_get_next_initial,
    fsm_get_next_state, fsm_get_next_state_arc, fsm_get_num_states, fsm_get_symbol_number,
    fsm_read_done, fsm_read_init, fsm_read_is_final, fsm_read_reset, fsm_state_add_arc,
    fsm_state_close, fsm_state_end_state, fsm_state_init, fsm_state_set_current_state,
};
use crate::extract::{fsm_lower, fsm_upper};
use crate::flags::flag_check;
use crate::int_stack::{int_stack_clear, int_stack_isempty, int_stack_pop, int_stack_push};
use crate::mem::{G_COMPOSE_TRISTATE, G_FLAG_IS_EPSILON};
use crate::minimize::fsm_minimize;
use crate::rewrite::fsm_clear_contexts;
use crate::sigma::{
    sigma_add, sigma_add_special, sigma_cleanup, sigma_find, sigma_find_number, sigma_max,
    sigma_remove, sigma_size, sigma_sort, sigma_substitute,
};
use crate::structures::{
    find_arccount, fsm_copy, fsm_create, fsm_destroy, fsm_empty_set, fsm_empty_string,
    fsm_identity, fsm_isempty, fsm_sigma_destroy, fsm_sigma_pairs_net, FSM_OPTIONS,
};
use crate::topsort::fsm_topsort;
use crate::types::{
    Fsm, FsmState, Fsmcontexts, Sigma, EPSILON, IDENTITY, M_LOWER, M_UPPER, NO, OP_IGNORE_ALL,
    OP_IGNORE_INTERNAL, PATHCOUNT_CYCLIC, PATHCOUNT_UNKNOWN, UNK, UNKNOWN, YES,
};
use crate::utf8::{utf8skip, utf8strlen};

/* C: #define KLEENE_STAR 0 / KLEENE_PLUS 1 / OPTIONALITY 2 and
#define COMPLEMENT 0 / COMPLETE 1 — file-local constants, no spec ids */
pub const KLEENE_STAR: i32 = 0;
pub const KLEENE_PLUS: i32 = 1;
pub const OPTIONALITY: i32 = 2;

pub const COMPLEMENT: i32 = 0;
pub const COMPLETE: i32 = 1;

/* C: #define STACK_3_PUSH(a,b,c) / STACK_2_PUSH(a,b) — expanded inline at
each use site below (int_stack_push calls in the same order) */

// [spec:foma:def:constructions.mergesigma]
#[derive(Debug, Clone)]
pub struct Mergesigma {
    /* C: char *symbol aliases the source sigma node's string (no copy);
    owned clone here — observably equivalent (copy_mergesigma deep-copies
    again and the C list is freed without freeing the symbols) */
    pub symbol: Option<String>,
    /// 1 = in net 1, 2 = in net 2, 3 = in both
    pub presence: u8,
    pub number: i32,
    pub next: Option<Box<Mergesigma>>,
}

// [spec:foma:def:constructions.sort-cmp-fn]
// [spec:foma:sem:constructions.sort-cmp-fn]
// [spec:foma:def:fomalibconf.sort-cmp-fn]
// [spec:foma:sem:fomalibconf.sort-cmp-fn]
pub fn sort_cmp(a: &FsmState, b: &FsmState) -> i32 {
    a.state_no - b.state_no
}

// [spec:foma:def:constructions.fsm-kleene-star-fn]
// [spec:foma:sem:constructions.fsm-kleene-star-fn]
// [spec:foma:def:fomalib.fsm-kleene-star-fn]
// [spec:foma:sem:fomalib.fsm-kleene-star-fn]
pub fn fsm_kleene_star(net: Box<Fsm>) -> Box<Fsm> {
    fsm_kleene_closure(net, KLEENE_STAR)
}

// [spec:foma:def:constructions.fsm-kleene-plus-fn]
// [spec:foma:sem:constructions.fsm-kleene-plus-fn]
// [spec:foma:def:fomalib.fsm-kleene-plus-fn]
// [spec:foma:sem:fomalib.fsm-kleene-plus-fn]
pub fn fsm_kleene_plus(net: Box<Fsm>) -> Box<Fsm> {
    fsm_kleene_closure(net, KLEENE_PLUS)
}

// [spec:foma:def:constructions.fsm-optionality-fn]
// [spec:foma:sem:constructions.fsm-optionality-fn]
// [spec:foma:def:fomalib.fsm-optionality-fn]
// [spec:foma:sem:fomalib.fsm-optionality-fn]
pub fn fsm_optionality(net: Box<Fsm>) -> Box<Fsm> {
    fsm_kleene_closure(net, OPTIONALITY)
}

// [spec:foma:def:constructions.fsm-sort-lines-fn]
// [spec:foma:sem:constructions.fsm-sort-lines-fn]
// [spec:foma:def:fomalibconf.fsm-sort-lines-fn]
// [spec:foma:sem:fomalibconf.fsm-sort-lines-fn]
pub fn fsm_sort_lines(net: &mut Fsm) {
    let count = find_arccount(&net.states);
    /* C: qsort (unstable) over the lines before the sentinel; a slice
    sort_unstable is an admissible qsort behavior */
    net.states[..count as usize].sort_unstable_by(|a, b| sort_cmp(a, b).cmp(&0));
}

// [spec:foma:def:constructions.fsm-update-flags-fn]
// [spec:foma:sem:constructions.fsm-update-flags-fn]
// [spec:foma:def:fomalibconf.fsm-update-flags-fn]
// [spec:foma:sem:fomalibconf.fsm-update-flags-fn]
pub fn fsm_update_flags(
    net: &mut Fsm,
    det: i32,
    pru: i32,
    min: i32,
    eps: i32,
    r#loop: i32,
    completed: i32,
) {
    net.is_deterministic = det;
    net.is_pruned = pru;
    net.is_minimized = min;
    net.is_epsilon_free = eps;
    net.is_loop_free = r#loop;
    net.is_completed = completed;
    net.arcs_sorted_in = NO;
    net.arcs_sorted_out = NO;
}

// [spec:foma:def:constructions.fsm-count-states-fn]
// [spec:foma:sem:constructions.fsm-count-states-fn]
pub fn fsm_count_states(fsm: &[FsmState]) -> i32 {
    let mut temp = -1;
    let mut states = 0;
    let mut i = 0usize;
    while fsm[i].state_no != -1 {
        if temp != fsm[i].state_no {
            states += 1;
            temp = fsm[i].state_no;
        }
        i += 1;
    }
    states
}

// [spec:foma:def:constructions.state-arr]
#[derive(Debug, Clone)]
pub struct StateArr {
    pub r#final: i32,
    pub start: i32,
    /* C: struct fsm_state *transitions — pointer to the state's first line;
    an index into the same line table here (interior pointer convention) */
    pub transitions: usize,
}

// [spec:foma:def:constructions.init-state-pointers-fn]
// [spec:foma:sem:constructions.init-state-pointers-fn]
pub fn init_state_pointers(fsm_state: &[FsmState]) -> Vec<StateArr> {
    /* Create an array for quick lookup of whether states are final, and a pointer to the first line regarding each state */

    let mut sold = -1;
    let states = fsm_count_states(fsm_state);
    /* C: malloc((states+1) entries) — uninitialized; the spare entry and the
    transitions fields start zeroed here */
    let mut state_arr: Vec<StateArr> = vec![
        StateArr {
            r#final: 0,
            start: 0,
            transitions: 0,
        };
        (states + 1) as usize
    ];
    for i in 0..states {
        state_arr[i as usize].r#final = 0;
        state_arr[i as usize].start = 0;
    }

    let mut i = 0usize;
    while fsm_state[i].state_no != -1 {
        if fsm_state[i].final_state == 1 {
            state_arr[fsm_state[i].state_no as usize].r#final = 1;
        }
        if fsm_state[i].start_state == 1 {
            state_arr[fsm_state[i].state_no as usize].start = 1;
        }
        if fsm_state[i].state_no != sold {
            state_arr[fsm_state[i].state_no as usize].transitions = i;
            sold = fsm_state[i].state_no;
        }
        i += 1;
    }
    state_arr
}

/* An open addressing scheme (with linear probing) to store triplets of ints */
/* and hashing them with an automatically increasing key at every insert     */
/* The table is rehashed whenever occupancy reaches 0.5                      */

// [spec:foma:def:constructions.triplethash-triplets]
#[derive(Debug, Clone)]
pub struct TriplethashTriplets {
    pub a: i32,
    pub b: i32,
    pub c: i32,
    pub key: i32,
}

// [spec:foma:def:constructions.triplethash]
#[derive(Debug)]
pub struct Triplethash {
    pub triplets: Vec<TriplethashTriplets>,
    pub tablesize: u32,
    pub occupancy: i32,
}

// [spec:foma:def:constructions.triplet-hash-init-fn]
// [spec:foma:sem:constructions.triplet-hash-init-fn]
pub fn triplet_hash_init() -> Box<Triplethash> {
    let mut th = Box::new(Triplethash {
        tablesize: 128,
        occupancy: 0,
        /* C: malloc'd — the a/b/c fields of empty slots are uninitialized
        (zeroed here); only key is initialized below */
        triplets: Vec::new(),
    });
    th.triplets = vec![
        TriplethashTriplets {
            a: 0,
            b: 0,
            c: 0,
            key: 0,
        };
        th.tablesize as usize
    ];
    let mut i = 0usize;
    while i < th.tablesize as usize {
        th.triplets[i].key = -1;
        i += 1;
    }
    th
}

// [spec:foma:def:constructions.triplethash-hashf-fn]
// [spec:foma:sem:constructions.triplethash-hashf-fn]
pub fn triplethash_hashf(a: i32, b: i32, c: i32) -> u32 {
    /* C: a * 7907 + b * 86028157 + c * 7919 in signed int arithmetic
    (overflow is UB that wraps in practice) — explicit wrapping i32 ops
    reproduce the same slot sequence */
    a.wrapping_mul(7907)
        .wrapping_add(b.wrapping_mul(86028157))
        .wrapping_add(c.wrapping_mul(7919)) as u32
}

// [spec:foma:def:constructions.triplet-hash-free-fn]
// [spec:foma:sem:constructions.triplet-hash-free-fn]
pub fn triplet_hash_free(th: Option<Box<Triplethash>>) {
    if let Some(th) = th {
        /* free(th->triplets); free(th) */
        drop(th);
    }
}

// [spec:foma:def:constructions.triplet-hash-insert-with-key-fn]
// [spec:foma:sem:constructions.triplet-hash-insert-with-key-fn]
pub fn triplet_hash_insert_with_key(th: &mut Triplethash, a: i32, b: i32, c: i32, key: i32) {
    let mut hash = triplethash_hashf(a, b, c) % th.tablesize;
    loop {
        if th.triplets[hash as usize].key == -1 {
            th.triplets[hash as usize].key = key;
            th.triplets[hash as usize].a = a;
            th.triplets[hash as usize].b = b;
            th.triplets[hash as usize].c = c;
            break;
        }
        hash += 1;
        if hash >= th.tablesize {
            hash -= th.tablesize;
        }
    }
}

// [spec:foma:def:constructions.triplet-hash-insert-fn]
// [spec:foma:sem:constructions.triplet-hash-insert-fn]
pub fn triplet_hash_insert(th: &mut Triplethash, a: i32, b: i32, c: i32) -> i32 {
    let mut hash = triplethash_hashf(a, b, c) % th.tablesize;
    loop {
        if th.triplets[hash as usize].key == -1 {
            th.triplets[hash as usize].key = th.occupancy;
            th.triplets[hash as usize].a = a;
            th.triplets[hash as usize].b = b;
            th.triplets[hash as usize].c = c;
            th.occupancy = th.occupancy + 1;
            /* C: int occupancy > unsigned tablesize/2 — the int converts
            to unsigned in the comparison (occupancy is never negative) */
            if th.occupancy as u32 > th.tablesize / 2 {
                triplet_hash_rehash(th);
            }
            return th.occupancy - 1;
        }
        hash += 1;
        if hash >= th.tablesize {
            hash -= th.tablesize;
        }
    }
}

// [spec:foma:def:constructions.triplet-hash-rehash-fn]
// [spec:foma:sem:constructions.triplet-hash-rehash-fn]
pub fn triplet_hash_rehash(th: &mut Triplethash) {
    let newtablesize = th.tablesize * 2;
    let oldtablesize = th.tablesize;
    /* C: malloc'd new table (a/b/c uninitialized; zeroed here) */
    let oldtriplets = std::mem::replace(
        &mut th.triplets,
        vec![
            TriplethashTriplets {
                a: 0,
                b: 0,
                c: 0,
                key: 0,
            };
            newtablesize as usize
        ],
    );
    /* tablesize updated BEFORE reinserting so probes use the new size */
    th.tablesize = newtablesize;
    for i in 0..newtablesize as usize {
        th.triplets[i].key = -1;
    }
    for i in 0..oldtablesize as usize {
        if oldtriplets[i].key != -1 {
            triplet_hash_insert_with_key(
                th,
                oldtriplets[i].a,
                oldtriplets[i].b,
                oldtriplets[i].c,
                oldtriplets[i].key,
            );
        }
    }
    /* free(oldtriplets) — dropped here */
}

// [spec:foma:def:constructions.triplet-hash-find-fn]
// [spec:foma:sem:constructions.triplet-hash-find-fn]
pub fn triplet_hash_find(th: &Triplethash, a: i32, b: i32, c: i32) -> i32 {
    let mut hash = triplethash_hashf(a, b, c) % th.tablesize;
    let mut j: u32 = 0;
    while j < th.tablesize {
        if th.triplets[hash as usize].key == -1 {
            return -1;
        }
        if th.triplets[hash as usize].a == a
            && th.triplets[hash as usize].b == b
            && th.triplets[hash as usize].c == c
        {
            return th.triplets[hash as usize].key;
        }
        hash += 1;
        if hash >= th.tablesize {
            hash -= th.tablesize;
        }
        j += 1;
    }
    -1
}

// [spec:foma:def:constructions.fsm-intersect-fn]
// [spec:foma:sem:constructions.fsm-intersect-fn]
// [spec:foma:def:fomalib.fsm-intersect-fn]
// [spec:foma:sem:fomalib.fsm-intersect-fn]
pub fn fsm_intersect(net1: Box<Fsm>, net2: Box<Fsm>) -> Box<Fsm> {
    /* C: struct blookup {int mainloop; int target; } *array, *bptr; —
    function-local type */
    #[derive(Clone)]
    struct Blookup {
        mainloop: i32,
        target: i32,
    }

    let mut net1 = fsm_minimize(net1);
    let mut net2 = fsm_minimize(net2);

    if fsm_isempty(&mut net1) != 0 || fsm_isempty(&mut net2) != 0 {
        fsm_destroy(net1);
        fsm_destroy(net2);
        return fsm_empty_set();
    }

    fsm_merge_sigma(&mut net1, &mut net2);

    fsm_update_flags(&mut net1, YES, NO, UNK, YES, UNK, UNK);

    let sigma2size = sigma_max(net2.sigma.as_deref()) + 1;
    /* calloc — zeroed entries; mainloop stamps start at 1 below, so all
    entries begin stale */
    let mut array: Vec<Blookup> = vec![
        Blookup {
            mainloop: 0,
            target: 0,
        };
        (sigma2size * sigma2size) as usize
    ];
    let mut mainloop = 0;

    /* Intersect two networks by the running-in-parallel method */
    /* new state 0 = {0,0} */

    /* STACK_2_PUSH(0,0) */
    int_stack_push(0);
    int_stack_push(0);

    let mut th = triplet_hash_init();
    triplet_hash_insert(&mut th, 0, 0, 0);

    fsm_state_init(sigma_max(net1.sigma.as_deref()));

    let point_a = init_state_pointers(&net1.states);
    let point_b = init_state_pointers(&net2.states);

    while int_stack_isempty() == 0 {
        /* Get a pair of states to examine */

        let a = int_stack_pop();
        let b = int_stack_pop();

        let current_state = triplet_hash_find(&th, a, b, 0);
        let current_start = if point_a[a as usize].start == 1 && point_b[b as usize].start == 1 {
            1
        } else {
            0
        };
        let current_final = if point_a[a as usize].r#final == 1 && point_b[b as usize].r#final == 1
        {
            1
        } else {
            0
        };

        fsm_state_set_current_state(current_state, current_final, current_start);

        /* Create a lookup index for machine b */
        /* array[in][out] holds the target for this state and the symbol pair in:out */
        /* Also, we keep track of whether an entry is fresh by the mainloop counter */
        /* so we don't mistakenly use an old entry and don't have to clear the table */
        /* between each state pair we encounter */

        mainloop += 1;
        let mut bi = point_b[b as usize].transitions;
        while net2.states[bi].state_no == b {
            if net2.states[bi].r#in < 0 {
                bi += 1;
                continue;
            }
            let bptr =
                ((net2.states[bi].r#in as i32) * sigma2size + net2.states[bi].out as i32) as usize;
            array[bptr].mainloop = mainloop;
            array[bptr].target = net2.states[bi].target;
            bi += 1;
        }

        /* The main loop where we run the machines in parallel */
        /* We look at each transition of a in this state, and consult the index of b */
        /* we just created */

        let mut ai = point_a[a as usize].transitions;
        while net1.states[ai].state_no == a {
            if net1.states[ai].r#in < 0 || net1.states[ai].out < 0 {
                ai += 1;
                continue;
            }
            let bptr =
                ((net1.states[ai].r#in as i32) * sigma2size + net1.states[ai].out as i32) as usize;

            if array[bptr].mainloop != mainloop {
                ai += 1;
                continue;
            }

            let atarget = net1.states[ai].target;
            let btarget = array[bptr].target;
            let mut target_number = triplet_hash_find(&th, atarget, btarget, 0);
            if target_number == -1 {
                /* STACK_2_PUSH(bptr->target, machine_a->target) */
                int_stack_push(btarget);
                int_stack_push(atarget);
                target_number = triplet_hash_insert(&mut th, atarget, btarget, 0);
            }

            let (ain, aout) = (net1.states[ai].r#in as i32, net1.states[ai].out as i32);
            fsm_state_add_arc(
                current_state,
                ain,
                aout,
                target_number,
                current_final,
                current_start,
            );

            ai += 1;
        }
        fsm_state_end_state();
    }
    let mut new_net = fsm_create("");
    fsm_sigma_destroy(new_net.sigma.take());
    new_net.sigma = net1.sigma.take();
    fsm_destroy(net2);
    fsm_destroy(net1);
    fsm_state_close(&mut new_net);
    /* free(point_a); free(point_b); free(array) */
    drop(point_a);
    drop(point_b);
    drop(array);
    triplet_hash_free(Some(th));
    fsm_coaccessible(new_net)
}

// [spec:foma:def:constructions.fsm-compose-fn]
// [spec:foma:sem:constructions.fsm-compose-fn]
// [spec:foma:def:fomalib.fsm-compose-fn]
// [spec:foma:sem:fomalib.fsm-compose-fn]
pub fn fsm_compose(net1: Box<Fsm>, net2: Box<Fsm>) -> Box<Fsm> {
    /* The composition algorithm is the basic naive composition where we lazily      */
    /* take the cross-product of states P and Q and move to a new state with symbols */
    /* ain, bout if the symbols aout = bin.  Also, if aout = 0 state p goes to       */
    /* its target, while q stays.  Similarly, if bin = 0, q goes to its target       */
    /* while p stays.                                                                */

    /* We have two variants of the algorithm to avoid creating multiple paths:       */
    /* 1) Bistate composition.  In this variant, when we create a new state, we call it */
    /*    (p,q,mode) where mode = 0 or 1, depending on what kind of an arc we followed  */
    /*    to get there.  If we followed an x:y arc where x and y are both real symbols  */
    /*    we always go to mode 0, however, if we followed an 0:y arc, we go to mode 1.  */
    /*    from mode 1, we do not follow x:0 arcs.  Each (p,q,mode) is unique, and       */
    /*    from (p,q,X) we always consider the transitions from p and q.                 */
    /*    We never create arcs (x:0 0:y) yielding x:y.                                  */

    /* 2) Tristate composition. Here we always go to mode 0 with a x:y arc.             */
    /*    (x:0,0:y) yielding x:y is allowed, but only in mode 0                         */
    /*    (x:y y:z) is always allowed and results in target = mode 0                    */
    /*    0:y arcs lead to mode 2, and from there we stay in mode 2 with 0:y            */
    /*    in mode 2 we only consider 0:y and x:y arcs                                   */
    /*    x:0 arcs lead to mode 1, and from there we stay in mode 1 with x:0            */
    /*    in mode 1 we only consider x:0 and x:y arcs                                   */

    /* It seems unsettled which type of composition is better.  Tristate is similar to  */
    /* the filter transducer given in Mohri, Pereira and Riley (1996) and works well    */
    /* for cases such as [a:0 b:0 c:0 .o. 0:d 0:e 0:f], yielding the shortest path.     */
    /* However, for generic cases, bistate seems to yield smaller transducers.          */
    /* The global variable g_compose_tristate is set to OFF by default                  */

    /* C: struct outarray { short int symin; short int symout; int target;
    int mainloop; } and struct index { struct outarray *tail; } —
    function-local types; tail is an index into outarray here */
    #[derive(Clone)]
    struct OutarrayEntry {
        symin: i16,
        symout: i16,
        target: i32,
        mainloop: i32,
    }
    #[derive(Clone)]
    struct Index {
        tail: usize,
    }

    let g_compose_tristate = G_COMPOSE_TRISTATE.with(|c| c.get());
    let g_flag_is_epsilon = G_FLAG_IS_EPSILON.with(|c| c.get());

    let mut net1 = fsm_minimize(net1);
    let mut net2 = fsm_minimize(net2);

    if fsm_isempty(&mut net1) != 0 || fsm_isempty(&mut net2) != 0 {
        fsm_destroy(net1);
        fsm_destroy(net2);
        return fsm_empty_set();
    }

    /* If flag-is-epsilon is on, we need to add the flag symbols    */
    /* in both networks to each other's sigma so that UNKNOWN       */
    /* or IDENTITY symbols do not match these flags, since they are */
    /* supposed to have the behavior of EPSILON                     */
    /* And we need to do this before merging the sigmas, of course  */

    if g_flag_is_epsilon != 0 {
        let mut flags1 = 0;
        let mut flags2 = 0;
        let max2sigma = sigma_max(net2.sigma.as_deref());
        let mut sig1 = net1.sigma.as_deref();
        while let Some(s1) = sig1 {
            if flag_check(s1.symbol.as_deref().unwrap_or("")) != 0 {
                flags1 = 1;
                if sigma_find(s1.symbol.as_deref().unwrap_or(""), net2.sigma.as_deref()) == -1 {
                    sigma_add(
                        s1.symbol.as_deref().unwrap_or(""),
                        net2.sigma.as_deref_mut().unwrap(),
                    );
                }
            }
            sig1 = s1.next.as_deref();
        }

        let mut sig2 = net2.sigma.as_deref();
        while let Some(s2) = sig2 {
            if flag_check(s2.symbol.as_deref().unwrap_or("")) != 0 {
                if s2.number <= max2sigma {
                    flags2 = 1;
                }
                if sigma_find(s2.symbol.as_deref().unwrap_or(""), net1.sigma.as_deref()) == -1 {
                    sigma_add(
                        s2.symbol.as_deref().unwrap_or(""),
                        net1.sigma.as_deref_mut().unwrap(),
                    );
                }
            }
            sig2 = s2.next.as_deref();
        }
        sigma_sort(&mut net2);
        sigma_sort(&mut net1);
        if flags1 != 0 && flags2 != 0 {
            print!("***Warning: flag-is-epsilon is ON and both networks contain flags in composition.  This may yield incorrect results.  Set flag-is-epsilon to OFF.\n");
        }
    }

    fsm_merge_sigma(&mut net1, &mut net2);

    let mut is_flag: Vec<bool> = Vec::new();
    if g_flag_is_epsilon != 0 {
        /* Create lookup table for quickly checking if a symbol is a flag */
        /* C: malloc'd (uninitialized for numbers absent from the sigma);
        zero-initialized here */
        is_flag = vec![false; (sigma_max(net1.sigma.as_deref()) + 1) as usize];
        let mut sig1 = net1.sigma.as_deref();
        while let Some(s1) = sig1 {
            if flag_check(s1.symbol.as_deref().unwrap_or("")) != 0 {
                is_flag[s1.number as usize] = true;
            } else {
                is_flag[s1.number as usize] = false;
            }
            sig1 = s1.next.as_deref();
        }
    }

    fsm_update_flags(&mut net1, YES, NO, UNK, YES, UNK, UNK);

    let max2sigma = sigma_max(net2.sigma.as_deref());

    /* Create an index for looking up input symbols in machine b quickly */
    /* We store each machine_b->in symbol in outarray[symin][...] */
    /* the array index[symin] points to the tail of the current list in outarray */
    /* (we may have many entries for one input symbol as there may be many outputs */
    /* The field mainloop tells us whether the entry is current as we want to loop */
    /* UNKNOWN and IDENTITY are indexed as UNKNOWN because we need to find both */
    /* as they share some semantics */

    let mut index: Vec<Index> = vec![Index { tail: 0 }; (max2sigma + 1) as usize];
    let mut outarray: Vec<OutarrayEntry> = vec![
        OutarrayEntry {
            symin: 0,
            symout: 0,
            target: 0,
            mainloop: 0,
        };
        ((max2sigma + 2) * (max2sigma + 1)) as usize
    ];

    for i in 0..=max2sigma {
        index[i as usize].tail = ((max2sigma + 2) * i) as usize;
    }

    /* Mode, a, b */
    /* STACK_3_PUSH(0,0,0) */
    int_stack_push(0);
    int_stack_push(0);
    int_stack_push(0);

    let mut th = triplet_hash_init();
    triplet_hash_insert(&mut th, 0, 0, 0);

    fsm_state_init(sigma_max(net1.sigma.as_deref()));

    let point_a = init_state_pointers(&net1.states);
    let point_b = init_state_pointers(&net2.states);

    let mut mainloop = 0;

    while int_stack_isempty() == 0 {
        /* Get a pair of states to examine */

        let a = int_stack_pop();
        let b = int_stack_pop();
        let mode = int_stack_pop();

        let current_state = triplet_hash_find(&th, a, b, mode);
        let current_start = if point_a[a as usize].start == 1
            && point_b[b as usize].start == 1
            && mode == 0
        {
            1
        } else {
            0
        };
        let current_final = if point_a[a as usize].r#final == 1 && point_b[b as usize].r#final == 1
        {
            1
        } else {
            0
        };

        fsm_state_set_current_state(current_state, current_final, current_start);

        /* Create the index for machine b in this state */
        mainloop += 1;
        let mut bi = point_b[b as usize].transitions;
        while net2.states[bi].state_no == b {
            /* Index b */
            let bindex = if net2.states[bi].r#in as i32 == IDENTITY {
                UNKNOWN
            } else {
                net2.states[bi].r#in as i32
            };
            if bindex < 0 || net2.states[bi].target < 0 {
                bi += 1;
                continue;
            }

            let mut iptr = index[bindex as usize].tail;
            if outarray[iptr].mainloop != mainloop {
                iptr = (bindex * (max2sigma + 2)) as usize;
                index[bindex as usize].tail = iptr;
            } else {
                iptr += 1;
            }
            outarray[iptr].symin = net2.states[bi].r#in;
            outarray[iptr].symout = net2.states[bi].out;
            outarray[iptr].mainloop = mainloop;
            outarray[iptr].target = net2.states[bi].target;
            index[bindex as usize].tail = iptr;
            bi += 1;
        }

        let mut ai = point_a[a as usize].transitions;
        while net1.states[ai].state_no == a {
            /* If we have the same transition from (a,b)-> some state */
            /* If we have x:y y:z trans to some state */
            let aout = net1.states[ai].out as i32;
            /* IDENTITY is indexed under UNKNOWN (see above) */
            let asearch = if aout == IDENTITY { UNKNOWN } else { aout };
            if aout < 0 {
                ai += 1;
                continue;
            }
            let mut iptr = (asearch * (max2sigma + 2)) as usize;
            let currtail = index[asearch as usize].tail + 1;
            while iptr != currtail && outarray[iptr].mainloop == mainloop {
                let mut ain = net1.states[ai].r#in as i32;
                let mut aout = net1.states[ai].out as i32;
                let mut bin = outarray[iptr].symin as i32;
                let mut bout = outarray[iptr].symout as i32;

                if aout == IDENTITY && bin == UNKNOWN {
                    ain = UNKNOWN;
                    aout = UNKNOWN;
                } else if aout == UNKNOWN && bin == IDENTITY {
                    bin = UNKNOWN;
                    bout = UNKNOWN;
                }

                if g_compose_tristate == 0 {
                    if bin == aout && bin != -1 && (bin != EPSILON || mode == 0) {
                        /* mode -> 0 */
                        let atarget = net1.states[ai].target;
                        let btarget = outarray[iptr].target;
                        let mut target_number = triplet_hash_find(&th, atarget, btarget, 0);
                        if target_number == -1 {
                            /* STACK_3_PUSH(0, iptr->target, machine_a->target) */
                            int_stack_push(0);
                            int_stack_push(btarget);
                            int_stack_push(atarget);
                            target_number = triplet_hash_insert(&mut th, atarget, btarget, 0);
                        }

                        fsm_state_add_arc(
                            current_state,
                            ain,
                            bout,
                            target_number,
                            current_final,
                            current_start,
                        );
                    }
                } else if g_compose_tristate != 0 {
                    /* C: this branch is literally identical to the bistate
                    branch above — reproduced */
                    if bin == aout && bin != -1 && (bin != EPSILON || mode == 0) {
                        /* mode -> 0 */
                        let atarget = net1.states[ai].target;
                        let btarget = outarray[iptr].target;
                        let mut target_number = triplet_hash_find(&th, atarget, btarget, 0);
                        if target_number == -1 {
                            /* STACK_3_PUSH(0, iptr->target, machine_a->target) */
                            int_stack_push(0);
                            int_stack_push(btarget);
                            int_stack_push(atarget);
                            target_number = triplet_hash_insert(&mut th, atarget, btarget, 0);
                        }

                        fsm_state_add_arc(
                            current_state,
                            ain,
                            bout,
                            target_number,
                            current_final,
                            current_start,
                        );
                    }
                }

                iptr += 1;
            }
            ai += 1;
        }

        /* Treat epsilon outputs on machine a (may include flags) */
        let mut ai = point_a[a as usize].transitions;
        while net1.states[ai].state_no == a {
            let aout = net1.states[ai].out as i32;
            if aout != EPSILON && g_flag_is_epsilon == 0 {
                ai += 1;
                continue;
            }
            let ain = net1.states[ai].r#in as i32;

            if g_flag_is_epsilon != 0 && aout != -1 && mode == 0 && is_flag[aout as usize] {
                let atarget = net1.states[ai].target;
                let mut target_number = triplet_hash_find(&th, atarget, b, 0);
                if target_number == -1 {
                    /* STACK_3_PUSH(0, b, machine_a->target) */
                    int_stack_push(0);
                    int_stack_push(b);
                    int_stack_push(atarget);
                    target_number = triplet_hash_insert(&mut th, atarget, b, 0);
                }
                fsm_state_add_arc(
                    current_state,
                    ain,
                    aout,
                    target_number,
                    current_final,
                    current_start,
                );
            }

            if g_compose_tristate == 0 {
                /* Check A:0 arcs on upper side */
                if aout == EPSILON && mode == 0 {
                    /* mode -> 0 */
                    let atarget = net1.states[ai].target;
                    let mut target_number = triplet_hash_find(&th, atarget, b, 0);
                    if target_number == -1 {
                        /* STACK_3_PUSH(0, b, machine_a->target) */
                        int_stack_push(0);
                        int_stack_push(b);
                        int_stack_push(atarget);
                        target_number = triplet_hash_insert(&mut th, atarget, b, 0);
                    }

                    fsm_state_add_arc(
                        current_state,
                        ain,
                        EPSILON,
                        target_number,
                        current_final,
                        current_start,
                    );
                }
            } else if g_compose_tristate != 0 {
                if aout == EPSILON && mode != 2 {
                    /* mode -> 1 */
                    let atarget = net1.states[ai].target;
                    let mut target_number = triplet_hash_find(&th, atarget, b, 1);
                    if target_number == -1 {
                        /* STACK_3_PUSH(1, b, machine_a->target) */
                        int_stack_push(1);
                        int_stack_push(b);
                        int_stack_push(atarget);
                        target_number = triplet_hash_insert(&mut th, atarget, b, 1);
                    }

                    fsm_state_add_arc(
                        current_state,
                        ain,
                        EPSILON,
                        target_number,
                        current_final,
                        current_start,
                    );
                }
            }

            ai += 1;
        }
        /* Treat epsilon inputs on machine b (may include flags) */
        let mut bi = point_b[b as usize].transitions;
        while net2.states[bi].state_no == b {
            let bin = net2.states[bi].r#in as i32;
            if bin != EPSILON && g_flag_is_epsilon == 0 {
                bi += 1;
                continue;
            }

            let bout = net2.states[bi].out as i32;

            if g_flag_is_epsilon != 0 && bin != -1 && is_flag[bin as usize] {
                let btarget = net2.states[bi].target;
                let mut target_number = triplet_hash_find(&th, a, btarget, 1);
                if target_number == -1 {
                    /* STACK_3_PUSH(1, machine_b->target, a) */
                    int_stack_push(1);
                    int_stack_push(btarget);
                    int_stack_push(a);
                    target_number = triplet_hash_insert(&mut th, a, btarget, 1);
                }
                fsm_state_add_arc(
                    current_state,
                    bin,
                    bout,
                    target_number,
                    current_final,
                    current_start,
                );
            }

            if g_compose_tristate == 0 {
                /* Check 0:A arcs on lower side */
                if bin == EPSILON {
                    /* mode -> 1 */
                    let btarget = net2.states[bi].target;
                    let mut target_number = triplet_hash_find(&th, a, btarget, 1);
                    if target_number == -1 {
                        /* STACK_3_PUSH(1, machine_b->target, a) */
                        int_stack_push(1);
                        int_stack_push(btarget);
                        int_stack_push(a);
                        target_number = triplet_hash_insert(&mut th, a, btarget, 1);
                    }

                    fsm_state_add_arc(
                        current_state,
                        EPSILON,
                        bout,
                        target_number,
                        current_final,
                        current_start,
                    );
                }
            } else if g_compose_tristate != 0 {
                /* Check 0:A arcs on lower side */
                if bin == EPSILON && mode != 1 {
                    /* mode -> 1 */
                    let btarget = net2.states[bi].target;
                    let mut target_number = triplet_hash_find(&th, a, btarget, 2);
                    if target_number == -1 {
                        /* STACK_3_PUSH(2, machine_b->target, a) */
                        int_stack_push(2);
                        int_stack_push(btarget);
                        int_stack_push(a);
                        target_number = triplet_hash_insert(&mut th, a, btarget, 2);
                    }

                    fsm_state_add_arc(
                        current_state,
                        EPSILON,
                        bout,
                        target_number,
                        current_final,
                        current_start,
                    );
                }
            }
            bi += 1;
        }
        fsm_state_end_state();
    }

    /* free(net1->states) */
    net1.states = Vec::new();
    fsm_destroy(net2);
    fsm_state_close(&mut net1);
    /* free(point_a); free(point_b); free(index); free(outarray) */
    drop(point_a);
    drop(point_b);
    drop(index);
    drop(outarray);

    if g_flag_is_epsilon != 0 {
        /* free(is_flag) */
        drop(is_flag);
    }
    triplet_hash_free(Some(th));
    let net1 = fsm_topsort(fsm_coaccessible(net1));
    fsm_coaccessible(net1)
}

// [spec:foma:def:constructions.add-to-mergesigma-fn]
// [spec:foma:sem:constructions.add-to-mergesigma-fn]
pub fn add_to_mergesigma<'a>(
    msigma: &'a mut Mergesigma,
    sigma: &Sigma,
    presence: i16,
) -> &'a mut Mergesigma {
    let mut number = 0;

    let msigma = if msigma.number == -1 {
        number = 2;
        msigma
    } else {
        msigma.next = Some(Box::new(Mergesigma {
            symbol: None,
            presence: 0,
            number: 0,
            next: None,
        }));
        number = msigma.number;
        let msigma = msigma.next.as_deref_mut().unwrap();
        msigma.next = None;
        msigma
    };

    if sigma.number < 3 {
        msigma.number = sigma.number;
    } else {
        if number < 3 {
            number = 2;
        }
        msigma.number = number + 1;
    }
    /* C: msigma->symbol = sigma->symbol (aliased, no copy) — owned clone
    here, see the Mergesigma type comment */
    msigma.symbol = sigma.symbol.clone();
    msigma.presence = presence as u8;
    msigma
}

// [spec:foma:def:constructions.copy-mergesigma-fn]
// [spec:foma:sem:constructions.copy-mergesigma-fn]
pub fn copy_mergesigma(mergesigma: Option<&Mergesigma>) -> Option<Box<Sigma>> {
    let mut new_sigma: Option<Box<Sigma>> = None;

    /* C: tail-pointer append (sigma cursor trails the freshly malloc'd
    node); a tail cursor into the owned chain here */
    let mut tail: &mut Option<Box<Sigma>> = &mut new_sigma;
    let mut mergesigma = mergesigma;
    while let Some(m) = mergesigma {
        *tail = Some(Box::new(Sigma {
            number: m.number,
            /* sigma->symbol = NULL; if (mergesigma->symbol != NULL)
            sigma->symbol = strdup(mergesigma->symbol); */
            symbol: m.symbol.clone(),
            next: None,
        }));
        tail = &mut tail.as_deref_mut().unwrap().next;
        mergesigma = m.next.as_deref();
    }
    new_sigma
}

// [spec:foma:def:constructions.fsm-merge-sigma-fn]
// [spec:foma:sem:constructions.fsm-merge-sigma-fn]
// [spec:foma:def:fomalib.fsm-merge-sigma-fn]
// [spec:foma:sem:fomalib.fsm-merge-sigma-fn]
pub fn fsm_merge_sigma(net1: &mut Fsm, net2: &mut Fsm) {
    let mut end_1 = 0;
    let mut end_2 = 0;
    let mut equal = 1;
    let mut unknown_1 = 0;
    let mut unknown_2 = 0;
    let mut net_unk = 0;

    if !FSM_OPTIONS.with(|o| o.borrow().skip_word_boundary_marker) {
        let i = sigma_find(".#.", net1.sigma.as_deref());
        let j = sigma_find(".#.", net2.sigma.as_deref());
        if i != -1 && j == -1 {
            sigma_add(".#.", net2.sigma.as_deref_mut().unwrap());
            sigma_sort(net2);
        }
        if j != -1 && i == -1 {
            sigma_add(".#.", net1.sigma.as_deref_mut().unwrap());
            sigma_sort(net1);
        }
    }

    let sigmasizes = sigma_max(net1.sigma.as_deref()) + sigma_max(net2.sigma.as_deref()) + 3;

    /* C: malloc'd (uninitialized); zero-filled here — entries are always
    written before being read for well-formed nets */
    let mut mapping_1: Vec<i32> = vec![0; sigmasizes as usize];
    let mut mapping_2: Vec<i32> = vec![0; sigmasizes as usize];

    /* Fill mergesigma */

    let mut start_mergesigma = Box::new(Mergesigma {
        number: -1,
        symbol: None,
        presence: 0,
        next: None,
    });

    /* Loop over sigma 1, sigma 2 */
    {
        let mut sigma_1 = net1.sigma.as_deref();
        let mut sigma_2 = net2.sigma.as_deref();
        let mut mergesigma: &mut Mergesigma = &mut start_mergesigma;
        loop {
            if sigma_1.is_none() {
                end_1 = 1;
            }
            if sigma_2.is_none() {
                end_2 = 1;
            }
            if end_1 != 0 && end_2 != 0 {
                break;
            }
            if end_2 != 0 {
                /* Treating only 1 now */
                let s1 = sigma_1.unwrap();
                mergesigma = add_to_mergesigma(mergesigma, s1, 1);
                mapping_1[s1.number as usize] = mergesigma.number;
                sigma_1 = s1.next.as_deref();
                equal = 0;
                continue;
            } else if end_1 != 0 {
                /* Treating only 2 now */
                let s2 = sigma_2.unwrap();
                mergesigma = add_to_mergesigma(mergesigma, s2, 2);
                mapping_2[s2.number as usize] = mergesigma.number;
                sigma_2 = s2.next.as_deref();
                equal = 0;
                continue;
            } else {
                /* Both alive */

                let s1 = sigma_1.unwrap();
                let s2 = sigma_2.unwrap();

                /* 1 or 2 contains special characters */
                if s1.number <= IDENTITY || s2.number <= IDENTITY {
                    /* Treating zeros or unknowns */

                    if s1.number == UNKNOWN || s1.number == IDENTITY {
                        unknown_1 = 1;
                    }
                    if s2.number == UNKNOWN || s2.number == IDENTITY {
                        unknown_2 = 1;
                    }

                    if s1.number == s2.number {
                        mergesigma = add_to_mergesigma(mergesigma, s1, 3);
                        sigma_1 = s1.next.as_deref();
                        sigma_2 = s2.next.as_deref();
                    } else if s1.number < s2.number {
                        mergesigma = add_to_mergesigma(mergesigma, s1, 1);
                        sigma_1 = s1.next.as_deref();
                        equal = 0;
                    } else {
                        mergesigma = add_to_mergesigma(mergesigma, s2, 2);
                        sigma_2 = s2.next.as_deref();
                        equal = 0;
                    }
                    continue;
                }
                /* Both contain non-special chars */
                /* strcmp — Rust str comparison is bytewise, as strcmp */
                let cmp = s1
                    .symbol
                    .as_deref()
                    .unwrap()
                    .cmp(s2.symbol.as_deref().unwrap());
                if cmp == std::cmp::Ordering::Equal {
                    mergesigma = add_to_mergesigma(mergesigma, s1, 3);
                    /* Add symbol numbers to mapping */
                    mapping_1[s1.number as usize] = mergesigma.number;
                    mapping_2[s2.number as usize] = mergesigma.number;

                    sigma_1 = s1.next.as_deref();
                    sigma_2 = s2.next.as_deref();
                } else if cmp == std::cmp::Ordering::Less {
                    mergesigma = add_to_mergesigma(mergesigma, s1, 1);
                    mapping_1[s1.number as usize] = mergesigma.number;
                    sigma_1 = s1.next.as_deref();
                    equal = 0;
                } else {
                    mergesigma = add_to_mergesigma(mergesigma, s2, 2);
                    mapping_2[s2.number as usize] = mergesigma.number;
                    sigma_2 = s2.next.as_deref();
                    equal = 0;
                }
                continue;
            }
        }
    }

    /* Go over both net1 and net2 and replace arc numbers with new mappings */

    let mut i = 0usize;
    while net1.states[i].state_no != -1 {
        if net1.states[i].r#in > 2 {
            net1.states[i].r#in = mapping_1[net1.states[i].r#in as usize] as i16;
        }
        if net1.states[i].out > 2 {
            net1.states[i].out = mapping_1[net1.states[i].out as usize] as i16;
        }
        i += 1;
    }
    let mut i = 0usize;
    while net2.states[i].state_no != -1 {
        if net2.states[i].r#in > 2 {
            net2.states[i].r#in = mapping_2[net2.states[i].r#in as usize] as i16;
        }
        if net2.states[i].out > 2 {
            net2.states[i].out = mapping_2[net2.states[i].out as usize] as i16;
        }
        i += 1;
    }

    /* Copy mergesigma to net1, net2 */

    let new_sigma_1 = copy_mergesigma(Some(&start_mergesigma));
    let new_sigma_2 = copy_mergesigma(Some(&start_mergesigma));

    fsm_sigma_destroy(net1.sigma.take());
    fsm_sigma_destroy(net2.sigma.take());

    net1.sigma = new_sigma_1;
    net2.sigma = new_sigma_2;

    /* Expand on ?, ?:x, y:? */

    if unknown_1 != 0 && equal == 0 {
        /* Expand net 1 */
        let net_lines = find_arccount(&net1.states);
        /* C: net_unk carries its function-entry 0 here (only net 2's
        branch re-zeroes it) */
        let mut ms = Some(&*start_mergesigma);
        while let Some(m) = ms {
            if m.presence == 2 {
                net_unk += 1;
            }
            ms = m.next.as_deref();
        }
        let mut net_adds = 0;
        let mut i = 0usize;
        while net1.states[i].state_no != -1 {
            let (line_in, line_out) = (net1.states[i].r#in as i32, net1.states[i].out as i32);
            if line_in == IDENTITY {
                net_adds += net_unk;
            }
            if line_in == UNKNOWN && line_out != UNKNOWN {
                net_adds += net_unk;
            }
            if line_out == UNKNOWN && line_in != UNKNOWN {
                net_adds += net_unk;
            }
            if line_in == UNKNOWN && line_out == UNKNOWN {
                net_adds += net_unk * net_unk - net_unk + 2 * net_unk;
            }
            i += 1;
        }

        /* C: malloc'd (uninitialized); zeroed lines here */
        let mut new_1_state: Vec<FsmState> = vec![
            FsmState {
                state_no: 0,
                r#in: 0,
                out: 0,
                target: 0,
                final_state: 0,
                start_state: 0,
            };
            (net_adds + net_lines + 1) as usize
        ];
        let mut j: i32 = 0;
        let mut i = 0usize;
        while net1.states[i].state_no != -1 {
            let state_no = net1.states[i].state_no;
            let line_in = net1.states[i].r#in as i32;
            let line_out = net1.states[i].out as i32;
            let target = net1.states[i].target;
            let final_state = net1.states[i].final_state as i32;
            let start_state = net1.states[i].start_state as i32;

            if line_in == IDENTITY {
                add_fsm_arc(
                    &mut new_1_state,
                    j,
                    state_no,
                    line_in,
                    line_out,
                    target,
                    final_state,
                    start_state,
                );
                j += 1;
                let mut ms = Some(&*start_mergesigma);
                while let Some(m) = ms {
                    if m.presence == 2 && m.number > IDENTITY {
                        add_fsm_arc(
                            &mut new_1_state,
                            j,
                            state_no,
                            m.number,
                            m.number,
                            target,
                            final_state,
                            start_state,
                        );
                        j += 1;
                    }
                    ms = m.next.as_deref();
                }
            }

            if line_in == UNKNOWN && line_out != UNKNOWN {
                add_fsm_arc(
                    &mut new_1_state,
                    j,
                    state_no,
                    line_in,
                    line_out,
                    target,
                    final_state,
                    start_state,
                );
                j += 1;
                let mut ms = Some(&*start_mergesigma);
                while let Some(m) = ms {
                    if m.presence == 2 && m.number > IDENTITY {
                        add_fsm_arc(
                            &mut new_1_state,
                            j,
                            state_no,
                            m.number,
                            line_out,
                            target,
                            final_state,
                            start_state,
                        );
                        j += 1;
                    }
                    ms = m.next.as_deref();
                }
            }

            if line_in != UNKNOWN && line_out == UNKNOWN {
                add_fsm_arc(
                    &mut new_1_state,
                    j,
                    state_no,
                    line_in,
                    line_out,
                    target,
                    final_state,
                    start_state,
                );
                j += 1;
                let mut ms = Some(&*start_mergesigma);
                while let Some(m) = ms {
                    if m.presence == 2 && m.number > IDENTITY {
                        add_fsm_arc(
                            &mut new_1_state,
                            j,
                            state_no,
                            line_in,
                            m.number,
                            target,
                            final_state,
                            start_state,
                        );
                        j += 1;
                    }
                    ms = m.next.as_deref();
                }
            }

            /* Replace ?:? with ?:[all unknowns] [all unknowns]:? and [all unknowns]:[all unknowns] where a != b */
            if line_in == UNKNOWN && line_out == UNKNOWN {
                add_fsm_arc(
                    &mut new_1_state,
                    j,
                    state_no,
                    line_in,
                    line_out,
                    target,
                    final_state,
                    start_state,
                );
                j += 1;
                let mut ms2 = Some(&*start_mergesigma);
                while let Some(m2) = ms2 {
                    let mut ms = Some(&*start_mergesigma);
                    while let Some(m) = ms {
                        if ((m.presence == 2
                            && m2.presence == 2
                            && m.number > IDENTITY
                            && m2.number > IDENTITY)
                            || (m.number == UNKNOWN
                                && m2.number > IDENTITY
                                && m2.presence == 2)
                            || (m2.number == UNKNOWN
                                && m.number > IDENTITY
                                && m.presence == 2))
                            && m.number != m2.number
                        {
                            add_fsm_arc(
                                &mut new_1_state,
                                j,
                                state_no,
                                m.number,
                                m2.number,
                                target,
                                final_state,
                                start_state,
                            );
                            j += 1;
                        }
                        ms = m.next.as_deref();
                    }
                    ms2 = m2.next.as_deref();
                }
            }

            /* Simply copy arcs that are not IDENTITY or UNKNOWN */
            if (line_in > IDENTITY || line_in == EPSILON)
                && (line_out > IDENTITY || line_out == EPSILON)
            {
                add_fsm_arc(
                    &mut new_1_state,
                    j,
                    state_no,
                    line_in,
                    line_out,
                    target,
                    final_state,
                    start_state,
                );
                j += 1;
            }

            if line_in == -1 {
                add_fsm_arc(
                    &mut new_1_state,
                    j,
                    state_no,
                    line_in,
                    line_out,
                    target,
                    final_state,
                    start_state,
                );
                j += 1;
            }
            i += 1;
        }

        add_fsm_arc(&mut new_1_state, j, -1, -1, -1, -1, -1, -1);
        /* free(net1->states); net1->states = new_1_state */
        net1.states = new_1_state;
    }

    if unknown_2 != 0 && equal == 0 {
        /* Expand net 2 */
        let net_lines = find_arccount(&net2.states);
        net_unk = 0;
        let mut ms = Some(&*start_mergesigma);
        while let Some(m) = ms {
            if m.presence == 1 {
                net_unk += 1;
            }
            ms = m.next.as_deref();
        }

        let mut net_adds = 0;
        let mut i = 0usize;
        while net2.states[i].state_no != -1 {
            let (line_in, line_out) = (net2.states[i].r#in as i32, net2.states[i].out as i32);
            if line_in == IDENTITY {
                net_adds += net_unk;
            }
            if line_in == UNKNOWN && line_out != UNKNOWN {
                net_adds += net_unk;
            }
            if line_out == UNKNOWN && line_in != UNKNOWN {
                net_adds += net_unk;
            }
            if line_in == UNKNOWN && line_out == UNKNOWN {
                net_adds += net_unk * net_unk - net_unk + 2 * net_unk;
            }
            i += 1;
        }

        /* We need net_add new lines in fsm_state */
        let mut new_2_state: Vec<FsmState> = vec![
            FsmState {
                state_no: 0,
                r#in: 0,
                out: 0,
                target: 0,
                final_state: 0,
                start_state: 0,
            };
            (net_adds + net_lines + 1) as usize
        ];
        let mut j: i32 = 0;
        let mut i = 0usize;
        while net2.states[i].state_no != -1 {
            let state_no = net2.states[i].state_no;
            let line_in = net2.states[i].r#in as i32;
            let line_out = net2.states[i].out as i32;
            let target = net2.states[i].target;
            let final_state = net2.states[i].final_state as i32;
            let start_state = net2.states[i].start_state as i32;

            if line_in == IDENTITY {
                add_fsm_arc(
                    &mut new_2_state,
                    j,
                    state_no,
                    line_in,
                    line_out,
                    target,
                    final_state,
                    start_state,
                );
                j += 1;
                let mut ms = Some(&*start_mergesigma);
                while let Some(m) = ms {
                    if m.presence == 1 && m.number > IDENTITY {
                        add_fsm_arc(
                            &mut new_2_state,
                            j,
                            state_no,
                            m.number,
                            m.number,
                            target,
                            final_state,
                            start_state,
                        );
                        j += 1;
                    }
                    ms = m.next.as_deref();
                }
            }

            if line_in == UNKNOWN && line_out != UNKNOWN {
                add_fsm_arc(
                    &mut new_2_state,
                    j,
                    state_no,
                    line_in,
                    line_out,
                    target,
                    final_state,
                    start_state,
                );
                j += 1;
                let mut ms = Some(&*start_mergesigma);
                while let Some(m) = ms {
                    if m.presence == 1 && m.number > IDENTITY {
                        add_fsm_arc(
                            &mut new_2_state,
                            j,
                            state_no,
                            m.number,
                            line_out,
                            target,
                            final_state,
                            start_state,
                        );
                        j += 1;
                    }
                    ms = m.next.as_deref();
                }
            }

            if line_in != UNKNOWN && line_out == UNKNOWN {
                add_fsm_arc(
                    &mut new_2_state,
                    j,
                    state_no,
                    line_in,
                    line_out,
                    target,
                    final_state,
                    start_state,
                );
                j += 1;
                let mut ms = Some(&*start_mergesigma);
                while let Some(m) = ms {
                    if m.presence == 1 && m.number > IDENTITY {
                        add_fsm_arc(
                            &mut new_2_state,
                            j,
                            state_no,
                            line_in,
                            m.number,
                            target,
                            final_state,
                            start_state,
                        );
                        j += 1;
                    }
                    ms = m.next.as_deref();
                }
            }

            if line_in == UNKNOWN && line_out == UNKNOWN {
                add_fsm_arc(
                    &mut new_2_state,
                    j,
                    state_no,
                    line_in,
                    line_out,
                    target,
                    final_state,
                    start_state,
                );
                j += 1;
                let mut ms2 = Some(&*start_mergesigma);
                while let Some(m2) = ms2 {
                    let mut ms = Some(&*start_mergesigma);
                    while let Some(m) = ms {
                        if ((m.presence == 1
                            && m2.presence == 1
                            && m.number > IDENTITY
                            && m2.number > IDENTITY)
                            || (m.number == UNKNOWN
                                && m2.number > IDENTITY
                                && m2.presence == 1)
                            || (m2.number == UNKNOWN
                                && m.number > IDENTITY
                                && m.presence == 1))
                            && m.number != m2.number
                        {
                            add_fsm_arc(
                                &mut new_2_state,
                                j,
                                state_no,
                                m.number,
                                m2.number,
                                target,
                                final_state,
                                start_state,
                            );
                            j += 1;
                        }
                        ms = m.next.as_deref();
                    }
                    ms2 = m2.next.as_deref();
                }
            }

            /* Simply copy arcs that are not IDENTITY or UNKNOWN */
            if (line_in > IDENTITY || line_in == EPSILON)
                && (line_out > IDENTITY || line_out == EPSILON)
            {
                add_fsm_arc(
                    &mut new_2_state,
                    j,
                    state_no,
                    line_in,
                    line_out,
                    target,
                    final_state,
                    start_state,
                );
                j += 1;
            }

            if line_in == -1 {
                add_fsm_arc(
                    &mut new_2_state,
                    j,
                    state_no,
                    line_in,
                    line_out,
                    target,
                    final_state,
                    start_state,
                );
                j += 1;
            }
            i += 1;
        }

        add_fsm_arc(&mut new_2_state, j, -1, -1, -1, -1, -1, -1);
        /* free(net2->states); net2->states = new_2_state */
        net2.states = new_2_state;
    }
    /* free(mapping_1); free(mapping_2) */
    drop(mapping_1);
    drop(mapping_2);

    /* Free structure */
    drop(start_mergesigma);
}

// [spec:foma:def:constructions.add-fsm-arc-fn]
// [spec:foma:sem:constructions.add-fsm-arc-fn]
// [spec:foma:def:fomalibconf.add-fsm-arc-fn]
// [spec:foma:sem:fomalibconf.add-fsm-arc-fn]
pub fn add_fsm_arc(
    fsm: &mut [FsmState],
    offset: i32,
    state_no: i32,
    r#in: i32,
    out: i32,
    target: i32,
    final_state: i32,
    start_state: i32,
) -> i32 {
    let mut offset = offset;
    let line = &mut fsm[offset as usize];
    line.state_no = state_no;
    /* int→short / int→char truncation as in C */
    line.r#in = r#in as i16;
    line.out = out as i16;
    line.target = target;
    line.final_state = final_state as i8;
    line.start_state = start_state as i8;
    offset += 1;
    offset
}

// [spec:foma:def:constructions.fsm-count-fn]
// [spec:foma:sem:constructions.fsm-count-fn]
// [spec:foma:def:fomalibconf.fsm-count-fn]
// [spec:foma:sem:fomalibconf.fsm-count-fn]
pub fn fsm_count(net: &mut Fsm) {
    let mut linecount = 0;
    let mut arccount = 0;
    let mut finalcount = 0;
    let mut maxstate = 0;

    let mut oldstate = -1;

    let mut i = 0usize;
    while net.states[i].state_no != -1 {
        if net.states[i].state_no > maxstate {
            maxstate = net.states[i].state_no;
        }

        linecount += 1;
        if net.states[i].target != -1 {
            arccount += 1;
            //        if (((fsm+i)->in != (fsm+i)->out) || ((fsm+i)->in == UNKNOWN) || ((fsm+i)->out == UNKNOWN))
            //    arity = 2;
        }
        if net.states[i].state_no != oldstate {
            if net.states[i].final_state != 0 {
                finalcount += 1;
            }
            oldstate = net.states[i].state_no;
        }
        i += 1;
    }

    linecount += 1;
    net.statecount = maxstate + 1;
    net.linecount = linecount;
    net.arccount = arccount;
    net.finalcount = finalcount;
}

// [spec:foma:def:constructions.fsm-add-to-states-fn]
// [spec:foma:sem:constructions.fsm-add-to-states-fn]
pub(crate) fn fsm_add_to_states(net: &mut Fsm, add: i32) {
    let mut i = 0usize;
    while net.states[i].state_no != -1 {
        net.states[i].state_no = net.states[i].state_no + add;
        if net.states[i].target != -1 {
            net.states[i].target = net.states[i].target + add;
        }
        i += 1;
    }
}

// [spec:foma:def:constructions.fsm-concat-fn]
// [spec:foma:sem:constructions.fsm-concat-fn]
// [spec:foma:def:fomalib.fsm-concat-fn]
// [spec:foma:sem:fomalib.fsm-concat-fn]
pub fn fsm_concat(net1: Box<Fsm>, net2: Box<Fsm>) -> Box<Fsm> {
    let mut net1 = net1;
    let mut net2 = net2;

    fsm_merge_sigma(&mut net1, &mut net2);

    fsm_count(&mut net1);
    fsm_count(&mut net2);
    /* The concatenation of a language with no final state should yield the empty language */
    if net1.finalcount == 0 || net2.finalcount == 0 {
        fsm_destroy(net1);
        fsm_destroy(net2);
        let net1 = fsm_empty_set();
        return net1;
    }

    /* Add |fsm1| states to the state numbers of fsm2 */
    let statecount1 = net1.statecount;
    fsm_add_to_states(&mut net2, statecount1);

    /* C: malloc'd (uninitialized); zeroed lines here */
    let mut new_fsm: Vec<FsmState> = vec![
        FsmState {
            state_no: 0,
            r#in: 0,
            out: 0,
            target: 0,
            final_state: 0,
            start_state: 0,
        };
        (net1.linecount + net2.linecount + net1.finalcount + 2) as usize
    ];
    let mut current_final = -1;
    /* Copy fsm1, fsm2 after each other, adding appropriate epsilon arcs */
    let mut j: i32 = 0;
    let mut i = 0usize;
    while net1.states[i].state_no != -1 {
        if net1.states[i].final_state == 1 && net1.states[i].state_no != current_final {
            let (state_no, start_state) = (net1.states[i].state_no, net1.states[i].start_state);
            add_fsm_arc(
                &mut new_fsm,
                j,
                state_no,
                EPSILON,
                EPSILON,
                net1.statecount,
                0,
                start_state as i32,
            );
            current_final = net1.states[i].state_no;
            j += 1;
        }
        if !(net1.states[i].target == -1 && net1.states[i].final_state == 1) {
            let (state_no, line_in, line_out, target, start_state) = (
                net1.states[i].state_no,
                net1.states[i].r#in as i32,
                net1.states[i].out as i32,
                net1.states[i].target,
                net1.states[i].start_state as i32,
            );
            add_fsm_arc(
                &mut new_fsm,
                j,
                state_no,
                line_in,
                line_out,
                target,
                0,
                start_state,
            );
            j += 1;
        }
        i += 1;
    }

    let mut i = 0usize;
    while net2.states[i].state_no != -1 {
        let (state_no, line_in, line_out, target, final_state) = (
            net2.states[i].state_no,
            net2.states[i].r#in as i32,
            net2.states[i].out as i32,
            net2.states[i].target,
            net2.states[i].final_state as i32,
        );
        add_fsm_arc(
            &mut new_fsm,
            j,
            state_no,
            line_in,
            line_out,
            target,
            final_state,
            0,
        );
        i += 1;
        j += 1;
    }
    add_fsm_arc(&mut new_fsm, j, -1, -1, -1, -1, -1, -1);
    /* free(net1->states) */
    fsm_destroy(net2);
    net1.states = new_fsm;
    if sigma_find_number(EPSILON, net1.sigma.as_deref()) == -1 {
        sigma_add_special(EPSILON, net1.sigma.as_deref_mut().unwrap());
    }
    fsm_count(&mut net1);
    net1.is_epsilon_free = NO;
    net1.is_deterministic = NO;
    net1.is_minimized = NO;
    net1.is_pruned = NO;
    fsm_minimize(net1)
}

// [spec:foma:def:constructions.fsm-union-fn]
// [spec:foma:sem:constructions.fsm-union-fn]
// [spec:foma:def:fomalib.fsm-union-fn]
// [spec:foma:sem:fomalib.fsm-union-fn]
pub fn fsm_union(net1: Box<Fsm>, net2: Box<Fsm>) -> Box<Fsm> {
    let mut net1 = net1;
    let mut net2 = net2;

    fsm_merge_sigma(&mut net1, &mut net2);

    fsm_count(&mut net1);
    fsm_count(&mut net2);

    let net1_offset = 1;
    let net2_offset = net1.statecount + 1;
    /* C: malloc'd (uninitialized); zeroed lines here */
    let mut new_fsm: Vec<FsmState> = vec![
        FsmState {
            state_no: 0,
            r#in: 0,
            out: 0,
            target: 0,
            final_state: 0,
            start_state: 0,
        };
        (net1.linecount + net2.linecount + 2) as usize
    ];

    let mut j: i32 = 0;

    add_fsm_arc(&mut new_fsm, j, 0, EPSILON, EPSILON, net1_offset, 0, 1);
    j += 1;
    add_fsm_arc(&mut new_fsm, j, 0, EPSILON, EPSILON, net2_offset, 0, 1);
    j += 1;
    let mut arccount = 2;
    let mut i = 0usize;
    while net1.states[i].state_no != -1 {
        let new_target = if net1.states[i].target == -1 {
            -1
        } else {
            net1.states[i].target + net1_offset
        };
        let (state_no, line_in, line_out, final_state) = (
            net1.states[i].state_no,
            net1.states[i].r#in as i32,
            net1.states[i].out as i32,
            net1.states[i].final_state as i32,
        );
        add_fsm_arc(
            &mut new_fsm,
            j,
            state_no + net1_offset,
            line_in,
            line_out,
            new_target,
            final_state,
            0,
        );
        j += 1;
        if new_target != -1 {
            arccount += 1;
        }
        i += 1;
    }
    let mut i = 0usize;
    while net2.states[i].state_no != -1 {
        let new_target = if net2.states[i].target == -1 {
            -1
        } else {
            net2.states[i].target + net2_offset
        };
        let (state_no, line_in, line_out, final_state) = (
            net2.states[i].state_no,
            net2.states[i].r#in as i32,
            net2.states[i].out as i32,
            net2.states[i].final_state as i32,
        );
        add_fsm_arc(
            &mut new_fsm,
            j,
            state_no + net2_offset,
            line_in,
            line_out,
            new_target,
            final_state,
            0,
        );
        j += 1;
        if new_target != -1 {
            arccount += 1;
        }
        i += 1;
    }
    add_fsm_arc(&mut new_fsm, j, -1, -1, -1, -1, -1, -1);
    j += 1;
    /* free(net1->states) */
    net1.states = new_fsm;
    net1.statecount = net1.statecount + net2.statecount + 1;
    net1.linecount = j;
    net1.arccount = arccount;
    net1.finalcount = net1.finalcount + net2.finalcount;
    fsm_destroy(net2);
    fsm_update_flags(&mut net1, NO, NO, NO, NO, UNK, NO);
    if sigma_find_number(EPSILON, net1.sigma.as_deref()) == -1 {
        sigma_add_special(EPSILON, net1.sigma.as_deref_mut().unwrap());
    }
    net1
}

// [spec:foma:def:constructions.fsm-completes-fn]
// [spec:foma:sem:constructions.fsm-completes-fn]
pub fn fsm_completes(net: Box<Fsm>, operation: i32) -> Box<Fsm> {
    /* TODO: this currently relies on that the sigma is gap-free in its numbering  */
    /* which can't always be counted on, especially when reading external machines */

    /* TODO: check arity */

    let mut net = net;
    if net.is_minimized != YES {
        net = fsm_minimize(net);
    }

    let mut incomplete = 0;
    if sigma_find_number(UNKNOWN, net.sigma.as_deref()) != -1 {
        /* C: sigma_remove's returned new head is discarded (harmless
        unless UNKNOWN were the head node); the owned list here must be
        reassigned */
        net.sigma = sigma_remove("@_UNKNOWN_SYMBOL_@", net.sigma.take());
    }
    if sigma_find_number(IDENTITY, net.sigma.as_deref()) == -1 {
        sigma_add_special(IDENTITY, net.sigma.as_deref_mut().unwrap());
        incomplete = 1;
    }

    let mut sigsize = sigma_size(net.sigma.as_deref());
    let last_sigma = sigma_max(net.sigma.as_deref());

    if sigma_find_number(EPSILON, net.sigma.as_deref()) != -1 {
        sigsize -= 1;
    }

    fsm_count(&mut net);
    let mut statecount = net.statecount;
    /* C: malloc'd short arrays (+1 for sink state; the spare entry is
    uninitialized in C, zeroed here) */
    let mut starts: Vec<i16> = vec![0; (statecount + 1) as usize];
    let mut finals: Vec<i16> = vec![0; (statecount + 1) as usize];
    let mut sinks: Vec<i16> = vec![0; (statecount + 1) as usize];

    /* Init starts, finals, sinks arrays */

    for i in 0..statecount {
        sinks[i as usize] = 1;
        finals[i as usize] = 0;
        starts[i as usize] = 0;
    }
    let mut arccount = 0;
    let mut i = 0usize;
    while net.states[i].state_no != -1 {
        if operation == COMPLEMENT {
            if net.states[i].final_state == 1 {
                net.states[i].final_state = 0;
            } else if net.states[i].final_state == 0 {
                net.states[i].final_state = 1;
            }
        }
        if net.states[i].target != -1 {
            arccount += 1;
        }
        starts[net.states[i].state_no as usize] = net.states[i].start_state as i16;
        finals[net.states[i].state_no as usize] = net.states[i].final_state as i16;
        if net.states[i].final_state != 0 && operation != COMPLEMENT {
            sinks[net.states[i].state_no as usize] = 0;
        }
        if net.states[i].final_state == 0 && operation == COMPLEMENT {
            sinks[net.states[i].state_no as usize] = 0;
        }
        if net.states[i].target != -1 && net.states[i].state_no != net.states[i].target {
            sinks[net.states[i].state_no as usize] = 0;
        }
        i += 1;
    }

    net.is_loop_free = NO;
    net.pathcount = PATHCOUNT_CYCLIC;

    if incomplete == 0 && arccount == sigsize * statecount {
        /*    printf("Already complete!\n"); */

        /*     if (operation == COMPLEMENT) { */
        /*       for (i=0; (fsm+i)->state_no != -1; i++) { */
        /* 	if ((fsm+i)->final_state) { */
        /* 	  (fsm+i)->final_state = 0; */
        /* 	} else { */
        /* 	  (fsm+i)->final_state = 1; */
        /* 	} */
        /*       } */
        /*     } */
        drop(starts);
        drop(finals);
        drop(sinks);
        net.is_completed = YES;
        net.is_minimized = YES;
        net.is_pruned = NO;
        net.is_deterministic = YES;
        return net;
    }

    /* Find an existing sink state, or invent a new one */

    let mut sink_state = -1;
    for i in 0..statecount {
        if sinks[i as usize] == 1 {
            sink_state = i;
            break;
        }
    }

    if sink_state == -1 {
        sink_state = statecount;
        starts[sink_state as usize] = 0;
        if operation == COMPLEMENT {
            finals[sink_state as usize] = 1;
        } else {
            finals[sink_state as usize] = 0;
        }
        statecount += 1;
    }

    /* We can build a state table without memory problems since the size */
    /* of the completed machine will be |Sigma| * |States| in all cases */

    sigsize += 2;

    /* C: malloc'd (uninitialized); initialized to -1 just below */
    let mut state_table: Vec<i32> = vec![0; (sigsize * statecount) as usize];

    /* Init state table */
    /* i = state #, j = sigma # */
    for i in 0..statecount {
        for j in 0..sigsize {
            state_table[(i * sigsize + j) as usize] = -1;
        }
    }

    let mut i = 0usize;
    while net.states[i].state_no != -1 {
        if net.states[i].target != -1 {
            state_table
                [(net.states[i].state_no * sigsize + net.states[i].r#in as i32) as usize] =
                net.states[i].target;
        }
        i += 1;
    }
    /* Add looping arcs from and to sink state */
    for j in 2..=last_sigma {
        state_table[(sink_state * sigsize + j) as usize] = sink_state;
    }
    /* Add missing arcs to sink state from all states */
    for i in 0..statecount {
        for j in 2..=last_sigma {
            if state_table[(i * sigsize + j) as usize] == -1 {
                state_table[(i * sigsize + j) as usize] = sink_state;
            }
        }
    }

    /* C: malloc'd (uninitialized); zeroed lines here */
    let mut new_fsm: Vec<FsmState> = vec![
        FsmState {
            state_no: 0,
            r#in: 0,
            out: 0,
            target: 0,
            final_state: 0,
            start_state: 0,
        };
        (sigsize * statecount + 1) as usize
    ];

    /* Complement requires toggling final, nonfinal states */
    /*   if (operation == COMPLEMENT) */
    /*     for (i=0; i < statecount; i++) */
    /*       *(finals+i) = *(finals+i) == 0 ? 1 : 0; */

    let mut offset: i32 = 0;
    for i in 0..statecount {
        for j in 2..=last_sigma {
            let target = if state_table[(i * sigsize + j) as usize] == -1 {
                sink_state
            } else {
                state_table[(i * sigsize + j) as usize]
            };
            add_fsm_arc(
                &mut new_fsm,
                offset,
                i,
                j,
                j,
                target,
                finals[i as usize] as i32,
                starts[i as usize] as i32,
            );
            offset += 1;
        }
    }
    add_fsm_arc(&mut new_fsm, offset, -1, -1, -1, -1, -1, -1);
    /* offset++ — the C bumps the counter one final time (unused) */
    /* free(net->states) */
    net.states = new_fsm;
    /* free(starts); free(finals); free(sinks); free(state_table) */
    drop(starts);
    drop(finals);
    drop(sinks);
    drop(state_table);
    net.is_minimized = NO;
    net.is_pruned = NO;
    net.is_completed = YES;
    net.statecount = statecount;
    net
}

// [spec:foma:def:constructions.fsm-complete-fn]
// [spec:foma:sem:constructions.fsm-complete-fn]
// [spec:foma:def:fomalib.fsm-complete-fn]
// [spec:foma:sem:fomalib.fsm-complete-fn]
pub fn fsm_complete(net: Box<Fsm>) -> Box<Fsm> {
    fsm_completes(net, COMPLETE)
}

// [spec:foma:def:constructions.fsm-complement-fn]
// [spec:foma:sem:constructions.fsm-complement-fn]
// [spec:foma:def:fomalib.fsm-complement-fn]
// [spec:foma:sem:fomalib.fsm-complement-fn]
pub fn fsm_complement(net: Box<Fsm>) -> Box<Fsm> {
    fsm_completes(net, COMPLEMENT)
}

// [spec:foma:def:constructions.fsm-kleene-closure-fn]
// [spec:foma:sem:constructions.fsm-kleene-closure-fn]
pub fn fsm_kleene_closure(net: Box<Fsm>, operation: i32) -> Box<Fsm> {
    if operation == OPTIONALITY {
        return fsm_union(net, fsm_empty_string());
    }

    let mut net = fsm_minimize(net);
    fsm_count(&mut net);

    /* C: malloc'd (uninitialized); zeroed lines here */
    let mut new_fsm: Vec<FsmState> = vec![
        FsmState {
            state_no: 0,
            r#in: 0,
            out: 0,
            target: 0,
            final_state: 0,
            start_state: 0,
        };
        (net.linecount + net.finalcount + 1) as usize
    ];

    let mut j: i32 = 0;
    if operation == KLEENE_STAR {
        add_fsm_arc(&mut new_fsm, j, 0, EPSILON, EPSILON, 1, 1, 1);
        j += 1;
    }
    if operation == KLEENE_PLUS {
        add_fsm_arc(&mut new_fsm, j, 0, EPSILON, EPSILON, 1, 0, 1);
        j += 1;
    }
    let mut laststate = 0;
    let mut arccount = 1;
    let mut i = 0usize;
    while net.states[i].state_no != -1 {
        let curr_state = net.states[i].state_no + 1;
        let curr_target = if net.states[i].target == -1 {
            -1
        } else {
            net.states[i].target + 1
        };
        if curr_target == -1 && net.states[i].final_state == 1 {
            add_fsm_arc(&mut new_fsm, j, curr_state, EPSILON, EPSILON, 0, 1, 0);
            j += 1;
            arccount += 1;
            i += 1;
            laststate = curr_state;
            continue;
        }
        if curr_state != laststate && net.states[i].final_state == 1 {
            arccount += 1;
            add_fsm_arc(&mut new_fsm, j, curr_state, EPSILON, EPSILON, 0, 1, 0);
            j += 1;
        }
        let (line_in, line_out, final_state) = (
            net.states[i].r#in as i32,
            net.states[i].out as i32,
            net.states[i].final_state as i32,
        );
        add_fsm_arc(
            &mut new_fsm,
            j,
            curr_state,
            line_in,
            line_out,
            curr_target,
            final_state,
            0,
        );
        j += 1;
        if curr_target != -1 {
            arccount += 1;
        }
        i += 1;
        laststate = curr_state;
    }
    add_fsm_arc(&mut new_fsm, j, -1, -1, -1, -1, -1, -1);
    j += 1;
    net.statecount = net.statecount + 1;
    net.linecount = j;
    net.finalcount = if operation == KLEENE_STAR {
        net.finalcount + 1
    } else {
        net.finalcount
    };
    net.arccount = arccount;
    net.pathcount = PATHCOUNT_UNKNOWN;
    /* free(net->states) */
    net.states = new_fsm;
    if sigma_find_number(EPSILON, net.sigma.as_deref()) == -1 {
        sigma_add_special(EPSILON, net.sigma.as_deref_mut().unwrap());
    }
    fsm_update_flags(&mut net, NO, NO, NO, NO, UNK, NO);
    net
}

// [spec:foma:def:constructions.fsm-cross-product-fn]
// [spec:foma:sem:constructions.fsm-cross-product-fn]
// [spec:foma:def:fomalib.fsm-cross-product-fn]
// [spec:foma:sem:fomalib.fsm-cross-product-fn]
pub fn fsm_cross_product(net1: Box<Fsm>, net2: Box<Fsm>) -> Box<Fsm> {
    /* Perform a cross product by running two machines in parallel */
    /* The approach here allows a state to stay, creating a a:0 or 0:b transition */
    /* with the a/b-state waiting, and the arc going to {a,stay} or {stay,b} */
    /* the wait maneuver is only possible if the waiting state is final */

    /* For the rewrite rules compilation, a different cross-product is used:  */
    /* rewrite_cp() synchronizes A and B as long as possible to get a unique  */
    /* output match for each cross product.                                   */

    /* This behavior where we postpone zeroes on either side and perform */
    /* and equal length cross-product as long as possible and never intermix */
    /* ?:0 and 0:? arcs (i.e. we keep both machines synchronized as long as possible */
    /* can be done by [A .x. B] & ?:?* [?:0*|0:?*] at the cost of possibly */
    /* up to three times larger transducers. */
    /* This is very similar to the idea in "tristate composition" in fsm_compose() */

    /* This function is only used for explicit cross products */
    /* such as a:b or A.x.B, etc.  In rewrite rules, we use rewrite_cp() */

    let mut net1 = fsm_minimize(net1);
    let mut net2 = fsm_minimize(net2);

    fsm_merge_sigma(&mut net1, &mut net2);

    fsm_count(&mut net1);
    fsm_count(&mut net2);

    /* new state 0 = {0,0} */

    /* STACK_2_PUSH(0,0) */
    int_stack_push(0);
    int_stack_push(0);

    let mut th = triplet_hash_init();
    triplet_hash_insert(&mut th, 0, 0, 0);

    fsm_state_init(sigma_max(net1.sigma.as_deref()));

    let point_a = init_state_pointers(&net1.states);
    let point_b = init_state_pointers(&net2.states);

    while int_stack_isempty() == 0 {
        /* Get a pair of states to examine */

        let a = int_stack_pop();
        let b = int_stack_pop();

        /* printf("Treating pair: {%i,%i}\n",a,b); */

        let current_state = triplet_hash_find(&th, a, b, 0);
        let current_start = if point_a[a as usize].start == 1 && point_b[b as usize].start == 1 {
            1
        } else {
            0
        };
        let current_final = if point_a[a as usize].r#final == 1 && point_b[b as usize].r#final == 1
        {
            1
        } else {
            0
        };

        fsm_state_set_current_state(current_state, current_final, current_start);

        let mut ai = point_a[a as usize].transitions;
        while net1.states[ai].state_no == a {
            let mut bi = point_b[b as usize].transitions;
            while net2.states[bi].state_no == b {
                if net1.states[ai].target == -1 && net2.states[bi].target == -1 {
                    bi += 1;
                    continue;
                }
                if net1.states[ai].target == -1 && net1.states[ai].final_state == 0 {
                    bi += 1;
                    continue;
                }
                if net2.states[bi].target == -1 && net2.states[bi].final_state == 0 {
                    bi += 1;
                    continue;
                }
                /* Main check */
                if !(net1.states[ai].target == -1 || net2.states[bi].target == -1) {
                    let atarget = net1.states[ai].target;
                    let btarget = net2.states[bi].target;
                    let mut target_number = triplet_hash_find(&th, atarget, btarget, 0);
                    if target_number == -1 {
                        /* STACK_2_PUSH(machine_b->target, machine_a->target) */
                        int_stack_push(btarget);
                        int_stack_push(atarget);
                        target_number = triplet_hash_insert(&mut th, atarget, btarget, 0);
                    }
                    let mut symbol1 = net1.states[ai].r#in as i32;
                    let mut symbol2 = net2.states[bi].r#in as i32;
                    if symbol1 == IDENTITY && symbol2 != IDENTITY {
                        symbol1 = UNKNOWN;
                    }
                    if symbol2 == IDENTITY && symbol1 != IDENTITY {
                        symbol2 = UNKNOWN;
                    }

                    fsm_state_add_arc(
                        current_state,
                        symbol1,
                        symbol2,
                        target_number,
                        current_final,
                        current_start,
                    );
                    /* @:@ -> @:@ and also ?:? */
                    if net1.states[ai].r#in as i32 == IDENTITY
                        && net2.states[bi].r#in as i32 == IDENTITY
                    {
                        fsm_state_add_arc(
                            current_state,
                            UNKNOWN,
                            UNKNOWN,
                            target_number,
                            current_final,
                            current_start,
                        );
                    }
                }
                if net1.states[ai].final_state == 1 && net2.states[bi].target != -1 {
                    /* Add 0:b i.e. stay in state A */
                    let astate = net1.states[ai].state_no;
                    let btarget = net2.states[bi].target;
                    let mut target_number = triplet_hash_find(&th, astate, btarget, 0);
                    if target_number == -1 {
                        /* STACK_2_PUSH(machine_b->target, machine_a->state_no) */
                        int_stack_push(btarget);
                        int_stack_push(astate);
                        target_number = triplet_hash_insert(&mut th, astate, btarget, 0);
                    }
                    /* @:0 becomes ?:0 */
                    let symbol2 = if net2.states[bi].r#in as i32 == IDENTITY {
                        UNKNOWN
                    } else {
                        net2.states[bi].r#in as i32
                    };
                    fsm_state_add_arc(
                        current_state,
                        EPSILON,
                        symbol2,
                        target_number,
                        current_final,
                        current_start,
                    );
                }

                if net2.states[bi].final_state == 1 && net1.states[ai].target != -1 {
                    /* Add a:0 i.e. stay in state B */
                    let atarget = net1.states[ai].target;
                    let bstate = net2.states[bi].state_no;
                    let mut target_number = triplet_hash_find(&th, atarget, bstate, 0);
                    if target_number == -1 {
                        /* STACK_2_PUSH(machine_b->state_no, machine_a->target) */
                        int_stack_push(bstate);
                        int_stack_push(atarget);
                        target_number = triplet_hash_insert(&mut th, atarget, bstate, 0);
                    }
                    /* @:0 becomes ?:0 */
                    let symbol1 = if net1.states[ai].r#in as i32 == IDENTITY {
                        UNKNOWN
                    } else {
                        net1.states[ai].r#in as i32
                    };
                    fsm_state_add_arc(
                        current_state,
                        symbol1,
                        EPSILON,
                        target_number,
                        current_final,
                        current_start,
                    );
                }
                bi += 1;
            }
            ai += 1;
        }
        /* Check arctrack */
        fsm_state_end_state();
    }

    /* free(net1->states) */
    net1.states = Vec::new();
    fsm_state_close(&mut net1);

    let mut epsilon = 0;
    let mut unknown = 0;
    let mut i = 0usize;
    while net1.states[i].state_no != -1 {
        if net1.states[i].r#in as i32 == EPSILON || net1.states[i].out as i32 == EPSILON {
            epsilon = 1;
        }
        if net1.states[i].r#in as i32 == UNKNOWN || net1.states[i].out as i32 == UNKNOWN {
            unknown = 1;
        }
        i += 1;
    }
    if epsilon == 1 {
        if sigma_find_number(EPSILON, net1.sigma.as_deref()) == -1 {
            sigma_add_special(EPSILON, net1.sigma.as_deref_mut().unwrap());
        }
    }
    if unknown == 1 {
        if sigma_find_number(UNKNOWN, net1.sigma.as_deref()) == -1 {
            sigma_add_special(UNKNOWN, net1.sigma.as_deref_mut().unwrap());
        }
    }
    /* free(point_a); free(point_b) */
    drop(point_a);
    drop(point_b);
    fsm_destroy(net2);
    triplet_hash_free(Some(th));
    fsm_coaccessible(net1)
}

// [spec:foma:def:constructions.fsm-minus-fn]
// [spec:foma:sem:constructions.fsm-minus-fn]
// [spec:foma:def:fomalib.fsm-minus-fn]
// [spec:foma:sem:fomalib.fsm-minus-fn]
pub fn fsm_minus(net1: Box<Fsm>, net2: Box<Fsm>) -> Box<Fsm> {
    let mut statecount = 0;

    let mut net1 = fsm_minimize(net1);
    let mut net2 = fsm_minimize(net2);

    fsm_merge_sigma(&mut net1, &mut net2);

    fsm_count(&mut net1);
    fsm_count(&mut net2);

    /* new state 0 = {1,1} */

    int_stack_clear();
    /* STACK_2_PUSH(1,1) */
    int_stack_push(1);
    int_stack_push(1);

    let mut th = triplet_hash_init();
    triplet_hash_insert(&mut th, 1, 1, 0);

    let point_a = init_state_pointers(&net1.states);
    let point_b = init_state_pointers(&net2.states);

    fsm_state_init(sigma_max(net1.sigma.as_deref()));

    while int_stack_isempty() == 0 {
        statecount += 1;
        /* Get a pair of states to examine */

        let mut a = int_stack_pop();
        let mut b = int_stack_pop();

        let current_state = triplet_hash_find(&th, a, b, 0);
        a -= 1;
        b -= 1;

        let (current_start, current_final);
        if b == -1 {
            current_start = 0;
            current_final = point_a[a as usize].r#final;
        } else {
            current_start = if a == 0 && b == 0 { 1 } else { 0 };
            current_final = if point_a[a as usize].r#final == 1 && point_b[b as usize].r#final == 0
            {
                1
            } else {
                0
            };
        }

        fsm_state_set_current_state(current_state, current_final, current_start);

        let mut ai = point_a[a as usize].transitions;
        while net1.states[ai].state_no == a {
            if net1.states[ai].target == -1 {
                break;
            }
            let target_number;
            if b == -1 {
                /* b is dead */
                let atarget = net1.states[ai].target;
                let found = triplet_hash_find(&th, atarget + 1, 0, 0);
                if found == -1 {
                    /* STACK_2_PUSH(0, (machine_a->target)+1) */
                    int_stack_push(0);
                    int_stack_push(atarget + 1);
                    target_number = triplet_hash_insert(&mut th, atarget + 1, 0, 0);
                } else {
                    target_number = found;
                }
            } else {
                /* b is alive */
                let mut b_has_trans = 0;
                let mut btarget = 0;
                let mut bi = point_b[b as usize].transitions;
                while net2.states[bi].state_no == b {
                    if net1.states[ai].r#in == net2.states[bi].r#in
                        && net1.states[ai].out == net2.states[bi].out
                    {
                        b_has_trans = 1;
                        btarget = net2.states[bi].target;
                        break;
                    }
                    bi += 1;
                }
                if b_has_trans != 0 {
                    let atarget = net1.states[ai].target;
                    let found = triplet_hash_find(&th, atarget + 1, btarget + 1, 0);
                    if found == -1 {
                        /* STACK_2_PUSH(btarget+1, (machine_a->target)+1) */
                        int_stack_push(btarget + 1);
                        int_stack_push(atarget + 1);
                        /* C inserts (machine_b->target)+1, which equals
                        btarget+1 (the scan broke at the matching line) */
                        let mbtarget = net2.states[bi].target;
                        target_number = triplet_hash_insert(&mut th, atarget + 1, mbtarget + 1, 0);
                    } else {
                        target_number = found;
                    }
                } else {
                    /* b is dead */
                    let atarget = net1.states[ai].target;
                    let found = triplet_hash_find(&th, atarget + 1, 0, 0);
                    if found == -1 {
                        /* STACK_2_PUSH(0, (machine_a->target)+1) */
                        int_stack_push(0);
                        int_stack_push(atarget + 1);
                        target_number = triplet_hash_insert(&mut th, atarget + 1, 0, 0);
                    } else {
                        target_number = found;
                    }
                }
            }
            let (line_in, line_out) = (net1.states[ai].r#in as i32, net1.states[ai].out as i32);
            fsm_state_add_arc(
                current_state,
                line_in,
                line_out,
                target_number,
                current_final,
                current_start,
            );
            ai += 1;
        }
        fsm_state_end_state();
    }

    let _ = statecount;
    /* free(net1->states) */
    net1.states = Vec::new();
    fsm_state_close(&mut net1);
    /* free(point_a); free(point_b) */
    drop(point_a);
    drop(point_b);
    fsm_destroy(net2);
    triplet_hash_free(Some(th));
    fsm_minimize(net1)
}

/* _marktail(?* L, 0:x) does ~$x .o. [..] -> x || L _ ;   */
/* _marktail(?* R.r, 0:x).r does ~$x .o. [..] -> x || _ R */

// [spec:foma:def:constructions.fsm-mark-fsm-tail-fn]
// [spec:foma:sem:constructions.fsm-mark-fsm-tail-fn]
// [spec:foma:def:fomalib.fsm-mark-fsm-tail-fn]
// [spec:foma:sem:fomalib.fsm-mark-fsm-tail-fn]
pub fn fsm_mark_fsm_tail(net: Box<Fsm>, marker: &Fsm) -> Box<Fsm> {
    let mut inh = fsm_read_init(Some(net)).unwrap();
    /* C: the read handle borrows marker (which is NOT destroyed); the
    Rust handle owns its net, so it reads a deep copy of marker —
    read-only, observably equivalent */
    let mut minh = fsm_read_init(Some(Box::new(marker.clone()))).unwrap();

    let name = inh.net.as_ref().unwrap().name.clone();
    let mut outh = fsm_construct_init(&name);
    fsm_construct_copy_sigma(&mut outh, inh.net.as_ref().unwrap().sigma.as_deref());

    let statecount = inh.net.as_ref().unwrap().statecount;
    /* calloc — zeroed; 0 means "unset" (fresh numbers start at
    statecount >= 1) */
    let mut mappings: Vec<i32> = vec![0; statecount as usize];
    let mut maxstate = statecount;

    while fsm_get_next_arc(&mut inh) != 0 {
        let target = fsm_get_arc_target(&inh);
        if fsm_read_is_final(&inh, target) != 0 {
            let newtarget;
            if mappings[target as usize] == 0 {
                newtarget = maxstate;
                mappings[target as usize] = newtarget;
                fsm_read_reset(Some(&mut minh));
                while fsm_get_next_arc(&mut minh) != 0 {
                    let min_in = fsm_get_arc_in(&minh).unwrap().to_string();
                    let min_out = fsm_get_arc_out(&minh).unwrap().to_string();
                    fsm_construct_add_arc(&mut outh, newtarget, target, &min_in, &min_out);
                }
                maxstate += 1;
            } else {
                newtarget = mappings[target as usize];
            }
            let (source, num_in, num_out) = (
                fsm_get_arc_source(&inh),
                fsm_get_arc_num_in(&inh),
                fsm_get_arc_num_out(&inh),
            );
            fsm_construct_add_arc_nums(&mut outh, source, newtarget, num_in, num_out);
        } else {
            let (source, num_in, num_out) = (
                fsm_get_arc_source(&inh),
                fsm_get_arc_num_in(&inh),
                fsm_get_arc_num_out(&inh),
            );
            fsm_construct_add_arc_nums(&mut outh, source, target, num_in, num_out);
        }
    }
    for i in 0..statecount {
        fsm_construct_set_final(&mut outh, i);
    }

    fsm_construct_set_initial(&mut outh, 0);
    let net = fsm_read_done(inh);
    /* fsm_read_done(minh) — frees the handle; the marker copy is dropped
    with it (the C caller keeps the original marker) */
    let marker_copy = fsm_read_done(minh);
    drop(marker_copy);
    let newnet = fsm_construct_done(outh);
    fsm_destroy(net);
    /* free(mappings) */
    drop(mappings);
    newnet
}

// [spec:foma:def:constructions.fsm-escape-fn]
// [spec:foma:sem:constructions.fsm-escape-fn]
// [spec:foma:def:fomalib.fsm-escape-fn]
// [spec:foma:sem:fomalib.fsm-escape-fn]
pub fn fsm_escape(symbol: &str) -> Box<Fsm> {
    /* C: fsm_symbol(symbol+1) — skip the first byte (the escape char) */
    fsm_symbol(&symbol[1..])
}

/* Convert a multicharacter-string-containing machine */
/* to the equivalent "letter" machine where all arcs  */
/* are single utf8 letters.                           */

// [spec:foma:def:constructions.fsm-letter-machine-fn]
// [spec:foma:sem:constructions.fsm-letter-machine-fn]
// [spec:foma:def:fomalib.fsm-letter-machine-fn]
// [spec:foma:sem:fomalib.fsm-letter-machine-fn]
pub fn fsm_letter_machine(net: Box<Fsm>) -> Box<Fsm> {
    /* C: char tmpin[128], tmpout[128] — uninitialized stack buffers reused
    across iterations; zero-initialized here (stale bytes persist between
    iterations as in C) */
    let mut tmpin = [0u8; 128];
    let mut tmpout = [0u8; 128];

    // DEVIATION from C (discarded minimize return; C reads net->statecount
    // through the original pointer after fsm_minimize and dangles under
    // Brzozowski — bind the returned Box and continue with it)
    let net = fsm_minimize(net);
    let mut addstate = net.statecount;
    let mut inh = fsm_read_init(Some(net)).unwrap();
    let mut outh = fsm_construct_init("name");

    while fsm_get_next_arc(&mut inh) != 0 {
        let in_full = fsm_get_arc_in(&inh).unwrap().to_string();
        let out_full = fsm_get_arc_out(&inh).unwrap().to_string();
        let innum = fsm_get_arc_num_in(&inh);
        let outnum = fsm_get_arc_num_out(&inh);
        let mut source = fsm_get_arc_source(&inh);
        let mut target = fsm_get_arc_target(&inh);

        if (innum > IDENTITY && utf8strlen(in_full.as_bytes()) > 1)
            || (outnum > IDENTITY && utf8strlen(out_full.as_bytes()) > 1)
        {
            let mut inlen = if innum <= IDENTITY {
                1
            } else {
                utf8strlen(in_full.as_bytes())
            };
            let mut outlen = if outnum <= IDENTITY {
                1
            } else {
                utf8strlen(out_full.as_bytes())
            };
            let steps = if inlen > outlen { inlen } else { outlen };

            /* C: char *in / *out advance through the label bytes — byte
            cursors here */
            let mut in_bytes: &[u8] = in_full.as_bytes();
            let mut out_bytes: &[u8] = out_full.as_bytes();

            target = addstate;
            let mut i = 0;
            while i < steps {
                let currin: String;
                if innum <= IDENTITY || inlen < 1 {
                    if inlen < 1 {
                        currin = "@_EPSILON_SYMBOL_@".to_string();
                    } else {
                        /* special label string repeated at every step */
                        currin = String::from_utf8_lossy(in_bytes).into_owned();
                    }
                } else {
                    /* strncpy(tmpin, in, utf8skip(in)+1);
                    *(tmpin+utf8skip(in)+1) = '\0' */
                    let n = (utf8skip(in_bytes) + 1) as usize;
                    let copy = std::cmp::min(n, in_bytes.len());
                    tmpin[..copy].copy_from_slice(&in_bytes[..copy]);
                    for k in copy..n {
                        tmpin[k] = 0;
                    }
                    tmpin[n] = 0;
                    let end = tmpin.iter().position(|&b| b == 0).unwrap_or(128);
                    currin = String::from_utf8_lossy(&tmpin[..end]).into_owned();
                    inlen -= 1;
                    in_bytes = &in_bytes[n..];
                }
                let currout: String;
                if outnum <= IDENTITY || outlen < 1 {
                    if outlen < 1 {
                        currout = "@_EPSILON_SYMBOL_@".to_string();
                    } else {
                        currout = String::from_utf8_lossy(out_bytes).into_owned();
                    }
                } else {
                    /* C BUG (reproduced): strncpy(tmpout, out, utf8skip(in)+1)
                    sizes the copy by the INPUT cursor's current character
                    (`in` already advanced above), while the NUL terminator is
                    placed at utf8skip(out)+1 — correct only when the input
                    character's encoding is at least as long as the output's */
                    let nbug = (utf8skip(in_bytes) + 1) as usize;
                    let nout = (utf8skip(out_bytes) + 1) as usize;
                    let copy = std::cmp::min(nbug, out_bytes.len());
                    tmpout[..copy].copy_from_slice(&out_bytes[..copy]);
                    for k in copy..nbug {
                        tmpout[k] = 0;
                    }
                    tmpout[nout] = 0;
                    let end = tmpout.iter().position(|&b| b == 0).unwrap_or(128);
                    // DEVIATION from C (stale/garbage buffer bytes may not be
                    // UTF-8; lossy-decoded here — C passes raw bytes through)
                    currout = String::from_utf8_lossy(&tmpout[..end]).into_owned();
                    out_bytes = &out_bytes[nout..];
                    outlen -= 1;
                }
                if i == 0 && steps > 1 {
                    target = addstate;
                    addstate += 1;
                }
                if i > 0 && (steps - i == 1) {
                    source = addstate - 1;
                    target = fsm_get_arc_target(&inh);
                }
                if i > 0 && (steps - i != 1) {
                    source = addstate - 1;
                    target = addstate;
                    addstate += 1;
                }
                fsm_construct_add_arc(&mut outh, source, target, &currin, &currout);
                i += 1;
            }
        } else {
            fsm_construct_add_arc(&mut outh, source, target, &in_full, &out_full);
        }
    }
    loop {
        let i = fsm_get_next_final(&mut inh);
        if i == -1 {
            break;
        }
        fsm_construct_set_final(&mut outh, i);
    }
    loop {
        let i = fsm_get_next_initial(&mut inh);
        if i == -1 {
            break;
        }
        fsm_construct_set_initial(&mut outh, i);
    }
    /* fsm_read_done(inh) — C never fsm_destroy's the minimized input net
    (leaked); the returned Box is dropped here */
    let minimized = fsm_read_done(inh);
    drop(minimized);
    fsm_construct_done(outh)
}

// [spec:foma:def:constructions.fsm-explode-fn]
// [spec:foma:sem:constructions.fsm-explode-fn]
// [spec:foma:def:fomalib.fsm-explode-fn]
// [spec:foma:sem:fomalib.fsm-explode-fn]
pub fn fsm_explode(symbol: &str) -> Box<Fsm> {
    let mut h = fsm_construct_init("");
    fsm_construct_set_initial(&mut h, 0);

    let bytes = symbol.as_bytes();
    let length = bytes.len() as i32 - 2;
    let mut i: i32 = 1;
    let mut j: i32 = 1;
    while i <= length {
        let skip = utf8skip(&bytes[i as usize..]) + 1;
        /* xxstrndup(symbol+i, skip) — stops at the string's end like
        strndup stops at NUL */
        let end = std::cmp::min((i + skip) as usize, bytes.len());
        let tempstring = String::from_utf8_lossy(&bytes[i as usize..end]).into_owned();
        fsm_construct_add_arc(&mut h, j - 1, j, &tempstring, &tempstring);
        /* free(tempstring) — dropped */
        i += skip;
        j += 1;
    }
    fsm_construct_set_final(&mut h, j - 1);
    fsm_construct_done(h)
}

// [spec:foma:def:constructions.fsm-symbol-fn]
// [spec:foma:sem:constructions.fsm-symbol-fn]
// [spec:foma:def:fomalib.fsm-symbol-fn]
// [spec:foma:sem:fomalib.fsm-symbol-fn]
pub fn fsm_symbol(symbol: &str) -> Box<Fsm> {
    let mut net = fsm_create("");
    fsm_update_flags(&mut net, YES, YES, YES, YES, YES, NO);
    if symbol == "@_EPSILON_SYMBOL_@" {
        /* Epsilon */
        sigma_add_special(EPSILON, net.sigma.as_deref_mut().unwrap());
        /* C: malloc(2 lines), uninitialized; written by add_fsm_arc below */
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
        net.arccount = 0;
        net.statecount = 1;
        net.linecount = 1;
        net.finalcount = 1;
        net.is_deterministic = NO;
        net.is_minimized = NO;
        net.is_epsilon_free = NO;
    } else {
        let symbol_no = if symbol == "@_IDENTITY_SYMBOL_@" {
            sigma_add_special(IDENTITY, net.sigma.as_deref_mut().unwrap())
        } else {
            sigma_add(symbol, net.sigma.as_deref_mut().unwrap())
        };
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
        add_fsm_arc(&mut net.states, 0, 0, symbol_no, symbol_no, 1, 0, 1);
        add_fsm_arc(&mut net.states, 1, 1, -1, -1, -1, 1, 0);
        add_fsm_arc(&mut net.states, 2, -1, -1, -1, -1, -1, -1);
        net.arity = 1;
        net.pathcount = 1;
        net.arccount = 1;
        net.statecount = 2;
        net.linecount = 2;
        net.finalcount = 1;
        net.arcs_sorted_in = YES;
        net.arcs_sorted_out = YES;
        net.is_deterministic = YES;
        net.is_minimized = YES;
        net.is_epsilon_free = YES;
    }
    net
}

// [spec:foma:def:constructions.fsm-concat-m-n-fn]
// [spec:foma:sem:constructions.fsm-concat-m-n-fn]
// [spec:foma:def:fomalib.fsm-concat-m-n-fn]
// [spec:foma:sem:fomalib.fsm-concat-m-n-fn]
pub fn fsm_concat_m_n(net1: Box<Fsm>, m: i32, n: i32) -> Box<Fsm> {
    let mut net1 = net1;
    let mut acc = fsm_empty_string();
    let mut i = 1;
    while i <= n {
        if i > m {
            acc = fsm_concat(acc, fsm_optionality(fsm_copy(&mut net1)));
        } else {
            acc = fsm_concat(acc, fsm_copy(&mut net1));
        }
        i += 1;
    }
    fsm_destroy(net1);
    acc
}

// [spec:foma:def:constructions.fsm-concat-n-fn]
// [spec:foma:sem:constructions.fsm-concat-n-fn]
// [spec:foma:def:fomalib.fsm-concat-n-fn]
// [spec:foma:sem:fomalib.fsm-concat-n-fn]
pub fn fsm_concat_n(net1: Box<Fsm>, n: i32) -> Box<Fsm> {
    fsm_concat_m_n(net1, n, n)
}

// [spec:foma:def:constructions.fsm-network-to-char-fn]
// [spec:foma:sem:constructions.fsm-network-to-char-fn]
// [spec:foma:def:fomalib.fsm-network-to-char-fn]
// [spec:foma:sem:fomalib.fsm-network-to-char-fn]
pub fn fsm_network_to_char(net: &Fsm) -> Option<String> {
    /* C crashes if net->sigma is NULL (cannot happen via fsm_create) —
    unwrap panics likewise */
    let mut sigma = net.sigma.as_deref();
    if sigma.unwrap().number == -1 {
        return None;
    }
    let mut sigprev: Option<&Sigma> = None;
    while let Some(s) = sigma {
        if s.number == -1 {
            break;
        }
        sigprev = Some(s);
        sigma = s.next.as_deref();
    }
    /* strdup(sigprev->symbol) */
    Some(sigprev.unwrap().symbol.as_deref().unwrap().to_string())
}

// [spec:foma:def:constructions.fsm-substitute-label-fn]
// [spec:foma:sem:constructions.fsm-substitute-label-fn]
// [spec:foma:def:fomalib.fsm-substitute-label-fn]
// [spec:foma:sem:fomalib.fsm-substitute-label-fn]
pub fn fsm_substitute_label(net: &mut Fsm, original: &str, substitute: &mut Fsm) -> Box<Fsm> {
    fsm_merge_sigma(net, substitute);
    let mut addstate1 = net.statecount;
    let addstate2 = substitute.statecount;

    /* C: the read handles borrow net and substitute (NEITHER is consumed
    on any path); the Rust handles own deep copies — read-only, observably
    equivalent */
    let mut inh = fsm_read_init(Some(Box::new(net.clone()))).unwrap();
    let mut subh = fsm_read_init(Some(Box::new(substitute.clone()))).unwrap();
    let repsym = fsm_get_symbol_number(&inh, original);
    if repsym == -1 {
        /* fsm_read_done(inh) — subh and the substitute handle are leaked
        in C (dropped here) */
        let _ = fsm_read_done(inh);
        // DEVIATION from C (C returns the input net aliased; a deep copy here)
        return Box::new(net.clone());
    }
    let name = net.name.clone();
    let mut outh = fsm_construct_init(&name);
    fsm_construct_copy_sigma(&mut outh, net.sigma.as_deref());
    while fsm_get_next_arc(&mut inh) != 0 {
        let mut source = fsm_get_arc_source(&inh);
        let mut target = fsm_get_arc_target(&inh);
        let r#in = fsm_get_arc_num_in(&inh);
        let out = fsm_get_arc_num_out(&inh);

        /* Double-sided arc, splice in substitute network */
        if r#in == repsym && out == repsym {
            fsm_read_reset(Some(&mut subh));
            fsm_construct_add_arc_nums(&mut outh, source, addstate1, EPSILON, EPSILON);
            while fsm_get_next_arc(&mut subh) != 0 {
                source = fsm_get_arc_source(&subh);
                target = fsm_get_arc_target(&subh);
                let subin = fsm_get_arc_in(&subh).unwrap().to_string();
                let subout = fsm_get_arc_out(&subh).unwrap().to_string();
                fsm_construct_add_arc(
                    &mut outh,
                    source + addstate1,
                    target + addstate1,
                    &subin,
                    &subout,
                );
            }
            loop {
                let i = fsm_get_next_final(&mut subh);
                if i == -1 {
                    break;
                }
                target = fsm_get_arc_target(&inh);
                fsm_construct_add_arc_nums(&mut outh, addstate1 + i, target, EPSILON, EPSILON);
            }
            addstate1 = addstate1 + addstate2;
            /* One-sided replace, splice in repsym .x. sub or sub .x. repsym */
        } else if r#in == repsym || out == repsym {
            let subnet2 = if r#in == repsym {
                let outlabel = fsm_get_arc_out(&inh).unwrap().to_string();
                fsm_minimize(fsm_cross_product(
                    fsm_copy(substitute),
                    fsm_symbol(&outlabel),
                ))
            } else {
                let inlabel = fsm_get_arc_in(&inh).unwrap().to_string();
                fsm_minimize(fsm_cross_product(
                    fsm_symbol(&inlabel),
                    fsm_copy(substitute),
                ))
            };
            fsm_construct_add_arc_nums(&mut outh, source, addstate1, EPSILON, EPSILON);
            let mut subh2 = fsm_read_init(Some(subnet2)).unwrap();
            while fsm_get_next_arc(&mut subh2) != 0 {
                source = fsm_get_arc_source(&subh2);
                target = fsm_get_arc_target(&subh2);
                let subin = fsm_get_arc_in(&subh2).unwrap().to_string();
                let subout = fsm_get_arc_out(&subh2).unwrap().to_string();
                fsm_construct_add_arc(
                    &mut outh,
                    source + addstate1,
                    target + addstate1,
                    &subin,
                    &subout,
                );
            }
            loop {
                let i = fsm_get_next_final(&mut subh2);
                if i == -1 {
                    break;
                }
                target = fsm_get_arc_target(&inh);
                fsm_construct_add_arc_nums(&mut outh, addstate1 + i, target, EPSILON, EPSILON);
            }
            let subnet2 = fsm_read_done(subh2);
            addstate1 = addstate1 + subnet2.statecount;
            fsm_destroy(subnet2);
        } else {
            /* Default, just copy arc */
            fsm_construct_add_arc_nums(&mut outh, source, target, r#in, out);
        }
    }

    loop {
        let i = fsm_get_next_final(&mut inh);
        if i == -1 {
            break;
        }
        fsm_construct_set_final(&mut outh, i);
    }
    loop {
        let i = fsm_get_next_initial(&mut inh);
        if i == -1 {
            break;
        }
        fsm_construct_set_initial(&mut outh, i);
    }
    let _ = fsm_read_done(inh);
    let _ = fsm_read_done(subh);
    fsm_construct_done(outh)
}

// [spec:foma:def:constructions.fsm-substitute-symbol-fn]
// [spec:foma:sem:constructions.fsm-substitute-symbol-fn]
// [spec:foma:def:fomalib.fsm-substitute-symbol-fn]
// [spec:foma:sem:fomalib.fsm-substitute-symbol-fn]
pub fn fsm_substitute_symbol(net: Box<Fsm>, original: &str, substitute: &str) -> Box<Fsm> {
    let mut net = net;
    if original == substitute {
        return net;
    }
    let o = sigma_find(original, net.sigma.as_deref());
    if o == -1 {
        //fprintf(stderr, "\nSymbol '%s' not found in network!\n", original);
        return net;
    }
    let s: i32;
    if substitute == "0" {
        s = EPSILON;
    } else {
        /* C: substitute != NULL && (s = sigma_find(...)) == -1 → sigma_add
        (substitute is never NULL here) */
        let found = sigma_find(substitute, net.sigma.as_deref());
        s = if found == -1 {
            sigma_add(substitute, net.sigma.as_deref_mut().unwrap())
        } else {
            found
        };
    }
    let mut i = 0usize;
    while net.states[i].state_no != -1 {
        if net.states[i].r#in as i32 == o {
            net.states[i].r#in = s as i16;
        }
        if net.states[i].out as i32 == o {
            net.states[i].out = s as i16;
        }
        i += 1;
    }
    net.sigma = sigma_remove(original, net.sigma.take());
    sigma_sort(&mut net);
    fsm_update_flags(&mut net, NO, NO, NO, NO, NO, NO);
    sigma_cleanup(&mut net, 0);
    /* if s = epsilon */
    net.is_minimized = NO;
    fsm_determinize(net)
}

// [spec:foma:def:constructions.fsm-precedes-fn]
// [spec:foma:sem:constructions.fsm-precedes-fn]
// [spec:foma:def:fomalib.fsm-precedes-fn]
// [spec:foma:sem:fomalib.fsm-precedes-fn]
pub fn fsm_precedes(net1: &mut Fsm, net2: &mut Fsm) -> Box<Fsm> {
    /* Neither net1 nor net2 is consumed (copies only) */
    fsm_complement(fsm_minimize(fsm_contains(fsm_minimize(fsm_concat(
        fsm_minimize(fsm_copy(net2)),
        fsm_concat(fsm_universal(), fsm_minimize(fsm_copy(net1))),
    )))))
}

// [spec:foma:def:constructions.fsm-follows-fn]
// [spec:foma:sem:constructions.fsm-follows-fn]
// [spec:foma:def:fomalib.fsm-follows-fn]
// [spec:foma:sem:fomalib.fsm-follows-fn]
pub fn fsm_follows(net1: &mut Fsm, net2: &mut Fsm) -> Box<Fsm> {
    /* Neither net1 nor net2 is consumed (copies only) */
    fsm_complement(fsm_minimize(fsm_contains(fsm_minimize(fsm_concat(
        fsm_minimize(fsm_copy(net1)),
        fsm_concat(fsm_universal(), fsm_minimize(fsm_copy(net2))),
    )))))
}

// [spec:foma:def:constructions.fsm-unflatten-fn]
// [spec:foma:sem:constructions.fsm-unflatten-fn]
// [spec:foma:def:fomalib.fsm-unflatten-fn]
// [spec:foma:sem:fomalib.fsm-unflatten-fn]
pub fn fsm_unflatten(net: Box<Fsm>, epsilon_sym: &str, repeat_sym: &str) -> Box<Fsm> {
    // DEVIATION from C (discarded minimize return; C dangles under Brzozowski)
    let mut net = fsm_minimize(net);
    fsm_count(&mut net);

    let epsilon = sigma_find(epsilon_sym, net.sigma.as_deref());
    let repeat = sigma_find(repeat_sym, net.sigma.as_deref());

    /* new state 0 = {0,0} */

    /* STACK_2_PUSH(0,0) */
    int_stack_push(0);
    int_stack_push(0);

    let mut th = triplet_hash_init();
    triplet_hash_insert(&mut th, 0, 0, 0);

    fsm_state_init(sigma_max(net.sigma.as_deref()));

    let point_a = init_state_pointers(&net.states);

    while int_stack_isempty() == 0 {
        /* Get a pair of states to examine */

        /* C: both pops are assigned to a (pairs are always (s,s)) */
        let mut a = int_stack_pop();
        a = int_stack_pop();

        let current_state = triplet_hash_find(&th, a, a, 0);
        let current_start = if point_a[a as usize].start == 1 { 1 } else { 0 };
        let current_final = if point_a[a as usize].r#final == 1 { 1 } else { 0 };

        fsm_state_set_current_state(current_state, current_final, current_start);

        let mut ei = point_a[a as usize].transitions;
        while net.states[ei].state_no == a {
            if net.states[ei].target == -1 {
                ei += 1;
                continue;
            }
            let b = net.states[ei].target;
            let mut oi = point_a[b as usize].transitions;
            while net.states[oi].state_no == b {
                if net.states[oi].target == -1 {
                    oi += 1;
                    continue;
                }
                let odd_target = net.states[oi].target;
                let mut target_number = triplet_hash_find(&th, odd_target, odd_target, 0);
                if target_number == -1 {
                    /* STACK_2_PUSH(odd_state->target, odd_state->target) */
                    int_stack_push(odd_target);
                    int_stack_push(odd_target);
                    target_number = triplet_hash_insert(&mut th, odd_target, odd_target, 0);
                }
                let mut r#in = net.states[ei].r#in as i32;
                let mut out = net.states[oi].r#in as i32;
                if out == repeat {
                    out = r#in;
                } else if r#in == IDENTITY || out == IDENTITY {
                    r#in = if r#in == IDENTITY { UNKNOWN } else { r#in };
                    out = if out == IDENTITY { UNKNOWN } else { out };
                }
                if r#in == epsilon {
                    r#in = EPSILON;
                }
                if out == epsilon {
                    out = EPSILON;
                }
                fsm_state_add_arc(
                    current_state,
                    r#in,
                    out,
                    target_number,
                    current_final,
                    current_start,
                );
                oi += 1;
            }
            ei += 1;
        }
        fsm_state_end_state();
    }
    /* free(net->states) */
    net.states = Vec::new();
    fsm_state_close(&mut net);
    /* free(point_a) */
    drop(point_a);
    triplet_hash_free(Some(th));
    net
}

// [spec:foma:def:constructions.fsm-shuffle-fn]
// [spec:foma:sem:constructions.fsm-shuffle-fn]
// [spec:foma:def:fomalib.fsm-shuffle-fn]
// [spec:foma:sem:fomalib.fsm-shuffle-fn]
pub fn fsm_shuffle(net1: Box<Fsm>, net2: Box<Fsm>) -> Box<Fsm> {
    /* Shuffle A and B by making alternatively A move and B stay at each or */
    /* vice versa at each step */

    // DEVIATION from C (discarded minimize returns; C dangles under Brzozowski)
    let mut net1 = fsm_minimize(net1);
    let mut net2 = fsm_minimize(net2);

    fsm_merge_sigma(&mut net1, &mut net2);

    fsm_count(&mut net1);
    fsm_count(&mut net2);

    /* new state 0 = {0,0} */

    /* STACK_2_PUSH(0,0) */
    int_stack_push(0);
    int_stack_push(0);

    let mut th = triplet_hash_init();
    triplet_hash_insert(&mut th, 0, 0, 0);

    fsm_state_init(sigma_max(net1.sigma.as_deref()));

    let point_a = init_state_pointers(&net1.states);
    let point_b = init_state_pointers(&net2.states);

    while int_stack_isempty() == 0 {
        /* Get a pair of states to examine */

        let a = int_stack_pop();
        let b = int_stack_pop();

        /* printf("Treating pair: {%i,%i}\n",a,b); */

        let current_state = triplet_hash_find(&th, a, b, 0);
        let current_start = if point_a[a as usize].start == 1 && point_b[b as usize].start == 1 {
            1
        } else {
            0
        };
        let current_final = if point_a[a as usize].r#final == 1 && point_b[b as usize].r#final == 1
        {
            1
        } else {
            0
        };

        fsm_state_set_current_state(current_state, current_final, current_start);

        /* Follow A, B stays */
        let mut ai = point_a[a as usize].transitions;
        while net1.states[ai].state_no == a {
            if net1.states[ai].target == -1 {
                ai += 1;
                continue;
            }
            let atarget = net1.states[ai].target;
            let mut target_number = triplet_hash_find(&th, atarget, b, 0);
            if target_number == -1 {
                /* STACK_2_PUSH(b, machine_a->target) */
                int_stack_push(b);
                int_stack_push(atarget);
                target_number = triplet_hash_insert(&mut th, atarget, b, 0);
            }
            let (ain, aout) = (net1.states[ai].r#in as i32, net1.states[ai].out as i32);
            fsm_state_add_arc(
                current_state,
                ain,
                aout,
                target_number,
                current_final,
                current_start,
            );
            ai += 1;
        }

        /* Follow B, A stays */
        let mut bi = point_b[b as usize].transitions;
        while net2.states[bi].state_no == b {
            if net2.states[bi].target == -1 {
                bi += 1;
                continue;
            }
            let btarget = net2.states[bi].target;
            let mut target_number = triplet_hash_find(&th, a, btarget, 0);
            if target_number == -1 {
                /* STACK_2_PUSH(machine_b->target, a) */
                int_stack_push(btarget);
                int_stack_push(a);
                target_number = triplet_hash_insert(&mut th, a, btarget, 0);
            }
            let (bin, bout) = (net2.states[bi].r#in as i32, net2.states[bi].out as i32);
            fsm_state_add_arc(
                current_state,
                bin,
                bout,
                target_number,
                current_final,
                current_start,
            );
            bi += 1;
        }

        /* Check arctrack */
        fsm_state_end_state();
    }

    /* free(net1->states) */
    net1.states = Vec::new();
    fsm_state_close(&mut net1);
    /* free(point_a); free(point_b) */
    drop(point_a);
    drop(point_b);
    fsm_destroy(net2);
    triplet_hash_free(Some(th));
    net1
}

// [spec:foma:def:constructions.fsm-equivalent-fn]
// [spec:foma:sem:constructions.fsm-equivalent-fn]
// [spec:foma:def:fomalib.fsm-equivalent-fn]
// [spec:foma:sem:fomalib.fsm-equivalent-fn]
pub fn fsm_equivalent(net1: Box<Fsm>, net2: Box<Fsm>) -> i32 {
    /* Test path equivalence of two FSMs by traversing both in parallel */
    let mut net1 = net1;
    let mut net2 = net2;

    fsm_merge_sigma(&mut net1, &mut net2);

    fsm_count(&mut net1);
    fsm_count(&mut net2);

    let mut equivalent = 0;
    /* new state 0 = {0,0} */
    /* STACK_2_PUSH(0,0) */
    int_stack_push(0);
    int_stack_push(0);

    let mut th = triplet_hash_init();
    triplet_hash_insert(&mut th, 0, 0, 0);

    let point_a = init_state_pointers(&net1.states);
    let point_b = init_state_pointers(&net2.states);

    /* C: goto not_equivalent — labelled block with the same target */
    'not_equivalent: {
        while int_stack_isempty() == 0 {
            /* Get a pair of states to examine */

            let a = int_stack_pop();
            let b = int_stack_pop();

            if point_a[a as usize].r#final != point_b[b as usize].r#final {
                break 'not_equivalent;
            }
            /* Check that all arcs in A have matching arc in B, push new state pair on stack */
            let mut ai = point_a[a as usize].transitions;
            while net1.states[ai].state_no == a {
                if net1.states[ai].target == -1 {
                    break;
                }
                let mut matching_arc = 0;
                let mut bi = point_b[b as usize].transitions;
                while net2.states[bi].state_no == b {
                    if net2.states[bi].target == -1 {
                        break;
                    }
                    if net1.states[ai].r#in == net2.states[bi].r#in
                        && net1.states[ai].out == net2.states[bi].out
                    {
                        matching_arc = 1;
                        let (atarget, btarget) = (net1.states[ai].target, net2.states[bi].target);
                        if triplet_hash_find(&th, atarget, btarget, 0) == -1 {
                            /* STACK_2_PUSH(machine_b->target, machine_a->target) */
                            int_stack_push(btarget);
                            int_stack_push(atarget);
                            triplet_hash_insert(&mut th, atarget, btarget, 0);
                        }
                        break;
                    }
                    bi += 1;
                }
                if matching_arc == 0 {
                    break 'not_equivalent;
                }
                ai += 1;
            }
            let mut bi = point_b[b as usize].transitions;
            while net2.states[bi].state_no == b {
                if net2.states[bi].target == -1 {
                    break;
                }
                let mut matching_arc = 0;
                let mut ai = point_a[a as usize].transitions;
                while net1.states[ai].state_no == a {
                    if net1.states[ai].r#in == net2.states[bi].r#in
                        && net1.states[ai].out == net2.states[bi].out
                    {
                        matching_arc = 1;
                        break;
                    }
                    ai += 1;
                }
                if matching_arc == 0 {
                    break 'not_equivalent;
                }
                bi += 1;
            }
        }
        equivalent = 1;
    }
    fsm_destroy(net1);
    fsm_destroy(net2);
    /* free(point_a); free(point_b) */
    drop(point_a);
    drop(point_b);
    triplet_hash_free(Some(th));
    equivalent
}

// [spec:foma:def:constructions.fsm-contains-fn]
// [spec:foma:sem:constructions.fsm-contains-fn]
// [spec:foma:def:fomalib.fsm-contains-fn]
// [spec:foma:sem:fomalib.fsm-contains-fn]
pub fn fsm_contains(net: Box<Fsm>) -> Box<Fsm> {
    /* [?* A ?*] */
    fsm_concat(fsm_concat(fsm_universal(), net), fsm_universal())
}

// [spec:foma:def:constructions.fsm-universal-fn]
// [spec:foma:sem:constructions.fsm-universal-fn]
// [spec:foma:def:fomalib.fsm-universal-fn]
// [spec:foma:sem:fomalib.fsm-universal-fn]
pub fn fsm_universal() -> Box<Fsm> {
    let mut net = fsm_create("");
    fsm_update_flags(&mut net, YES, YES, YES, YES, NO, NO);
    /* C: malloc(2 lines), uninitialized; written by add_fsm_arc below */
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
    let s = sigma_add_special(IDENTITY, net.sigma.as_deref_mut().unwrap());
    add_fsm_arc(&mut net.states, 0, 0, s, s, 0, 1, 1);
    add_fsm_arc(&mut net.states, 1, -1, -1, -1, -1, -1, -1);
    net.arccount = 1;
    net.statecount = 1;
    net.linecount = 2;
    net.finalcount = 1;
    net.pathcount = PATHCOUNT_CYCLIC;
    net
}

// [spec:foma:def:constructions.fsm-contains-one-fn]
// [spec:foma:sem:constructions.fsm-contains-one-fn]
// [spec:foma:def:fomalib.fsm-contains-one-fn]
// [spec:foma:sem:fomalib.fsm-contains-one-fn]
pub fn fsm_contains_one(net: Box<Fsm>) -> Box<Fsm> {
    /* $A - $[[?+ A ?* & A ?*] | [A ?+ & A]] */
    let mut net = net;
    let ret = fsm_minus(
        fsm_contains(fsm_copy(&mut net)),
        fsm_contains(fsm_union(
            fsm_intersect(
                fsm_concat(
                    fsm_kleene_plus(fsm_identity()),
                    fsm_concat(fsm_copy(&mut net), fsm_universal()),
                ),
                fsm_concat(fsm_copy(&mut net), fsm_universal()),
            ),
            fsm_intersect(
                fsm_concat(fsm_copy(&mut net), fsm_kleene_plus(fsm_identity())),
                fsm_copy(&mut net),
            ),
        )),
    );
    fsm_destroy(net);
    ret
}

// [spec:foma:def:constructions.fsm-contains-opt-one-fn]
// [spec:foma:sem:constructions.fsm-contains-opt-one-fn]
// [spec:foma:def:fomalib.fsm-contains-opt-one-fn]
// [spec:foma:sem:fomalib.fsm-contains-opt-one-fn]
pub fn fsm_contains_opt_one(net: Box<Fsm>) -> Box<Fsm> {
    /* $.A | ~$A */
    let mut net = net;
    let ret = fsm_union(
        fsm_contains_one(fsm_copy(&mut net)),
        fsm_complement(fsm_contains(fsm_copy(&mut net))),
    );
    fsm_destroy(net);
    ret
}

// [spec:foma:def:constructions.fsm-simple-replace-fn]
// [spec:foma:sem:constructions.fsm-simple-replace-fn]
// [spec:foma:def:fomalib.fsm-simple-replace-fn]
// [spec:foma:sem:fomalib.fsm-simple-replace-fn]
pub fn fsm_simple_replace(net1: Box<Fsm>, net2: Box<Fsm>) -> Box<Fsm> {
    /* [~[?* [A-0] ?*] [A.x.B]]* ~[?* [A-0] ?*] */

    let mut net1 = net1;
    let mut net2 = net2;
    let mut uplus = fsm_minimize(fsm_kleene_plus(fsm_identity()));
    let ret = fsm_concat(
        fsm_minimize(fsm_kleene_star(fsm_minimize(fsm_concat(
            fsm_complement(fsm_minimize(fsm_concat(
                fsm_concat(
                    fsm_universal(),
                    fsm_minimize(fsm_intersect(fsm_copy(&mut net1), fsm_copy(&mut uplus))),
                ),
                fsm_universal(),
            ))),
            fsm_minimize(fsm_cross_product(fsm_copy(&mut net1), fsm_copy(&mut net2))),
        )))),
        fsm_minimize(fsm_complement(fsm_minimize(fsm_concat(
            fsm_concat(
                fsm_universal(),
                fsm_intersect(fsm_copy(&mut net1), fsm_copy(&mut uplus)),
            ),
            fsm_universal(),
        )))),
    );
    fsm_destroy(net1);
    fsm_destroy(net2);
    fsm_destroy(uplus);
    ret
}

// [spec:foma:def:constructions.fsm-priority-union-upper-fn]
// [spec:foma:sem:constructions.fsm-priority-union-upper-fn]
// [spec:foma:def:fomalib.fsm-priority-union-upper-fn]
// [spec:foma:sem:fomalib.fsm-priority-union-upper-fn]
pub fn fsm_priority_union_upper(net1: Box<Fsm>, net2: Box<Fsm>) -> Box<Fsm> {
    /* A .P. B = A | [~[A.u] .o. B] */
    let mut net1 = net1;
    let ret = fsm_union(
        fsm_copy(&mut net1),
        fsm_compose(fsm_complement(fsm_upper(fsm_copy(&mut net1))), net2),
    );
    fsm_destroy(net1);
    ret
}

// [spec:foma:def:constructions.fsm-priority-union-lower-fn]
// [spec:foma:sem:constructions.fsm-priority-union-lower-fn]
// [spec:foma:def:fomalib.fsm-priority-union-lower-fn]
// [spec:foma:sem:fomalib.fsm-priority-union-lower-fn]
pub fn fsm_priority_union_lower(net1: Box<Fsm>, net2: Box<Fsm>) -> Box<Fsm> {
    /* A .p. B = A | B .o. ~[A.l] */
    let mut net1 = net1;
    let ret = fsm_union(
        fsm_copy(&mut net1),
        fsm_compose(net2, fsm_complement(fsm_lower(fsm_copy(&mut net1)))),
    );
    fsm_destroy(net1);
    ret
}

// [spec:foma:def:constructions.fsm-lenient-compose-fn]
// [spec:foma:sem:constructions.fsm-lenient-compose-fn]
// [spec:foma:def:fomalib.fsm-lenient-compose-fn]
// [spec:foma:sem:fomalib.fsm-lenient-compose-fn]
pub fn fsm_lenient_compose(net1: Box<Fsm>, net2: Box<Fsm>) -> Box<Fsm> {
    /* A .O. B = [A .o. B] .P. B */
    /* NOTE: the C comment above (reproduced) claims .P. B, but the code
    passes a COPY OF NET1 (A) as the fallback — [A .o. B] .P. A. Port of
    the code, not the comment. */
    let mut net1 = net1;
    let ret = fsm_priority_union_upper(
        fsm_compose(fsm_copy(&mut net1), net2),
        fsm_copy(&mut net1),
    );
    fsm_destroy(net1);
    ret
}

// [spec:foma:def:constructions.fsm-term-negation-fn]
// [spec:foma:sem:constructions.fsm-term-negation-fn]
// [spec:foma:def:fomalib.fsm-term-negation-fn]
// [spec:foma:sem:fomalib.fsm-term-negation-fn]
pub fn fsm_term_negation(net1: Box<Fsm>) -> Box<Fsm> {
    fsm_intersect(fsm_identity(), fsm_complement(net1))
}

// [spec:foma:def:constructions.fsm-quotient-interleave-fn]
// [spec:foma:sem:constructions.fsm-quotient-interleave-fn]
// [spec:foma:def:fomalib.fsm-quotient-interleave-fn]
// [spec:foma:sem:fomalib.fsm-quotient-interleave-fn]
pub fn fsm_quotient_interleave(net1: Box<Fsm>, net2: Box<Fsm>) -> Box<Fsm> {
    /* A/\/B = The set of strings you can interleave in B and get a string from A */
    /* [B/[x \x* x] & A/x .o. [[[\x]:0]* (x:0 \x* x:0)]*].l */
    let mut result = fsm_lower(fsm_compose(
        fsm_intersect(
            fsm_ignore(
                net2,
                fsm_concat(
                    fsm_symbol("@>@"),
                    fsm_concat(
                        fsm_kleene_star(fsm_term_negation(fsm_symbol("@>@"))),
                        fsm_symbol("@>@"),
                    ),
                ),
                OP_IGNORE_ALL,
            ),
            fsm_ignore(net1, fsm_symbol("@>@"), OP_IGNORE_ALL),
        ),
        fsm_kleene_star(fsm_concat(
            fsm_kleene_star(fsm_cross_product(
                fsm_term_negation(fsm_symbol("@>@")),
                fsm_empty_string(),
            )),
            fsm_optionality(fsm_concat(
                fsm_cross_product(fsm_symbol("@>@"), fsm_empty_string()),
                fsm_concat(
                    fsm_kleene_star(fsm_term_negation(fsm_symbol("@>@"))),
                    fsm_cross_product(fsm_symbol("@>@"), fsm_empty_string()),
                ),
            )),
        )),
    ));

    result.sigma = sigma_remove("@>@", result.sigma.take());
    /* Could clean up sigma */
    result
}

// [spec:foma:def:constructions.fsm-quotient-left-fn]
// [spec:foma:sem:constructions.fsm-quotient-left-fn]
// [spec:foma:def:fomalib.fsm-quotient-left-fn]
// [spec:foma:sem:fomalib.fsm-quotient-left-fn]
pub fn fsm_quotient_left(net1: Box<Fsm>, net2: Box<Fsm>) -> Box<Fsm> {
    /* A\\\B = [B .o. A:0 ?*].l; */
    /* A\\\B = the set of suffixes you can add to A to get a string in B */
    fsm_lower(fsm_compose(
        net2,
        fsm_concat(
            fsm_cross_product(net1, fsm_empty_string()),
            fsm_universal(),
        ),
    ))
}

// [spec:foma:def:constructions.fsm-quotient-right-fn]
// [spec:foma:sem:constructions.fsm-quotient-right-fn]
// [spec:foma:def:fomalib.fsm-quotient-right-fn]
// [spec:foma:sem:fomalib.fsm-quotient-right-fn]
pub fn fsm_quotient_right(net1: Box<Fsm>, net2: Box<Fsm>) -> Box<Fsm> {
    /* A///B = [A .o. ?* B:0].l; */
    /* A///B = the set of prefixes you can add to B to get strings in A */
    fsm_lower(fsm_compose(
        net1,
        fsm_concat(
            fsm_universal(),
            fsm_cross_product(net2, fsm_empty_string()),
        ),
    ))
}

// [spec:foma:def:constructions.fsm-ignore-fn]
// [spec:foma:sem:constructions.fsm-ignore-fn]
// [spec:foma:def:fomalib.fsm-ignore-fn]
// [spec:foma:sem:fomalib.fsm-ignore-fn]
pub fn fsm_ignore(net1: Box<Fsm>, net2: Box<Fsm>, operation: i32) -> Box<Fsm> {
    let mut net1 = fsm_minimize(net1);
    let mut net2 = fsm_minimize(net2);

    if fsm_isempty(&mut net2) != 0 {
        fsm_destroy(net2);
        return net1;
    }
    fsm_merge_sigma(&mut net1, &mut net2);

    fsm_count(&mut net1);
    fsm_count(&mut net2);

    let states1 = net1.statecount;
    let states2 = net2.statecount;
    let lines1 = net1.linecount;
    let lines2 = net2.linecount;

    if operation == OP_IGNORE_INTERNAL {
        let mut result = fsm_lower(fsm_compose(
            fsm_ignore(fsm_copy(&mut net1), fsm_symbol("@i<@"), OP_IGNORE_ALL),
            fsm_compose(
                fsm_complement(fsm_union(
                    fsm_concat(fsm_symbol("@i<@"), fsm_universal()),
                    fsm_concat(fsm_universal(), fsm_symbol("@i<@")),
                )),
                fsm_simple_replace(fsm_symbol("@i<@"), fsm_copy(&mut net2)),
            ),
        ));
        result.sigma = sigma_remove("@i<@", result.sigma.take());
        fsm_destroy(net1);
        fsm_destroy(net2);
        return result;
    }

    let malloc_size = lines1 + states1 * (lines2 + net2.finalcount + 1);
    /* C: malloc'd (uninitialized); zeroed lines here */
    let mut new_fsm: Vec<FsmState> = vec![
        FsmState {
            state_no: 0,
            r#in: 0,
            out: 0,
            target: 0,
            final_state: 0,
            start_state: 0,
        };
        (malloc_size + 1) as usize
    ];

    /* Mark if a state has been handled with ignore */
    /* C: malloc'd (uninitialized); handled_states1 is initialized below,
    handled_states2 is re-zeroed per splice — zero-filled here */
    let mut handled_states1: Vec<i16> = vec![0; states1 as usize];
    let mut handled_states2: Vec<i16> = vec![0; states2 as usize];

    /* Mark which ignores return to which state */
    let mut return_state: Vec<i32> = vec![0; states1 as usize];
    let splice_size = states2;
    let start_splice = states1;
    for k in 0..states1 {
        handled_states1[k as usize] = 0;
    }

    let mut splices = 0;
    let mut j: i32 = 0;
    let mut i = 0usize;
    while net1.states[i].state_no != -1 {
        if handled_states1[net1.states[i].state_no as usize] == 0 {
            let target = start_splice + splices * splice_size;
            let (state_no, final_state, start_state) = (
                net1.states[i].state_no,
                net1.states[i].final_state as i32,
                net1.states[i].start_state as i32,
            );
            add_fsm_arc(
                &mut new_fsm,
                j,
                state_no,
                EPSILON,
                EPSILON,
                target,
                final_state,
                start_state,
            );
            return_state[splices as usize] = state_no;
            handled_states1[state_no as usize] = 1;
            j += 1;
            splices += 1;
            if net1.states[i].r#in != -1 {
                let (line_in, line_out, tgt) = (
                    net1.states[i].r#in as i32,
                    net1.states[i].out as i32,
                    net1.states[i].target,
                );
                add_fsm_arc(
                    &mut new_fsm,
                    j,
                    state_no,
                    line_in,
                    line_out,
                    tgt,
                    final_state,
                    start_state,
                );
                j += 1;
            }
        } else {
            let (state_no, line_in, line_out, tgt, final_state, start_state) = (
                net1.states[i].state_no,
                net1.states[i].r#in as i32,
                net1.states[i].out as i32,
                net1.states[i].target,
                net1.states[i].final_state as i32,
                net1.states[i].start_state as i32,
            );
            add_fsm_arc(
                &mut new_fsm,
                j,
                state_no,
                line_in,
                line_out,
                tgt,
                final_state,
                start_state,
            );
            j += 1;
        }
        i += 1;
    }

    /* Add a sequence of fsm2s at the end, with arcs back to the appropriate states */

    let mut state_add_counter = start_splice;

    let mut returns = 0;
    while splices > 0 {
        /* Zero handled return arc states */

        for k in 0..states2 {
            handled_states2[k as usize] = 0;
        }

        let mut i = 0usize;
        while net2.states[i].state_no != -1 {
            if net2.states[i].final_state == 1
                && handled_states2[net2.states[i].state_no as usize] == 0
            {
                let state_no = net2.states[i].state_no;
                add_fsm_arc(
                    &mut new_fsm,
                    j,
                    state_no + state_add_counter,
                    EPSILON,
                    EPSILON,
                    return_state[returns as usize],
                    0,
                    0,
                );
                j += 1;
                handled_states2[state_no as usize] = 1;
                if net2.states[i].target != -1 {
                    let (line_in, line_out, tgt) = (
                        net2.states[i].r#in as i32,
                        net2.states[i].out as i32,
                        net2.states[i].target,
                    );
                    add_fsm_arc(
                        &mut new_fsm,
                        j,
                        state_no + state_add_counter,
                        line_in,
                        line_out,
                        tgt + state_add_counter,
                        0,
                        0,
                    );
                    j += 1;
                }
            } else {
                /* C: target shifted unconditionally — a target of -1 would
                become a bogus state number (cannot occur for minimized
                net2); reproduced literally */
                let (state_no, line_in, line_out, tgt) = (
                    net2.states[i].state_no,
                    net2.states[i].r#in as i32,
                    net2.states[i].out as i32,
                    net2.states[i].target,
                );
                add_fsm_arc(
                    &mut new_fsm,
                    j,
                    state_no + state_add_counter,
                    line_in,
                    line_out,
                    tgt + state_add_counter,
                    0,
                    0,
                );
                j += 1;
            }
            i += 1;
        }
        state_add_counter = state_add_counter + states2;
        splices -= 1;
        returns += 1;
    }

    add_fsm_arc(&mut new_fsm, j, -1, -1, -1, -1, -1, -1);
    /* free(handled_states1); free(handled_states2); free(return_state) */
    drop(handled_states1);
    drop(handled_states2);
    drop(return_state);
    /* free(net1->states) */
    fsm_destroy(net2);
    net1.states = new_fsm;
    fsm_update_flags(&mut net1, NO, NO, NO, NO, NO, NO);
    fsm_count(&mut net1);
    net1
}

/* Remove those symbols from sigma that have the same distribution as IDENTITY */

// [spec:foma:def:constructions.fsm-compact-fn]
// [spec:foma:sem:constructions.fsm-compact-fn]
// [spec:foma:def:fomalib.fsm-compact-fn]
// [spec:foma:sem:fomalib.fsm-compact-fn]
pub fn fsm_compact(net: &mut Fsm) {
    /* C: struct checktable { int state_no; int target; } — function-local */
    #[derive(Clone)]
    struct Checktable {
        state_no: i32,
        target: i32,
    }

    let numsymbols = sigma_max(net.sigma.as_deref());

    /* C: malloc'd (uninitialized); every entry initialized just below */
    let mut potential: Vec<bool> = vec![false; (numsymbols + 1) as usize];
    let mut checktable: Vec<Checktable> = vec![
        Checktable {
            state_no: 0,
            target: 0,
        };
        (numsymbols + 1) as usize
    ];

    let mut i: i32 = 0;
    while i <= numsymbols {
        potential[i as usize] = true;
        checktable[i as usize].state_no = -1;
        checktable[i as usize].target = -1;
        i += 1;
    }
    /* For consistency reasons, can't remove symbols longer than 1 */
    /* since @ and ? only match utf8 symbols of length 1           */

    let mut sig = net.sigma.as_deref();
    while let Some(s) = sig {
        if s.number == -1 {
            break;
        }
        if utf8strlen(s.symbol.as_deref().unwrap_or("").as_bytes()) > 1 {
            potential[s.number as usize] = false;
        }
        sig = s.next.as_deref();
    }

    let mut prevstate = 0;

    let mut i = 0usize;
    loop {
        if net.states[i].state_no != prevstate {
            let mut j: i32 = 3;
            while j <= numsymbols {
                if checktable[j as usize].state_no != prevstate
                    && checktable[IDENTITY as usize].state_no != prevstate
                {
                    j += 1;
                    continue;
                }
                if checktable[j as usize].target == checktable[IDENTITY as usize].target
                    && checktable[j as usize].state_no == checktable[IDENTITY as usize].state_no
                {
                    j += 1;
                    continue;
                }
                potential[j as usize] = false;
                j += 1;
            }
        }

        if net.states[i].state_no == -1 {
            break;
        }

        let r#in = net.states[i].r#in as i32;
        let out = net.states[i].out as i32;
        let state = net.states[i].state_no;
        let target = net.states[i].target;

        if r#in != -1 && out != -1 {
            if (r#in == out && r#in > 2) || r#in == IDENTITY {
                checktable[r#in as usize].state_no = state;
                checktable[r#in as usize].target = target;
            }
            if r#in != out && r#in > 2 {
                potential[r#in as usize] = false;
            }
            if r#in != out && out > 2 {
                potential[out as usize] = false;
            }
        }
        prevstate = state;
        i += 1;
    }
    let mut removable = 0;
    let mut i: i32 = 3;
    while i <= numsymbols {
        if potential[i as usize] {
            removable = 1;
        }
        i += 1;
    }
    if removable == 0 {
        /* free(potential); free(checktable) */
        drop(potential);
        drop(checktable);
        return;
    }
    let mut i = 0usize;
    let mut j: i32 = 0;
    loop {
        let r#in = net.states[i].r#in as i32;

        let (state_no, out, target, final_state, start_state) = (
            net.states[i].state_no,
            net.states[i].out as i32,
            net.states[i].target,
            net.states[i].final_state as i32,
            net.states[i].start_state as i32,
        );
        add_fsm_arc(
            &mut net.states,
            j,
            state_no,
            r#in,
            out,
            target,
            final_state,
            start_state,
        );
        if r#in == -1 {
            i += 1;
            j += 1;
        } else if potential[r#in as usize] && r#in > 2 {
            i += 1;
        } else {
            i += 1;
            j += 1;
        }
        if net.states[i].state_no == -1 {
            break;
        }
    }
    let (state_no, r#in, out, target, final_state, start_state) = (
        net.states[i].state_no,
        net.states[i].r#in as i32,
        net.states[i].out as i32,
        net.states[i].target,
        net.states[i].final_state as i32,
        net.states[i].start_state as i32,
    );
    add_fsm_arc(
        &mut net.states,
        j,
        state_no,
        r#in,
        out,
        target,
        final_state,
        start_state,
    );

    /* C: unlink via sigprev->next with no NULL check — a removable FIRST
    sigma entry would deref NULL (cannot occur: sigmas start with a
    special <= 2 entry). DEVIATION from C (head removal removes instead
    of crashing) */
    let mut cur: &mut Option<Box<Sigma>> = &mut net.sigma;
    loop {
        let remove = match cur.as_deref() {
            Some(s) if s.number != -1 => s.number > 2 && potential[s.number as usize],
            _ => break,
        };
        if remove {
            /* free(sig->symbol); free(sig) */
            let next = cur.as_mut().unwrap().next.take();
            *cur = next;
        } else {
            cur = &mut cur.as_mut().unwrap().next;
        }
    }
    /* free(potential); free(checktable) */
    drop(potential);
    drop(checktable);
    sigma_cleanup(net, 0);
}

// [spec:foma:def:constructions.fsm-symbol-occurs-fn]
// [spec:foma:sem:constructions.fsm-symbol-occurs-fn]
// [spec:foma:def:fomalib.fsm-symbol-occurs-fn]
// [spec:foma:sem:fomalib.fsm-symbol-occurs-fn]
pub fn fsm_symbol_occurs(net: &Fsm, symbol: &str, side: i32) -> i32 {
    let sym = sigma_find(symbol, net.sigma.as_deref());
    if sym == -1 {
        return 0;
    }
    let mut i = 0usize;
    while net.states[i].state_no != -1 {
        if side == M_UPPER && net.states[i].r#in as i32 == sym {
            return 1;
        }
        if side == M_LOWER && net.states[i].out as i32 == sym {
            return 1;
        }
        if side == (M_UPPER + M_LOWER)
            && (net.states[i].r#in as i32 == sym || net.states[i].out as i32 == sym)
        {
            return 1;
        }
        i += 1;
    }
    0
}

// [spec:foma:def:constructions.fsm-equal-substrings-fn]
// [spec:foma:sem:constructions.fsm-equal-substrings-fn]
// [spec:foma:def:fomalib.fsm-equal-substrings-fn]
// [spec:foma:sem:fomalib.fsm-equal-substrings-fn]
pub fn fsm_equal_substrings(net: Box<Fsm>, left: &mut Fsm, right: &mut Fsm) -> Box<Fsm> {
    /* The algorithm extracts from the lower side all and only those strings where   */
    /* every X occurring in different substrings ... left X right ... is identical.  */

    /* Caveat: there is no reliable termination condition for the loop that extracts */
    /* identities.  This means that if run on languages where there are potentially  */
    /* infinite-length identical delimited substrings, it will not terminate.        */

    let mut net = net;
    let oldnet = fsm_copy(&mut net);

    /* LB = "@<eq<@" */
    /* RB = "@>eq>@" */

    let mut lb = fsm_symbol("@<eq<@");
    let mut nolb = fsm_minimize(fsm_term_negation(fsm_copy(&mut lb)));
    let mut rb = fsm_symbol("@>eq>@");
    let mut norb = fsm_minimize(fsm_term_negation(fsm_copy(&mut rb)));
    /* NOBR = ~$[LB|RB] */
    let mut nobr = fsm_minimize(fsm_complement(fsm_contains(fsm_union(
        fsm_copy(&mut lb),
        fsm_copy(&mut rb),
    ))));

    sigma_add("@<eq<@", net.sigma.as_deref_mut().unwrap());
    sigma_add("@>eq>@", net.sigma.as_deref_mut().unwrap());
    sigma_sort(&mut net);

    /* Insert our aux markers into the language                */

    /* InsertBrackets = [~$[L|R] [L 0:LB|0:RB R]]* ~$[L|R];    */

    let insert_brackets = fsm_minimize(fsm_concat(
        fsm_kleene_star(fsm_concat(
            fsm_complement(fsm_contains(fsm_union(fsm_copy(left), fsm_copy(right)))),
            fsm_union(
                fsm_concat(
                    fsm_copy(left),
                    fsm_cross_product(fsm_empty_string(), fsm_copy(&mut lb)),
                ),
                fsm_concat(
                    fsm_cross_product(fsm_empty_string(), fsm_copy(&mut rb)),
                    fsm_copy(right),
                ),
            ),
        )),
        fsm_complement(fsm_contains(fsm_union(fsm_copy(left), fsm_copy(right)))),
    ));

    /* Lbracketed = L .o. InsertBrackets                       */

    let mut lbracketed = fsm_compose(fsm_copy(&mut net), insert_brackets);

    /* Filter out improper nestings, or languages with less than two marker pairs */

    /* BracketFilter = NOBR LB NOBR RB NOBR [LB NOBR RB NOBR]+  */

    let mut bracket_filter = fsm_concat(
        fsm_copy(&mut nobr),
        fsm_concat(
            fsm_copy(&mut lb),
            fsm_concat(
                fsm_copy(&mut nobr),
                fsm_concat(
                    fsm_copy(&mut rb),
                    fsm_concat(
                        fsm_copy(&mut nobr),
                        fsm_kleene_plus(fsm_concat(
                            fsm_copy(&mut lb),
                            fsm_concat(
                                fsm_copy(&mut nobr),
                                fsm_concat(fsm_copy(&mut rb), fsm_copy(&mut nobr)),
                            ),
                        )),
                    ),
                ),
            ),
        ),
    );

    /* RemoveBrackets = [LB:0|RB:0|NOBR]*                       */
    /* Lbypass = [Lbracketed .o. ~BracketFilter .o. LB|RB -> 0] */
    /* Leq     = [Lbracketed .o.  BracketFilter]                */

    let remove_brackets = fsm_kleene_star(fsm_union(
        fsm_cross_product(fsm_copy(&mut lb), fsm_empty_string()),
        fsm_union(
            fsm_cross_product(fsm_copy(&mut rb), fsm_empty_string()),
            fsm_copy(&mut nobr),
        ),
    ));

    let lbypass = fsm_lower(fsm_compose(
        fsm_copy(&mut lbracketed),
        fsm_compose(
            fsm_complement(fsm_copy(&mut bracket_filter)),
            remove_brackets,
        ),
    ));
    let mut leq = fsm_compose(lbracketed, bracket_filter);

    /* Extract labels from lower side of L */
    /* [Leq .o. [\LB:0* LB:0 \RB* RB:0]* \LB:0*].l */

    let labels = fsm_sigma_pairs_net(fsm_lower(fsm_compose(
        fsm_copy(&mut leq),
        fsm_concat(
            fsm_kleene_star(fsm_concat(
                fsm_kleene_star(fsm_cross_product(fsm_copy(&mut nolb), fsm_empty_string())),
                fsm_concat(
                    fsm_cross_product(fsm_copy(&mut lb), fsm_empty_string()),
                    fsm_concat(
                        fsm_kleene_star(fsm_copy(&mut norb)),
                        fsm_cross_product(fsm_copy(&mut rb), fsm_empty_string()),
                    ),
                ),
            )),
            fsm_kleene_star(fsm_cross_product(fsm_copy(&mut nolb), fsm_empty_string())),
        ),
    )));

    /* Cleanup = \LB* [LB:0 RB:0 \LB*]* | ~$[LB RB] */

    let mut cleanup = fsm_minimize(fsm_union(
        fsm_concat(
            fsm_kleene_star(fsm_copy(&mut nolb)),
            fsm_kleene_star(fsm_concat(
                fsm_cross_product(fsm_copy(&mut lb), fsm_empty_string()),
                fsm_concat(
                    fsm_cross_product(fsm_copy(&mut rb), fsm_empty_string()),
                    fsm_kleene_star(fsm_copy(&mut nolb)),
                ),
            )),
        ),
        fsm_complement(fsm_contains(fsm_concat(
            fsm_copy(&mut lb),
            fsm_copy(&mut rb),
        ))),
    ));

    /* Construct the move function */

    let mut r#move = fsm_empty_string();

    let mut syms = 0;
    let mut sig = labels.sigma.as_deref();
    while let Some(s) = sig {
        /* Unclear which is faster: the first or the second version */
        /* ThisMove = [\LB* LB:X X:LB]* \LB*       */
        /* ThisMove = [\LB* LB:0 X 0:LB]* \LB*     */
        if s.number >= 3 {
            let mut this_symbol = fsm_symbol(s.symbol.as_deref().unwrap());
            let this_move = fsm_concat(
                fsm_kleene_star(fsm_concat(
                    fsm_kleene_star(fsm_copy(&mut nolb)),
                    fsm_concat(
                        fsm_cross_product(fsm_copy(&mut lb), fsm_empty_string()),
                        fsm_concat(
                            fsm_copy(&mut this_symbol),
                            fsm_cross_product(fsm_empty_string(), fsm_copy(&mut lb)),
                        ),
                    ),
                )),
                fsm_kleene_star(fsm_copy(&mut nolb)),
            );

            r#move = fsm_union(r#move, this_move);
            syms += 1;
        }
        sig = s.next.as_deref();
    }
    let mut r#move = fsm_minimize(r#move);
    if syms == 0 {
        //printf("no syms");
        fsm_destroy(net);
        return oldnet;
    }

    /* Move until no bracket symbols remain */
    loop {
        //printf("Zapping\n");
        leq = fsm_compose(leq, fsm_copy(&mut cleanup));
        if fsm_symbol_occurs(&leq, "@<eq<@", M_LOWER) == 0 {
            break;
        }
        leq = fsm_compose(leq, fsm_copy(&mut r#move));
    }

    /* Result = L .o. [Leq | Lbypass] */
    let mut result = fsm_minimize(fsm_compose(net, fsm_union(fsm_lower(leq), lbypass)));
    /* C: sigma_remove's returned new head is discarded (harmless unless
    the removed node were the head); the owned list is reassigned here */
    result.sigma = sigma_remove("@<eq<@", result.sigma.take());
    result.sigma = sigma_remove("@>eq>@", result.sigma.take());
    fsm_compact(&mut result);
    sigma_sort(&mut result);
    fsm_destroy(oldnet);
    result
}

// [spec:foma:def:constructions.fsm-invert-fn]
// [spec:foma:sem:constructions.fsm-invert-fn]
// [spec:foma:def:fomalib.fsm-invert-fn]
// [spec:foma:sem:fomalib.fsm-invert-fn]
pub fn fsm_invert(net: Box<Fsm>) -> Box<Fsm> {
    let mut net = net;
    let mut i = 0usize;
    while net.states[i].state_no != -1 {
        let temp = net.states[i].r#in;
        net.states[i].r#in = net.states[i].out;
        net.states[i].out = temp;
        i += 1;
    }
    let i = net.arcs_sorted_in;
    net.arcs_sorted_in = net.arcs_sorted_out;
    net.arcs_sorted_out = i;
    net
}

// [spec:foma:def:constructions.fsm-sequentialize-fn]
// [spec:foma:sem:constructions.fsm-sequentialize-fn]
// [spec:foma:def:fomalib.fsm-sequentialize-fn]
// [spec:foma:sem:fomalib.fsm-sequentialize-fn]
pub fn fsm_sequentialize(net: Box<Fsm>) -> Box<Fsm> {
    /* C: unimplemented stub — prints and returns the input unchanged */
    print!("Implementation pending\n");
    net
}

// [spec:foma:def:constructions.fsm-bimachine-fn]
// [spec:foma:sem:constructions.fsm-bimachine-fn]
// [spec:foma:def:fomalib.fsm-bimachine-fn]
// [spec:foma:sem:fomalib.fsm-bimachine-fn]
pub fn fsm_bimachine(net: Box<Fsm>) -> Box<Fsm> {
    /* C: unimplemented stub — prints and returns the input unchanged */
    print!("implementation pending\n");
    net
}

/* _leftrewr(L, a:b) does a -> b || .#. L _    */
/* _leftrewr(?* L, a:b) does a -> b || L _     */
/* works only with single symbols, but is fast */

// [spec:foma:def:constructions.fsm-left-rewr-fn]
// [spec:foma:sem:constructions.fsm-left-rewr-fn]
// [spec:foma:def:fomalib.fsm-left-rewr-fn]
// [spec:foma:sem:fomalib.fsm-left-rewr-fn]
pub fn fsm_left_rewr(net: Box<Fsm>, rewr: Box<Fsm>) -> Box<Fsm> {
    let mut net = net;
    let mut rewr = rewr;
    fsm_merge_sigma(&mut net, &mut rewr);
    let relabelin = rewr.states[0].r#in as i32;
    let relabelout = rewr.states[0].out as i32;

    let mut inh = fsm_read_init(Some(net)).unwrap();
    let sinkstate = fsm_get_num_states(&inh);
    let name = inh.net.as_ref().unwrap().name.clone();
    let mut outh = fsm_construct_init(&name);
    fsm_construct_copy_sigma(&mut outh, inh.net.as_ref().unwrap().sigma.as_deref());
    let mut maxsigma = sigma_max(inh.net.as_ref().unwrap().sigma.as_deref());
    maxsigma += 1;
    /* C: malloc'd (uninitialized); initialized to -1 just below */
    let mut sigmatable: Vec<i32> = vec![0; maxsigma as usize];
    for i in 0..maxsigma {
        sigmatable[i as usize] = -1;
    }
    let mut addedsink = 0;
    loop {
        let currstate = fsm_get_next_state(&mut inh);
        if currstate == -1 {
            break;
        }
        let mut seensource = 0;
        fsm_construct_set_final(&mut outh, currstate);

        while fsm_get_next_state_arc(&mut inh) != 0 {
            let innum = fsm_get_arc_num_in(&inh);
            let mut outnum = fsm_get_arc_num_out(&inh);
            sigmatable[innum as usize] = currstate;
            if innum == relabelin {
                seensource = 1;
                if fsm_read_is_final(&inh, currstate) != 0 {
                    outnum = relabelout;
                }
            }
            let (source, target) = (fsm_get_arc_source(&inh), fsm_get_arc_target(&inh));
            fsm_construct_add_arc_nums(&mut outh, source, target, innum, outnum);
        }
        for i in 2..maxsigma {
            if sigmatable[i as usize] != currstate && i != relabelin {
                fsm_construct_add_arc_nums(&mut outh, currstate, sinkstate, i, i);
                addedsink = 1;
            }
        }
        if seensource == 0 {
            addedsink = 1;
            if fsm_read_is_final(&inh, currstate) != 0 {
                fsm_construct_add_arc_nums(&mut outh, currstate, sinkstate, relabelin, relabelout);
            } else {
                fsm_construct_add_arc_nums(&mut outh, currstate, sinkstate, relabelin, relabelin);
            }
        }
    }
    if addedsink != 0 {
        for i in 2..maxsigma {
            fsm_construct_add_arc_nums(&mut outh, sinkstate, sinkstate, i, i);
        }
        fsm_construct_set_final(&mut outh, sinkstate);
    }
    fsm_construct_set_initial(&mut outh, 0);
    let net = fsm_read_done(inh);
    let newnet = fsm_construct_done(outh);
    /* free(sigmatable) */
    drop(sigmatable);
    fsm_destroy(net);
    fsm_destroy(rewr);
    newnet
}

// [spec:foma:def:constructions.fsm-add-sink-fn]
// [spec:foma:sem:constructions.fsm-add-sink-fn]
// [spec:foma:def:fomalib.fsm-add-sink-fn]
// [spec:foma:sem:fomalib.fsm-add-sink-fn]
pub fn fsm_add_sink(net: Box<Fsm>, r#final: i32) -> Box<Fsm> {
    let mut inh = fsm_read_init(Some(net)).unwrap();
    let sinkstate = fsm_get_num_states(&inh);
    let name = inh.net.as_ref().unwrap().name.clone();
    let mut outh = fsm_construct_init(&name);
    fsm_construct_copy_sigma(&mut outh, inh.net.as_ref().unwrap().sigma.as_deref());
    let mut maxsigma = sigma_max(inh.net.as_ref().unwrap().sigma.as_deref());
    maxsigma += 1;
    /* C: malloc'd (uninitialized); initialized to -1 just below */
    let mut sigmatable: Vec<i32> = vec![0; maxsigma as usize];
    for i in 0..maxsigma {
        sigmatable[i as usize] = -1;
    }
    loop {
        let currstate = fsm_get_next_state(&mut inh);
        if currstate == -1 {
            break;
        }
        while fsm_get_next_state_arc(&mut inh) != 0 {
            let (source, target, num_in, num_out) = (
                fsm_get_arc_source(&inh),
                fsm_get_arc_target(&inh),
                fsm_get_arc_num_in(&inh),
                fsm_get_arc_num_out(&inh),
            );
            fsm_construct_add_arc_nums(&mut outh, source, target, num_in, num_out);
            sigmatable[fsm_get_arc_num_in(&inh) as usize] = currstate;
        }
        for i in 2..maxsigma {
            if sigmatable[i as usize] != currstate {
                fsm_construct_add_arc_nums(&mut outh, currstate, sinkstate, i, i);
            }
        }
    }
    for i in 2..maxsigma {
        fsm_construct_add_arc_nums(&mut outh, sinkstate, sinkstate, i, i);
    }

    loop {
        let i = fsm_get_next_final(&mut inh);
        if i == -1 {
            break;
        }
        fsm_construct_set_final(&mut outh, i);
    }
    if r#final == 1 {
        fsm_construct_set_final(&mut outh, sinkstate);
    }
    fsm_construct_set_initial(&mut outh, 0);
    let net = fsm_read_done(inh);
    let newnet = fsm_construct_done(outh);
    fsm_destroy(net);
    newnet
}

/* _addfinalloop(L, "#":0) adds "#":0 at all final states */
/* _addnonfinalloop(L, "#":0) adds "#":0 at all nonfinal states */
/* _addloop(L, "#":0) adds "#":0 at all states */

/* Adds loops at finals = 0 nonfinals, finals = 1 finals, finals = 2, all */

// [spec:foma:def:constructions.fsm-add-loop-fn]
// [spec:foma:sem:constructions.fsm-add-loop-fn]
// [spec:foma:def:fomalib.fsm-add-loop-fn]
// [spec:foma:sem:fomalib.fsm-add-loop-fn]
pub fn fsm_add_loop(net: Box<Fsm>, marker: &Fsm, finals: i32) -> Box<Fsm> {
    let mut inh = fsm_read_init(Some(net)).unwrap();
    /* C: the read handle borrows marker (which is NOT destroyed); the
    Rust handle owns a deep copy — read-only, observably equivalent */
    let mut minh = fsm_read_init(Some(Box::new(marker.clone()))).unwrap();

    let name = inh.net.as_ref().unwrap().name.clone();
    let mut outh = fsm_construct_init(&name);
    fsm_construct_copy_sigma(&mut outh, inh.net.as_ref().unwrap().sigma.as_deref());

    while fsm_get_next_arc(&mut inh) != 0 {
        let (source, target, num_in, num_out) = (
            fsm_get_arc_source(&inh),
            fsm_get_arc_target(&inh),
            fsm_get_arc_num_in(&inh),
            fsm_get_arc_num_out(&inh),
        );
        fsm_construct_add_arc_nums(&mut outh, source, target, num_in, num_out);
    }
    /* Where to put the loops */
    if finals == 1 {
        loop {
            let i = fsm_get_next_final(&mut inh);
            if i == -1 {
                break;
            }
            fsm_construct_set_final(&mut outh, i);
            fsm_read_reset(Some(&mut minh));
            while fsm_get_next_arc(&mut minh) != 0 {
                let min_in = fsm_get_arc_in(&minh).unwrap().to_string();
                let min_out = fsm_get_arc_out(&minh).unwrap().to_string();
                fsm_construct_add_arc(&mut outh, i, i, &min_in, &min_out);
            }
        }
    } else if finals == 0 || finals == 2 {
        let statecount = inh.net.as_ref().unwrap().statecount;
        for i in 0..statecount {
            if finals == 2 || fsm_read_is_final(&inh, i) == 0 {
                fsm_read_reset(Some(&mut minh));
                while fsm_get_next_arc(&mut minh) != 0 {
                    let min_in = fsm_get_arc_in(&minh).unwrap().to_string();
                    let min_out = fsm_get_arc_out(&minh).unwrap().to_string();
                    fsm_construct_add_arc(&mut outh, i, i, &min_in, &min_out);
                }
            }
        }
    }
    loop {
        let i = fsm_get_next_final(&mut inh);
        if i == -1 {
            break;
        }
        fsm_construct_set_final(&mut outh, i);
    }
    fsm_construct_set_initial(&mut outh, 0);
    let net = fsm_read_done(inh);
    /* fsm_read_done(minh) — frees the handle; the marker copy is dropped
    with it (the C caller keeps the original marker) */
    let marker_copy = fsm_read_done(minh);
    drop(marker_copy);
    let newnet = fsm_construct_done(outh);
    fsm_destroy(net);
    newnet
}

// [spec:foma:def:constructions.fsm-context-restrict-fn]
// [spec:foma:sem:constructions.fsm-context-restrict-fn]
// [spec:foma:def:fomalib.fsm-context-restrict-fn]
// [spec:foma:sem:fomalib.fsm-context-restrict-fn]
pub fn fsm_context_restrict(x: Box<Fsm>, lr: Option<Box<Fsmcontexts>>) -> Box<Fsm> {
    /* [.#. \.#.* .#.]-`[[ [\X* X C X \X*]&~[\X* [L1 X \X* X R1|...|Ln X \X* X Rn] \X*]],X,0] */
    /* Where X = variable symbol */

    let mut x = x;
    let mut lr = lr;

    let mut var = fsm_symbol("@VARX@");
    let mut notvar = fsm_minimize(fsm_kleene_star(fsm_term_negation(fsm_symbol("@VARX@"))));

    /* We add the variable symbol to all alphabets to avoid ? mathing it */
    /* which would cause extra nondeterminism */
    sigma_add("@VARX@", x.sigma.as_deref_mut().unwrap());
    sigma_sort(&mut x);

    /* Also, if any L or R is undeclared we add 0 */
    let mut pairs = lr.as_deref_mut();
    while let Some(p) = pairs {
        if p.left.is_none() {
            p.left = Some(fsm_empty_string());
        } else {
            let left = p.left.as_deref_mut().unwrap();
            sigma_add("@VARX@", left.sigma.as_deref_mut().unwrap());
            sigma_substitute(".#.", "@#@", left.sigma.as_deref_mut().unwrap());
            sigma_sort(left);
        }
        if p.right.is_none() {
            p.right = Some(fsm_empty_string());
        } else {
            let right = p.right.as_deref_mut().unwrap();
            sigma_add("@VARX@", right.sigma.as_deref_mut().unwrap());
            sigma_substitute(".#.", "@#@", right.sigma.as_deref_mut().unwrap());
            sigma_sort(right);
        }
        pairs = p.next.as_deref_mut();
    }

    let mut union_p = fsm_empty_set();

    let mut pairs = lr.as_deref_mut();
    while let Some(p) = pairs {
        union_p = fsm_minimize(fsm_union(
            fsm_minimize(fsm_concat(
                fsm_copy(p.left.as_deref_mut().unwrap()),
                fsm_concat(
                    fsm_copy(&mut var),
                    fsm_concat(
                        fsm_copy(&mut notvar),
                        fsm_concat(
                            fsm_copy(&mut var),
                            fsm_copy(p.right.as_deref_mut().unwrap()),
                        ),
                    ),
                ),
            )),
            union_p,
        ));
        pairs = p.next.as_deref_mut();
    }

    let union_l = fsm_minimize(fsm_concat(
        fsm_copy(&mut notvar),
        fsm_concat(
            fsm_copy(&mut var),
            fsm_concat(
                fsm_copy(&mut x),
                fsm_concat(fsm_copy(&mut var), fsm_copy(&mut notvar)),
            ),
        ),
    ));

    let mut result = fsm_intersect(
        union_l,
        fsm_complement(fsm_concat(
            fsm_copy(&mut notvar),
            fsm_minimize(fsm_concat(fsm_copy(&mut union_p), fsm_copy(&mut notvar))),
        )),
    );
    if sigma_find("@VARX@", result.sigma.as_deref()) != -1 {
        result = fsm_complement(fsm_substitute_symbol(
            result,
            "@VARX@",
            "@_EPSILON_SYMBOL_@",
        ));
    } else {
        result = fsm_complement(result);
    }

    if sigma_find("@#@", result.sigma.as_deref()) != -1 {
        let word = fsm_minimize(fsm_concat(
            fsm_symbol("@#@"),
            fsm_concat(
                fsm_kleene_star(fsm_term_negation(fsm_symbol("@#@"))),
                fsm_symbol("@#@"),
            ),
        ));
        result = fsm_intersect(word, result);
        result = fsm_substitute_symbol(result, "@#@", "@_EPSILON_SYMBOL_@");
    }
    fsm_destroy(union_p);
    fsm_destroy(var);
    fsm_destroy(notvar);
    fsm_destroy(x);
    /* C: fsm_clear_contexts(pairs) — pairs is the loop cursor, NULL after
    the loops, so the LR context list is never freed (latent leak;
    fsm_clear_contexts(LR) was clearly intended). Literal NULL call: */
    fsm_clear_contexts(None);
    /* C leaks LR; the owned list drops here (nothing to reproduce) */
    drop(lr);
    result
}

// [spec:foma:def:constructions.fsm-flatten-fn]
// [spec:foma:sem:constructions.fsm-flatten-fn]
// [spec:foma:def:fomalib.fsm-flatten-fn]
// [spec:foma:sem:fomalib.fsm-flatten-fn]
pub fn fsm_flatten(net: Box<Fsm>, epsilon: Box<Fsm>) -> Option<Box<Fsm>> {
    let net = fsm_minimize(net);

    let mut inh = fsm_read_init(Some(net)).unwrap();
    let mut eps = fsm_read_init(Some(epsilon)).unwrap();
    /* C: dead guard (reproduced literally) — fsm_get_next_arc returns 0/1,
    never -1, so this branch never fires; an arc-less epsilon machine
    reads an invalid arc below instead */
    if fsm_get_next_arc(&mut eps) == -1 {
        let net = fsm_read_done(inh);
        let epsilon = fsm_read_done(eps);
        fsm_destroy(net);
        fsm_destroy(epsilon);
        return None;
    }
    /* strdup(fsm_get_arc_in(eps)) */
    let epssym = fsm_get_arc_in(&eps).unwrap().to_string();
    let epsilon = fsm_read_done(eps);

    let name = inh.net.as_ref().unwrap().name.clone();
    let mut outh = fsm_construct_init(&name);
    let mut maxstate = inh.net.as_ref().unwrap().statecount;

    fsm_construct_copy_sigma(&mut outh, inh.net.as_ref().unwrap().sigma.as_deref());

    while fsm_get_next_arc(&mut inh) != 0 {
        let target = fsm_get_arc_target(&inh);
        let r#in = fsm_get_arc_num_in(&inh);
        let out = fsm_get_arc_num_out(&inh);
        if r#in == EPSILON || out == EPSILON {
            let mut instring = fsm_get_arc_in(&inh).unwrap().to_string();
            let mut outstring = fsm_get_arc_out(&inh).unwrap().to_string();
            if r#in == EPSILON {
                instring = epssym.clone();
            }
            if out == EPSILON {
                outstring = epssym.clone();
            }

            let source = fsm_get_arc_source(&inh);
            fsm_construct_add_arc(&mut outh, source, maxstate, &instring, &instring);
            fsm_construct_add_arc(&mut outh, maxstate, target, &outstring, &outstring);
        } else {
            let source = fsm_get_arc_source(&inh);
            fsm_construct_add_arc_nums(&mut outh, source, maxstate, r#in, r#in);
            fsm_construct_add_arc_nums(&mut outh, maxstate, target, out, out);
        }
        maxstate += 1;
    }
    loop {
        let i = fsm_get_next_final(&mut inh);
        if i == -1 {
            break;
        }
        fsm_construct_set_final(&mut outh, i);
    }
    loop {
        let i = fsm_get_next_initial(&mut inh);
        if i == -1 {
            break;
        }
        fsm_construct_set_initial(&mut outh, i);
    }

    let net = fsm_read_done(inh);
    let newnet = fsm_construct_done(outh);
    fsm_destroy(net);
    fsm_destroy(epsilon);
    /* free(epssym) */
    drop(epssym);
    Some(newnet)
}

/* Removes IDENTITY and UNKNOWN transitions. If mode = 1, only removes UNKNOWNs */
// [spec:foma:def:constructions.fsm-close-sigma-fn]
// [spec:foma:sem:constructions.fsm-close-sigma-fn]
// [spec:foma:def:fomalib.fsm-close-sigma-fn]
// [spec:foma:sem:fomalib.fsm-close-sigma-fn]
pub fn fsm_close_sigma(net: Box<Fsm>, mode: i32) -> Box<Fsm> {
    let mut inh = fsm_read_init(Some(net)).unwrap();
    let name = inh.net.as_ref().unwrap().name.clone();
    let mut newh = fsm_construct_init(&name);
    fsm_construct_copy_sigma(&mut newh, inh.net.as_ref().unwrap().sigma.as_deref());

    while fsm_get_next_arc(&mut inh) != 0 {
        let num_in = fsm_get_arc_num_in(&inh);
        let num_out = fsm_get_arc_num_out(&inh);
        if (num_in != UNKNOWN && num_in != IDENTITY && num_out != UNKNOWN && num_out != IDENTITY)
            || (mode == 1 && num_in != UNKNOWN && num_out != UNKNOWN)
        {
            let (source, target) = (fsm_get_arc_source(&inh), fsm_get_arc_target(&inh));
            fsm_construct_add_arc_nums(&mut newh, source, target, num_in, num_out);
        }
    }
    loop {
        let i = fsm_get_next_final(&mut inh);
        if i == -1 {
            break;
        }
        fsm_construct_set_final(&mut newh, i);
    }
    loop {
        let i = fsm_get_next_initial(&mut inh);
        if i == -1 {
            break;
        }
        fsm_construct_set_initial(&mut newh, i);
    }
    let net = fsm_read_done(inh);
    let newnet = fsm_construct_done(newh);
    fsm_destroy(net);
    fsm_minimize(newnet)
}

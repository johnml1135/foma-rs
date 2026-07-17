//! Wave-4 split of constructions.c (see mod.rs). Cross-module and
//! external names come via `use super::*` (re-exported by mod.rs).
use super::*;

pub const COMPLEMENT: i32 = 0;
pub const COMPLETE: i32 = 1;

// [spec:foma:def:constructions.fsm-concat-fn]
// [spec:foma:sem:constructions.fsm-concat-fn]
// [spec:foma:def:fomalib.fsm-concat-fn]
// [spec:foma:sem:fomalib.fsm-concat-fn]
pub fn fsm_concat(opts: &FomaOptions, net1: Box<Fsm>, net2: Box<Fsm>) -> Box<Fsm> {
    let mut net1 = net1;
    let mut net2 = net2;

    fsm_merge_sigma(opts, &mut net1, &mut net2);

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
        (net1.linecount + net2.linecount + net1.finalcount + 2)
            as usize
    ];
    let mut current_final = -1;
    /* Copy fsm1, fsm2 after each other, adding appropriate epsilon arcs */
    let mut j: i32 = 0;
    let fsm1 = net1.states.rows();
    let fsm2 = net2.states.rows();
    let mut i = 0usize;
    while fsm1[i].state_no != -1 {
        if fsm1[i].final_state == 1 && fsm1[i].state_no != current_final {
            let (state_no, start_state) = (fsm1[i].state_no, fsm1[i].start_state);
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
            current_final = fsm1[i].state_no;
            j += 1;
        }
        if !(fsm1[i].target == -1 && fsm1[i].final_state == 1) {
            let (state_no, line_in, line_out, target, start_state) = (
                fsm1[i].state_no,
                fsm1[i].r#in as i32,
                fsm1[i].out as i32,
                fsm1[i].target,
                fsm1[i].start_state as i32,
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
    while fsm2[i].state_no != -1 {
        let (state_no, line_in, line_out, target, final_state) = (
            fsm2[i].state_no,
            fsm2[i].r#in as i32,
            fsm2[i].out as i32,
            fsm2[i].target,
            fsm2[i].final_state as i32,
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
    net1.states = new_fsm.into();
    if sigma_find_number(EPSILON, &net1.sigma).is_none() {
        sigma_add_special(EPSILON, &mut net1.sigma);
    }
    fsm_count(&mut net1);
    net1.is_epsilon_free = Tern::No;
    net1.is_deterministic = Tern::No;
    net1.is_minimized = Tern::No;
    net1.is_pruned = Tern::No;
    fsm_minimize(opts, net1)
}

// [spec:foma:def:constructions.fsm-union-fn]
// [spec:foma:sem:constructions.fsm-union-fn]
// [spec:foma:def:fomalib.fsm-union-fn]
// [spec:foma:sem:fomalib.fsm-union-fn]
pub fn fsm_union(opts: &FomaOptions, net1: Box<Fsm>, net2: Box<Fsm>) -> Box<Fsm> {
    let mut net1 = net1;
    let mut net2 = net2;

    fsm_merge_sigma(opts, &mut net1, &mut net2);

    fsm_count(&mut net1);
    fsm_count(&mut net2);

    let net1_offset = 1;
    let net2_offset = net1.statecount + 1;
    let real1 = (net1.linecount - 1) as usize;
    let real2 = (net2.linecount - 1) as usize;

    /* Build the union by reusing (and growing) net1's own line table rather
    than allocating a fresh one each call: the incremental-union fold would
    otherwise reallocate and zero-fill the whole accumulator on every step.
    The output is byte-identical to the fresh-array build —
    [start1, start2, net1 lines +net1_offset, net2 lines +net2_offset, term]. */
    let mut arccount = 2;
    {
        let mut fsm1 = net1.states.rows_mut();
        let fsm2 = net2.states.rows();
        fsm1.truncate(real1); /* drop net1's terminator line */
        for line in fsm1.iter_mut() {
            line.state_no += net1_offset;
            if line.target != -1 {
                line.target += net1_offset;
                arccount += 1;
            }
            line.start_state = 0;
        }
        fsm1.reserve(real2 + 3);
        /* prepend the shared start state's two epsilon arcs */
        fsm1.splice(
            0..0,
            [
                FsmState {
                    state_no: 0,
                    r#in: EPSILON as i16,
                    out: EPSILON as i16,
                    target: net1_offset,
                    final_state: 0,
                    start_state: 1,
                },
                FsmState {
                    state_no: 0,
                    r#in: EPSILON as i16,
                    out: EPSILON as i16,
                    target: net2_offset,
                    final_state: 0,
                    start_state: 1,
                },
            ],
        );
        for i in 0..real2 {
            let src = fsm2[i];
            let new_target = if src.target == -1 {
                -1
            } else {
                arccount += 1;
                src.target + net2_offset
            };
            fsm1.push(FsmState {
                state_no: src.state_no + net2_offset,
                r#in: src.r#in,
                out: src.out,
                target: new_target,
                final_state: src.final_state,
                start_state: 0,
            });
        }
        fsm1.push(FsmState {
            state_no: -1,
            r#in: -1,
            out: -1,
            target: -1,
            final_state: -1,
            start_state: -1,
        });
    }

    net1.statecount = net1.statecount + net2.statecount + 1;
    net1.linecount = (real1 + real2 + 3) as i32;
    net1.arccount = arccount;
    net1.finalcount += net2.finalcount;
    fsm_destroy(net2);
    fsm_update_flags(&mut net1, NO, NO, NO, NO, UNK, NO);
    if sigma_find_number(EPSILON, &net1.sigma).is_none() {
        sigma_add_special(EPSILON, &mut net1.sigma);
    }
    net1
}

// [spec:foma:def:constructions.fsm-completes-fn]
// [spec:foma:sem:constructions.fsm-completes-fn]
pub fn fsm_completes(opts: &FomaOptions, net: Box<Fsm>, operation: i32) -> Box<Fsm> {
    /* TODO: this currently relies on that the sigma is gap-free in its numbering  */
    /* which can't always be counted on, especially when reading external machines */

    /* TODO: check arity */

    let mut net = net;
    if net.is_minimized != Tern::Yes {
        net = fsm_minimize(opts, net);
    }

    let mut incomplete = 0;
    if sigma_contains_number(UNKNOWN, &net.sigma) {
        /* C: sigma_remove's returned new head is discarded (harmless
        unless UNKNOWN were the head node); the owned list here must be
        reassigned */
        sigma_remove("@_UNKNOWN_SYMBOL_@", &mut net.sigma);
    }
    if sigma_find_number(IDENTITY, &net.sigma).is_none() {
        sigma_add_special(IDENTITY, &mut net.sigma);
        incomplete = 1;
    }

    let mut sigsize = sigma_size(&net.sigma);
    let last_sigma = sigma_max(&net.sigma);

    if sigma_contains_number(EPSILON, &net.sigma) {
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
    {
        let mut fsm = net.states.rows_mut();
        let mut i = 0usize;
        while fsm[i].state_no != -1 {
            if operation == COMPLEMENT {
                if fsm[i].final_state == 1 {
                    fsm[i].final_state = 0;
                } else if fsm[i].final_state == 0 {
                    fsm[i].final_state = 1;
                }
            }
            if fsm[i].target != -1 {
                arccount += 1;
            }
            starts[fsm[i].state_no as usize] = fsm[i].start_state as i16;
            finals[fsm[i].state_no as usize] = fsm[i].final_state as i16;
            if fsm[i].final_state != 0 && operation != COMPLEMENT {
                sinks[fsm[i].state_no as usize] = 0;
            }
            if fsm[i].final_state == 0 && operation == COMPLEMENT {
                sinks[fsm[i].state_no as usize] = 0;
            }
            if fsm[i].target != -1 && fsm[i].state_no != fsm[i].target {
                sinks[fsm[i].state_no as usize] = 0;
            }
            i += 1;
        }
    }

    net.is_loop_free = Tern::No;
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
        net.is_completed = Tern::Yes;
        net.is_minimized = Tern::Yes;
        net.is_pruned = Tern::No;
        net.is_deterministic = Tern::Yes;
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

    {
        let fsm = net.states.rows();
        let mut i = 0usize;
        while fsm[i].state_no != -1 {
            if fsm[i].target != -1 {
                state_table[(fsm[i].state_no * sigsize + fsm[i].r#in as i32) as usize] =
                    fsm[i].target;
            }
            i += 1;
        }
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
    net.states = new_fsm.into();
    /* free(starts); free(finals); free(sinks); free(state_table) */
    drop(starts);
    drop(finals);
    drop(sinks);
    drop(state_table);
    net.is_minimized = Tern::No;
    net.is_pruned = Tern::No;
    net.is_completed = Tern::Yes;
    net.statecount = statecount;
    net
}

// [spec:foma:def:constructions.fsm-complete-fn]
// [spec:foma:sem:constructions.fsm-complete-fn]
// [spec:foma:def:fomalib.fsm-complete-fn]
// [spec:foma:sem:fomalib.fsm-complete-fn]
pub fn fsm_complete(opts: &FomaOptions, net: Box<Fsm>) -> Box<Fsm> {
    fsm_completes(opts, net, COMPLETE)
}

// [spec:foma:def:constructions.fsm-complement-fn]
// [spec:foma:sem:constructions.fsm-complement-fn]
// [spec:foma:def:fomalib.fsm-complement-fn]
// [spec:foma:sem:fomalib.fsm-complement-fn]
pub fn fsm_complement(opts: &FomaOptions, net: Box<Fsm>) -> Box<Fsm> {
    fsm_completes(opts, net, COMPLEMENT)
}

// [spec:foma:def:constructions.fsm-minus-fn]
// [spec:foma:sem:constructions.fsm-minus-fn]
// [spec:foma:def:fomalib.fsm-minus-fn]
// [spec:foma:sem:fomalib.fsm-minus-fn]
pub fn fsm_minus(opts: &FomaOptions, net1: Box<Fsm>, net2: Box<Fsm>) -> Box<Fsm> {
    let mut int_stack = IntStack::new();
    let mut statecount = 0;

    let mut net1 = fsm_minimize(opts, net1);
    let mut net2 = fsm_minimize(opts, net2);

    fsm_merge_sigma(opts, &mut net1, &mut net2);

    fsm_count(&mut net1);
    fsm_count(&mut net2);

    /* new state 0 = {1,1} */

    int_stack.clear();
    /* STACK_2_PUSH(1,1) */
    int_stack.push(1);
    int_stack.push(1);

    let mut th = triplet_hash_init();
    triplet_hash_insert(&mut th, 1, 1, 0);

    let fsm1 = net1.states.rows();
    let fsm2 = net2.states.rows();
    let point_a = init_state_pointers(&fsm1);
    let point_b = init_state_pointers(&fsm2);

    let mut builder = fsm_state_init(sigma_max(&net1.sigma));

    while !int_stack.is_empty() {
        statecount += 1;
        /* Get a pair of states to examine */

        let mut a = int_stack.pop();
        let mut b = int_stack.pop();

        let current_state = triplet_hash_find(&th, a, b, 0)
            .expect("state pair popped off the work stack was inserted into the triplet hash");
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

        fsm_state_set_current_state(&mut builder, current_state, current_final, current_start);

        let mut ai = point_a[a as usize].transitions;
        while fsm1[ai].state_no == a {
            if fsm1[ai].target == -1 {
                break;
            }
            let target_number;
            if b == -1 {
                /* b is dead */
                let atarget = fsm1[ai].target;
                target_number = match triplet_hash_find(&th, atarget + 1, 0, 0) {
                    Some(found) => found,
                    None => {
                        /* STACK_2_PUSH(0, (machine_a->target)+1) */
                        int_stack.push(0);
                        int_stack.push(atarget + 1);
                        triplet_hash_insert(&mut th, atarget + 1, 0, 0)
                    }
                };
            } else {
                /* b is alive */
                let mut b_has_trans = 0;
                let mut btarget = 0;
                let mut bi = point_b[b as usize].transitions;
                while fsm2[bi].state_no == b {
                    if fsm1[ai].r#in == fsm2[bi].r#in && fsm1[ai].out == fsm2[bi].out {
                        b_has_trans = 1;
                        btarget = fsm2[bi].target;
                        break;
                    }
                    bi += 1;
                }
                if b_has_trans != 0 {
                    let atarget = fsm1[ai].target;
                    target_number = match triplet_hash_find(&th, atarget + 1, btarget + 1, 0) {
                        Some(found) => found,
                        None => {
                            /* STACK_2_PUSH(btarget+1, (machine_a->target)+1) */
                            int_stack.push(btarget + 1);
                            int_stack.push(atarget + 1);
                            /* C inserts (machine_b->target)+1, which equals
                            btarget+1 (the scan broke at the matching line) */
                            let mbtarget = fsm2[bi].target;
                            triplet_hash_insert(&mut th, atarget + 1, mbtarget + 1, 0)
                        }
                    };
                } else {
                    /* b is dead */
                    let atarget = fsm1[ai].target;
                    target_number = match triplet_hash_find(&th, atarget + 1, 0, 0) {
                        Some(found) => found,
                        None => {
                            /* STACK_2_PUSH(0, (machine_a->target)+1) */
                            int_stack.push(0);
                            int_stack.push(atarget + 1);
                            triplet_hash_insert(&mut th, atarget + 1, 0, 0)
                        }
                    };
                }
            }
            let (line_in, line_out) = (fsm1[ai].r#in as i32, fsm1[ai].out as i32);
            fsm_state_add_arc(
                &mut builder,
                current_state,
                line_in,
                line_out,
                target_number,
                current_final,
                current_start,
            );
            ai += 1;
        }
        fsm_state_end_state(&mut builder);
    }

    let _ = statecount;
    /* free(net1->states) */
    net1.states = Vec::new().into();
    fsm_state_close(&mut builder, &mut net1);
    /* free(point_a); free(point_b) */
    drop(point_a);
    drop(point_b);
    fsm_destroy(net2);
    triplet_hash_free(Some(th));
    fsm_minimize(opts, net1)
}

// [spec:foma:def:constructions.fsm-concat-m-n-fn]
// [spec:foma:sem:constructions.fsm-concat-m-n-fn]
// [spec:foma:def:fomalib.fsm-concat-m-n-fn]
// [spec:foma:sem:fomalib.fsm-concat-m-n-fn]
pub fn fsm_concat_m_n(opts: &FomaOptions, net1: Box<Fsm>, m: i32, n: i32) -> Box<Fsm> {
    let mut net1 = net1;
    let mut acc = fsm_empty_string();
    let mut i = 1;
    while i <= n {
        if i > m {
            acc = fsm_concat(opts, acc, fsm_optionality(opts, fsm_copy(&mut net1)));
        } else {
            acc = fsm_concat(opts, acc, fsm_copy(&mut net1));
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
pub fn fsm_concat_n(opts: &FomaOptions, net1: Box<Fsm>, n: i32) -> Box<Fsm> {
    fsm_concat_m_n(opts, net1, n, n)
}

// [spec:foma:def:constructions.fsm-term-negation-fn]
// [spec:foma:sem:constructions.fsm-term-negation-fn]
// [spec:foma:def:fomalib.fsm-term-negation-fn]
// [spec:foma:sem:fomalib.fsm-term-negation-fn]
pub fn fsm_term_negation(opts: &FomaOptions, net1: Box<Fsm>) -> Box<Fsm> {
    fsm_intersect(opts, fsm_identity(), fsm_complement(opts, net1))
}

// [spec:foma:def:constructions.fsm-invert-fn]
// [spec:foma:sem:constructions.fsm-invert-fn]
// [spec:foma:def:fomalib.fsm-invert-fn]
// [spec:foma:sem:fomalib.fsm-invert-fn]
pub fn fsm_invert(net: Box<Fsm>) -> Box<Fsm> {
    let mut net = net;
    {
        let mut fsm = net.states.rows_mut();
        let mut i = 0usize;
        while fsm[i].state_no != -1 {
            let s = &mut fsm[i];
            std::mem::swap(&mut s.r#in, &mut s.out);
            i += 1;
        }
    }
    std::mem::swap(&mut net.arcs_sorted_in, &mut net.arcs_sorted_out);
    net
}

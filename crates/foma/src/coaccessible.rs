//! foma/coaccessible.c — literal (bug-for-bug) Wave-2 port per
//! docs/port/rust-conventions.md. Sem rules:
//! docs/spec/port/foma/coaccessible.md (per-file ids) plus the fomalib.h
//! prototype id.
//!
//! fsm_coaccessible prunes states from which no final state is reachable,
//! compacting the line array in place. Kept quirks: mapping[0] = 0 is set
//! unconditionally ("state 0 always exists") — if state 0 is NOT
//! coaccessible, surviving states are numbered from 1 and the result has no
//! state 0; when the write index has caught up with the read index the
//! post-write re-reads of line i observe the remapped values, as in C.

use crate::constructions::add_fsm_arc;
use crate::int_stack::{int_stack_clear, int_stack_isempty, int_stack_pop, int_stack_push};
use crate::sigma::sigma_create;
use crate::structures::{fsm_empty, fsm_sigma_destroy};
use crate::types::{Fsm, YES};

// [spec:foma:def:coaccessible.invtable]
pub struct Invtable {
    pub state: i32,
    pub next: Option<Box<Invtable>>,
}

// [spec:foma:def:coaccessible.fsm-coaccessible-fn]
// [spec:foma:sem:coaccessible.fsm-coaccessible-fn]
// [spec:foma:def:fomalib.fsm-coaccessible-fn]
// [spec:foma:sem:fomalib.fsm-coaccessible-fn]
pub fn fsm_coaccessible(net: Box<Fsm>) -> Box<Fsm> {
    let mut net = net;

    /* C: fsm = net->states — reads/writes below index net.states directly */
    let mut new_arccount = 0;
    /* printf("statecount %i\n",net->statecount); */
    let _old_statecount = net.statecount;
    /* calloc(net->statecount, sizeof(struct invtable)) — zeroed heads */
    let mut inverses: Vec<Invtable> = (0..net.statecount)
        .map(|_| Invtable {
            state: 0,
            next: None,
        })
        .collect();
    let mut coacc: Vec<i32> = vec![0; net.statecount as usize];
    /* C mallocs mapping uninitialized; only entries of coaccessible
    states (and slot 0) are ever read back */
    let mut mapping: Vec<i32> = vec![0; net.statecount as usize];
    let mut added: Vec<i32> = vec![0; net.statecount as usize];

    for i in 0..net.statecount {
        inverses[i as usize].state = -1;
        coacc[i as usize] = 0;
        added[i as usize] = 0;
    }

    let mut i: i32 = 0;
    while net.states[i as usize].state_no != -1 {
        let s = net.states[i as usize].state_no;
        let t = net.states[i as usize].target;
        if t != -1 && s != t {
            if inverses[t as usize].state == -1 {
                inverses[t as usize].state = s;
            } else {
                /* malloc'd chain node spliced directly after the head */
                let temp_i = Box::new(Invtable {
                    state: s,
                    next: inverses[t as usize].next.take(),
                });
                inverses[t as usize].next = Some(temp_i);
            }
        }
        i += 1;
    }

    /* Push & mark finals */

    let mut markcount = 0;
    let mut i: i32 = 0;
    while net.states[i as usize].state_no != -1 {
        if net.states[i as usize].final_state != 0
            && coacc[net.states[i as usize].state_no as usize] == 0
        {
            int_stack_push(net.states[i as usize].state_no);
            coacc[net.states[i as usize].state_no as usize] = 1;
            markcount += 1;
        }
        i += 1;
    }

    let mut terminate = 0;
    while int_stack_isempty() == 0 {
        let current_state = int_stack_pop();
        /* current_ptr = inverses+current_state; the array-resident head,
        then its malloc'd chain */
        let mut current_ptr: Option<&Invtable> = Some(&inverses[current_state as usize]);
        while let Some(p) = current_ptr {
            if p.state == -1 {
                break;
            }
            if coacc[p.state as usize] == 0 {
                coacc[p.state as usize] = 1;
                int_stack_push(p.state);
                markcount += 1;
            }
            current_ptr = p.next.as_deref();
        }
        if markcount >= net.statecount {
            /* printf("Already coacc\n");  */
            terminate = 1;
            int_stack_clear();
            break;
        }
    }

    if terminate == 0 {
        mapping[0] = 0; /* state 0 always exists */
        let mut new_linecount = 0;
        {
            let mut j = 0;
            for i in 1..net.statecount {
                if coacc[i as usize] == 1 {
                    j += 1;
                    mapping[i as usize] = j;
                }
            }
        }

        let mut i: i32 = 0;
        let mut j: i32 = 0;
        while net.states[i as usize].state_no != -1 {
            if i > 0
                && net.states[i as usize].state_no != net.states[(i - 1) as usize].state_no
                && net.states[(i - 1) as usize].final_state != 0
                && added[net.states[(i - 1) as usize].state_no as usize] == 0
            {
                /* synthetic final line for a state all of whose arcs were
                pruned */
                let state_no = mapping[net.states[(i - 1) as usize].state_no as usize];
                let start_state = net.states[(i - 1) as usize].start_state as i32;
                add_fsm_arc(&mut net.states, j, state_no, -1, -1, -1, 1, start_state);
                j += 1;
                new_linecount += 1;
                added[net.states[(i - 1) as usize].state_no as usize] = 1;
                /* printf("addf ad %i\n",i); */
            }
            if coacc[net.states[i as usize].state_no as usize] != 0
                && (net.states[i as usize].target == -1
                    || coacc[net.states[i as usize].target as usize] != 0)
            {
                net.states[j as usize].state_no =
                    mapping[net.states[i as usize].state_no as usize];
                if net.states[i as usize].target == -1 {
                    net.states[j as usize].target = -1;
                } else {
                    net.states[j as usize].target =
                        mapping[net.states[i as usize].target as usize];
                }
                net.states[j as usize].final_state = net.states[i as usize].final_state;
                net.states[j as usize].start_state = net.states[i as usize].start_state;
                net.states[j as usize].r#in = net.states[i as usize].r#in;
                net.states[j as usize].out = net.states[i as usize].out;
                j += 1;
                new_linecount += 1;
                added[net.states[i as usize].state_no as usize] = 1;
                if net.states[i as usize].target != -1 {
                    new_arccount += 1;
                }
            }
            i += 1;
        }

        if i > 1
            && net.states[(i - 1) as usize].final_state != 0
            && added[net.states[(i - 1) as usize].state_no as usize] == 0
        {
            /* printf("addf\n"); */
            let state_no = mapping[net.states[(i - 1) as usize].state_no as usize];
            let start_state = net.states[(i - 1) as usize].start_state as i32;
            add_fsm_arc(&mut net.states, j, state_no, -1, -1, -1, 1, start_state);
            j += 1;
            new_linecount += 1;
        }

        if new_linecount == 0 {
            add_fsm_arc(&mut net.states, j, 0, -1, -1, -1, -1, -1);
            j += 1;
        }

        add_fsm_arc(&mut net.states, j, -1, -1, -1, -1, -1, -1);
        if markcount == 0 {
            /* We're dealing with the empty language */
            /* free(fsm) — dropped by the assignment */
            net.states = fsm_empty();
            fsm_sigma_destroy(net.sigma.take());
            net.sigma = Some(sigma_create());
        }
        net.linecount = new_linecount;
        net.arccount = new_arccount;
        net.statecount = markcount;
    }

    /* printf("Markccount %i \n",markcount); */

    /* C frees each inverse list's malloc'd chain nodes (heads live in the
    array and are not individually freed), then the head array, coacc,
    added and mapping — all dropped here */
    drop(inverses);

    net.is_pruned = YES;
    net
}

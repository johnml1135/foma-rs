//! foma/extract.c — literal (bug-for-bug) Wave-2 port per
//! docs/port/rust-conventions.md. Sem rules: docs/spec/port/foma/extract.md
//! (per-file ids) plus the fomalib.h prototype ids.
//!
//! fsm_lower / fsm_upper project a transducer onto its lower (output) /
//! upper (input) side in place, rebuilding the state array with the
//! fsm_state_* dynarray builder. A lone UNKNOWN (1) label becomes IDENTITY
//! (2) in the projection.

use crate::constructions::fsm_update_flags;
use crate::dynarray::{
    fsm_state_add_arc, fsm_state_close, fsm_state_end_state, fsm_state_init,
    fsm_state_set_current_state,
};
use crate::sigma::{sigma_cleanup, sigma_max};
use crate::types::{Fsm, IDENTITY, NO, UNK, UNKNOWN};

// [spec:foma:def:extract.fsm-lower-fn]
// [spec:foma:sem:extract.fsm-lower-fn]
// [spec:foma:def:fomalib.fsm-lower-fn]
// [spec:foma:sem:fomalib.fsm-lower-fn]
pub fn fsm_lower(net: Box<Fsm>) -> Box<Fsm> {
    let mut net = net;
    /* C: fsm = net->states — reads below index net.states directly */
    fsm_state_init(sigma_max(net.sigma.as_deref()));
    let mut prevstate = -1;
    let mut i: i32 = 0;
    while net.states[i as usize].state_no != -1 {
        if prevstate != -1 && prevstate != net.states[i as usize].state_no {
            fsm_state_end_state();
        }
        if prevstate != net.states[i as usize].state_no {
            fsm_state_set_current_state(
                net.states[i as usize].state_no,
                net.states[i as usize].final_state as i32,
                net.states[i as usize].start_state as i32,
            );
        }
        if net.states[i as usize].target != -1 {
            let out = if net.states[i as usize].out as i32 == UNKNOWN {
                IDENTITY
            } else {
                net.states[i as usize].out as i32
            };
            fsm_state_add_arc(
                net.states[i as usize].state_no,
                out,
                out,
                net.states[i as usize].target,
                net.states[i as usize].final_state as i32,
                net.states[i as usize].start_state as i32,
            );
        }
        /* C for-loop increment clause: prevstate = (fsm+i)->state_no, i++ */
        prevstate = net.states[i as usize].state_no;
        i += 1;
    }
    fsm_state_end_state();
    /* free(net->states) */
    net.states = Vec::new();
    fsm_state_close(&mut net);
    fsm_update_flags(&mut net, NO, NO, NO, UNK, UNK, UNK);
    sigma_cleanup(&mut net, 0);
    net
}

// [spec:foma:def:extract.fsm-upper-fn]
// [spec:foma:sem:extract.fsm-upper-fn]
// [spec:foma:def:fomalib.fsm-upper-fn]
// [spec:foma:sem:fomalib.fsm-upper-fn]
pub fn fsm_upper(net: Box<Fsm>) -> Box<Fsm> {
    let mut net = net;
    /* C: fsm = net->states — reads below index net.states directly */
    fsm_state_init(sigma_max(net.sigma.as_deref()));
    let mut prevstate = -1;
    let mut i: i32 = 0;
    while net.states[i as usize].state_no != -1 {
        if prevstate != -1 && prevstate != net.states[i as usize].state_no {
            fsm_state_end_state();
        }
        if prevstate != net.states[i as usize].state_no {
            fsm_state_set_current_state(
                net.states[i as usize].state_no,
                net.states[i as usize].final_state as i32,
                net.states[i as usize].start_state as i32,
            );
        }
        if net.states[i as usize].target != -1 {
            let r#in = if net.states[i as usize].r#in as i32 == UNKNOWN {
                IDENTITY
            } else {
                net.states[i as usize].r#in as i32
            };
            fsm_state_add_arc(
                net.states[i as usize].state_no,
                r#in,
                r#in,
                net.states[i as usize].target,
                net.states[i as usize].final_state as i32,
                net.states[i as usize].start_state as i32,
            );
        }
        /* C for-loop increment clause: prevstate = (fsm+i)->state_no, i++ */
        prevstate = net.states[i as usize].state_no;
        i += 1;
    }
    fsm_state_end_state();
    /* free(net->states) */
    net.states = Vec::new();
    fsm_state_close(&mut net);
    fsm_update_flags(&mut net, NO, NO, NO, UNK, UNK, UNK);
    sigma_cleanup(&mut net, 0);
    net
}

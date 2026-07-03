//! foma/reverse.c — literal (bug-for-bug) Wave-2 port per
//! docs/port/rust-conventions.md. Sem rules: docs/spec/port/foma/reverse.md
//! (per-file id) plus the fomalib.h prototype id.
//!
//! fsm_reverse builds the reversal via the read/construct handle APIs: all
//! original state numbers are shifted up by 1, a brand-new state 0 becomes
//! the sole initial state, with EPSILON:EPSILON arcs to every (old) final
//! state; label sides are NOT swapped. The input net is consumed
//! (fsm_destroy'd).

use crate::dynarray::{
    fsm_construct_add_arc_nums, fsm_construct_copy_sigma, fsm_construct_done, fsm_construct_init,
    fsm_construct_set_final, fsm_construct_set_initial, fsm_get_arc_num_in, fsm_get_arc_num_out,
    fsm_get_arc_source, fsm_get_arc_target, fsm_get_next_arc, fsm_get_next_final,
    fsm_get_next_initial, fsm_read_done, fsm_read_init,
};
use crate::structures::fsm_destroy;
use crate::types::{Fsm, EPSILON};

// [spec:foma:def:reverse.fsm-reverse-fn]
// [spec:foma:sem:reverse.fsm-reverse-fn]
// [spec:foma:def:fomalib.fsm-reverse-fn]
// [spec:foma:sem:fomalib.fsm-reverse-fn]
pub fn fsm_reverse(net: Box<Fsm>) -> Box<Fsm> {
    /* C: net stays a caller pointer alongside the read handle; here the
    handle owns the net until fsm_read_done returns it, so net->name /
    net->sigma are reached through inh (observably equivalent) */
    let mut inh = fsm_read_init(Some(net)).unwrap();
    let name = inh.net.as_ref().unwrap().name.clone();
    let mut revh = fsm_construct_init(&name);
    fsm_construct_copy_sigma(&mut revh, inh.net.as_ref().unwrap().sigma.as_deref());

    while fsm_get_next_arc(&mut inh) != 0 {
        let (target, source) = (fsm_get_arc_target(&inh), fsm_get_arc_source(&inh));
        let (num_in, num_out) = (fsm_get_arc_num_in(&inh), fsm_get_arc_num_out(&inh));
        fsm_construct_add_arc_nums(&mut revh, target + 1, source + 1, num_in, num_out);
    }

    let mut i;
    loop {
        i = fsm_get_next_final(&mut inh);
        if i == -1 {
            break;
        }
        fsm_construct_add_arc_nums(&mut revh, 0, i + 1, EPSILON, EPSILON);
    }
    loop {
        i = fsm_get_next_initial(&mut inh);
        if i == -1 {
            break;
        }
        fsm_construct_set_final(&mut revh, i + 1);
    }
    fsm_construct_set_initial(&mut revh, 0);
    let net = fsm_read_done(inh);
    let mut revnet = fsm_construct_done(revh);
    revnet.is_deterministic = 0;
    revnet.is_epsilon_free = 0;
    fsm_destroy(net);
    revnet
}

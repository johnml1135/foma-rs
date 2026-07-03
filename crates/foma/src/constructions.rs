//! foma/constructions.c — stubs only (per docs/port/rust-conventions.md).
//!
//! Called by structures.rs (and others); the real port lands with the
//! w2-constructions concern, which replaces these stubs and adds the spec
//! annotations. Signatures follow the C prototypes in foma/fomalib.h and
//! foma/fomalibconf.h under the types.rs mappings: functions that consume
//! (free) their `struct fsm *` arguments take `Box<Fsm>`, borrowing
//! functions take `&Fsm`/`&mut Fsm`.

use crate::types::{Fsm, FsmState};

pub fn fsm_compose(net1: Box<Fsm>, net2: Box<Fsm>) -> Box<Fsm> {
    let _ = (net1, net2);
    todo!("ported by w2-constructions")
}

pub fn fsm_invert(net: Box<Fsm>) -> Box<Fsm> {
    let _ = net;
    todo!("ported by w2-constructions")
}

pub fn fsm_union(net_1: Box<Fsm>, net_2: Box<Fsm>) -> Box<Fsm> {
    let _ = (net_1, net_2);
    todo!("ported by w2-constructions")
}

pub fn fsm_intersect(net1: Box<Fsm>, net2: Box<Fsm>) -> Box<Fsm> {
    let _ = (net1, net2);
    todo!("ported by w2-constructions")
}

pub fn fsm_concat(net1: Box<Fsm>, net2: Box<Fsm>) -> Box<Fsm> {
    let _ = (net1, net2);
    todo!("ported by w2-constructions")
}

pub fn fsm_kleene_star(net: Box<Fsm>) -> Box<Fsm> {
    let _ = net;
    todo!("ported by w2-constructions")
}

pub fn fsm_complement(net: Box<Fsm>) -> Box<Fsm> {
    let _ = net;
    todo!("ported by w2-constructions")
}

pub fn fsm_term_negation(net1: Box<Fsm>) -> Box<Fsm> {
    let _ = net1;
    todo!("ported by w2-constructions")
}

pub fn fsm_symbol(symbol: &str) -> Box<Fsm> {
    let _ = symbol;
    todo!("ported by w2-constructions")
}

pub fn fsm_universal() -> Box<Fsm> {
    todo!("ported by w2-constructions")
}

pub fn fsm_contains(net: Box<Fsm>) -> Box<Fsm> {
    let _ = net;
    todo!("ported by w2-constructions")
}

pub fn fsm_ignore(net1: Box<Fsm>, net2: Box<Fsm>, operation: i32) -> Box<Fsm> {
    let _ = (net1, net2, operation);
    todo!("ported by w2-constructions")
}

pub fn fsm_compact(net: &mut Fsm) {
    let _ = net;
    todo!("ported by w2-constructions")
}

pub fn fsm_count(net: &mut Fsm) {
    let _ = net;
    todo!("ported by w2-constructions")
}

pub fn fsm_update_flags(
    net: &mut Fsm,
    det: i32,
    pru: i32,
    min: i32,
    eps: i32,
    r#loop: i32,
    completed: i32,
) {
    let _ = (net, det, pru, min, eps, r#loop, completed);
    todo!("ported by w2-constructions")
}

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
    let _ = (fsm, offset, state_no, r#in, out, target, final_state, start_state);
    todo!("ported by w2-constructions")
}

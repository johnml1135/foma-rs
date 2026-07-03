//! foma/determinize.c — stubs only (per docs/port/rust-conventions.md).
//!
//! The real port lands with the w2-graph-algorithms concern, which replaces
//! these stubs and adds the spec annotations. C prototypes:
//! `struct fsm *fsm_determinize(struct fsm *net)` (fomalib.h) and the
//! file-static `static struct fsm *fsm_subset(struct fsm *net, int operation)`
//! — both consume their argument.

use crate::types::Fsm;

pub fn fsm_determinize(net: Box<Fsm>) -> Box<Fsm> {
    let _ = net;
    todo!("ported by w2-graph-algorithms")
}

pub(crate) fn fsm_subset(net: Box<Fsm>, operation: i32) -> Box<Fsm> {
    let _ = (net, operation);
    todo!("ported by w2-graph-algorithms")
}

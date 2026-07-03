//! foma/extract.c — stubs only (per docs/port/rust-conventions.md).
//!
//! Called by structures.rs; the real port lands with the
//! w2-graph-algorithms concern, which replaces these stubs and adds the
//! spec annotations. C: `struct fsm *fsm_upper(struct fsm *net)` /
//! `struct fsm *fsm_lower(struct fsm *net)` — project in place and return
//! the same net (consume-and-return convention).

use crate::types::Fsm;

pub fn fsm_upper(net: Box<Fsm>) -> Box<Fsm> {
    let _ = net;
    todo!("ported by w2-graph-algorithms")
}

pub fn fsm_lower(net: Box<Fsm>) -> Box<Fsm> {
    let _ = net;
    todo!("ported by w2-graph-algorithms")
}

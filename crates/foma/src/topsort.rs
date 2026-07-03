//! foma/topsort.c — stubs only (per docs/port/rust-conventions.md).
//!
//! Called by structures.rs; the real port lands with the
//! w2-graph-algorithms concern, which replaces this stub and adds the spec
//! annotations. C: `struct fsm *fsm_topsort(struct fsm *net)` — sorts in
//! place and returns the same net (consume-and-return convention).

use crate::types::Fsm;

pub fn fsm_topsort(net: Box<Fsm>) -> Box<Fsm> {
    let _ = net;
    todo!("ported by w2-graph-algorithms")
}

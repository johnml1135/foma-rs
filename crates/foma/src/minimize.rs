//! foma/minimize.c — stubs only (per docs/port/rust-conventions.md).
//!
//! Called by structures.rs; the real port lands with the
//! w2-graph-algorithms concern, which replaces this stub and adds the spec
//! annotations. C: `struct fsm *fsm_minimize(struct fsm *net)` — consumes
//! its argument (chained consuming calls) and returns the minimized net.

use crate::types::Fsm;

pub fn fsm_minimize(net: Box<Fsm>) -> Box<Fsm> {
    let _ = net;
    todo!("ported by w2-graph-algorithms")
}

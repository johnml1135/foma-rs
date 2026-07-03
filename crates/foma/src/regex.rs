//! foma/regex.y + regex.l (the regex compiler) — stubs only, per the stub
//! protocol in docs/port/rust-conventions.md. Replaced (and annotated) by
//! the owning concern.

use crate::types::{DefinedFunctions, DefinedNetworks, Fsm};

pub fn fsm_parse_regex(
    regex: &str,
    defined_nets: Option<&mut DefinedNetworks>,
    defined_funcs: Option<&mut DefinedFunctions>,
) -> Option<Box<Fsm>> {
    let _ = (regex, defined_nets, defined_funcs);
    todo!("ported by w2-cli")
}

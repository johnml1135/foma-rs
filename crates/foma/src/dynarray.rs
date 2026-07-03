//! foma/dynarray.c — stubs only (per docs/port/rust-conventions.md).
//!
//! The fsm_construct_* functions below are called by trie.rs; their real
//! port lands with the dynarray concern, which replaces these stubs and
//! adds the spec annotations. Signatures follow the C prototypes in
//! foma/fomalib.h under the types.rs mappings.

use crate::types::{Fsm, FsmConstructHandle};

pub fn fsm_construct_init(name: &str) -> Box<FsmConstructHandle> {
    let _ = name;
    todo!("ported by w2-structures-dynarray")
}

pub fn fsm_construct_add_arc(
    handle: &mut FsmConstructHandle,
    source: i32,
    target: i32,
    r#in: &str,
    out: &str,
) {
    let _ = (handle, source, target, r#in, out);
    todo!("ported by w2-structures-dynarray")
}

pub fn fsm_construct_set_final(handle: &mut FsmConstructHandle, state_no: i32) {
    let _ = (handle, state_no);
    todo!("ported by w2-structures-dynarray")
}

pub fn fsm_construct_set_initial(handle: &mut FsmConstructHandle, state_no: i32) {
    let _ = (handle, state_no);
    todo!("ported by w2-structures-dynarray")
}

pub fn fsm_construct_done(handle: Box<FsmConstructHandle>) -> Box<Fsm> {
    let _ = handle;
    todo!("ported by w2-structures-dynarray")
}

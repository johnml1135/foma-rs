//! foma: a finite-state toolkit and library — Rust port.
//!
//! Wave-2 literal (bug-for-bug) port of the C foma library. See
//! docs/port/rust-conventions.md for the binding conventions. Modules
//! mirror the C source files one-to-one and are added as each Wave-2
//! concern lands.

pub mod dynarray;
pub mod int_stack;
pub mod mem;
pub mod sigma;
pub mod stringhash;
pub mod trie;
pub mod types;
pub mod utf8;

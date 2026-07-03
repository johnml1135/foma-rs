//! foma: a finite-state toolkit and library — Rust port.
//!
//! Wave-2 literal (bug-for-bug) port of the C foma library. See
//! docs/port/rust-conventions.md for the binding conventions. Modules
//! mirror the C source files one-to-one and are added as each Wave-2
//! concern lands.

pub mod constructions;
pub mod determinize;
pub mod define;
pub mod dynarray;
pub mod coaccessible;
pub mod extract;
pub mod flags;
pub mod int_stack;
pub mod mem;
pub mod regex;
pub mod reverse;
pub mod rewrite;
pub mod minimize;
pub mod sigma;
pub mod structures;
pub mod stringhash;
pub mod topsort;
pub mod trie;
pub mod types;
pub mod utf8;

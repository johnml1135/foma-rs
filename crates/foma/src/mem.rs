//! foma/mem.c — literal Wave-2 (bug-for-bug) port per
//! docs/port/rust-conventions.md. Sem rules: docs/spec/port/foma/mem.md.

use std::cell::{Cell, RefCell};

/* Global variables */
// C: non-static globals defined at the top of mem.c and `extern`'d by the
// other translation units (iface.c, stack.c, io.c, ...). They carry no spec
// ids of their own (the annotated C sites are the three functions below).
// Per the conventions, mutable globals become module-level thread_local
// statics with the C names upper-cased; non-reentrancy is part of the
// contract.
thread_local! {
    pub static G_SHOW_FLAGS: Cell<i32> = const { Cell::new(0) };
    pub static G_OBEY_FLAGS: Cell<i32> = const { Cell::new(1) };
    pub static G_FLAG_IS_EPSILON: Cell<i32> = const { Cell::new(0) };
    pub static G_PRINT_SPACE: Cell<i32> = const { Cell::new(0) };
    pub static G_PRINT_PAIRS: Cell<i32> = const { Cell::new(0) };
    pub static G_MINIMAL: Cell<i32> = const { Cell::new(1) };
    pub static G_NAME_NETS: Cell<i32> = const { Cell::new(0) };
    pub static G_PRINT_SIGMA: Cell<i32> = const { Cell::new(1) };
    pub static G_QUIT_ON_FAIL: Cell<i32> = const { Cell::new(1) };
    pub static G_QUOTE_SPECIAL: Cell<i32> = const { Cell::new(0) };
    pub static G_RECURSIVE_DEFINE: Cell<i32> = const { Cell::new(0) };
    pub static G_SORT_ARCS: Cell<i32> = const { Cell::new(1) };
    pub static G_VERBOSE: Cell<i32> = const { Cell::new(1) };
    pub static G_MINIMIZE_HOPCROFT: Cell<i32> = const { Cell::new(1) };
    pub static G_COMPOSE_TRISTATE: Cell<i32> = const { Cell::new(0) };
    pub static G_LIST_LIMIT: Cell<i32> = const { Cell::new(100) };
    pub static G_LIST_RANDOM_LIMIT: Cell<i32> = const { Cell::new(15) };
    pub static G_MED_LIMIT: Cell<i32> = const { Cell::new(3) };
    pub static G_MED_CUTOFF: Cell<i32> = const { Cell::new(15) };
    pub static G_LEXC_ALIGN: Cell<i32> = const { Cell::new(0) };
    /// C: `char *g_att_epsilon = "@0@";` — reassignable at runtime via the
    /// iface variable table (`set att-epsilon ...`), hence an owned String.
    pub static G_ATT_EPSILON: RefCell<String> = RefCell::new(String::from("@0@"));
}

// [spec:foma:def:mem.xxstrndup-fn]
// [spec:foma:sem:mem.xxstrndup-fn]
// [spec:foma:def:fomalibconf.xxstrndup-fn]
// [spec:foma:sem:fomalibconf.xxstrndup-fn]
pub fn xxstrndup(s: &str, n: usize) -> String {
    // C: p = s; while (*p++ && n--); n = p - s - 1;
    // Effective length = min(n, strlen(s)), scanning at most n + 1 bytes.
    // The end of the &str stands in for the C NUL terminator.
    let bytes = s.as_bytes();
    let mut n = n;
    let mut p: usize = 0;
    loop {
        let c = if p < bytes.len() { bytes[p] } else { 0 };
        p += 1;
        if c == 0 {
            break;
        }
        let old_n = n;
        n = n.wrapping_sub(1);
        if old_n == 0 {
            break;
        }
    }
    n = p - 1;
    // C: malloc(n + 1) returning NULL yields a NULL result; in Rust
    // allocation failure aborts, so that branch is unrepresentable.
    // DEVIATION from C (a cut inside a UTF-8 codepoint would yield an invalid
    // byte string in C; String must be valid UTF-8, so lossy-decode — every
    // foma call site cuts at symbol boundaries, where this is byte-identical)
    String::from_utf8_lossy(&bytes[..n]).into_owned()
}

// [spec:foma:def:mem.next-power-of-two-fn]
// [spec:foma:sem:mem.next-power-of-two-fn]
// [spec:foma:def:fomalibconf.next-power-of-two-fn]
// [spec:foma:sem:fomalibconf.next-power-of-two-fn]
pub fn next_power_of_two(v: i32) -> i32 {
    let mut v = v;
    let mut i: i32 = 0;
    // C: for (i=0; v > 0; i++) v = v >> 1;
    while v > 0 {
        v = v >> 1;
        i += 1;
    }
    // C: 1 << i overflows int for i == 31 (UB in C); Rust yields i32::MIN.
    1 << i
}

// [spec:foma:def:mem.round-up-to-power-of-two-fn]
// [spec:foma:sem:mem.round-up-to-power-of-two-fn]
// [spec:foma:def:fomalibconf.round-up-to-power-of-two-fn]
// [spec:foma:sem:fomalibconf.round-up-to-power-of-two-fn]
pub fn round_up_to_power_of_two(v: u32) -> u32 {
    // C v--/v++ wrap on unsigned: v = 0 smears to 0xFFFFFFFF and returns 0.
    let mut v = v;
    v = v.wrapping_sub(1);
    v |= v >> 1;
    v |= v >> 2;
    v |= v >> 4;
    v |= v >> 8;
    v |= v >> 16;
    v = v.wrapping_add(1);
    v
}

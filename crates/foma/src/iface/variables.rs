//! foma/iface.c Wave-4 split: variable get/set, the global-vars table, the
//! pair split helpers, stack-check, and the foma_net_print re-export.
//! See iface/mod.rs.
use super::*;

// [spec:foma:def:iface.foma-net-print-fn]
// [spec:foma:sem:iface.foma-net-print-fn]
// C: `extern int foma_net_print(struct fsm *net, gzFile outfile);` — a forward
// declaration in iface.c of the function implemented in foma/io.c
// (`io.foma-net-print-fn`). The Rust twin re-exports io's implementation at this
// site so this file carries the iface.c extern-declaration annotation.
pub use crate::io::foma_net_print;

const FVAR_BOOL: i32 = 1;

const FVAR_INT: i32 = 2;

const FVAR_STRING: i32 = 3;

/// DEVIATION from C: `struct g_v`'s `void *ptr` points at either an `int` global
/// (FVAR_BOOL / FVAR_INT) or a `char *` global (FVAR_STRING). Safe Rust cannot
/// hold a stable raw pointer into a thread_local, so the target is modelled as an
/// enum over the two real global kinds; the `type` field still distinguishes
/// FVAR_BOOL from FVAR_INT (both `int`), exactly as in C.
pub enum GvPtr {
    Int(&'static std::thread::LocalKey<Cell<i32>>),
    Str(&'static std::thread::LocalKey<RefCell<String>>),
}

// [spec:foma:def:iface.g-v]
// C: struct g_v { void *ptr; char *name; int type; } — element type of the
// global-variable dispatch table `global_vars[]`. The table and its consumers
// (iface_set_variable/iface_show_variable/iface_show_variables) are in the second
// half of iface.c; the table is built by `global_vars()` below.
pub struct Gv {
    pub ptr: GvPtr,
    pub name: &'static str,
    pub r#type: i32,
}

/// C: the file-static `struct g_v global_vars[]` table (NULL-terminated). Built
/// fresh here (read-only data, observably equivalent to the static array); the
/// trailing `{NULL, NULL, 0}` sentinel is represented by the end of the Vec.
pub(crate) fn global_vars() -> Vec<Gv> {
    vec![
        Gv { ptr: GvPtr::Int(&G_FLAG_IS_EPSILON), name: "flag-is-epsilon", r#type: FVAR_BOOL },
        Gv { ptr: GvPtr::Int(&G_MINIMAL), name: "minimal", r#type: FVAR_BOOL },
        Gv { ptr: GvPtr::Int(&G_NAME_NETS), name: "name-nets", r#type: FVAR_BOOL },
        Gv { ptr: GvPtr::Int(&G_OBEY_FLAGS), name: "obey-flags", r#type: FVAR_BOOL },
        Gv { ptr: GvPtr::Int(&G_PRINT_PAIRS), name: "print-pairs", r#type: FVAR_BOOL },
        Gv { ptr: GvPtr::Int(&G_PRINT_SIGMA), name: "print-sigma", r#type: FVAR_BOOL },
        Gv { ptr: GvPtr::Int(&G_PRINT_SPACE), name: "print-space", r#type: FVAR_BOOL },
        Gv { ptr: GvPtr::Int(&G_QUIT_ON_FAIL), name: "quit-on-fail", r#type: FVAR_BOOL },
        Gv { ptr: GvPtr::Int(&G_RECURSIVE_DEFINE), name: "recursive-define", r#type: FVAR_BOOL },
        Gv { ptr: GvPtr::Int(&G_QUOTE_SPECIAL), name: "quote-special", r#type: FVAR_BOOL },
        Gv { ptr: GvPtr::Int(&G_SHOW_FLAGS), name: "show-flags", r#type: FVAR_BOOL },
        Gv { ptr: GvPtr::Int(&G_SORT_ARCS), name: "sort-arcs", r#type: FVAR_BOOL },
        Gv { ptr: GvPtr::Int(&G_VERBOSE), name: "verbose", r#type: FVAR_BOOL },
        Gv { ptr: GvPtr::Int(&G_MINIMIZE_HOPCROFT), name: "hopcroft-min", r#type: FVAR_BOOL },
        Gv { ptr: GvPtr::Int(&G_COMPOSE_TRISTATE), name: "compose-tristate", r#type: FVAR_BOOL },
        Gv { ptr: GvPtr::Int(&G_MED_LIMIT), name: "med-limit", r#type: FVAR_INT },
        Gv { ptr: GvPtr::Int(&G_MED_CUTOFF), name: "med-cutoff", r#type: FVAR_INT },
        Gv { ptr: GvPtr::Int(&G_LEXC_ALIGN), name: "lexc-align", r#type: FVAR_BOOL },
        Gv { ptr: GvPtr::Str(&G_ATT_EPSILON), name: "att-epsilon", r#type: FVAR_STRING },
    ]
}

// [spec:foma:def:iface.iface-stack-check-fn]
// [spec:foma:sem:iface.iface-stack-check-fn]
// [spec:foma:def:foma.iface-stack-check-fn]
// [spec:foma:sem:foma.iface-stack-check-fn]
pub fn iface_stack_check(size: i32) -> i32 {
    if stack_size() < size {
        print!(
            "Not enough networks on stack. Operation requires at least {}.\n",
            size
        );
        return 0;
    }
    1
}

// C strncmp(a, b, 8): compares at most 8 bytes as unsigned char, stopping early
// when a shared NUL is reached; == 0 iff the first 8 bytes (or up to a common
// NUL) match. Unannotated plumbing for the variable-name lookup (the 8-char
// prefix match is the documented latent bug in iface_{set,show}_variable).
fn strncmp8(a: &str, b: &str) -> i32 {
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    for i in 0..8 {
        let ca = ab.get(i).copied().unwrap_or(0);
        let cb = bb.get(i).copied().unwrap_or(0);
        if ca != cb {
            return ca as i32 - cb as i32;
        }
        if ca == 0 {
            return 0;
        }
    }
    0
}

// C strtol(value, &endptr, 10) semantics used by iface_set_variable's FVAR_INT
// branch. Returns (result truncated to `long`=i64, endptr==value i.e. no digits
// consumed, errno==ERANGE i.e. out of long range). Unannotated plumbing.
fn c_strtol_base10(value: &str) -> (i64, bool, bool) {
    let bytes = value.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c) {
        i += 1;
    }
    let mut neg = false;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        neg = bytes[i] == b'-';
        i += 1;
    }
    let mut any = false;
    let mut acc: i64 = 0;
    let mut range = false;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        any = true;
        let d = (bytes[i] - b'0') as i64;
        if !range {
            match acc.checked_mul(10).and_then(|v| v.checked_add(d)) {
                Some(v) => acc = v,
                None => range = true,
            }
        }
        i += 1;
    }
    let result = if range {
        if neg { i64::MIN } else { i64::MAX }
    } else if neg {
        -acc
    } else {
        acc
    };
    (result, !any, range)
}

// [spec:foma:def:iface.iface-show-variables-fn]
// [spec:foma:sem:iface.iface-show-variables-fn]
// [spec:foma:def:foma.iface-show-variables-fn]
// [spec:foma:sem:foma.iface-show-variables-fn]
pub fn iface_show_variables() {
    for gv in global_vars() {
        // "%-17.17s" — left-justified, padded/truncated to exactly 17 chars.
        if gv.r#type == FVAR_BOOL {
            let v = match &gv.ptr {
                GvPtr::Int(c) => c.with(|x| x.get()),
                GvPtr::Str(_) => 0,
            };
            print!("{:<17.17}: {}\n", gv.name, if v == 1 { "ON" } else { "OFF" });
        }
        if gv.r#type == FVAR_INT {
            let v = match &gv.ptr {
                GvPtr::Int(c) => c.with(|x| x.get()),
                GvPtr::Str(_) => 0,
            };
            print!("{:<17.17}: {}\n", gv.name, v);
        }
        if gv.r#type == FVAR_STRING {
            let v = match &gv.ptr {
                GvPtr::Str(rc) => rc.with(|s| s.borrow().clone()),
                GvPtr::Int(_) => String::new(),
            };
            print!("{:<17.17}: {}\n", gv.name, v);
        }
    }
}

// [spec:foma:def:iface.iface-show-variable-fn]
// [spec:foma:sem:iface.iface-show-variable-fn+1]
// [spec:foma:def:foma.iface-show-variable-fn]
// [spec:foma:sem:foma.iface-show-variable-fn+1]
pub fn iface_show_variable(name: &str) {
    for gv in global_vars() {
        if strncmp8(name, gv.name) == 0 {
            // Wave 4 fix: the C printed ON/OFF from `*(int*)ptr == 1` for EVERY
            // type (INT variables only showed ON at value 1; STRING reinterpreted
            // the char* bytes as int). Print by declared type instead: BOOL as
            // ON/OFF, INT as its value, STRING as its string.
            if gv.r#type == FVAR_INT {
                let v = match &gv.ptr {
                    GvPtr::Int(c) => c.with(|x| x.get()),
                    GvPtr::Str(_) => 0,
                };
                print!("{} = {}\n", gv.name, v);
            } else if gv.r#type == FVAR_STRING {
                let v = match &gv.ptr {
                    GvPtr::Str(rc) => rc.with(|s| s.borrow().clone()),
                    GvPtr::Int(_) => String::new(),
                };
                print!("{} = {}\n", gv.name, v);
            } else {
                let v = match &gv.ptr {
                    GvPtr::Int(c) => c.with(|x| x.get()),
                    GvPtr::Str(_) => 0,
                };
                print!("{} = {}\n", gv.name, if v == 1 { "ON" } else { "OFF" });
            }
            return;
        }
    }
    print!("*There is no global variable '{}'.\n", name);
}

// [spec:foma:def:iface.iface-set-variable-fn]
// [spec:foma:sem:iface.iface-set-variable-fn]
// [spec:foma:def:foma.iface-set-variable-fn]
// [spec:foma:sem:foma.iface-set-variable-fn]
pub fn iface_set_variable(name: &str, value: &str) {
    for gv in global_vars() {
        if strncmp8(name, gv.name) == 0 {
            if gv.r#type == FVAR_BOOL {
                let j: i32;
                if value == "ON" || value == "1" {
                    j = 1;
                } else if value == "OFF" || value == "0" {
                    j = 0;
                } else {
                    print!("Invalid value '{}' for variable '{}'\n", value, gv.name);
                    return;
                }
                if let GvPtr::Int(c) = &gv.ptr {
                    c.with(|x| x.set(j));
                }
                let cur = match &gv.ptr {
                    GvPtr::Int(c) => c.with(|x| x.get()),
                    GvPtr::Str(_) => 0,
                };
                print!(
                    "variable {} = {}\n",
                    gv.name,
                    if cur == 1 { "ON" } else { "OFF" }
                );
                return;
            }
            if gv.r#type == FVAR_STRING {
                // *ptr = strdup(value): C leaks the old string; here it is replaced.
                if let GvPtr::Str(rc) = &gv.ptr {
                    rc.with(|s| *s.borrow_mut() = value.to_string());
                }
                print!("variable {} = {}\n", gv.name, value);
                return;
            }
            if gv.r#type == FVAR_INT {
                let (result, no_digits, range) = c_strtol_base10(value);
                // j = (int)strtol(...) — truncation to int.
                let j = result as i32;
                if range || no_digits || j < 0 {
                    print!("invalid value {} for variable {}\n", value, gv.name);
                    return;
                } else {
                    print!("variable {} = {}\n", gv.name, j);
                    if let GvPtr::Int(c) = &gv.ptr {
                        c.with(|x| x.set(j));
                    }
                    return;
                }
            }
        }
    }
    print!("*There is no global variable '{}'.\n", name);
}

// [spec:foma:def:iface.iface-split-string-fn]
// [spec:foma:sem:iface.iface-split-string-fn]
pub fn iface_split_string(result: &[u8], string: &mut Vec<u8>) {
    let space = 1u8;
    let epsilon = 2u8;
    let separator = 3u8;
    /* Simulate: SEPARATOR \SPACE+ @-> 0 .o. SPACE|SEPARATOR|EPSILON -> 0 */
    /*           to extract only the upper side of `result`.             */
    // Two-state filter (C's goto zero/one). End-of-Vec is the NUL terminator.
    let mut i = 0usize;
    let mut state = 0; // 0 = "zero" (initial), 1 = "one"
    loop {
        let c = result.get(i).copied().unwrap_or(0);
        if state == 0 {
            if c == 0 {
                break;
            } else if c == space || c == epsilon {
                i += 1;
            } else if c == separator {
                i += 1;
                state = 1;
            } else {
                string.push(c); // strncat(string, result+i, 1)
                i += 1;
            }
        } else if c == 0 {
            break;
        } else if c == space {
            i += 1;
            state = 0;
        } else {
            i += 1;
        }
    }
}

// [spec:foma:def:iface.iface-split-result-fn]
// [spec:foma:sem:iface.iface-split-result-fn]
pub fn iface_split_result(result: &mut Vec<u8>, upper: &mut Vec<u8>, lower: &mut Vec<u8>) {
    upper.clear();
    lower.clear();
    /* Split string into upper by filtering the input side, and lower by the */
    /* same filter but on the reversed string.                               */
    iface_split_string(result, upper);
    xstrrev(Some(result));
    iface_split_string(result, lower);
    xstrrev(Some(lower));
    xstrrev(Some(result));
}

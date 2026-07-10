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

/// DEVIATION from C: `struct g_v`'s `void *ptr` pointed at either an `int`
/// global (FVAR_BOOL / FVAR_INT) or a `char *` global (FVAR_STRING). The
/// options live in `Session.opts` now, so each table entry carries a field
/// projection into `FomaOptions` instead of a pointer; C's FVAR_BOOL/FVAR_INT
/// distinction (both `int` there) is carried by the Bool/Int variants.
pub enum GvField {
    Bool(fn(&mut FomaOptions) -> &mut bool),
    Int(fn(&mut FomaOptions) -> &mut i32),
    Str(fn(&mut FomaOptions) -> &mut String),
}

// [spec:foma:def:iface.g-v]
// C: struct g_v { void *ptr; char *name; int type; } — element type of the
// global-variable dispatch table `global_vars[]`. The table and its consumers
// (iface_set_variable/iface_show_variable/iface_show_variables) are in the second
// half of iface.c; the table is built by `global_vars()` below.
pub struct Gv {
    pub field: GvField,
    pub name: &'static str,
}

/// C: the file-static `struct g_v global_vars[]` table (NULL-terminated). Built
/// fresh here (read-only data, observably equivalent to the static array); the
/// trailing `{NULL, NULL, 0}` sentinel is represented by the end of the Vec.
pub(crate) fn global_vars() -> Vec<Gv> {
    vec![
        Gv {
            field: GvField::Bool(|o| &mut o.flag_is_epsilon),
            name: "flag-is-epsilon",
        },
        Gv {
            field: GvField::Bool(|o| &mut o.minimal),
            name: "minimal",
        },
        Gv {
            field: GvField::Bool(|o| &mut o.name_nets),
            name: "name-nets",
        },
        Gv {
            field: GvField::Bool(|o| &mut o.obey_flags),
            name: "obey-flags",
        },
        Gv {
            field: GvField::Bool(|o| &mut o.print_pairs),
            name: "print-pairs",
        },
        Gv {
            field: GvField::Bool(|o| &mut o.print_sigma),
            name: "print-sigma",
        },
        Gv {
            field: GvField::Bool(|o| &mut o.print_space),
            name: "print-space",
        },
        Gv {
            field: GvField::Bool(|o| &mut o.quit_on_fail),
            name: "quit-on-fail",
        },
        Gv {
            field: GvField::Bool(|o| &mut o.recursive_define),
            name: "recursive-define",
        },
        Gv {
            field: GvField::Bool(|o| &mut o.quote_special),
            name: "quote-special",
        },
        Gv {
            field: GvField::Bool(|o| &mut o.show_flags),
            name: "show-flags",
        },
        Gv {
            field: GvField::Bool(|o| &mut o.sort_arcs),
            name: "sort-arcs",
        },
        Gv {
            field: GvField::Bool(|o| &mut o.verbose),
            name: "verbose",
        },
        Gv {
            field: GvField::Bool(|o| &mut o.minimize_hopcroft),
            name: "hopcroft-min",
        },
        Gv {
            field: GvField::Bool(|o| &mut o.compose_tristate),
            name: "compose-tristate",
        },
        Gv {
            field: GvField::Int(|o| &mut o.med_limit),
            name: "med-limit",
        },
        Gv {
            field: GvField::Int(|o| &mut o.med_cutoff),
            name: "med-cutoff",
        },
        Gv {
            field: GvField::Bool(|o| &mut o.lexc_align),
            name: "lexc-align",
        },
        Gv {
            field: GvField::Str(|o| &mut o.att_epsilon),
            name: "att-epsilon",
        },
    ]
}

// [spec:foma:def:iface.iface-stack-check-fn]
// [spec:foma:sem:iface.iface-stack-check-fn]
// [spec:foma:def:foma.iface-stack-check-fn]
// [spec:foma:sem:foma.iface-stack-check-fn]
pub fn iface_stack_check(session: &mut Session, size: i32) -> bool {
    if session.stack_size() < size {
        print!(
            "Not enough networks on stack. Operation requires at least {}.\n",
            size
        );
        return false;
    }
    true
}

// Full variable-name comparison: returns 0 iff the names are equal. Plumbing for
// the variable-name lookup in iface_{set,show}_variable. C used strncmp(a, b, 8),
// comparing only the first 8 bytes, so any name sharing an 8-char prefix with a
// real variable collided (e.g. "hopcroft-XYZ" matched "hopcroft-min").
fn namecmp(a: &str, b: &str) -> i32 {
    if a == b { 0 } else { 1 }
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
pub fn iface_show_variables(session: &mut Session) {
    for gv in global_vars() {
        // "%-17.17s" — left-justified, padded/truncated to exactly 17 chars.
        match gv.field {
            GvField::Bool(f) => print!(
                "{:<17.17}: {}\n",
                gv.name,
                if *f(&mut session.opts) { "ON" } else { "OFF" }
            ),
            GvField::Int(f) => print!("{:<17.17}: {}\n", gv.name, *f(&mut session.opts)),
            GvField::Str(f) => print!("{:<17.17}: {}\n", gv.name, f(&mut session.opts)),
        }
    }
}

// [spec:foma:def:iface.iface-show-variable-fn]
// [spec:foma:sem:iface.iface-show-variable-fn+2]
// [spec:foma:def:foma.iface-show-variable-fn]
// [spec:foma:sem:foma.iface-show-variable-fn+2]
pub fn iface_show_variable(session: &mut Session, name: &str) {
    for gv in global_vars() {
        if namecmp(name, gv.name) == 0 {
            // Wave 4 fix: the C printed ON/OFF from `*(int*)ptr == 1` for EVERY
            // type (INT variables only showed ON at value 1; STRING reinterpreted
            // the char* bytes as int). Print by declared type instead: BOOL as
            // ON/OFF, INT as its value, STRING as its string.
            match gv.field {
                GvField::Int(f) => print!("{} = {}\n", gv.name, *f(&mut session.opts)),
                GvField::Str(f) => print!("{} = {}\n", gv.name, f(&mut session.opts)),
                GvField::Bool(f) => print!(
                    "{} = {}\n",
                    gv.name,
                    if *f(&mut session.opts) { "ON" } else { "OFF" }
                ),
            }
            return;
        }
    }
    print!("*There is no global variable '{}'.\n", name);
}

// [spec:foma:def:iface.iface-set-variable-fn]
// [spec:foma:sem:iface.iface-set-variable-fn+1]
// [spec:foma:def:foma.iface-set-variable-fn]
// [spec:foma:sem:foma.iface-set-variable-fn+1]
pub fn iface_set_variable(session: &mut Session, name: &str, value: &str) {
    for gv in global_vars() {
        if namecmp(name, gv.name) == 0 {
            match gv.field {
                GvField::Bool(f) => {
                    let j: bool;
                    if value == "ON" || value == "1" {
                        j = true;
                    } else if value == "OFF" || value == "0" {
                        j = false;
                    } else {
                        print!("Invalid value '{}' for variable '{}'\n", value, gv.name);
                        return;
                    }
                    *f(&mut session.opts) = j;
                    print!(
                        "variable {} = {}\n",
                        gv.name,
                        if *f(&mut session.opts) { "ON" } else { "OFF" }
                    );
                }
                GvField::Str(f) => {
                    // *ptr = strdup(value): C leaks the old string; replaced here.
                    *f(&mut session.opts) = value.to_string();
                    print!("variable {} = {}\n", gv.name, value);
                }
                GvField::Int(f) => {
                    let (result, no_digits, range) = c_strtol_base10(value);
                    // j = (int)strtol(...) — truncation to int.
                    let j = result as i32;
                    if range || no_digits || j < 0 {
                        print!("invalid value {} for variable {}\n", value, gv.name);
                    } else {
                        print!("variable {} = {}\n", gv.name, j);
                        *f(&mut session.opts) = j;
                    }
                }
            }
            return;
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

//! foma/iface.c Wave-4 split: single-net commands (minimize/determinize/
//! invert/reverse/…/extract/sort). See iface/mod.rs.
use super::*;

// [spec:foma:def:iface.iface-ambiguous-upper-fn]
// [spec:foma:sem:iface.iface-ambiguous-upper-fn]
// [spec:foma:def:foma.iface-ambiguous-upper-fn]
// [spec:foma:sem:foma.iface-ambiguous-upper-fn]
pub fn iface_ambiguous_upper() {
    if iface_stack_check(1) != 0 {
        stack_add(fsm_extract_ambiguous_domain(stack_pop().unwrap()));
    }
}

// [spec:foma:def:iface.iface-close-fn]
// [spec:foma:sem:iface.iface-close-fn]
// [spec:foma:def:foma.iface-close-fn]
// [spec:foma:sem:foma.iface-close-fn]
pub fn iface_close() {
    if iface_stack_check(1) != 0 {
        stack_add(fsm_topsort(fsm_minimize(fsm_close_sigma(
            stack_pop().unwrap(),
            0,
        ))));
    }
}

// [spec:foma:def:iface.iface-compact-fn]
// [spec:foma:sem:iface.iface-compact-fn]
// [spec:foma:def:foma.iface-compact-fn]
// [spec:foma:sem:foma.iface-compact-fn]
pub fn iface_compact() {
    if iface_stack_check(1) != 0 {
        let top = stack_find_top().unwrap();
        stack_entry_fsm(top, |f| fsm_compact(f));
        stack_entry_fsm(top, |f| sigma_sort(f));
        stack_add(fsm_topsort(fsm_minimize(stack_pop().unwrap())));
    }
}

// [spec:foma:def:iface.iface-complete-fn]
// [spec:foma:sem:iface.iface-complete-fn]
// [spec:foma:def:foma.iface-complete-fn]
// [spec:foma:sem:foma.iface-complete-fn]
pub fn iface_complete() {
    if iface_stack_check(1) != 0 {
        stack_add(fsm_complete(stack_pop().unwrap()));
    }
}

// [spec:foma:def:iface.iface-determinize-fn]
// [spec:foma:sem:iface.iface-determinize-fn]
// [spec:foma:def:foma.iface-determinize-fn]
// [spec:foma:sem:foma.iface-determinize-fn]
pub fn iface_determinize() {
    if iface_stack_check(1) != 0 {
        stack_add(fsm_determinize(stack_pop().unwrap()));
    }
}

// [spec:foma:def:iface.iface-eliminate-flags-fn]
// [spec:foma:sem:iface.iface-eliminate-flags-fn]
// [spec:foma:def:foma.iface-eliminate-flags-fn]
// [spec:foma:sem:foma.iface-eliminate-flags-fn]
pub fn iface_eliminate_flags() {
    if iface_stack_check(1) != 0 {
        stack_add(flag_eliminate(stack_pop().unwrap(), None));
    }
}

// [spec:foma:def:iface.iface-extract-ambiguous-fn]
// [spec:foma:sem:iface.iface-extract-ambiguous-fn]
// [spec:foma:def:foma.iface-extract-ambiguous-fn]
// [spec:foma:sem:foma.iface-extract-ambiguous-fn]
pub fn iface_extract_ambiguous() {
    if iface_stack_check(1) != 0 {
        stack_add(fsm_extract_ambiguous(stack_pop().unwrap()));
    }
}

// [spec:foma:def:iface.iface-extract-unambiguous-fn]
// [spec:foma:sem:iface.iface-extract-unambiguous-fn]
// [spec:foma:def:foma.iface-extract-unambiguous-fn]
// [spec:foma:sem:foma.iface-extract-unambiguous-fn]
pub fn iface_extract_unambiguous() {
    if iface_stack_check(1) != 0 {
        stack_add(fsm_extract_unambiguous(stack_pop().unwrap()));
    }
}

// C atoi: skip leading whitespace, optional +/-, then decimal digits until a
// non-digit; empty/no-digit → 0. Overflow is UB in C; wrapping here. Unannotated
// plumbing used by iface_extract_number.
fn atoi(s: &str) -> i32 {
    let bytes = s.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c) {
        i += 1;
    }
    let mut sign: i32 = 1;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        if bytes[i] == b'-' {
            sign = -1;
        }
        i += 1;
    }
    let mut n: i32 = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        n = n.wrapping_mul(10).wrapping_add((bytes[i] - b'0') as i32);
        i += 1;
    }
    sign.wrapping_mul(n)
}

// [spec:foma:def:iface.iface-extract-number-fn]
// [spec:foma:sem:iface.iface-extract-number-fn+1]
// [spec:foma:def:foma.iface-extract-number-fn]
// [spec:foma:sem:foma.iface-extract-number-fn+1]
pub fn iface_extract_number(s: &str) -> i32 {
    // Scan to the first ASCII digit (compared as unsigned char), then atoi.
    let bytes = s.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() && (bytes[i] < b'0' || bytes[i] > b'9') {
        i += 1;
    }
    // Wave 4 fix: the C scan stopped AT the first digit, dropping a leading '-'
    // so negatives read positive ("abc-5" -> 5). Include a '-' immediately before
    // the first digit so the sign is parsed ("abc-5" -> -5).
    if i > 0 && i < bytes.len() && bytes[i - 1] == b'-' {
        i -= 1;
    }
    atoi(&s[i..])
}

// [spec:foma:def:iface.iface-eliminate-flag-fn]
// [spec:foma:sem:iface.iface-eliminate-flag-fn]
// [spec:foma:def:foma.iface-eliminate-flag-fn]
// [spec:foma:sem:foma.iface-eliminate-flag-fn]
pub fn iface_eliminate_flag(name: &str) {
    if iface_stack_check(1) != 0 {
        stack_add(flag_eliminate(stack_pop().unwrap(), Some(name)));
    }
}

// [spec:foma:def:iface.iface-factorize-fn]
// [spec:foma:sem:iface.iface-factorize-fn]
// [spec:foma:def:foma.iface-factorize-fn]
// [spec:foma:sem:foma.iface-factorize-fn]
pub fn iface_factorize() {
    if iface_stack_check(1) != 0 {
        stack_add(fsm_bimachine(stack_pop().unwrap()));
    }
}

// [spec:foma:def:iface.iface-sequentialize-fn]
// [spec:foma:sem:iface.iface-sequentialize-fn]
// [spec:foma:def:foma.iface-sequentialize-fn]
// [spec:foma:sem:foma.iface-sequentialize-fn]
pub fn iface_sequentialize() {
    if iface_stack_check(1) != 0 {
        stack_add(fsm_sequentialize(stack_pop().unwrap()));
    }
}

// [spec:foma:def:iface.iface-invert-fn]
// [spec:foma:sem:iface.iface-invert-fn]
// [spec:foma:def:foma.iface-invert-fn]
// [spec:foma:sem:foma.iface-invert-fn]
pub fn iface_invert() {
    if iface_stack_check(1) != 0 {
        stack_add(fsm_invert(stack_pop().unwrap()));
    }
}

// [spec:foma:def:iface.iface-label-net-fn]
// [spec:foma:sem:iface.iface-label-net-fn]
// [spec:foma:def:foma.iface-label-net-fn]
// [spec:foma:sem:foma.iface-label-net-fn]
pub fn iface_label_net() {
    if iface_stack_check(1) != 0 {
        stack_add(fsm_sigma_pairs_net(stack_pop().unwrap()));
    }
}

// [spec:foma:def:iface.iface-letter-machine-fn]
// [spec:foma:sem:iface.iface-letter-machine-fn]
// [spec:foma:def:foma.iface-letter-machine-fn]
// [spec:foma:sem:foma.iface-letter-machine-fn]
pub fn iface_letter_machine() {
    if iface_stack_check(1) != 0 {
        stack_add(fsm_topsort(fsm_minimize(fsm_letter_machine(
            stack_pop().unwrap(),
        ))));
    }
}

// [spec:foma:def:iface.iface-lower-side-fn]
// [spec:foma:sem:iface.iface-lower-side-fn]
// [spec:foma:def:foma.iface-lower-side-fn]
// [spec:foma:sem:foma.iface-lower-side-fn]
pub fn iface_lower_side() {
    if iface_stack_check(1) != 0 {
        stack_add(fsm_topsort(fsm_minimize(fsm_lower(stack_pop().unwrap()))));
    }
}

// [spec:foma:def:iface.iface-minimize-fn]
// [spec:foma:sem:iface.iface-minimize-fn]
// [spec:foma:def:foma.iface-minimize-fn]
// [spec:foma:sem:foma.iface-minimize-fn]
pub fn iface_minimize() {
    if iface_stack_check(1) != 0 {
        let store_minimal_var = G_MINIMAL.with(|v| v.get());
        G_MINIMAL.with(|v| v.set(1));
        stack_add(fsm_topsort(fsm_minimize(stack_pop().unwrap())));
        G_MINIMAL.with(|v| v.set(store_minimal_var));
    }
}

// [spec:foma:def:iface.iface-one-plus-fn]
// [spec:foma:sem:iface.iface-one-plus-fn]
// [spec:foma:def:foma.iface-one-plus-fn]
// [spec:foma:sem:foma.iface-one-plus-fn]
pub fn iface_one_plus() {
    if iface_stack_check(1) != 0 {
        stack_add(fsm_topsort(fsm_minimize(fsm_kleene_plus(
            stack_pop().unwrap(),
        ))));
    }
}

// [spec:foma:def:iface.iface-negate-fn]
// [spec:foma:sem:iface.iface-negate-fn]
// [spec:foma:def:foma.iface-negate-fn]
// [spec:foma:sem:foma.iface-negate-fn]
pub fn iface_negate() {
    if iface_stack_check(1) != 0 {
        stack_add(fsm_topsort(fsm_minimize(fsm_complement(
            stack_pop().unwrap(),
        ))));
    }
}

// [spec:foma:def:iface.iface-prune-fn]
// [spec:foma:sem:iface.iface-prune-fn]
// [spec:foma:def:foma.iface-prune-fn]
// [spec:foma:sem:foma.iface-prune-fn]
pub fn iface_prune() {
    if iface_stack_check(1) != 0 {
        stack_add(fsm_topsort(fsm_coaccessible(stack_pop().unwrap())));
    }
}

// [spec:foma:def:iface.iface-reverse-fn]
// [spec:foma:sem:iface.iface-reverse-fn]
// [spec:foma:def:foma.iface-reverse-fn]
// [spec:foma:sem:foma.iface-reverse-fn]
pub fn iface_reverse() {
    if iface_stack_check(1) != 0 {
        stack_add(fsm_topsort(fsm_determinize(fsm_reverse(
            stack_pop().unwrap(),
        ))));
    }
}

// [spec:foma:def:iface.iface-sigma-net-fn]
// [spec:foma:sem:iface.iface-sigma-net-fn]
// [spec:foma:def:foma.iface-sigma-net-fn]
// [spec:foma:sem:foma.iface-sigma-net-fn]
pub fn iface_sigma_net() {
    if iface_stack_check(1) != 0 {
        stack_add(fsm_sigma_net(stack_pop().unwrap()));
    }
}

// [spec:foma:def:iface.iface-sort-input-fn]
// [spec:foma:sem:iface.iface-sort-input-fn]
// [spec:foma:def:foma.iface-sort-input-fn]
// [spec:foma:sem:foma.iface-sort-input-fn]
pub fn iface_sort_input() {
    if iface_stack_check(1) != 0 {
        let top = stack_find_top().unwrap();
        stack_entry_fsm(top, |f| fsm_sort_arcs(f, 1));
    }
}

// [spec:foma:def:iface.iface-sort-output-fn]
// [spec:foma:sem:iface.iface-sort-output-fn]
// [spec:foma:def:foma.iface-sort-output-fn]
// [spec:foma:sem:foma.iface-sort-output-fn]
pub fn iface_sort_output() {
    if iface_stack_check(1) != 0 {
        let top = stack_find_top().unwrap();
        stack_entry_fsm(top, |f| fsm_sort_arcs(f, 2));
    }
}

// [spec:foma:def:iface.iface-sort-fn]
// [spec:foma:sem:iface.iface-sort-fn]
// [spec:foma:def:foma.iface-sort-fn]
// [spec:foma:sem:foma.iface-sort-fn]
pub fn iface_sort() {
    if iface_stack_check(1) != 0 {
        let top = stack_find_top().unwrap();
        stack_entry_fsm(top, |f| sigma_sort(f));
        stack_add(fsm_topsort(stack_pop().unwrap()));
    }
}

// [spec:foma:def:iface.iface-twosided-flags-fn]
// [spec:foma:sem:iface.iface-twosided-flags-fn]
// [spec:foma:def:foma.iface-twosided-flags-fn]
// [spec:foma:sem:foma.iface-twosided-flags-fn]
pub fn iface_twosided_flags() {
    if iface_stack_check(1) != 0 {
        stack_add(flag_twosided(stack_pop().unwrap()));
    }
}

// [spec:foma:def:iface.iface-upper-side-fn]
// [spec:foma:sem:iface.iface-upper-side-fn]
// [spec:foma:def:foma.iface-upper-side-fn]
// [spec:foma:sem:foma.iface-upper-side-fn]
pub fn iface_upper_side() {
    if iface_stack_check(1) != 0 {
        stack_add(fsm_topsort(fsm_minimize(fsm_upper(stack_pop().unwrap()))));
    }
}

// [spec:foma:def:iface.iface-zero-plus-fn]
// [spec:foma:sem:iface.iface-zero-plus-fn]
// [spec:foma:def:foma.iface-zero-plus-fn]
// [spec:foma:sem:foma.iface-zero-plus-fn]
pub fn iface_zero_plus() {
    if iface_stack_check(1) != 0 {
        stack_add(fsm_topsort(fsm_minimize(fsm_kleene_star(
            stack_pop().unwrap(),
        ))));
    }
}

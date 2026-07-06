//! foma/iface.c Wave-4 split: test-predicate commands (iface_test_*).
//! See iface/mod.rs.
use super::*;

// [spec:foma:def:iface.iface-test-equivalent-fn]
// [spec:foma:sem:iface.iface-test-equivalent-fn]
// [spec:foma:def:foma.iface-test-equivalent-fn]
// [spec:foma:sem:foma.iface-test-equivalent-fn]
pub fn iface_test_equivalent() {
    if iface_stack_check(2) != 0 {
        let top = stack_find_top().unwrap();
        let second = stack_find_second().unwrap();
        let mut one = stack_entry_fsm(top, |f| fsm_copy(f));
        let mut two = stack_entry_fsm(second, |f| fsm_copy(f));
        fsm_count(&mut one);
        fsm_count(&mut two);
        // Latent leak in C: the two copies are never fsm_destroy'd; here they are
        // consumed (freed) by fsm_equivalent — no-op observable difference.
        iface_print_bool(fsm_equivalent(one, two));
    }
}

// [spec:foma:def:iface.iface-test-functional-fn]
// [spec:foma:sem:iface.iface-test-functional-fn]
// [spec:foma:def:foma.iface-test-functional-fn]
// [spec:foma:sem:foma.iface-test-functional-fn]
pub fn iface_test_functional() {
    if iface_stack_check(1) != 0 {
        let top = stack_find_top().unwrap();
        let r = stack_entry_fsm(top, |f| fsm_isfunctional(f));
        iface_print_bool(r);
    }
}

// [spec:foma:def:iface.iface-test-identity-fn]
// [spec:foma:sem:iface.iface-test-identity-fn]
// [spec:foma:def:foma.iface-test-identity-fn]
// [spec:foma:sem:foma.iface-test-identity-fn]
pub fn iface_test_identity() {
    if iface_stack_check(1) != 0 {
        let top = stack_find_top().unwrap();
        let r = stack_entry_fsm(top, |f| fsm_isidentity(f));
        iface_print_bool(r);
    }
}

// [spec:foma:def:iface.iface-test-nonnull-fn]
// [spec:foma:sem:iface.iface-test-nonnull-fn]
// [spec:foma:def:foma.iface-test-nonnull-fn]
// [spec:foma:sem:foma.iface-test-nonnull-fn]
pub fn iface_test_nonnull() {
    if iface_stack_check(1) != 0 {
        let top = stack_find_top().unwrap();
        // C: iface_print_bool(!fsm_isempty(...)) — logical NOT of the int result.
        let e = stack_entry_fsm(top, |f| fsm_isempty(f));
        iface_print_bool((e == 0) as i32);
    }
}

// [spec:foma:def:iface.iface-test-null-fn]
// [spec:foma:sem:iface.iface-test-null-fn]
// [spec:foma:def:foma.iface-test-null-fn]
// [spec:foma:sem:foma.iface-test-null-fn]
pub fn iface_test_null() {
    if iface_stack_check(1) != 0 {
        let top = stack_find_top().unwrap();
        let r = stack_entry_fsm(top, |f| fsm_isempty(f));
        iface_print_bool(r);
    }
}

// [spec:foma:def:iface.iface-test-unambiguous-fn]
// [spec:foma:sem:iface.iface-test-unambiguous-fn]
// [spec:foma:def:foma.iface-test-unambiguous-fn]
// [spec:foma:sem:foma.iface-test-unambiguous-fn]
pub fn iface_test_unambiguous() {
    if iface_stack_check(1) != 0 {
        let top = stack_find_top().unwrap();
        let r = stack_entry_fsm(top, |f| fsm_isunambiguous(f));
        iface_print_bool(r);
    }
}

// [spec:foma:def:iface.iface-test-lower-universal-fn]
// [spec:foma:sem:iface.iface-test-lower-universal-fn]
// [spec:foma:def:foma.iface-test-lower-universal-fn]
// [spec:foma:sem:foma.iface-test-lower-universal-fn]
pub fn iface_test_lower_universal() {
    if iface_stack_check(1) != 0 {
        let top = stack_find_top().unwrap();
        let mut tmp = fsm_complement(fsm_lower(stack_entry_fsm(top, |f| fsm_copy(f))));
        iface_print_bool(fsm_isempty(&mut tmp));
        fsm_destroy(tmp);
    }
}

// [spec:foma:def:iface.iface-test-sequential-fn]
// [spec:foma:sem:iface.iface-test-sequential-fn]
// [spec:foma:def:foma.iface-test-sequential-fn]
// [spec:foma:sem:foma.iface-test-sequential-fn]
pub fn iface_test_sequential() {
    if iface_stack_check(1) != 0 {
        let top = stack_find_top().unwrap();
        let r = stack_entry_fsm(top, |f| fsm_issequential(f));
        iface_print_bool(r);
    }
}

// [spec:foma:def:iface.iface-test-upper-universal-fn]
// [spec:foma:sem:iface.iface-test-upper-universal-fn]
// [spec:foma:def:foma.iface-test-upper-universal-fn]
// [spec:foma:sem:foma.iface-test-upper-universal-fn]
pub fn iface_test_upper_universal() {
    if iface_stack_check(1) != 0 {
        let top = stack_find_top().unwrap();
        let mut tmp = fsm_complement(fsm_upper(stack_entry_fsm(top, |f| fsm_copy(f))));
        iface_print_bool(fsm_isempty(&mut tmp));
        fsm_destroy(tmp);
    }
}

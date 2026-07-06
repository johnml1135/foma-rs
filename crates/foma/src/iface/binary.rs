//! foma/iface.c Wave-4 split: multi-net commands (compose/concat/union/
//! intersect/crossproduct/ignore/shuffle/substitute). See iface/mod.rs.
use super::*;

// [spec:foma:def:iface.iface-compose-fn]
// [spec:foma:sem:iface.iface-compose-fn]
// [spec:foma:def:foma.iface-compose-fn]
// [spec:foma:sem:foma.iface-compose-fn]
pub fn iface_compose() {
    if iface_stack_check(2) != 0 {
        while stack_size() > 1 {
            let one = stack_pop().unwrap();
            let two = stack_pop().unwrap();
            stack_add(fsm_topsort(fsm_minimize(fsm_compose(one, two))));
        }
    }
}

// [spec:foma:def:iface.iface-conc-fn]
// [spec:foma:sem:iface.iface-conc-fn+1]
// [spec:foma:def:foma.iface-conc-fn]
// [spec:foma:sem:foma.iface-conc-fn+1]
pub fn iface_conc() {
    if iface_stack_check(2) != 0 {
        while stack_size() > 1 {
            // Wave 4 fix: the C left a stray debug printf("dd") here — deleted.
            let one = stack_pop().unwrap();
            let two = stack_pop().unwrap();
            stack_add(fsm_topsort(fsm_minimize(fsm_concat(one, two))));
        }
    }
}

// [spec:foma:def:iface.iface-crossproduct-fn]
// [spec:foma:sem:iface.iface-crossproduct-fn]
// [spec:foma:def:foma.iface-crossproduct-fn]
// [spec:foma:sem:foma.iface-crossproduct-fn]
pub fn iface_crossproduct() {
    if iface_stack_check(2) != 0 {
        let one = stack_pop().unwrap();
        let two = stack_pop().unwrap();
        stack_add(fsm_topsort(fsm_minimize(fsm_cross_product(one, two))));
    }
}

// [spec:foma:def:iface.iface-ignore-fn]
// [spec:foma:sem:iface.iface-ignore-fn]
// [spec:foma:def:foma.iface-ignore-fn]
// [spec:foma:sem:foma.iface-ignore-fn]
pub fn iface_ignore() {
    if iface_stack_check(2) != 0 {
        let one = stack_pop().unwrap();
        let two = stack_pop().unwrap();
        stack_add(fsm_topsort(fsm_minimize(fsm_ignore(one, two, OP_IGNORE_ALL))));
    }
}

// [spec:foma:def:iface.iface-intersect-fn]
// [spec:foma:sem:iface.iface-intersect-fn]
// [spec:foma:def:foma.iface-intersect-fn]
// [spec:foma:sem:foma.iface-intersect-fn]
pub fn iface_intersect() {
    if iface_stack_check(2) != 0 {
        while stack_size() > 1 {
            // C: fsm_intersect(stack_pop(), stack_pop()) — the two pops are one
            // expression (C order unspecified); Rust evaluates arguments
            // left-to-right. Intersection is commutative, so the language matches.
            let a = stack_pop().unwrap();
            let b = stack_pop().unwrap();
            stack_add(fsm_topsort(fsm_minimize(fsm_intersect(a, b))));
        }
    }
}

// [spec:foma:def:iface.iface-substitute-symbol-fn]
// [spec:foma:sem:iface.iface-substitute-symbol-fn]
// [spec:foma:def:foma.iface-substitute-symbol-fn]
// [spec:foma:sem:foma.iface-substitute-symbol-fn]
pub fn iface_substitute_symbol(original: &str, substitute: &str) {
    if iface_stack_check(1) != 0 {
        // DEVIATION from C: C dequotes the caller's `char *` buffers in place; the
        // args are &str here, so local byte copies are dequoted instead (observably
        // identical — the printed strings and the fsm op both use the dequoted text).
        let mut original = original.as_bytes().to_vec();
        let mut substitute = substitute.as_bytes().to_vec();
        dequote_string(&mut original);
        dequote_string(&mut substitute);
        let original = String::from_utf8_lossy(&original).into_owned();
        let substitute = String::from_utf8_lossy(&substitute).into_owned();
        stack_add(fsm_topsort(fsm_minimize(fsm_substitute_symbol(
            stack_pop().unwrap(),
            &original,
            &substitute,
        ))));
        print!("Substituted '{}' for '{}'.\n", substitute, original);
    }
}

// [spec:foma:def:iface.iface-substitute-defined-fn]
// [spec:foma:sem:iface.iface-substitute-defined-fn]
// [spec:foma:def:foma.iface-substitute-defined-fn]
// [spec:foma:sem:foma.iface-substitute-defined-fn]
pub fn iface_substitute_defined(original: &str, substitute: &str) {
    if iface_stack_check(1) != 0 {
        // DEVIATION from C: see iface_substitute_symbol — dequote on local copies.
        let mut original = original.as_bytes().to_vec();
        let mut substitute = substitute.as_bytes().to_vec();
        dequote_string(&mut original);
        dequote_string(&mut substitute);
        let original = String::from_utf8_lossy(&original).into_owned();
        let substitute = String::from_utf8_lossy(&substitute).into_owned();
        G_DEFINES.with(|g| {
            let mut g = g.borrow_mut();
            // find_defined(g_defines, substitute): NULL g_defines ↔ not found.
            let subnet = match g.as_deref_mut() {
                Some(defs) => find_defined(defs, &substitute),
                None => None,
            };
            match subnet {
                None => {
                    print!("No defined network '{}'.\n", substitute);
                }
                Some(subnet) => {
                    let top = stack_find_top().unwrap();
                    if stack_entry_fsm(top, |f| fsm_symbol_occurs(f, &original, M_UPPER + M_LOWER))
                        == 0
                    {
                        print!("Symbol '{}' does not occur.\n", original);
                    } else {
                        let newnet =
                            stack_entry_fsm(top, |f| fsm_substitute_label(f, &original, subnet));
                        // C: stack_pop() — the popped net is NOT fsm_destroy'd (latent
                        // leak); here the returned Box is dropped (freed) instead.
                        let _ = stack_pop();
                        print!("Substituted network '{}' for '{}'.\n", substitute, original);
                        stack_add(fsm_topsort(fsm_minimize(newnet)));
                    }
                }
            }
        });
    }
}

// [spec:foma:def:iface.iface-shuffle-fn]
// [spec:foma:sem:iface.iface-shuffle-fn]
// [spec:foma:def:foma.iface-shuffle-fn]
// [spec:foma:sem:foma.iface-shuffle-fn]
pub fn iface_shuffle() {
    if iface_stack_check(2) != 0 {
        while stack_size() > 1 {
            // C: fsm_shuffle(stack_pop(), stack_pop()) — two pops in one expression
            // (C order unspecified); Rust evaluates left-to-right. Shuffle is
            // commutative, so the resulting language is the same.
            let a = stack_pop().unwrap();
            let b = stack_pop().unwrap();
            stack_add(fsm_minimize(fsm_shuffle(a, b)));
        }
    }
}

// [spec:foma:def:iface.iface-union-fn]
// [spec:foma:sem:iface.iface-union-fn]
// [spec:foma:def:foma.iface-union-fn]
// [spec:foma:sem:foma.iface-union-fn]
pub fn iface_union() {
    if iface_stack_check(2) != 0 {
        while stack_size() > 1 {
            // C: fsm_union(stack_pop(), stack_pop()) — pops in one expression (C
            // order unspecified); union is commutative. Minimized, NOT topsorted.
            let a = stack_pop().unwrap();
            let b = stack_pop().unwrap();
            stack_add(fsm_minimize(fsm_union(a, b)));
        }
    }
}

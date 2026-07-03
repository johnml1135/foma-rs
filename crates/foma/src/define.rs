//! foma/define.c — literal Wave-2 (bug-for-bug) port per
//! docs/port/rust-conventions.md. Sem rules: docs/spec/port/foma/define.md.

use std::cell::RefCell;

use crate::constructions::fsm_count;
use crate::mem::G_VERBOSE;
use crate::structures::fsm_destroy;
use crate::types::{DefinedFunctions, DefinedNetworks, FSM_NAME_LEN, Fsm};

/* Global variables */
// C: non-static globals defined at the top of define.c and `extern`'d by
// iface.c/foma.c/stack.c. They carry no spec ids of their own (the annotated
// C sites are the functions below). Initialized by foma.c's main via
// defined_networks_init()/defined_functions_init() (w2-cli concern).
thread_local! {
    pub static G_DEFINES: RefCell<Option<Box<DefinedNetworks>>> = const { RefCell::new(None) };
    pub static G_DEFINES_F: RefCell<Option<Box<DefinedFunctions>>> = const { RefCell::new(None) };
}

/* Find a defined symbol from the symbol table */
/* Return the corresponding FSM                */
// [spec:foma:def:define.find-defined-fn]
// [spec:foma:sem:define.find-defined-fn]
// [spec:foma:def:fomalib.find-defined-fn]
// [spec:foma:sem:fomalib.find-defined-fn]
pub fn find_defined<'a>(def: &'a mut DefinedNetworks, string: &str) -> Option<&'a mut Fsm> {
    /* C: returns NULL when def == NULL (loop runs zero times); a &mut borrow
    is never NULL — NULL-able callers keep the check at the call site. The
    returned net is the registry's own copy (borrowed, not duplicated). */
    let mut d = Some(def);
    while let Some(node) = d {
        if node.name.is_some() && node.name.as_deref() == Some(string) {
            return node.net.as_deref_mut();
        }
        d = node.next.as_deref_mut();
    }
    None
}

// [spec:foma:def:define.defined-networks-init-fn]
// [spec:foma:sem:define.defined-networks-init-fn]
// [spec:foma:def:fomalib.defined-networks-init-fn]
// [spec:foma:sem:fomalib.defined-networks-init-fn]
pub fn defined_networks_init() -> Box<DefinedNetworks> {
    /* calloc: Dummy first entry, so we can maintain the ptr */
    Box::new(DefinedNetworks {
        name: None,
        net: None,
        next: None,
    })
}

// [spec:foma:def:define.defined-functions-init-fn]
// [spec:foma:sem:define.defined-functions-init-fn]
// [spec:foma:def:fomalib.defined-functions-init-fn]
// [spec:foma:sem:fomalib.defined-functions-init-fn]
pub fn defined_functions_init() -> Box<DefinedFunctions> {
    /* calloc: Dummy first entry */
    Box::new(DefinedFunctions {
        name: None,
        regex: None,
        numargs: 0,
        next: None,
    })
}

/* Removes a defined network from the list                 */
/* Returns 0 on success, 1 if the definition did not exist */
/* Undefines all if NULL is passed as the string argument  */

// [spec:foma:def:define.remove-defined-fn]
// [spec:foma:sem:define.remove-defined-fn]
// [spec:foma:def:fomalib.remove-defined-fn]
// [spec:foma:sem:fomalib.remove-defined-fn]
pub fn remove_defined(def: &mut DefinedNetworks, string: Option<&str>) -> i32 {
    /* Undefine all */
    if string.is_none() {
        let mut d: Option<&mut DefinedNetworks> = Some(def);
        while let Some(node) = d {
            if let Some(net) = node.net.take() {
                fsm_destroy(net);
            }
            /* free(d->name) */
            // DEVIATION from C (the C frees every node's net/name but leaves
            // the fields dangling and the nodes in place; safe Rust clears
            // name/net to None — the nodes themselves stay in the list, as
            // in C, so the registry keeps its node count but reads as empty
            // instead of reading freed memory)
            node.name = None;
            d = node.next.as_deref_mut();
        }
        return 0;
    }
    let string = string.unwrap();
    /* C scans once tracking d and d_prev; here: the same existence scan,
    then the head and non-head cases through fresh borrows */
    let mut exists = 0;
    {
        let mut d = Some(&*def);
        while let Some(node) = d {
            if node.name.is_some() && node.name.as_deref() == Some(string) {
                exists = 1;
                break;
            }
            d = node.next.as_deref();
        }
    }
    if exists == 0 {
        return 1;
    }
    if def.name.is_some() && def.name.as_deref() == Some(string) {
        /* d == def */
        if def.next.is_some() {
            /* fsm_destroy(d->net) — C's fsm_destroy is a no-op on NULL */
            if let Some(net) = def.net.take() {
                fsm_destroy(net);
            }
            /* free(d->name) — dropped by the overwrite below */
            let mut next = def.next.take().unwrap();
            def.name = next.name.take();
            def.net = next.net.take();
            let d_next = next.next.take();
            /* free(d->next) — `next` dropped */
            def.next = d_next;
        } else {
            if let Some(net) = def.net.take() {
                fsm_destroy(net);
            }
            /* free(d->name) */
            def.next = None;
            def.name = None;
            def.net = None;
        }
    } else {
        let mut d_prev = &mut *def;
        loop {
            let matched = match d_prev.next.as_deref() {
                Some(d) => d.name.is_some() && d.name.as_deref() == Some(string),
                None => break, /* unreachable: existence established above */
            };
            if matched {
                let mut node = d_prev.next.take().unwrap();
                if let Some(net) = node.net.take() {
                    fsm_destroy(net);
                }
                /* free(d->name); d_prev->next = d->next; free(d) */
                d_prev.next = node.next.take();
                break;
            }
            d_prev = d_prev.next.as_deref_mut().unwrap();
        }
    }
    0
}

/* Finds defined regex "function" based on name, numargs */
/* Returns the corresponding regex                       */
// [spec:foma:def:define.find-defined-function-fn]
// [spec:foma:sem:define.find-defined-function-fn]
// [spec:foma:def:fomalib.find-defined-function-fn]
// [spec:foma:sem:fomalib.find-defined-function-fn]
pub fn find_defined_function<'a>(
    deff: &'a DefinedFunctions,
    name: &str,
    numargs: i32,
) -> Option<&'a str> {
    /* returns the stored regex string, borrowed from the list (not a copy) */
    let mut d = Some(deff);
    while let Some(node) = d {
        if node.name.is_some() && node.name.as_deref() == Some(name) && node.numargs == numargs {
            return node.regex.as_deref();
        }
        d = node.next.as_deref();
    }
    None
}

/* Add a function to list of defined functions */
// [spec:foma:def:define.add-defined-function-fn]
// [spec:foma:sem:define.add-defined-function-fn]
// [spec:foma:def:fomalib.add-defined-function-fn]
// [spec:foma:sem:fomalib.add-defined-function-fn]
pub fn add_defined_function(
    deff: &mut DefinedFunctions,
    name: &str,
    regex: &str,
    numargs: i32,
) -> i32 {
    let mut d = Some(&mut *deff);
    while let Some(node) = d {
        if node.name.is_some() && node.name.as_deref() == Some(name) && node.numargs == numargs {
            /* free(d->regex); d->regex = strdup(regex) */
            node.regex = Some(regex.to_string());
            if G_VERBOSE.with(|v| v.get()) != 0 {
                /* literal C message, including the unbalanced trailing ')' */
                eprint!("redefined {}@{})\n", name, numargs);
                /* fflush(stderr) — stderr is unbuffered */
            }
            return 1;
        }
        d = node.next.as_deref_mut();
    }
    let d = if deff.name.is_none() {
        deff
    } else {
        let d = Box::new(DefinedFunctions {
            name: None,
            regex: None,
            numargs: 0,
            /* d->next = deff->next; deff->next = d */
            next: deff.next.take(),
        });
        deff.next = Some(d);
        deff.next.as_deref_mut().unwrap()
    };
    d.name = Some(name.to_string()); /* strdup(name) */
    d.regex = Some(regex.to_string()); /* strdup(regex) */
    d.numargs = numargs;
    0
}

/* Add a network to list of defined networks */
/* Returns 0 on success or 1 on redefinition or -1 if name is too long */
/* Always maintain head of list at same ptr */

// [spec:foma:def:define.add-defined-fn]
// [spec:foma:sem:define.add-defined-fn]
// [spec:foma:def:fomalib.add-defined-fn]
// [spec:foma:sem:fomalib.add-defined-fn]
pub fn add_defined(def: &mut DefinedNetworks, net: Option<Box<Fsm>>, string: &str) -> i32 {
    let mut net = match net {
        None => return 0,
        Some(net) => net,
    };
    if string.len() > FSM_NAME_LEN {
        return -1;
    }

    fsm_count(&mut net);
    let mut d = Some(&mut *def);
    while let Some(node) = d {
        if node.name.is_some() && node.name.as_deref() == Some(string) {
            /* fsm_destroy(d->net) — C's fsm_destroy is a no-op on NULL */
            if let Some(old) = node.net.take() {
                fsm_destroy(old);
            }
            /* free(d->name) */
            node.net = Some(net);
            node.name = Some(string.to_string()); /* strdup(string) */
            return 1;
        }
        d = node.next.as_deref_mut();
    }
    let d = if def.name.is_none() {
        def
    } else {
        let d = Box::new(DefinedNetworks {
            name: None,
            net: None,
            /* d->next = def->next; def->next = d */
            next: def.next.take(),
        });
        def.next = Some(d);
        def.next.as_deref_mut().unwrap()
    };
    d.name = Some(string.to_string()); /* strdup(string) */
    d.net = Some(net);
    0
}

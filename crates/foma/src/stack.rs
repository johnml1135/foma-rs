//! foma/stack.c — literal Wave-2 (bug-for-bug) port per
//! docs/port/rust-conventions.md. Sem rules: docs/spec/port/foma/stack.md
//! (per-file `stack.*` ids) plus the foma.h prototype ids (`foma.stack-*`)
//! carried at each single Rust site.
//!
//! Representation (all observably equivalent to C):
//!  - The CLI network stack is a sentinel-terminated doubly-linked list of
//!    `struct stack_entry {number, ah, amedh, fsm, next, previous}` whose head
//!    is the file-static `struct stack_entry *main_stack`. The list always ends
//!    in a sentinel (number == -1, fsm == NULL, next == NULL). Real entries sit
//!    between head and sentinel: head == bottom, entry just before the sentinel
//!    == top (most recently pushed).
//!  - The file-static `main_stack` becomes a thread_local `MAIN_STACK` holding
//!    the head's arena index (per the conventions: file-static → thread_local).
//!  - malloc'd `stack_entry` nodes live in a thread_local `ARENA` (Vec). Node
//!    pointers (`main_stack`, `next`, `previous`, and every returned
//!    `struct stack_entry *`) become arena indices (`usize`); None ↔ NULL.
//!    DEVIATION from C: `free()` cannot release a slot that other indices may
//!    still name, so freed nodes are left in the arena (leaked, memory-safe).
//!    The arena also grows across stack_init cycles (each re-init pushes a fresh
//!    sentinel and abandons the old list, mirroring C's leak-on-reinit).
//!  - stack_get_ah / stack_get_med_ah: C returns `se->ah` / `se->amedh` (a
//!    borrowed handle pointer). DEVIATION from C: the cached handle lives inside
//!    a thread_local arena entry and cannot be handed out as a safe borrow, so
//!    the Rust twin lazily creates + caches the handle exactly as C does, then
//!    returns the owning entry's arena index (the handle is reachable as that
//!    entry's `ah` / `amedh` field). NULL top (empty stack) ↔ None.

use std::cell::{Cell, RefCell};

use crate::apply::{apply_clear, apply_init};
use crate::constructions::fsm_count;
use crate::dynarray::rand;
use crate::iface::print_stats;
use crate::mem::G_VERBOSE;
use crate::spelling::{apply_med_clear, apply_med_init, apply_med_set_align_symbol};
use crate::structures::fsm_destroy;
use crate::types::{ApplyHandle, ApplyMedHandle, Fsm, StackEntry};

thread_local! {
    // C: `struct stack_entry *main_stack;` (file-static list head). None until
    // stack_init runs — matching the uninitialized/NULL global that every
    // stack_* function dereferences unconditionally.
    static MAIN_STACK: Cell<Option<usize>> = const { Cell::new(None) };
    // Arena backing the malloc'd stack_entry nodes (see module notes).
    static ARENA: RefCell<Vec<StackEntry>> = const { RefCell::new(Vec::new()) };
}

/* ------------------------------------------------------------------ */
/* Arena / global helpers (pointer ops become index ops)              */
/* ------------------------------------------------------------------ */

/// malloc(sizeof(struct stack_entry)) — push a node, return its index.
fn arena_alloc(entry: StackEntry) -> usize {
    ARENA.with(|a| {
        let mut a = a.borrow_mut();
        a.push(entry);
        a.len() - 1
    })
}

/// Read `main_stack`. DEVIATION from C: unwrap panics if stack_init was never
/// called (C would dereference a NULL/garbage global and crash).
fn main_stack() -> usize {
    MAIN_STACK
        .with(|m| m.get())
        .expect("main_stack uninitialized; call stack_init() first")
}

fn set_main_stack(i: usize) {
    MAIN_STACK.with(|m| m.set(Some(i)));
}

fn e_number(i: usize) -> i32 {
    ARENA.with(|a| a.borrow()[i].number)
}
fn e_next(i: usize) -> Option<usize> {
    ARENA.with(|a| a.borrow()[i].next)
}
fn e_previous(i: usize) -> Option<usize> {
    ARENA.with(|a| a.borrow()[i].previous)
}
fn set_next(i: usize, v: Option<usize>) {
    ARENA.with(|a| a.borrow_mut()[i].next = v);
}
fn set_previous(i: usize, v: Option<usize>) {
    ARENA.with(|a| a.borrow_mut()[i].previous = v);
}
fn take_fsm(i: usize) -> Option<Box<Fsm>> {
    ARENA.with(|a| a.borrow_mut()[i].fsm.take())
}
fn take_ah(i: usize) -> Option<Box<ApplyHandle>> {
    ARENA.with(|a| a.borrow_mut()[i].ah.take())
}
fn take_amedh(i: usize) -> Option<Box<ApplyMedHandle>> {
    ARENA.with(|a| a.borrow_mut()[i].amedh.take())
}

/* ------------------------------------------------------------------ */
/* Entry-field accessors for the iface layer                          */
/* ------------------------------------------------------------------ */
// DEVIATION from C: in C the caller dereferences a `struct stack_entry *`
// directly (e.g. `stack_find_top()->fsm`, `apply_down(stack_get_ah(), ...)`).
// Here those pointers are arena indices (see module notes) and the fsm/ah/amedh
// live inside the private thread_local ARENA, so they cannot be handed out as a
// `&mut`. These closure accessors let iface.c's twin operate on the entry-owned
// fsm / apply handle / med handle by index. No C counterpart, no spec ids —
// plumbing, like the private helpers above.

/// Run `f` on the fsm owned by the entry at `index` (C: `entry->fsm`).
pub fn stack_entry_fsm<R>(index: usize, f: impl FnOnce(&mut Fsm) -> R) -> R {
    ARENA.with(|a| {
        let mut a = a.borrow_mut();
        f(a[index].fsm.as_deref_mut().expect("stack entry has no fsm"))
    })
}

/// Run `f` on the apply handle owned by the entry at `index` (C: `entry->ah`).
pub fn stack_entry_ah<R>(index: usize, f: impl FnOnce(&mut ApplyHandle) -> R) -> R {
    ARENA.with(|a| {
        let mut a = a.borrow_mut();
        f(a[index].ah.as_deref_mut().expect("stack entry has no ah"))
    })
}

/// Read the `next` pointer of the entry at `index` (C: `entry->next`), so the
/// iface layer can walk the list (e.g. iface_save_stack). Sentinel/NULL ↔ None.
pub fn stack_entry_next(index: usize) -> Option<usize> {
    e_next(index)
}

/// Run `f` on the med handle owned by the entry at `index` (C: `entry->amedh`).
pub fn stack_entry_amedh<R>(index: usize, f: impl FnOnce(&mut ApplyMedHandle) -> R) -> R {
    ARENA.with(|a| {
        let mut a = a.borrow_mut();
        f(a[index]
            .amedh
            .as_deref_mut()
            .expect("stack entry has no amedh"))
    })
}

/* ------------------------------------------------------------------ */

// [spec:foma:def:stack.stack-size-fn]
// [spec:foma:sem:stack.stack-size-fn]
// [spec:foma:def:foma.stack-size-fn]
// [spec:foma:sem:foma.stack-size-fn]
pub fn stack_size() -> i32 {
    let mut i = 0;
    let mut stack_ptr = main_stack();
    while e_next(stack_ptr).is_some() {
        stack_ptr = e_next(stack_ptr).unwrap();
        i += 1;
    }
    i
}

// [spec:foma:def:stack.stack-init-fn]
// [spec:foma:sem:stack.stack-init-fn]
// [spec:foma:def:foma.stack-init-fn]
// [spec:foma:sem:foma.stack-init-fn]
pub fn stack_init() -> i32 {
    // malloc a fresh sentinel {number = -1, fsm = NULL, next = NULL,
    // previous = NULL} (ah/amedh left uninitialized in C; None here — never
    // read on the sentinel). Does not free any previous list (leaks, as in C).
    let idx = arena_alloc(StackEntry {
        number: -1,
        ah: None,
        amedh: None,
        fsm: None,
        next: None,
        previous: None,
    });
    set_main_stack(idx);
    1
}

// [spec:foma:def:stack.stack-add-fn]
// [spec:foma:sem:stack.stack-add-fn]
// [spec:foma:def:foma.stack-add-fn]
// [spec:foma:sem:foma.stack-add-fn]
pub fn stack_add(mut fsm: Box<Fsm>) -> i32 {
    let mut i = 0;
    let mut stack_ptr_previous: Option<usize> = None;

    fsm_count(&mut fsm);
    if fsm.name == "" {
        // sprintf(fsm->name, "%X", rand()) — uppercase hex of rand() into the
        // fixed 40-byte name buffer (%X of a 32-bit value is <= 8 chars).
        fsm.name = format!("{:X}", rand() as u32);
    }
    let mut stack_ptr = main_stack();
    while e_number(stack_ptr) != -1 {
        stack_ptr_previous = Some(stack_ptr);
        stack_ptr = e_next(stack_ptr).unwrap();
        i += 1;
    }
    // Allocate the fresh sentinel that becomes stack_ptr->next; its number = -1,
    // fsm = NULL, next = NULL, previous = stack_ptr.
    let new_sentinel = arena_alloc(StackEntry {
        number: -1,
        ah: None,
        amedh: None,
        fsm: None,
        next: None,
        previous: Some(stack_ptr),
    });
    // Convert the old sentinel (stack_ptr) into the new top entry, in C order.
    ARENA.with(|a| {
        let mut a = a.borrow_mut();
        a[stack_ptr].next = Some(new_sentinel);
        a[stack_ptr].fsm = Some(fsm);
        a[stack_ptr].ah = None;
        a[stack_ptr].amedh = None;
        a[stack_ptr].number = i;
        a[stack_ptr].previous = stack_ptr_previous;
    });
    if G_VERBOSE.with(|v| v.get()) != 0 {
        ARENA.with(|a| {
            let a = a.borrow();
            print_stats(a[stack_ptr].fsm.as_deref().unwrap());
        });
    }
    e_number(stack_ptr)
}

// [spec:foma:def:stack.stack-get-med-ah-fn]
// [spec:foma:sem:stack.stack-get-med-ah-fn]
// [spec:foma:def:foma.stack-get-med-ah-fn]
// [spec:foma:sem:foma.stack-get-med-ah-fn]
pub fn stack_get_med_ah() -> Option<usize> {
    let se = match stack_find_top() {
        None => return None,
        Some(x) => x,
    };
    if ARENA.with(|a| a.borrow()[se].amedh.is_none()) {
        // se->amedh = apply_med_init(se->fsm);
        let mut amedh = ARENA.with(|a| {
            let a = a.borrow();
            apply_med_init(a[se].fsm.as_deref().unwrap())
        });
        apply_med_set_align_symbol(&mut amedh, "-");
        ARENA.with(|a| a.borrow_mut()[se].amedh = Some(amedh));
    }
    // C: return se->amedh; here the owning entry index (see module notes).
    Some(se)
}

// [spec:foma:def:stack.stack-get-ah-fn]
// [spec:foma:sem:stack.stack-get-ah-fn]
// [spec:foma:def:foma.stack-get-ah-fn]
// [spec:foma:sem:foma.stack-get-ah-fn]
pub fn stack_get_ah() -> Option<usize> {
    let se = match stack_find_top() {
        None => return None,
        Some(x) => x,
    };
    if ARENA.with(|a| a.borrow()[se].ah.is_none()) {
        // se->ah = apply_init(se->fsm);
        let ah = ARENA.with(|a| {
            let a = a.borrow();
            apply_init(a[se].fsm.as_deref().unwrap())
        });
        ARENA.with(|a| a.borrow_mut()[se].ah = Some(ah));
    }
    // C: return se->ah; here the owning entry index (see module notes).
    Some(se)
}

// [spec:foma:def:stack.stack-pop-fn]
// [spec:foma:sem:stack.stack-pop-fn]
// [spec:foma:def:foma.stack-pop-fn]
// [spec:foma:sem:foma.stack-pop-fn]
pub fn stack_pop() -> Option<Box<Fsm>> {
    if stack_size() == 1 {
        // fsm = main_stack->fsm; main_stack->fsm = NULL; stack_clear();
        let fsm = take_fsm(main_stack());
        stack_clear();
        return fsm;
    }
    // Walk to the top entry (its next is the sentinel). No empty-stack guard:
    // on an empty stack e_next(sentinel) is None → unwrap panics (C dereferences
    // the sentinel's NULL next → crash). DEVIATION from C: UB → panic.
    let mut stack_ptr = main_stack();
    while e_number(e_next(stack_ptr).unwrap()) != -1 {
        stack_ptr = e_next(stack_ptr).unwrap();
    }
    // (stack_ptr->previous)->next = stack_ptr->next;
    // (stack_ptr->next)->previous = stack_ptr->previous;
    let prev = e_previous(stack_ptr).unwrap();
    let nxt = e_next(stack_ptr).unwrap();
    set_next(prev, Some(nxt));
    set_previous(nxt, Some(prev));
    let fsm = take_fsm(stack_ptr);
    let ah = take_ah(stack_ptr);
    if let Some(ah) = ah {
        apply_clear(ah);
    }
    let amedh = take_amedh(stack_ptr);
    if amedh.is_some() {
        apply_med_clear(amedh);
    }
    // stack_ptr->fsm = NULL (done by take_fsm); free(stack_ptr): slot leaked.
    fsm
}

// [spec:foma:def:stack.stack-isempty-fn]
// [spec:foma:sem:stack.stack-isempty-fn]
// [spec:foma:def:foma.stack-isempty-fn]
// [spec:foma:sem:foma.stack-isempty-fn]
pub fn stack_isempty() -> i32 {
    if e_next(main_stack()).is_none() {
        1
    } else {
        0
    }
}

// [spec:foma:def:stack.stack-turn-fn]
// [spec:foma:sem:stack.stack-turn-fn]
// [spec:foma:def:foma.stack-turn-fn]
// [spec:foma:sem:foma.stack-turn-fn]
pub fn stack_turn() -> i32 {
    if stack_isempty() != 0 {
        println!("Stack is empty.");
        return 0;
    }
    if stack_size() == 1 {
        return 1;
    }

    let mut stack_ptr = stack_find_top().unwrap();
    let ms = main_stack();
    set_next(ms, e_next(stack_ptr)); // main_stack->next = stack_ptr->next
    set_previous(e_next(stack_ptr).unwrap(), Some(ms)); // (stack_ptr->next)->previous = main_stack
    set_main_stack(stack_ptr); // main_stack = stack_ptr

    while e_previous(stack_ptr).is_some() {
        set_next(stack_ptr, e_previous(stack_ptr)); // stack_ptr->next = stack_ptr->previous
        stack_ptr = e_next(stack_ptr).unwrap(); // stack_ptr = stack_ptr->next
    }
    // [spec:foma:sem:stack.stack-turn-fn]: this previous-pointer fix-up loop
    // never advances stack_ptr, and the new head is a real entry (number != -1),
    // so on any stack of >= 2 entries it loops forever, repeatedly writing
    // new-second->previous = new-head. Dead code (the "turn stack" command goes
    // through iface_turn → stack_rotate). Reproduced literally — safe Rust
    // reproduces the non-terminating loop faithfully.
    stack_ptr = main_stack();
    while e_number(stack_ptr) != -1 {
        set_previous(e_next(stack_ptr).unwrap(), Some(stack_ptr)); // (stack_ptr->next)->previous = stack_ptr
    }
    1
}

// [spec:foma:def:stack.stack-find-top-fn]
// [spec:foma:sem:stack.stack-find-top-fn]
// [spec:foma:def:foma.stack-find-top-fn]
// [spec:foma:sem:foma.stack-find-top-fn]
pub fn stack_find_top() -> Option<usize> {
    if e_number(main_stack()) == -1 {
        return None;
    }
    let mut stack_ptr = main_stack();
    while e_number(e_next(stack_ptr).unwrap()) != -1 {
        stack_ptr = e_next(stack_ptr).unwrap();
    }
    Some(stack_ptr)
}

// [spec:foma:def:stack.stack-find-bottom-fn]
// [spec:foma:sem:stack.stack-find-bottom-fn]
// [spec:foma:def:foma.stack-find-bottom-fn]
// [spec:foma:sem:foma.stack-find-bottom-fn]
pub fn stack_find_bottom() -> Option<usize> {
    if e_number(main_stack()) == -1 {
        return None;
    }
    Some(main_stack())
}

// [spec:foma:def:stack.stack-find-second-fn]
// [spec:foma:sem:stack.stack-find-second-fn]
// [spec:foma:def:foma.stack-find-second-fn]
// [spec:foma:sem:foma.stack-find-second-fn]
pub fn stack_find_second() -> Option<usize> {
    // C's empty-stack guard is commented out. On an empty stack the walk
    // dereferences the sentinel's NULL next → crash; here e_next(sentinel) is
    // None → unwrap panics. DEVIATION from C: UB → panic.
    let mut stack_ptr = main_stack();
    while e_number(e_next(stack_ptr).unwrap()) != -1 {
        stack_ptr = e_next(stack_ptr).unwrap();
    }
    e_previous(stack_ptr)
}

// [spec:foma:def:stack.stack-clear-fn]
// [spec:foma:sem:stack.stack-clear-fn]
// [spec:foma:def:foma.stack-clear-fn]
// [spec:foma:sem:foma.stack-clear-fn]
pub fn stack_clear() -> i32 {
    let mut stack_ptr = main_stack();
    while e_next(stack_ptr).is_some() {
        let ah = take_ah(stack_ptr);
        if let Some(ah) = ah {
            apply_clear(ah);
        }
        let amedh = take_amedh(stack_ptr);
        if amedh.is_some() {
            apply_med_clear(amedh);
        }
        set_main_stack(e_next(stack_ptr).unwrap());
        let fsm = take_fsm(stack_ptr);
        if let Some(fsm) = fsm {
            // fsm_destroy(NULL) is a safe no-op in C; the None case is the guard.
            fsm_destroy(fsm);
        }
        // free(stack_ptr): slot leaked (memory-safe).
        stack_ptr = main_stack();
    }
    // free(stack_ptr): trailing sentinel — slot leaked.
    stack_init()
}

// [spec:foma:def:stack.stack-rotate-fn]
// [spec:foma:sem:stack.stack-rotate-fn]
// [spec:foma:def:foma.stack-rotate-fn]
// [spec:foma:sem:foma.stack-rotate-fn]
pub fn stack_rotate() -> i32 {
    /* Top element of stack to bottom */
    if stack_isempty() != 0 {
        println!("Stack is empty.");
        return -1;
    }
    if stack_size() == 1 {
        return 1;
    }
    let stack_ptr = stack_find_top().unwrap();
    let ms = main_stack();
    // Swap ONLY the fsm fields of bottom (main_stack) and top; number/ah/amedh
    // are NOT swapped, so any cached apply handles on those two entries now
    // refer to the other entry's former fsm (stale-handle quirk).
    ARENA.with(|a| {
        let mut a = a.borrow_mut();
        let temp_fsm = a[ms].fsm.take();
        a[ms].fsm = a[stack_ptr].fsm.take();
        a[stack_ptr].fsm = temp_fsm;
    });
    1
}

// [spec:foma:def:stack.stack-print-fn]
// [spec:foma:sem:stack.stack-print-fn]
// [spec:foma:def:foma.stack-print-fn]
// [spec:foma:sem:foma.stack-print-fn]
pub fn stack_print() -> i32 {
    // No-op stub: reads/writes no state, prints nothing, returns 1.
    1
}

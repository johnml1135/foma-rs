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

/// malloc a fresh sentinel node {number = -1, fsm/ah/amedh/next NULL}. Only the
/// terminating sentinel carries these defaults; `previous` links it to whatever
/// precedes it (None on a fresh empty stack).
fn arena_alloc_sentinel(previous: Option<usize>) -> usize {
    arena_alloc(StackEntry {
        number: -1,
        ah: None,
        amedh: None,
        fsm: None,
        next: None,
        previous,
    })
}

/// Walk from main_stack to the top real entry (the one whose `next` is the
/// sentinel). Requires a non-empty stack: on an empty stack e_next(sentinel) is
/// None → unwrap panics. DEVIATION from C: the C walk dereferences the
/// sentinel's NULL `next` here (UB/crash); the port panics instead.
fn walk_to_top() -> usize {
    let mut stack_ptr = main_stack();
    while e_number(e_next(stack_ptr).unwrap()) != -1 {
        stack_ptr = e_next(stack_ptr).unwrap();
    }
    stack_ptr
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
    let idx = arena_alloc_sentinel(None);
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
    let new_sentinel = arena_alloc_sentinel(Some(stack_ptr));
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
    // walk_to_top panics on an empty stack (DEVIATION: C's UB null-deref).
    let stack_ptr = walk_to_top();
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
    if e_next(main_stack()).is_none() { 1 } else { 0 }
}

// [spec:foma:def:stack.stack-turn-fn]
// [spec:foma:sem:stack.stack-turn-fn+1]
// [spec:foma:def:foma.stack-turn-fn]
// [spec:foma:sem:foma.stack-turn-fn+1]
pub fn stack_turn() -> i32 {
    // Wave 4 fix: the C reversal's final previous-link fix-up loop never
    // advanced its cursor, so on any stack of >= 2 entries it spun forever
    // (dead code — "turn stack" reaches iface_turn → stack_rotate, never here).
    // Implement the evident intent: reverse the order of the real entries in
    // place, relinking next/previous correctly and leaving the sentinel at the
    // tail. Each entry travels with its own fsm/ah/amedh/number (numbers are
    // not renumbered, matching the C code's evident intent), so afterwards the
    // former top is the new bottom and the former bottom is the new top.
    if stack_isempty() != 0 {
        println!("Stack is empty.");
        return 0;
    }
    if stack_size() == 1 {
        return 1;
    }

    // Collect the real entries bottom -> top, stopping at the sentinel.
    let mut entries = Vec::new();
    let mut stack_ptr = main_stack();
    while e_number(stack_ptr) != -1 {
        entries.push(stack_ptr);
        stack_ptr = e_next(stack_ptr).unwrap();
    }
    let sentinel = stack_ptr;

    // Relink in reversed order: new head = old top, ..., new top = old bottom.
    entries.reverse();
    set_main_stack(entries[0]);
    set_previous(entries[0], None);
    for pair in entries.windows(2) {
        set_next(pair[0], Some(pair[1]));
        set_previous(pair[1], Some(pair[0]));
    }
    let new_top = *entries.last().unwrap();
    set_next(new_top, Some(sentinel));
    set_previous(sentinel, Some(new_top));
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
    Some(walk_to_top())
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
    // C's empty-stack guard is commented out, so walk_to_top runs unconditionally
    // and panics on an empty stack (DEVIATION: C's UB null-deref of the sentinel).
    e_previous(walk_to_top())
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
// [spec:foma:sem:stack.stack-rotate-fn+1]
// [spec:foma:def:foma.stack-rotate-fn]
// [spec:foma:sem:foma.stack-rotate-fn+1]
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
    // [spec:foma:sem:stack.stack-rotate-fn+1] swap the cached apply/med handles
    // (ah/amedh) together with the fsm, so each handle stays bound to its own net.
    // C swapped only ->fsm, leaving cached handles pointing at the other entry's
    // former net — subsequent apply/med ran against the wrong transducer.
    ARENA.with(|a| {
        let mut a = a.borrow_mut();
        let temp_fsm = a[ms].fsm.take();
        a[ms].fsm = a[stack_ptr].fsm.take();
        a[stack_ptr].fsm = temp_fsm;
        let temp_ah = a[ms].ah.take();
        a[ms].ah = a[stack_ptr].ah.take();
        a[stack_ptr].ah = temp_ah;
        let temp_amedh = a[ms].amedh.take();
        a[ms].amedh = a[stack_ptr].amedh.take();
        a[stack_ptr].amedh = temp_amedh;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constructions::fsm_symbol;

    /// Push a symbol net with a caller-chosen name (fsm_symbol leaves name "").
    fn add_named(sym: &str, name: &str) -> i32 {
        let mut f = fsm_symbol(sym);
        f.name = name.to_string();
        stack_add(f)
    }

    fn top_fsm_name() -> String {
        stack_entry_fsm(stack_find_top().unwrap(), |f| f.name.clone())
    }

    fn bottom_fsm_name() -> String {
        stack_entry_fsm(stack_find_bottom().unwrap(), |f| f.name.clone())
    }

    // [spec:foma:sem:stack.stack-init-fn/test]
    // [spec:foma:sem:foma.stack-init-fn/test]
    #[test]
    fn stack_init_creates_fresh_empty_sentinel() {
        assert_eq!(stack_init(), 1);
        // Head is the sentinel: number == -1, no next, no previous, no fsm.
        let head = main_stack();
        assert_eq!(e_number(head), -1);
        assert_eq!(e_next(head), None);
        assert_eq!(e_previous(head), None);
        assert_eq!(stack_isempty(), 1);
        assert_eq!(stack_size(), 0);
        // Re-init on a populated stack abandons the old list (leak, as in C)
        // and starts empty again.
        add_named("a", "old");
        assert_eq!(stack_init(), 1);
        assert_eq!(stack_size(), 0);
        assert_eq!(stack_isempty(), 1);
    }

    // [spec:foma:sem:stack.stack-isempty-fn/test]
    // [spec:foma:sem:foma.stack-isempty-fn/test]
    #[test]
    fn stack_isempty_is_1_iff_no_real_entries() {
        stack_init();
        assert_eq!(stack_isempty(), 1);
        add_named("a", "x");
        assert_eq!(stack_isempty(), 0);
        stack_clear();
        assert_eq!(stack_isempty(), 1);
    }

    // [spec:foma:sem:stack.stack-size-fn/test]
    // [spec:foma:sem:foma.stack-size-fn/test]
    #[test]
    fn stack_size_counts_real_entries() {
        stack_init();
        assert_eq!(stack_size(), 0);
        add_named("a", "one");
        assert_eq!(stack_size(), 1);
        add_named("b", "two");
        assert_eq!(stack_size(), 2);
        stack_pop();
        assert_eq!(stack_size(), 1);
    }

    // [spec:foma:sem:stack.stack-add-fn/test]
    // [spec:foma:sem:foma.stack-add-fn/test]
    #[test]
    fn stack_add_appends_numbers_names_and_counts() {
        stack_init();
        // Return value is the new entry's number == stack size before the push.
        assert_eq!(add_named("a", "first"), 0);
        assert_eq!(add_named("b", "second"), 1);
        // Entries append at the tail: head == bottom == first pushed,
        // entry before the sentinel == top == most recently pushed.
        assert_eq!(bottom_fsm_name(), "first");
        assert_eq!(top_fsm_name(), "second");
        assert_eq!(e_number(stack_find_bottom().unwrap()), 0);
        assert_eq!(e_number(stack_find_top().unwrap()), 1);
        // An empty fsm->name gets sprintf(name, "%X", rand()): nonempty
        // uppercase hex.
        assert_eq!(stack_add(fsm_symbol("c")), 2);
        let name = top_fsm_name();
        assert!(!name.is_empty());
        assert!(
            name.bytes()
                .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b))
        );
        // fsm_count ran on the pushed net: single-symbol net is
        // "2 states, 1 arc, 1 path" (C foma `print size`), 1 final.
        stack_entry_fsm(stack_find_top().unwrap(), |f| {
            assert_eq!(f.statecount, 2);
            assert_eq!(f.arccount, 1);
            assert_eq!(f.finalcount, 1);
            assert_eq!(f.pathcount, 1);
        });
    }

    // [spec:foma:sem:stack.stack-pop-fn/test]
    // [spec:foma:sem:foma.stack-pop-fn/test]
    #[test]
    fn stack_add_pop_is_lifo() {
        stack_init();
        add_named("a", "first");
        add_named("b", "second");
        add_named("c", "third");
        // Pop returns the most recently pushed net and unlinks the top entry.
        assert_eq!(stack_pop().unwrap().name, "third");
        assert_eq!(stack_size(), 2);
        assert_eq!(e_number(stack_find_top().unwrap()), 1);
        assert_eq!(stack_pop().unwrap().name, "second");
        // Size-1 fast path: fsm is saved, stack_clear() re-inits empty.
        assert_eq!(stack_pop().unwrap().name, "first");
        assert_eq!(stack_isempty(), 1);
        assert_eq!(stack_size(), 0);
    }

    // DEVIATION pin: C dereferences the sentinel's NULL `next` on an empty
    // stack (UB/crash); the port panics.
    // [spec:foma:sem:stack.stack-pop-fn/test]
    // [spec:foma:sem:foma.stack-pop-fn/test]
    #[test]
    #[should_panic]
    fn stack_pop_on_empty_stack_panics() {
        stack_init();
        stack_pop();
    }

    // [spec:foma:sem:stack.stack-find-top-fn/test]
    // [spec:foma:sem:foma.stack-find-top-fn/test]
    // [spec:foma:sem:stack.stack-find-bottom-fn/test]
    // [spec:foma:sem:foma.stack-find-bottom-fn/test]
    // [spec:foma:sem:stack.stack-find-second-fn/test]
    // [spec:foma:sem:foma.stack-find-second-fn/test]
    #[test]
    fn stack_find_top_bottom_second_on_multi_entry_stacks() {
        stack_init();
        // Empty stack: top and bottom are NULL (None).
        assert_eq!(stack_find_top(), None);
        assert_eq!(stack_find_bottom(), None);
        add_named("a", "only");
        // One entry: top == bottom == head; second (top->previous) is NULL.
        assert_eq!(stack_find_top(), stack_find_bottom());
        assert_eq!(stack_find_second(), None);
        add_named("b", "mid");
        add_named("c", "newest");
        let top = stack_find_top().unwrap();
        let bottom = stack_find_bottom().unwrap();
        let second = stack_find_second().unwrap();
        assert_eq!(e_number(top), 2);
        assert_eq!(bottom, main_stack());
        assert_eq!(e_number(bottom), 0);
        // Second-from-top is the top entry's `previous`.
        assert_eq!(e_number(second), 1);
        assert_eq!(stack_entry_fsm(second, |f| f.name.clone()), "mid");
    }

    // DEVIATION pin: C's empty-stack guard is commented out, so it walks
    // through the sentinel's NULL `next` (UB/crash); the port panics.
    // [spec:foma:sem:stack.stack-find-second-fn/test]
    // [spec:foma:sem:foma.stack-find-second-fn/test]
    #[test]
    #[should_panic]
    fn stack_find_second_on_empty_stack_panics() {
        stack_init();
        stack_find_second();
    }

    // [spec:foma:sem:stack.stack-get-ah-fn/test]
    // [spec:foma:sem:foma.stack-get-ah-fn/test]
    #[test]
    fn stack_get_ah_lazily_creates_then_caches() {
        stack_init();
        // Empty stack: NULL (None).
        assert_eq!(stack_get_ah(), None);
        add_named("a", "net");
        let se = stack_get_ah().unwrap();
        assert_eq!(se, stack_find_top().unwrap());
        // Mark the handle; a second call must return the SAME cached handle
        // (not a fresh apply_init), so the mark survives.
        stack_entry_ah(se, |ah| ah.ptr = 424_242);
        let se2 = stack_get_ah().unwrap();
        assert_eq!(se2, se);
        assert_eq!(stack_entry_ah(se2, |ah| ah.ptr), 424_242);
    }

    // [spec:foma:sem:stack.stack-get-med-ah-fn/test]
    // [spec:foma:sem:foma.stack-get-med-ah-fn/test]
    #[test]
    fn stack_get_med_ah_lazily_creates_sets_align_then_caches() {
        stack_init();
        assert_eq!(stack_get_med_ah(), None);
        add_named("a", "net");
        let se = stack_get_med_ah().unwrap();
        assert_eq!(se, stack_find_top().unwrap());
        // apply_med_set_align_symbol(amedh, "-") ran on creation.
        assert_eq!(
            stack_entry_amedh(se, |m| m.align_symbol.clone()),
            Some("-".to_string())
        );
        // Cached: the marked handle is returned again, not re-created.
        stack_entry_amedh(se, |m| m.med_limit = 77);
        let se2 = stack_get_med_ah().unwrap();
        assert_eq!(se2, se);
        assert_eq!(stack_entry_amedh(se2, |m| m.med_limit), 77);
    }

    // [spec:foma:sem:stack.stack-rotate-fn+1/test]
    // [spec:foma:sem:foma.stack-rotate-fn+1/test]
    #[test]
    fn stack_rotate_swaps_top_and_bottom_fsms_with_their_handles() {
        stack_init();
        // Empty: prints "Stack is empty." and returns -1.
        assert_eq!(stack_rotate(), -1);
        add_named("a", "bottomnet");
        // Size 1: returns 1, no change.
        assert_eq!(stack_rotate(), 1);
        assert_eq!(top_fsm_name(), "bottomnet");
        add_named("b", "midnet");
        add_named("c", "topnet");
        // Cache an apply handle on the top entry (holding topnet) and mark it.
        let top = stack_get_ah().unwrap();
        stack_entry_ah(top, |ah| ah.ptr = 313_131);
        assert_eq!(stack_rotate(), 1);
        // The fsm pointers of bottom and top are exchanged; the middle entry is
        // untouched (for size > 2 this is a swap, not a rotate).
        assert_eq!(bottom_fsm_name(), "topnet");
        assert_eq!(top_fsm_name(), "bottomnet");
        assert_eq!(
            stack_entry_fsm(stack_find_second().unwrap(), |f| f.name.clone()),
            "midnet"
        );
        // Numbers are NOT swapped...
        assert_eq!(e_number(stack_find_bottom().unwrap()), 0);
        assert_eq!(e_number(stack_find_top().unwrap()), 2);
        // ...but the cached apply handle now travels WITH its net: the handle
        // built for topnet moved to the bottom entry alongside topnet's fsm, so
        // apply still runs against the transducer it was created for (the C
        // stale-handle quirk is fixed).
        assert_eq!(
            stack_entry_ah(stack_find_bottom().unwrap(), |ah| ah.ptr),
            313_131
        );
        assert_eq!(top, stack_find_top().unwrap());
    }

    // [spec:foma:sem:stack.stack-print-fn/test]
    // [spec:foma:sem:foma.stack-print-fn/test]
    #[test]
    fn stack_print_is_a_noop_returning_1() {
        stack_init();
        assert_eq!(stack_print(), 1);
        add_named("a", "x");
        assert_eq!(stack_print(), 1);
        assert_eq!(stack_size(), 1);
    }

    // [spec:foma:sem:stack.stack-clear-fn/test]
    // [spec:foma:sem:foma.stack-clear-fn/test]
    #[test]
    fn stack_clear_destroys_all_entries_and_reinits() {
        stack_init();
        add_named("a", "one");
        add_named("b", "two");
        // Cache handles on the top entry so clear exercises apply_clear /
        // apply_med_clear paths.
        stack_get_ah().unwrap();
        stack_get_med_ah().unwrap();
        assert_eq!(stack_clear(), 1);
        assert_eq!(stack_isempty(), 1);
        assert_eq!(stack_size(), 0);
        assert_eq!(e_number(main_stack()), -1);
    }

    // [spec:foma:sem:stack.stack-turn-fn+1/test]
    // [spec:foma:sem:foma.stack-turn-fn+1/test]
    #[test]
    fn stack_turn_reverses_the_stack() {
        stack_init();
        // Empty: prints "Stack is empty." and returns 0.
        assert_eq!(stack_turn(), 0);
        add_named("a", "solo");
        // Size 1: returns 1 with no change.
        assert_eq!(stack_turn(), 1);
        assert_eq!(stack_size(), 1);
        assert_eq!(top_fsm_name(), "solo");

        // Wave 4 fix: a real reversal of a 3-entry stack. Push first (bottom),
        // second, third (top).
        stack_init();
        add_named("a", "first");
        add_named("b", "second");
        add_named("c", "third");
        assert_eq!(stack_turn(), 1);
        // Former top is now the bottom, former bottom is now the top, the
        // middle is unchanged; the stack keeps all three entries.
        assert_eq!(stack_size(), 3);
        assert_eq!(bottom_fsm_name(), "third");
        assert_eq!(top_fsm_name(), "first");
        assert_eq!(
            stack_entry_fsm(stack_find_second().unwrap(), |f| f.name.clone()),
            "second"
        );
        // Entries travel with their own number (not renumbered): the new bottom
        // carries the former top's number 2, the new top the former bottom's 0.
        assert_eq!(e_number(stack_find_bottom().unwrap()), 2);
        assert_eq!(e_number(stack_find_top().unwrap()), 0);
        // Forward (next) order from the bottom is fully relinked...
        let bottom = stack_find_bottom().unwrap();
        let mid = e_next(bottom).unwrap();
        let top = e_next(mid).unwrap();
        assert_eq!(top, stack_find_top().unwrap());
        // ...and the previous links mirror it: bottom has no previous, and each
        // forward hop is matched by a backward hop.
        assert_eq!(e_previous(bottom), None);
        assert_eq!(e_previous(mid), Some(bottom));
        assert_eq!(e_previous(top), Some(mid));
        // Reversal is an involution: turning again restores the original order.
        assert_eq!(stack_turn(), 1);
        assert_eq!(bottom_fsm_name(), "first");
        assert_eq!(top_fsm_name(), "third");
    }
}

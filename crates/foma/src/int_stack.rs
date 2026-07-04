//! foma/int_stack.c — literal Wave-2 (bug-for-bug) port per
//! docs/port/rust-conventions.md. Sem rules: docs/spec/port/foma/int_stack.md
//! (per-file ids) and the fomalibconf.h prototype ids.
//!
//! `MAX_STACK` / `MAX_PTR_STACK` (2097152) live in crate::types.

use std::cell::{Cell, RefCell};

use crate::types::{MAX_PTR_STACK, MAX_STACK};

thread_local! {
    // C: static int a[MAX_STACK]; static int top = -1;
    static A: RefCell<Vec<i32>> = RefCell::new(vec![0; MAX_STACK]);
    static TOP: Cell<i32> = const { Cell::new(-1) };

    // C: static void *ptr_stack[MAX_PTR_STACK]; static int ptr_stack_top = -1;
    // DEVIATION from C (opaque pointer stack: the C stores raw void*, but
    // every foma call site — structures.c, determinize.c, spelling.c —
    // pushes interior pointers into arrays that the port represents as
    // indices, so the stack holds usize index/handle tokens; callers
    // push/pop indices into their own tables and may do +1 arithmetic on
    // them exactly as the C does on the pointers)
    static PTR_STACK: RefCell<Vec<usize>> = RefCell::new(vec![0; MAX_PTR_STACK]);
    static PTR_STACK_TOP: Cell<i32> = const { Cell::new(-1) };
}

// [spec:foma:def:int-stack.ptr-stack-isempty-fn]
// [spec:foma:sem:int-stack.ptr-stack-isempty-fn]
// [spec:foma:def:fomalibconf.ptr-stack-isempty-fn]
// [spec:foma:sem:fomalibconf.ptr-stack-isempty-fn]
pub fn ptr_stack_isempty() -> i32 {
    (PTR_STACK_TOP.with(|t| t.get()) == -1) as i32
}

// [spec:foma:def:int-stack.ptr-stack-clear-fn]
// [spec:foma:sem:int-stack.ptr-stack-clear-fn]
// [spec:foma:def:fomalibconf.ptr-stack-clear-fn]
// [spec:foma:sem:fomalibconf.ptr-stack-clear-fn]
pub fn ptr_stack_clear() {
    PTR_STACK_TOP.with(|t| t.set(-1));
}

// [spec:foma:def:int-stack.ptr-stack-pop-fn]
// [spec:foma:sem:int-stack.ptr-stack-pop-fn]
// [spec:foma:def:fomalibconf.ptr-stack-pop-fn]
// [spec:foma:sem:fomalibconf.ptr-stack-pop-fn]
pub fn ptr_stack_pop() -> usize {
    // C: return ptr_stack[ptr_stack_top--]; — no underflow check; popping an
    // empty stack reads ptr_stack[-1] (UB) and leaves ptr_stack_top at -2.
    // DEVIATION from C (OOB read on empty pop is UB in C; here the index
    // panics instead — callers must guard with ptr_stack_isempty, as in C)
    PTR_STACK_TOP.with(|t| {
        let top = t.get();
        let v = PTR_STACK.with(|s| s.borrow()[top as usize]);
        t.set(top - 1);
        v
    })
}

// [spec:foma:def:int-stack.ptr-stack-isfull-fn]
// [spec:foma:sem:int-stack.ptr-stack-isfull-fn]
// [spec:foma:def:fomalibconf.ptr-stack-isfull-fn]
// [spec:foma:sem:fomalibconf.ptr-stack-isfull-fn]
pub fn ptr_stack_isfull() -> i32 {
    (PTR_STACK_TOP.with(|t| t.get()) == (MAX_PTR_STACK as i32 - 1)) as i32
}

// [spec:foma:def:int-stack.ptr-stack-push-fn]
// [spec:foma:sem:int-stack.ptr-stack-push-fn]
// [spec:foma:def:fomalibconf.ptr-stack-push-fn]
// [spec:foma:sem:fomalibconf.ptr-stack-push-fn]
pub fn ptr_stack_push(ptr: usize) {
    if ptr_stack_isfull() != 0 {
        eprint!("Pointer stack full!\n");
        std::process::exit(1);
    }
    PTR_STACK_TOP.with(|t| {
        let top = t.get() + 1;
        t.set(top);
        PTR_STACK.with(|s| s.borrow_mut()[top as usize] = ptr);
    });
}

// [spec:foma:def:int-stack.int-stack-isempty-fn]
// [spec:foma:sem:int-stack.int-stack-isempty-fn]
// [spec:foma:def:fomalibconf.int-stack-isempty-fn]
// [spec:foma:sem:fomalibconf.int-stack-isempty-fn]
pub fn int_stack_isempty() -> i32 {
    (TOP.with(|t| t.get()) == -1) as i32
}

// [spec:foma:def:int-stack.int-stack-clear-fn]
// [spec:foma:sem:int-stack.int-stack-clear-fn]
// [spec:foma:def:fomalibconf.int-stack-clear-fn]
// [spec:foma:sem:fomalibconf.int-stack-clear-fn]
pub fn int_stack_clear() {
    TOP.with(|t| t.set(-1));
}

// [spec:foma:def:int-stack.int-stack-find-fn]
// [spec:foma:sem:int-stack.int-stack-find-fn]
// [spec:foma:def:fomalibconf.int-stack-find-fn]
// [spec:foma:sem:fomalibconf.int-stack-find-fn]
pub fn int_stack_find(entry: i32) -> i32 {
    if int_stack_isempty() != 0 {
        return 0;
    }
    let top = TOP.with(|t| t.get());
    A.with(|a| {
        let a = a.borrow();
        // C: for(i = 0; i <= top ; i++)
        let mut i: i32 = 0;
        while i <= top {
            if entry == a[i as usize] {
                return 1;
            }
            i += 1;
        }
        0
    })
}

// [spec:foma:def:int-stack.int-stack-size-fn]
// [spec:foma:sem:int-stack.int-stack-size-fn]
// [spec:foma:def:fomalibconf.int-stack-size-fn]
// [spec:foma:sem:fomalibconf.int-stack-size-fn]
pub fn int_stack_size() -> i32 {
    TOP.with(|t| t.get()) + 1
}

// [spec:foma:def:int-stack.int-stack-push-fn]
// [spec:foma:sem:int-stack.int-stack-push-fn]
// [spec:foma:def:fomalibconf.int-stack-push-fn]
// [spec:foma:sem:fomalibconf.int-stack-push-fn]
pub fn int_stack_push(c: i32) {
    if int_stack_isfull() != 0 {
        eprint!("Stack full!\n");
        std::process::exit(1);
    }
    TOP.with(|t| {
        let top = t.get() + 1;
        t.set(top);
        A.with(|a| a.borrow_mut()[top as usize] = c);
    });
}

// [spec:foma:def:int-stack.int-stack-pop-fn]
// [spec:foma:sem:int-stack.int-stack-pop-fn]
// [spec:foma:def:fomalibconf.int-stack-pop-fn]
// [spec:foma:sem:fomalibconf.int-stack-pop-fn]
pub fn int_stack_pop() -> i32 {
    // C: return a[top--]; — no underflow check; popping an empty stack reads
    // a[-1] (UB) and leaves top at -2.
    // DEVIATION from C (OOB read on empty pop is UB in C; here the index
    // panics instead — callers must guard with int_stack_isempty, as in C)
    TOP.with(|t| {
        let top = t.get();
        let v = A.with(|a| a.borrow()[top as usize]);
        t.set(top - 1);
        v
    })
}

// [spec:foma:def:int-stack.int-stack-isfull-fn]
// [spec:foma:sem:int-stack.int-stack-isfull-fn]
// [spec:foma:def:fomalibconf.int-stack-isfull-fn]
// [spec:foma:sem:fomalibconf.int-stack-isfull-fn]
pub fn int_stack_isfull() -> i32 {
    (TOP.with(|t| t.get()) == (MAX_STACK as i32 - 1)) as i32
}

// NOTE: fomalibconf.h also declares `int int_stack_status();` (rule id
// fomalibconf.int-stack-status-fn) but no definition exists anywhere in the
// C sources — it is a dead prototype and is deliberately NOT ported (no
// implementation is invented for it).

/* Dead prototype: declared in fomalibconf.h but never defined in any C
source. Calling it in C is a link error. DEVIATION from C (panics to
preserve the never-callable contract). */

// [spec:foma:def:fomalibconf.int-stack-status-fn]
// [spec:foma:sem:fomalibconf.int-stack-status-fn]
pub fn int_stack_status() -> i32 {
    panic!("int_stack_status: dead prototype in C foma (declared, never defined; link error)");
}

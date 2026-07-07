# foma/int_stack.c

> [spec:foma:def:int-stack.int-stack-clear-fn]
> void int_stack_clear()

> [spec:foma:sem:int-stack.int-stack-clear-fn]
> Resets the global int stack to empty by setting the static top index `top` to -1.
> The contents of the static backing array `a` are not erased or zeroed; previously
> pushed values remain in memory but are unreachable via the stack API. No return value.

> [spec:foma:def:int-stack.int-stack-find-fn]
> int int_stack_find (int entry)

> [spec:foma:sem:int-stack.int-stack-find-fn]
> Membership test on the global int stack. If the stack is empty (`top == -1`,
> checked via int_stack_isempty), returns 0 immediately. Otherwise scans the backing
> array `a` linearly from index 0 (bottom) through index `top` (topmost) inclusive;
> returns 1 as soon as an element equal to `entry` is found. Returns 0 if no element
> matches. The stack is not modified.

> [spec:foma:def:int-stack.int-stack-isempty-fn]
> int int_stack_isempty()

> [spec:foma:sem:int-stack.int-stack-isempty-fn]
> Returns 1 if the global int stack is empty, i.e. the static top index `top`
> equals -1; otherwise returns 0. No state is modified.

> [spec:foma:def:int-stack.int-stack-isfull-fn]
> int int_stack_isfull()

> [spec:foma:sem:int-stack.int-stack-isfull-fn+1]
> The int stack grows unbounded (a `Vec`), so it is never at capacity —
> always returns 0. The C fixed cap MAX_STACK (2097152, 2^21) and the
> `top == MAX_STACK - 1` boundary are gone. No state is modified.

> [spec:foma:def:int-stack.int-stack-pop-fn]
> int int_stack_pop()

> [spec:foma:sem:int-stack.int-stack-pop-fn]
> Pops the global int stack: returns the value at `a[top]`, then post-decrements the
> static top index `top`. There is no underflow check: calling this on an empty stack
> (`top == -1`) reads `a[-1]` — out-of-bounds, undefined behavior — and leaves `top`
> at -2; callers must guard with int_stack_isempty first.

> [spec:foma:def:int-stack.int-stack-push-fn]
> void int_stack_push(int c)

> [spec:foma:sem:int-stack.int-stack-push-fn+1]
> Pushes `c` onto the global int stack, growing its backing `Vec` on demand.
> The push is infallible and unbounded — the C `MAX_STACK` cap and the
> "Stack full!\n" + exit(1) overflow path are gone. `int_stack_size` reports the
> element count; the stack is empty only before the first push or after a clear.

> [spec:foma:def:int-stack.int-stack-size-fn]
> int int_stack_size ()

> [spec:foma:sem:int-stack.int-stack-size-fn]
> Returns the number of elements on the global int stack, computed as `top + 1`
> (0 when empty since `top` is -1 then). No state is modified.

> [spec:foma:def:int-stack.ptr-stack-clear-fn]
> void ptr_stack_clear()

> [spec:foma:sem:int-stack.ptr-stack-clear-fn]
> Resets the global pointer stack to empty by setting the static top index
> `ptr_stack_top` to -1. Stored pointers are not freed and the backing array
> `ptr_stack` is not erased; ownership of any still-stored pointers is the caller's
> problem (potential leak if they were heap-owned). No return value.

> [spec:foma:def:int-stack.ptr-stack-isempty-fn]
> int ptr_stack_isempty()

> [spec:foma:sem:int-stack.ptr-stack-isempty-fn]
> Returns 1 if the global pointer stack is empty, i.e. the static top index
> `ptr_stack_top` equals -1; otherwise returns 0. No state is modified.

> [spec:foma:def:int-stack.ptr-stack-isfull-fn]
> int ptr_stack_isfull()

> [spec:foma:sem:int-stack.ptr-stack-isfull-fn+1]
> The pointer stack grows unbounded (a `Vec`), so it is never at capacity —
> always returns 0. The C fixed cap MAX_PTR_STACK (2097152, 2^21) and the
> `ptr_stack_top == MAX_PTR_STACK - 1` boundary are gone. No state is modified.

> [spec:foma:def:int-stack.ptr-stack-pop-fn]
> void *ptr_stack_pop()

> [spec:foma:sem:int-stack.ptr-stack-pop-fn]
> Pops the global pointer stack: returns the pointer at `ptr_stack[ptr_stack_top]`,
> then post-decrements the static top index `ptr_stack_top`. No underflow check:
> popping an empty stack reads `ptr_stack[-1]` — out-of-bounds, undefined behavior —
> and leaves `ptr_stack_top` at -2. Ownership of the returned pointer transfers to
> the caller (the stack never frees stored pointers).

> [spec:foma:def:int-stack.ptr-stack-push-fn]
> void ptr_stack_push(void *ptr)

> [spec:foma:sem:int-stack.ptr-stack-push-fn+1]
> Pushes `ptr` (an index/handle token) onto the global pointer stack, growing its
> backing `Vec` on demand. The push is infallible and unbounded — the C
> `MAX_PTR_STACK` cap and the "Pointer stack full!\n" + exit(1) overflow path are
> gone. The stack stores the token only; it takes no ownership.

# foma/stack.c

> [spec:foma:def:stack.stack-add-fn]
> int stack_add(struct fsm *fsm)

> [spec:foma:sem:stack.stack-add-fn]
> Pushes `fsm` onto the global network stack (`main_stack` list; see stack-init-fn),
> taking ownership of it. Steps: (1) call fsm_count(fsm) to recompute and store the
> network's state/arc/final/path statistics on the fsm struct; (2) if fsm->name is the
> empty string, overwrite it with sprintf(fsm->name, "%X", rand()) — an uppercase-hex
> random name written into the fixed 40-byte name buffer (FSM_NAME_LEN = 40); (3) walk
> from main_stack following `next` until reaching the sentinel (entry with number == -1),
> counting hops in `i` and remembering the entry before the sentinel (NULL if the
> sentinel is the head); (4) convert the sentinel in place into the new top entry:
> allocate a fresh sentinel as its `next`, set fsm, ah = NULL, amedh = NULL,
> number = i (0-based index from head, equal to the stack size before the push), and
> previous = the remembered prior entry; (5) initialize the fresh sentinel with
> number = -1, fsm = NULL, next = NULL, previous = the new entry (its ah/amedh are left
> uninitialized); (6) if the global `g_verbose` is nonzero, call print_stats(fsm);
> (7) return the new entry's number. New entries thus append at the tail: head = bottom,
> entry just before the sentinel = top (most recently added).

> [spec:foma:def:stack.stack-clear-fn]
> int stack_clear(void)

> [spec:foma:sem:stack.stack-clear-fn]
> Destroys every entry on the global network stack and re-initializes it empty.
> Loop: starting at main_stack, while the current entry's `next` is non-NULL (i.e. it is
> not the sentinel): call apply_clear on its `ah` if non-NULL and apply_med_clear on its
> `amedh` if non-NULL, advance the global main_stack to the entry's `next`, call
> fsm_destroy on the entry's fsm (fsm_destroy(NULL) is a safe no-op), free the entry,
> and restart from the new main_stack. After the loop the current entry is the sentinel;
> free it too (without touching its ah/amedh, which are uninitialized). Finally call
> stack_init() to allocate a fresh empty sentinel into main_stack and return its result
> (always 1). All fsms and apply handles on the stack are freed.

> [spec:foma:def:stack.stack-find-bottom-fn]
> struct stack_entry *stack_find_bottom ()

> [spec:foma:sem:stack.stack-find-bottom-fn]
> Returns the bottom entry of the global network stack, which is simply the list head
> `main_stack`, or NULL if the stack is empty. Emptiness is detected by
> main_stack->number == -1 (the head is the sentinel). No state is modified.

> [spec:foma:def:stack.stack-find-second-fn]
> struct stack_entry *stack_find_second ()

> [spec:foma:sem:stack.stack-find-second-fn]
> Returns the second-from-top entry of the global network stack. Walks from main_stack
> following `next` until reaching the entry whose `next` has number == -1 (the top
> entry, just before the sentinel), then returns that entry's `previous` pointer.
> If the stack holds exactly one entry the top is the head and its previous is NULL,
> so NULL is returned. There is no empty-stack guard (one exists but is commented out
> in the source): calling this on an empty stack dereferences the sentinel's NULL
> `next` — undefined behavior/crash. No state is modified.

> [spec:foma:def:stack.stack-find-top-fn]
> struct stack_entry *stack_find_top ()

> [spec:foma:sem:stack.stack-find-top-fn]
> Returns the top entry of the global network stack (the most recently added real
> entry, located immediately before the terminating sentinel), or NULL if the stack is
> empty (main_stack->number == -1, i.e. the head is the sentinel). Walks from
> main_stack following `next` while the next entry's number != -1 and returns the entry
> where the walk stops. No state is modified.

> [spec:foma:def:stack.stack-get-ah-fn]
> struct apply_handle *stack_get_ah()

> [spec:foma:sem:stack.stack-get-ah-fn]
> Returns a lazily created, cached apply handle for the top network on the global
> stack. Calls stack_find_top(); if it returns NULL (empty stack), returns NULL.
> Otherwise, if the top entry's `ah` field is NULL, creates one via
> apply_init(top->fsm) and stores it in top->ah. Returns top->ah. The handle remains
> owned by the stack entry (subsequent calls return the same handle; it is destroyed
> with apply_clear by stack_pop or stack_clear).

> [spec:foma:def:stack.stack-get-med-ah-fn]
> struct apply_med_handle *stack_get_med_ah()

> [spec:foma:sem:stack.stack-get-med-ah-fn]
> Returns a lazily created, cached minimum-edit-distance apply handle for the top
> network on the global stack. Calls stack_find_top(); if NULL (empty stack), returns
> NULL. Otherwise, if the top entry's `amedh` field is NULL, creates one via
> apply_med_init(top->fsm), then calls apply_med_set_align_symbol(amedh, "-") to set
> the alignment padding symbol to "-", and stores it in top->amedh. Returns top->amedh.
> The handle remains owned by the stack entry (destroyed with apply_med_clear by
> stack_pop or stack_clear).

> [spec:foma:def:stack.stack-init-fn]
> int stack_init()

> [spec:foma:sem:stack.stack-init-fn]
> Initializes the global network stack. The stack is a doubly linked list of
> struct stack_entry {number, ah, amedh, fsm, next, previous} whose head is stored in
> the global `struct stack_entry *main_stack`; the list always terminates in a sentinel
> entry with number == -1, fsm == NULL, next == NULL. Real entries sit between the head
> and the sentinel: head = bottom, the entry just before the sentinel = top. This
> function mallocs one entry, sets number = -1, fsm = NULL, next = NULL,
> previous = NULL (ah and amedh are left uninitialized), assigns it to main_stack, and
> returns 1. It does not free any previous list (callers leak if main_stack was already
> populated); every other stack_* function dereferences main_stack unconditionally, so
> this must be called first.

> [spec:foma:def:stack.stack-isempty-fn]
> int stack_isempty ()

> [spec:foma:sem:stack.stack-isempty-fn]
> Returns 1 if the global network stack is empty — i.e. main_stack->next == NULL,
> meaning the head is the sentinel and there are no real entries — otherwise returns 0.
> No state is modified.

> [spec:foma:def:stack.stack-pop-fn]
> struct fsm *stack_pop(void)

> [spec:foma:sem:stack.stack-pop-fn]
> Removes and returns the top network from the global stack; ownership of the returned
> fsm transfers to the caller. If stack_size() == 1: save main_stack->fsm, set
> main_stack->fsm = NULL, call stack_clear() (which apply_clear/apply_med_clear's the
> entry's cached handles, frees all entries plus the sentinel, and re-inits an empty
> stack), and return the saved fsm. Otherwise: walk from main_stack to the top entry
> (the entry whose `next` has number == -1); unlink it by setting
> previous->next = its next (the sentinel) and sentinel->previous = its previous; if
> its `ah` is non-NULL call apply_clear on it and NULL the field; if its `amedh` is
> non-NULL call apply_med_clear on it and NULL the field; save its fsm, set the field
> to NULL, free the entry, and return the fsm. No empty-stack guard: on an empty stack
> the walk dereferences the sentinel's NULL `next` — undefined behavior/crash.

> [spec:foma:def:stack.stack-print-fn]
> int stack_print ()

> [spec:foma:sem:stack.stack-print-fn]
> No-op stub: performs nothing and unconditionally returns 1. Reads and writes no
> state, prints nothing.

> [spec:foma:def:stack.stack-rotate-fn]
> int stack_rotate ()

> [spec:foma:sem:stack.stack-rotate-fn]
> Despite the source comment "Top element of stack to bottom", this swaps the fsms of
> the bottom and top stack entries (for size 2 that equals a rotation; for larger
> stacks it is a bottom<->top swap, not a cyclic rotate). If the stack is empty:
> prints "Stack is empty.\n" to stdout and returns -1. If stack_size() == 1: returns 1
> with no change. Otherwise: exchange only the `fsm` pointers of main_stack (bottom)
> and stack_find_top() (top); the entries' number, ah, and amedh fields are NOT
> swapped, so any cached apply handles on those two entries now refer to the other
> entry's former fsm (stale-handle quirk). Returns 1.

> [spec:foma:def:stack.stack-size-fn]
> int stack_size()

> [spec:foma:sem:stack.stack-size-fn]
> Returns the number of real (non-sentinel) entries on the global network stack.
> Walks from main_stack following `next`, incrementing a counter once per hop, until
> reaching the entry whose `next` is NULL (the sentinel); the counter — the number of
> entries preceding the sentinel — is returned (0 when empty). No state is modified;
> requires main_stack to have been initialized via stack_init.

> [spec:foma:def:stack.stack-turn-fn]
> int stack_turn ()

> [spec:foma:sem:stack.stack-turn-fn+1]
> Reverses the order of the real entries on the global network stack in place. If empty:
> prints "Stack is empty.\n" to stdout and returns 0. If stack_size() == 1: returns 1,
> no change. For size >= 2: reverse the sequence of real entries so the former top
> becomes the new bottom (head/main_stack) and the former bottom becomes the new top
> (the entry immediately before the sentinel), relinking every `next`/`previous`
> pointer to match and leaving the sentinel at the tail (its `next` stays NULL, its
> `previous` becomes the new top). Each entry travels with its own fsm, ah, amedh and
> number — numbers are NOT renumbered, so from the head they now descend (new bottom
> carries the former top's number, new top carries the former bottom's 0). Returns 1.
> Wave 4 fix: the C code's final previous-link fix-up loop
> `for (p = main_stack; p->number != -1;) { p->next->previous = p; }` never advanced p,
> so for stacks of 2+ entries the function looped forever (dead code: the "turn stack"
> command reaches iface_turn → stack_rotate, never this function). This implements the
> evident intent — a genuine, terminating stack reversal.

# foma/dynarray.c

> [spec:foma:def:dynarray.foma-reserved-symbols]
> struct foma_reserved_symbols {
>   char *symbol;
>   int number;
>   char *prints_as;
> }

> [spec:foma:def:dynarray.fsm-construct-add-arc-fn]
> void fsm_construct_add_arc(struct fsm_construct_handle *handle, int source, int target, char *in, char *out)

> [spec:foma:sem:dynarray.fsm-construct-add-arc-fn]
> Adds a transition source→target with symbolic labels `in`/`out` to a construction handle.
> Calls fsm_construct_check_size for `source`, then for `target` (growing the state list as needed).
> Raises handle->maxstate to `source` and then to `target` if either exceeds it.
> Sets fsm_state_list[target].used = 1 and fsm_state_list[source].used = 1.
> Mallocs a new fsm_trans_list node and prepends it to fsm_state_list[source].fsm_trans_list
> (transitions are stored newest-first; num_trans is not updated).
> Resolves `in` with fsm_construct_check_symbol; if that returns -1 (absent), assigns a number via
> fsm_construct_add_symbol. Same for `out`. Stores the two numbers in the node's in/out fields and
> sets node->target = target. Returns nothing. The caller keeps ownership of the `in`/`out` strings
> (add_symbol strdups when it registers a new symbol).

> [spec:foma:def:dynarray.fsm-construct-add-arc-nums-fn]
> void fsm_construct_add_arc_nums(struct fsm_construct_handle *handle, int source, int target, int in, int out)

> [spec:foma:sem:dynarray.fsm-construct-add-arc-nums-fn]
> Like fsm_construct_add_arc but with numeric labels: calls fsm_construct_check_size for `source`
> then `target`; raises handle->maxstate to `source` and `target` if larger; sets .used = 1 on both
> the target and source entries of fsm_state_list; mallocs an fsm_trans_list node, prepends it to
> fsm_state_list[source].fsm_trans_list, and fills node->in = in, node->out = out,
> node->target = target. Does not touch the sigma list/hash or maxsigma: the caller is responsible
> for `in`/`out` being valid symbol numbers. num_trans is not updated.

> [spec:foma:def:dynarray.fsm-construct-add-symbol-fn]
> int fsm_construct_add_symbol(struct fsm_construct_handle *handle, char *symbol)

> [spec:foma:sem:dynarray.fsm-construct-add-symbol-fn]
> Assigns a number to `symbol` and registers it in the handle's sigma list and hash. Does NOT check
> whether the symbol is already registered (callers use fsm_construct_check_symbol first; adding a
> duplicate would allocate a fresh number).
> 1. Scan the reserved table {"@_EPSILON_SYMBOL_@"→0, "@_UNKNOWN_SYMBOL_@"→1, "@_IDENTITY_SYMBOL_@"→2};
>    on a strcmp match take that fixed number and raise handle->maxsigma to it if maxsigma is smaller.
> 2. If not reserved: number = maxsigma+1, but at least MINSIGMA = 3; set maxsigma = number.
> 3. If number >= fsm_sigma_list_size: set size = next_power_of_two(size) (smallest power of two
>    strictly greater than the old size, i.e. it doubles a power-of-two size) and realloc the list;
>    new slots are NOT zero-initialized.
> 4. symdup = strdup(symbol); store symdup at fsm_sigma_list[number].symbol.
> 5. hash = fsm_construct_hash_sym(symbol); bucket = fsm_sigma_hash[hash]. If the bucket head's
>    symbol is NULL, fill the head in place (symbol = symdup, sym = number); otherwise calloc a new
>    chain node, splice it directly after the head (new->next = head->next; head->next = new) and
>    fill it with symdup/number.
> Returns the assigned number.

> [spec:foma:def:dynarray.fsm-construct-check-size-fn]
> void fsm_construct_check_size(struct fsm_construct_handle *handle, int state_no)

> [spec:foma:sem:dynarray.fsm-construct-check-size-fn]
> Ensures handle->fsm_state_list is indexable at state_no. If fsm_state_list_size <= state_no:
> newsize = next_power_of_two(state_no) (smallest power of two strictly greater than state_no),
> realloc the list to newsize entries, set fsm_state_list_size = newsize, and initialize each new
> entry from index oldsize through newsize-1: is_final = 0, is_initial = 0, used = 0, num_trans = 0,
> fsm_trans_list = NULL. Otherwise a no-op. Does not update maxstate.

> [spec:foma:def:dynarray.fsm-construct-check-symbol-fn]
> int fsm_construct_check_symbol(struct fsm_construct_handle *handle, char *symbol)

> [spec:foma:sem:dynarray.fsm-construct-check-symbol-fn]
> Looks up `symbol` in the handle's sigma hash. Computes hash = fsm_construct_hash_sym(symbol) and
> takes bucket fsm_sigma_hash[hash]. If the bucket head's symbol pointer is NULL (empty bucket),
> returns -1. Otherwise walks the chain from the head via ->next and returns node->sym for the first
> node whose symbol strcmp-equals `symbol`. Returns -1 if no node matches.

> [spec:foma:def:dynarray.fsm-construct-convert-sigma-fn]
> struct sigma *fsm_construct_convert_sigma(struct fsm_construct_handle *handle)

> [spec:foma:sem:dynarray.fsm-construct-convert-sigma-fn]
> Converts the handle's dense fsm_sigma_list array into a singly linked `struct sigma` list ordered
> by ascending symbol number. Iterates i = 0 .. handle->maxsigma inclusive; for each i with
> fsm_sigma_list[i].symbol != NULL, mallocs a sigma node {number = i, symbol = that same char
> pointer (ownership moves to the sigma list; the string is not duplicated), next = NULL} and
> appends it at the tail. Returns the head node, or NULL if no slot had a symbol.

> [spec:foma:def:dynarray.fsm-construct-copy-sigma-fn]
> void fsm_construct_copy_sigma(struct fsm_construct_handle *handle, struct sigma *sigma)

> [spec:foma:sem:dynarray.fsm-construct-copy-sigma-fn+1]
> Bulk-loads an existing sigma linked list into the handle's sigma list and hash (no duplicate or
> reserved-symbol checks). Iterates while sigma != NULL and sigma->number != -1 (a node numbered -1
> terminates the walk). Per node:
> 1. If sigma->number > handle->maxsigma, set maxsigma = sigma->number.
> 2. If sigma->number >= fsm_sigma_list_size, grow until the number fits: repeatedly
>    size = next_power_of_two(size) until size > sigma->number, then realloc. The C source grew
>    once (a single doubling keyed on the current size), so a number >= twice the size still
>    overflowed the array (OOB write in C, index panic in the port); the loop guarantees the slot
>    fits. New slots are not zero-initialized.
> 3. symdup = strdup(sigma->symbol); store symdup at fsm_sigma_list[sigma->number].symbol.
> 4. Insert into fsm_sigma_hash exactly as in fsm_construct_add_symbol: hash the symbol; if the
>    bucket head's symbol is NULL fill the head (symbol = symdup, sym = number), otherwise calloc a
>    chain node, splice it directly after the head, and fill it.

> [spec:foma:def:dynarray.fsm-construct-done-fn]
> struct fsm *fsm_construct_done(struct fsm_construct_handle *handle)

> [spec:foma:sem:dynarray.fsm-construct-done-fn]
> Finalizes a construction handle into a struct fsm.
> 1. If handle->maxstate == -1, or numfinals == 0, or hasinitial == 0, return fsm_empty_set()
>    immediately (the handle and its contents are NOT freed on this path).
> 2. Call fsm_state_init(maxsigma+1) to start a fresh global line-array build. Set emptyfsm = 1.
> 3. For each state i = 0 .. maxstate: call fsm_state_set_current_state(i, is_final, is_initial)
>    (flags from fsm_state_list[i]); if the state is both initial and final, emptyfsm = 0; for each
>    node of its fsm_trans_list (reverse insertion order), set emptyfsm = 0 if the state is initial,
>    and call fsm_state_add_arc(i, node->in, node->out, node->target, is_final, is_initial); then
>    call fsm_state_end_state().
> 4. net = fsm_create(""); sprintf(net->name, "%X", rand()); free(net->sigma);
>    fsm_state_close(net) (installs the built array and counts into net).
> 5. net->sigma = fsm_construct_convert_sigma(handle). If handle->name != NULL,
>    strncpy(net->name, handle->name, 40) (no forced NUL terminator if the name is >= 40 chars) and
>    free(handle->name); else overwrite net->name with a new "%X"-formatted rand() value.
> 6. Free every fsm_trans_list node for all fsm_state_list_size slots (not just up to maxstate);
>    free every sigma-hash chain node hanging off each of the 1021 bucket heads (the inline heads
>    are part of the array); then free fsm_sigma_list, fsm_sigma_hash, fsm_state_list, and the
>    handle itself. The symbol strings now belong to net->sigma.
> 7. sigma_sort(net). If emptyfsm is still 1 (no initial state had an outgoing arc and no state was
>    both initial and final, i.e. the language is empty), fsm_destroy(net) and return
>    fsm_empty_set(); otherwise return net.

> [spec:foma:def:dynarray.fsm-construct-hash-sym-fn]
> unsigned int fsm_construct_hash_sym(char *symbol)

> [spec:foma:sem:dynarray.fsm-construct-hash-sym-fn]
> Hashes a NUL-terminated symbol string. hash starts at 0 (unsigned int); each byte of the string
> is added in turn as a plain `char` value (on signed-char platforms bytes >= 0x80 add negative
> values, wrapping the unsigned sum modulo 2^32). Returns hash % SIGMA_HASH_SIZE, i.e. hash % 1021.

> [spec:foma:def:dynarray.fsm-construct-init-fn]
> struct fsm_construct_handle *fsm_construct_init(char *name)

> [spec:foma:sem:dynarray.fsm-construct-init-fn]
> Allocates and returns a fresh fsm_construct_handle: fsm_state_list = calloc of 1024
> fsm_state_list entries (zeroed: not final/initial/used, num_trans 0, no transition lists),
> fsm_state_list_size = 1024; fsm_sigma_list = calloc of 1024 fsm_sigma_list entries,
> fsm_sigma_list_size = 1024; fsm_sigma_hash = calloc of SIGMA_HASH_SIZE = 1021 bucket-head nodes
> (symbol == NULL means empty bucket). maxstate = -1, maxsigma = -1, numfinals = 0, hasinitial = 0.
> handle->name = strdup(name), or NULL when name is NULL. The caller owns the handle;
> fsm_construct_done later consumes and frees it.

> [spec:foma:def:dynarray.fsm-construct-set-final-fn]
> void fsm_construct_set_final(struct fsm_construct_handle *handle, int state_no)

> [spec:foma:sem:dynarray.fsm-construct-set-final-fn]
> Marks state_no as final. Calls fsm_construct_check_size(handle, state_no); raises
> handle->maxstate to state_no if greater; then, only if fsm_state_list[state_no].is_final is 0,
> sets it to 1 and increments handle->numfinals (idempotent — repeated calls do not recount).
> Does not set the state's `used` flag.

> [spec:foma:def:dynarray.fsm-construct-set-initial-fn]
> void fsm_construct_set_initial(struct fsm_construct_handle *handle, int state_no)

> [spec:foma:sem:dynarray.fsm-construct-set-initial-fn]
> Marks state_no as initial. Calls fsm_construct_check_size(handle, state_no); raises
> handle->maxstate to state_no if greater; sets fsm_state_list[state_no].is_initial = 1
> unconditionally and handle->hasinitial = 1. No per-handle count of initials is kept and the
> state's `used` flag is not set.

> [spec:foma:def:dynarray.fsm-get-arc-in-fn]
> char *fsm_get_arc_in(struct fsm_read_handle *handle)

> [spec:foma:sem:dynarray.fsm-get-arc-in-fn]
> Returns the input-label symbol string of the arc under handle->arcs_cursor: NULL if arcs_cursor
> is NULL; otherwise handle->fsm_sigma_list[arcs_cursor->in].symbol — a borrowed pointer into the
> handle's sigma list, not a copy. No bounds or sentinel check: calling it while the cursor sits on
> a sentinel line (in == -1) indexes out of bounds.

> [spec:foma:def:dynarray.fsm-get-arc-num-in-fn]
> int fsm_get_arc_num_in(struct fsm_read_handle *handle)

> [spec:foma:sem:dynarray.fsm-get-arc-num-in-fn]
> Returns the numeric input label of the arc under handle->arcs_cursor, or -1 if arcs_cursor is
> NULL. On a sentinel line the stored value -1 is returned as-is.

> [spec:foma:def:dynarray.fsm-get-arc-num-out-fn]
> int fsm_get_arc_num_out(struct fsm_read_handle *handle)

> [spec:foma:sem:dynarray.fsm-get-arc-num-out-fn]
> Returns the numeric output label of the arc under handle->arcs_cursor, or -1 if arcs_cursor is
> NULL. On a sentinel line the stored value -1 is returned as-is.

> [spec:foma:def:dynarray.fsm-get-arc-out-fn]
> char *fsm_get_arc_out(struct fsm_read_handle *handle)

> [spec:foma:sem:dynarray.fsm-get-arc-out-fn]
> Returns the output-label symbol string of the arc under handle->arcs_cursor: NULL if arcs_cursor
> is NULL; otherwise handle->fsm_sigma_list[arcs_cursor->out].symbol — a borrowed pointer, not a
> copy. No bounds or sentinel check (out == -1 indexes out of bounds).

> [spec:foma:def:dynarray.fsm-get-arc-source-fn]
> int fsm_get_arc_source(struct fsm_read_handle *handle)

> [spec:foma:sem:dynarray.fsm-get-arc-source-fn]
> Returns the source state number (state_no field) of the line under handle->arcs_cursor, or -1 if
> arcs_cursor is NULL.

> [spec:foma:def:dynarray.fsm-get-arc-target-fn]
> int fsm_get_arc_target(struct fsm_read_handle *handle)

> [spec:foma:sem:dynarray.fsm-get-arc-target-fn]
> Returns the target state number of the line under handle->arcs_cursor, or -1 if arcs_cursor is
> NULL (target is also -1 on sentinel lines).

> [spec:foma:def:dynarray.fsm-get-has-unknowns-fn]
> int fsm_get_has_unknowns(struct fsm_read_handle *handle)

> [spec:foma:sem:dynarray.fsm-get-has-unknowns-fn]
> Returns handle->has_unknowns: 1 if fsm_read_init saw any line whose in or out label equals
> UNKNOWN (1) or IDENTITY (2), otherwise 0.

> [spec:foma:def:dynarray.fsm-get-next-arc-fn]
> int fsm_get_next_arc(struct fsm_read_handle *handle)

> [spec:foma:sem:dynarray.fsm-get-next-arc-fn]
> Advances handle->arcs_cursor to the next real arc of the whole machine in storage order, skipping
> sentinel lines (target == -1) and stopping at the array terminator (state_no == -1).
> If arcs_cursor is NULL (fresh handle or after fsm_read_reset): start at arcs_head and skip forward
> while state_no != -1 && target == -1; if the line reached has state_no == -1, return 0.
> Otherwise: if the cursor already sits on the terminator (state_no == -1), return 0 without moving;
> else advance one line and keep advancing while state_no != -1 && target == -1; if the terminator
> is reached, return 0.
> Returns 1 with arcs_cursor on the next real arc. After returning 0 the cursor rests on the
> terminator line and subsequent calls keep returning 0.

> [spec:foma:def:dynarray.fsm-get-next-final-fn]
> int fsm_get_next_final(struct fsm_read_handle *handle)

> [spec:foma:sem:dynarray.fsm-get-next-final-fn]
> Iterates over the -1-terminated finals_head array of final state numbers (ascending order).
> If finals_cursor is NULL, set it to finals_head and return *cursor (may already be -1 if there
> are no finals). Otherwise, if *cursor == -1 return -1 without advancing (the end is sticky); else
> advance the cursor one slot and return the new *cursor. -1 signals exhaustion.

> [spec:foma:def:dynarray.fsm-get-next-initial-fn]
> int fsm_get_next_initial(struct fsm_read_handle *handle)

> [spec:foma:sem:dynarray.fsm-get-next-initial-fn]
> Identical protocol to fsm_get_next_final but over initials_head/initials_cursor: first call sets
> the cursor to initials_head and returns *cursor; later calls return -1 without advancing if the
> cursor is on the -1 terminator, otherwise advance one slot and return the new value. Returns
> initial state numbers in ascending order; -1 signals exhaustion.

> [spec:foma:def:dynarray.fsm-get-next-state-arc-fn]
> int fsm_get_next_state_arc(struct fsm_read_handle *handle)

> [spec:foma:sem:dynarray.fsm-get-next-state-arc-fn]
> Advances to the next arc of the state most recently returned by fsm_get_next_state (which leaves
> arcs_cursor one line before the state's first line, so this pre-increments). Increment
> arcs_cursor; if the new line's state_no != handle->current_state, or its target == -1 (the
> no-arc sentinel line of the state), decrement the cursor back and return 0; otherwise return 1
> with the cursor on the arc.

> [spec:foma:def:dynarray.fsm-get-next-state-fn]
> int fsm_get_next_state(struct fsm_read_handle *handle)

> [spec:foma:sem:dynarray.fsm-get-next-state-fn]
> Iterates over the states via the states_head pointer array (one entry per state, pointing at each
> state's first line). If states_cursor is NULL, set it to states_head; otherwise increment it by
> one slot. If the cursor's index (states_cursor - states_head) >= net->statecount, return -1
> (exhausted). Otherwise: set arcs_cursor = *states_cursor (the state's first line), read
> stateno = that line's state_no, then decrement arcs_cursor by one line so that
> fsm_get_next_state_arc's pre-increment lands on the first line; set handle->current_state =
> stateno and return stateno.

> [spec:foma:def:dynarray.fsm-get-num-states-fn]
> int fsm_get_num_states(struct fsm_read_handle *handle)

> [spec:foma:sem:dynarray.fsm-get-num-states-fn]
> Returns handle->net->statecount, the number of states of the underlying network.

> [spec:foma:def:dynarray.fsm-get-symbol-number-fn]
> int fsm_get_symbol_number(struct fsm_read_handle *handle, char *symbol)

> [spec:foma:sem:dynarray.fsm-get-symbol-number-fn]
> Linear scan of handle->fsm_sigma_list indices 0 .. sigma_list_size-1, skipping slots whose symbol
> pointer is NULL; returns the first index whose symbol strcmp-equals `symbol`, or -1 if no slot
> matches.

> [spec:foma:def:dynarray.fsm-read-done-fn]
> void fsm_read_done(struct fsm_read_handle *handle)

> [spec:foma:sem:dynarray.fsm-read-done-fn]
> Frees a read handle and everything it owns, in order: lookuptable, fsm_sigma_list (the array
> only — the symbol strings are borrowed from net->sigma and are not freed), finals_head,
> initials_head, states_head, then the handle struct itself. The underlying net is untouched.
> No NULL check: passing NULL dereferences it and crashes.

> [spec:foma:def:dynarray.fsm-read-init-fn]
> struct fsm_read_handle *fsm_read_init(struct fsm *net)

> [spec:foma:sem:dynarray.fsm-read-init-fn]
> Builds an iteration handle over net's line array. Returns NULL if net is NULL.
> 1. lookuptable = calloc(net->statecount) bytes: one flag byte per state, bit 0 = initial,
>    bit 1 = final. handle = calloc'd fsm_read_handle (all cursors NULL, has_unknowns 0,
>    current_state 0). states_head = calloc(statecount+1) fsm_state pointers.
> 2. Single pass over net->states lines until the terminator (state_no == -1): if the line's
>    start_state flag is set and bit 0 of lookuptable[state_no] is clear, set it and count an
>    initial; likewise final_state/bit 1 for finals. If the line's in or out equals UNKNOWN (1) or
>    IDENTITY (2), set handle->has_unknowns = 1. Whenever a line's state_no differs from the
>    previous line's, record states_head[state_no] = pointer to that line (the state's first line;
>    assumes lines are grouped by state).
> 3. Allocate finals_head (num_finals+1 ints) and initials_head (num_initials+1 ints); scan states
>    i = 0 .. statecount-1 in ascending order appending i to initials_head if bit 0 set and to
>    finals_head if bit 1 set; terminate both arrays with -1.
> 4. Fill in the handle: finals_head, initials_head, states_head; fsm_sigma_list =
>    sigma_to_list(net->sigma) (dense array indexed by symbol number whose char pointers are
>    borrowed from net->sigma); sigma_list_size = sigma_max(net->sigma)+1; arcs_head = net->states;
>    lookuptable; net. All cursors start NULL. Returns the handle; release with fsm_read_done.

> [spec:foma:def:dynarray.fsm-read-is-final-fn]
> int fsm_read_is_final(struct fsm_read_handle *h, int state)

> [spec:foma:sem:dynarray.fsm-read-is-final-fn]
> Returns lookuptable[state] & 2: nonzero (the value 2) iff `state` is final, 0 otherwise.
> No bounds check on `state`.

> [spec:foma:def:dynarray.fsm-read-is-initial-fn]
> int fsm_read_is_initial(struct fsm_read_handle *h, int state)

> [spec:foma:sem:dynarray.fsm-read-is-initial-fn]
> Returns lookuptable[state] & 1: 1 iff `state` is initial, 0 otherwise. No bounds check on
> `state`.

> [spec:foma:def:dynarray.fsm-read-reset-fn]
> void fsm_read_reset(struct fsm_read_handle *handle)

> [spec:foma:sem:dynarray.fsm-read-reset-fn]
> Restarts all iterators of the handle by setting arcs_cursor, initials_cursor, finals_cursor and
> states_cursor to NULL. Returns immediately if handle is NULL.

> [spec:foma:def:dynarray.fsm-state-add-arc-fn]
> void fsm_state_add_arc(int state_no, int in, int out, int target, int final_state, int start_state)

> [spec:foma:sem:dynarray.fsm-state-add-arc-fn]
> Appends one line (arc or sentinel; sentinels carry -1 fields) to the global line array started by
> fsm_state_init. Operates entirely on module statics.
> 1. If in != out, set static arity = 2.
> 2. If in == EPSILON (0) and out == EPSILON: when state_no == target (epsilon self-loop) return
>    without adding anything; otherwise clear is_deterministic and is_epsilon_free and continue.
> 3. Only when in != -1 and out != -1: index slookup at ssize*in + out. If that cell's mainloop
>    stamp equals the current global mainloop (meaning this (in,out) pair was already added for the
>    current state): if the cell's recorded target == target, return (exact duplicate skipped);
>    otherwise clear is_deterministic and fall through. In all fall-through cases increment
>    arccount and write the cell: mainloop = current mainloop, target = target. (Sentinel lines
>    bypass this step, so arccount counts only real arcs.)
> 4. Set current_trans = 1 (the current state now has at least one emitted line).
> 5. If current_fsm_linecount >= current_fsm_size: double current_fsm_size and realloc the array;
>    if realloc returns NULL, perror("Fatal error: out of memory\n") and exit(1).
> 6. Write the six fields state_no, in, out, target, final_state, start_state into the line at
>    index current_fsm_linecount, then increment current_fsm_linecount.

> [spec:foma:def:dynarray.fsm-state-close-fn]
> void fsm_state_close(struct fsm *net)

> [spec:foma:sem:dynarray.fsm-state-close-fn]
> Ends the global build and installs the result into `net`. Appends the array terminator line
> (-1,-1,-1,-1,-1,-1) via fsm_state_add_arc (in == out so arity is unaffected; in == -1 so no
> slookup/arccount effect), then reallocs the array down to exactly current_fsm_linecount lines.
> Copies the statics into net: arity, arccount, statecount, linecount = current_fsm_linecount,
> finalcount = num_finals, pathcount = PATHCOUNT_UNKNOWN (-3). If num_initials > 1, clears the
> static is_deterministic first. Sets net->is_deterministic and net->is_epsilon_free from the
> statics; net->is_pruned, is_minimized, is_loop_free, is_completed all = UNK (2);
> arcs_sorted_in = arcs_sorted_out = 0. Sets net->states to the array (ownership transfers to net)
> and frees the global slookup table.

> [spec:foma:def:dynarray.fsm-state-end-state-fn]
> void fsm_state_end_state()

> [spec:foma:sem:dynarray.fsm-state-end-state-fn]
> Closes the current state of the global build. If current_trans == 0 (no line emitted since
> fsm_state_set_current_state), emits a placeholder line
> (current_state_no, -1, -1, -1, current_final, current_start) so every state occupies at least one
> line. Then increments statecount and increments mainloop, which invalidates all slookup
> duplicate-detection stamps for the next state.

> [spec:foma:def:dynarray.fsm-state-init-fn]
> struct fsm_state *fsm_state_init(int sigma_size)

> [spec:foma:sem:dynarray.fsm-state-init-fn]
> Begins a global (module-static, non-reentrant) dynamic build of an fsm_state line array.
> Mallocs INITIAL_SIZE = 16384 fsm_state entries into static current_fsm_head, sets
> current_fsm_size = 16384 and current_fsm_linecount = 0. Sets static ssize = sigma_size+1 and
> callocs slookup = ssize*ssize zeroed sigma_lookup cells (the per-state duplicate/determinism
> table indexed by ssize*in + out, so all symbol numbers used later must be <= sigma_size); sets
> mainloop = 1. Resets the remaining statics: is_deterministic = 1, is_epsilon_free = 1,
> arccount = 0, num_finals = 0, num_initials = 0, statecount = 0, arity = 1, current_trans = 1.
> Returns the array pointer (also retained in the static for subsequent calls).

> [spec:foma:def:dynarray.fsm-state-set-current-state-fn]
> void fsm_state_set_current_state(int state_no, int final_state, int start_state)

> [spec:foma:sem:dynarray.fsm-state-set-current-state-fn]
> Declares the state whose arcs will be added next in the global build: stores state_no,
> final_state, start_state into the statics current_state_no/current_final/current_start and clears
> current_trans (no line emitted for this state yet). Increments num_finals if final_state == 1 and
> num_initials if start_state == 1 (comparison is to exactly 1; other nonzero values are stored but
> not counted).

> [spec:foma:def:dynarray.sigma-lookup]
> struct sigma_lookup {
>   int target;
>   unsigned int mainloop;
> }


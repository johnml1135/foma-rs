# foma/structures.c

> [spec:foma:def:structures.add-quantifier-fn]
> void add_quantifier (char *string)

> [spec:foma:sem:structures.add-quantifier-fn]
> Appends a node to the file-scope static singly-linked list `quantifiers` (nodes are `struct defined_quantifiers { char *name; struct defined_quantifiers *next; }`).
> If the head is NULL, mallocs a node and makes it the head; otherwise walks to the tail node (next == NULL) and mallocs a new node linked after it.
> Sets the new node's `name = strdup(string)` (the argument is not retained) and `next = NULL`.
> No duplicate check: adding the same name twice creates two nodes. No return value.

> [spec:foma:def:structures.clear-quantifiers-fn]
> void clear_quantifiers()

> [spec:foma:sem:structures.clear-quantifiers-fn]
> Sets the file-scope static list head `quantifiers` to NULL.
> Existing nodes and their strdup'd names are not freed (deliberate leak in the C code).

> [spec:foma:def:structures.count-quantifiers-fn]
> int count_quantifiers()

> [spec:foma:sem:structures.count-quantifiers-fn]
> Walks the global static `quantifiers` linked list from the head, incrementing a counter per node, and returns the count.
> Returns 0 when the head is NULL.

> [spec:foma:def:structures.find-arccount-fn]
> int find_arccount(struct fsm_state *fsm)

> [spec:foma:sem:structures.find-arccount-fn]
> Scans the fsm_state line array from index 0 until the sentinel line whose `state_no == -1`, and returns the number of lines before the sentinel.
> Despite the name it counts table lines, not arcs: lines with `target == -1` (arcless state marker lines) are included; the sentinel itself is not.
> Reads only `state_no`; the argument must be non-NULL and sentinel-terminated.

> [spec:foma:def:structures.find-quantifier-fn]
> char *find_quantifier (char *string)

> [spec:foma:sem:structures.find-quantifier-fn]
> Walks the global static `quantifiers` list from the head and returns the stored `name` pointer of the first node with `strcmp(string, name) == 0`.
> Returns NULL if no node matches or the list is empty.
> The returned pointer aliases the node's own string; the caller must not free or mutate it.

> [spec:foma:def:structures.fsm-boolean-fn]
> struct fsm *fsm_boolean(int value)

> [spec:foma:sem:structures.fsm-boolean-fn]
> Returns `fsm_empty_set()` (network accepting nothing) when `value == 0`; for any nonzero value returns `fsm_empty_string()` (network accepting only the empty string).
> Always a freshly allocated, caller-owned network.

> [spec:foma:def:structures.fsm-copy-fn]
> struct fsm *fsm_copy (struct fsm *net)

> [spec:foma:sem:structures.fsm-copy-fn+1]
> A `&mut` borrow is never NULL; NULL-able callers keep the check at the call site.
> Calls `fsm_count(net)` on the SOURCE FIRST (refreshing net->statecount/linecount/arccount/finalcount), THEN captures those now-fresh scalar counts and flags into a new `struct fsm`. The C memcpy'd the whole struct BEFORE fsm_count ran, so the copy's counts were left stale; here source and copy carry the same fresh counts.
> Gives the copy its own `sigma` (sigma_copy(net->sigma)) and its own `states` (fsm_state_copy(net->states, net->linecount)); linecount includes the -1 sentinel line, so the full table is duplicated. The `medlookup` pointer is deep-cloned rather than shared (the C aliased it — a double-free hazard). Caller owns the returned net.

> [spec:foma:def:structures.fsm-create-fn]
> struct fsm *fsm_create (char *name)

> [spec:foma:sem:structures.fsm-create-fn+1]
> The in-memory name is stored in full. C printed `Network name '%s' should consist of at most %d characters.\n` to stdout when `strlen(name) > FSM_NAME_LEN` (40) and copied the name via `strncpy(fsm->name, name, FSM_NAME_LEN)` into a fixed 40-byte field (no NUL terminator when the name is >= 40 chars), truncating longer names. The binary file format still caps names at 40 bytes on read/write.
> Mallocs a `struct fsm`.
> Initializes: arity=1, arccount=0, is_deterministic/is_pruned/is_minimized/is_epsilon_free/is_loop_free/arcs_sorted_in/arcs_sorted_out = NO (0), sigma = sigma_create() (single node {number=-1, symbol=NULL, next=NULL}), states=NULL, medlookup=NULL.
> statecount, linecount, finalcount, pathcount and is_completed are left uninitialized. Caller owns the result.

> [spec:foma:def:structures.fsm-destroy-fn]
> int fsm_destroy(struct fsm *net)

> [spec:foma:sem:structures.fsm-destroy-fn+1]
> Does nothing when `net` is NULL (a NULL-able caller keeps the guard at the call site; a Box argument is never NULL).
> Otherwise: if net->medlookup and its confusion_matrix are both non-NULL, frees the matrix and NULLs the field; then if medlookup is non-NULL, frees it and NULLs the field.
> Calls fsm_sigma_destroy(net->sigma) (harmless on a NULL sigma list) and sets net->sigma = NULL; if net->states is non-NULL, frees it and NULLs it; finally frees net itself. Returns nothing (the C `int` return, always 1 on a non-NULL net, carries no information).

> [spec:foma:def:structures.fsm-empty-fn]
> struct fsm_state *fsm_empty()

> [spec:foma:sem:structures.fsm-empty-fn]
> Mallocs and returns an array of exactly 2 fsm_state lines: line 0 = {state_no=0, in=-1, out=-1, target=-1, final_state=0, start_state=1} (a single non-final start state with no arcs), line 1 = the all-(-1) sentinel {-1,-1,-1,-1,-1,-1}.
> This is the state table of the empty-language machine; the caller owns the array.

> [spec:foma:def:structures.fsm-empty-set-fn]
> struct fsm *fsm_empty_set()

> [spec:foma:sem:structures.fsm-empty-set-fn]
> Builds the network accepting the empty language: net = fsm_create("") (so sigma is the empty single-node sigma), states = fsm_empty() (one non-final start state plus sentinel).
> Calls fsm_update_flags(net, YES,YES,YES,YES,YES,NO): deterministic, pruned, minimized, epsilon-free, loop-free all YES; completed NO; arcs_sorted_in/out cleared.
> Sets statecount=1, finalcount=0, arccount=0, linecount=2, pathcount=0. Returns the caller-owned net.

> [spec:foma:def:structures.fsm-empty-string-fn]
> struct fsm *fsm_empty_string()

> [spec:foma:sem:structures.fsm-empty-string-fn]
> Builds the network accepting exactly the empty string: net = fsm_create(""); states = malloc of 2 lines: line 0 = {state_no=0, in=-1, out=-1, target=-1, final_state=1, start_state=1}, line 1 = all-(-1) sentinel.
> Calls fsm_update_flags(net, YES,YES,YES,YES,YES,NO) (all structural flags YES, completed NO, sort flags cleared).
> Sets statecount=1, finalcount=1, arccount=0, linecount=2, pathcount=1. Returns the caller-owned net.

> [spec:foma:def:structures.fsm-extract-ambiguous-domain-fn]
> struct fsm *fsm_extract_ambiguous_domain(struct fsm *net)

> [spec:foma:sem:structures.fsm-extract-ambiguous-domain-fn]
> Computes the acceptor of upper-side (input) words that transducer net maps ambiguously (to more than one output/path); consumes net.
> Implements the foma-regex definition `AmbiguousDom(T) = [_loweruniq(T) .o. _notid(_loweruniq(T).i .o. _loweruniq(T))].u`.
> Steps: L = fsm_lowerdet(net) (relabel each arc's output with a per-state-unique symbol; consumes net); result = fsm_topsort(fsm_minimize(fsm_upper(fsm_compose(fsm_copy(L), fsm_extract_nonidentity(fsm_compose(fsm_invert(fsm_copy(L)), fsm_copy(L))))))).
> Then fsm_destroy(L); sigma_cleanup(result, 1); fsm_compact(result); sigma_sort(result); return result (caller-owned).

> [spec:foma:def:structures.fsm-extract-ambiguous-fn]
> struct fsm *fsm_extract_ambiguous(struct fsm *net)

> [spec:foma:sem:structures.fsm-extract-ambiguous-fn]
> Returns fsm_topsort(fsm_minimize(fsm_compose(fsm_extract_ambiguous_domain(fsm_copy(net)), net))): the sub-transducer of net restricted to the input words net maps ambiguously.
> The ambiguous domain is computed from a copy; net itself is consumed as the second compose operand. Result is caller-owned.

> [spec:foma:def:structures.fsm-extract-nonidentity-fn]
> struct fsm *fsm_extract_nonidentity(struct fsm *net)

> [spec:foma:sem:structures.fsm-extract-nonidentity-fn]
> Extracts from transducer net the upper-side language of paths that violate the identity relation. Uses the same DFS-with-discrepancy algorithm as fsm_isidentity (see structures.fsm-isidentity-fn for the traversal, failure conditions a-e, and discrepancy update rules), but instead of aborting on a violation it marks the offending arc and continues.
> Setup: calls fsm_minimize(net) DISCARDING the return value (relies on in-place minimization), fsm_count(net), killnum = sigma_add("@KILL@", net->sigma); callocs one {short *string; short length; _Bool visited} discrepancy record per state; state_array = map_firstlines(net); pushes state 0's first line onto the global pointer stack (no ptr_stack_clear beforehand, unlike fsm_isidentity).
> Traversal differences from fsm_isidentity: on any failure condition, execution jumps to a fail label that sets curr_ptr->out = killnum (relabels the arc's output to @KILL@), pushes the sibling line curr_ptr+1 if it has the same state_no, and continues the main pop loop. When failure occurs at the revisit-comparison stage the sibling was already pushed once and is pushed again (redundant re-traversal). newstring buffers are never freed here (no free-before-realloc as in fsm_isidentity), so there is no aliasing hazard, only leaks.
> After the stack drains: ptr_stack_clear(); sigma_sort(net); net2 = fsm_upper(fsm_compose(net, fsm_contains(fsm_symbol("@KILL@")))) — consumes net and keeps the input side of exactly those paths whose (marked) output contains @KILL@; sigma_remove("@KILL@", net2->sigma); sigma_sort(net2); free state_array and the discrepancy array; return net2 (caller-owned).

> [spec:foma:def:structures.fsm-extract-unambiguous-fn]
> struct fsm *fsm_extract_unambiguous(struct fsm *net)

> [spec:foma:sem:structures.fsm-extract-unambiguous-fn]
> Returns fsm_topsort(fsm_minimize(fsm_compose(fsm_complement(fsm_extract_ambiguous_domain(fsm_copy(net))), net))): the sub-transducer of net restricted to input words that net maps unambiguously.
> The ambiguous domain is computed from a copy and complemented; net itself is consumed as the second compose operand. Result is caller-owned.

> [spec:foma:def:structures.fsm-get-library-version-string-fn]
> char *fsm_get_library_version_string()

> [spec:foma:sem:structures.fsm-get-library-version-string-fn]
> sprintf's "%i.%i.%i%s" with MAJOR_VERSION (0), MINOR_VERSION (10), BUILD_VERSION (0), STATUS_VERSION ("alpha") into a function-local static char[20] buffer, yielding "0.10.0alpha", and returns a pointer to that buffer.
> The buffer is rewritten on every call, must not be freed, and is not thread-safe.

> [spec:foma:def:structures.fsm-get-option-fn]
> void *fsm_get_option(unsigned long long option)

> [spec:foma:sem:structures.fsm-get-option-fn]
> If option == FSMO_SKIP_WORD_BOUNDARY_MARKER (enum value 0), returns a pointer to the `skip_word_boundary_marker` _Bool field inside the file-scope global `struct _fsm_options fsm_options`.
> For any other option value returns NULL. The pointer aliases the live global; writes through it change the option.

> [spec:foma:def:structures.fsm-identity-fn]
> struct fsm *fsm_identity()

> [spec:foma:sem:structures.fsm-identity-fn]
> Builds the identity-over-any-symbol transducer `?` : net = fsm_create(""); then free(net->sigma) (releases the single empty sigma node fsm_create made — just the node, its symbol is NULL).
> states = malloc of 3 lines: line 0 = {state_no=0, in=IDENTITY(2), out=IDENTITY(2), target=1, final_state=0, start_state=1}; line 1 = {state_no=1, in=-1, out=-1, target=-1, final_state=1, start_state=0}; line 2 = all-(-1) sentinel.
> net->sigma = a single malloc'd node {number=IDENTITY(2), symbol=strdup("@_IDENTITY_SYMBOL_@"), next=NULL}.
> fsm_update_flags(net, YES,YES,YES,YES,YES,NO); statecount=2, finalcount=1, arccount=1, linecount=3, pathcount=1. Returns the caller-owned net.

> [spec:foma:def:structures.fsm-isempty-fn]
> int fsm_isempty(struct fsm *net)

> [spec:foma:sem:structures.fsm-isempty-fn]
> Non-destructively tests whether net's language is empty: computes minimal = fsm_minimize(fsm_copy(net)).
> Returns 1 iff minimal's first state line has target == -1 and final_state == 0 and the second line is the sentinel (state_no == -1) — i.e. the minimized machine is the lone non-final arcless state; otherwise returns 0.
> Destroys the minimized copy before returning; net is untouched.

> [spec:foma:def:structures.fsm-isfunctional-fn]
> int fsm_isfunctional(struct fsm *net)

> [spec:foma:sem:structures.fsm-isfunctional-fn]
> Tests whether transducer net is functional (maps every input to at most one output): tmp = fsm_minimize(fsm_compose(fsm_invert(fsm_copy(net)), fsm_copy(net))) — i.e. T.i .o. T — then result = fsm_isidentity(tmp).
> Destroys tmp and returns result (1 = functional). net is not consumed; only copies are.

> [spec:foma:def:structures.fsm-isidentity-fn]
> int fsm_isidentity(struct fsm *net)

> [spec:foma:sem:structures.fsm-isidentity-fn]
> Non-destructively tests whether transducer net encodes a partial identity relation (every accepting path maps a string to itself). Works on tmp = fsm_minimize(fsm_copy(net)), then fsm_count(tmp).
> Data: per state a "discrepancy" record {short *string; short length; _Bool visited} in a calloc'd array indexed by state number (tmp->statecount entries). length > 0 means `length` upper-side (in) symbols are pending unmatched, string holds them in order; length < 0 means |length| lower-side (out) symbols pending; length == 0 means the tapes are level.
> Setup: state_array = map_firstlines(tmp); ptr_stack_clear(); push state 0's first line onto the global pointer stack.
> Main loop while the stack is non-empty: pop a line pointer curr_ptr; (label nopop:) let v = curr_ptr->state_no, vp = curr_ptr->target, currd = &discrepancy[v]; if v != -1 set currd->visited = 1; if v == -1 or vp == -1 (sentinel or arcless line) continue with the next pop; let in/out be the arc labels.
> Fail (see below) if any of: (e) in or out == UNKNOWN(1); (d) in == IDENTITY(2) while currd->length != 0; (b) currd->length > 0 and out is neither EPSILON(0) nor currd->string[0], or currd->length < 0 and in is neither EPSILON nor currd->string[0]; or currd->length == 0 and in != out with neither being EPSILON.
> New discrepancy: if currd->length != 0: factor = 0 if neither label is EPSILON, -1 if in == EPSILON, +1 if out == EPSILON; newlength = currd->length + factor; startfrom = 1 if abs(newlength) <= abs(currd->length) else 0 (the matched head symbol is consumed). If currd->length == 0: newlength = 0 when neither label is EPSILON, else +1 (out == EPSILON) or -1 (in == EPSILON); startfrom = 0.
> newstring = calloc(abs(newlength), sizeof(int)) (int-width elements although used as shorts); copy currd->string[startfrom .. abs(currd->length)-1] into it; if newlength != 0, append one symbol at the next position: `in` when (currd->length > 0 and newlength >= currd->length) or (currd->length == 0 and newlength > 0); `out` when (currd->length < 0 and newlength <= currd->length) or (currd->length == 0 and newlength < 0).
> Caution: the C code frees the previous iteration's newstring buffer just before this calloc; when the previous iteration descended into state v, that buffer IS currd->string, so the copy reads freed memory — a port must copy from currd->string before releasing any buffer.
> Then fail if (c) the target's first line (via state_array[vp]) is final and newlength != 0. If the next array line has the same state_no as curr_ptr, push curr_ptr+1 (remaining sibling arcs).
> If discrepancy[vp].visited: fail unless the stored length equals newlength and the stored string matches newstring over abs(newlength) symbols (condition a). Otherwise store {length=newlength, string=newstring} into discrepancy[vp], set curr_ptr to state vp's first line, and goto nopop (descend without pushing).
> Success (stack drains): free state_array, the discrepancy array, and the last newstring; fsm_destroy(tmp); return 1. Fail: same frees plus ptr_stack_clear(); return 0. Strings stored in discrepancy records are never individually freed (leak).

> [spec:foma:def:structures.fsm-issequential-fn]
> int fsm_issequential(struct fsm *net)

> [spec:foma:sem:structures.fsm-issequential-fn]
> Tests whether net (taken as-is, no minimization) is sequential: no state has two arcs with the same input symbol, and a state with an epsilon-input arc may have no other arcs. Returns 1 (sequential) or 0.
> Allocates sigtable with sigma_max(net->sigma)+1 int slots, every slot initialized to -2 (slot k records the last state number seen using input symbol k).
> Walks the state table in order until the sentinel. Per line: insym = in; lines with insym < 0 (arcless marker lines) are skipped entirely (they do not update the state tracker). When the line's state_no differs from the tracked state (tracker starts at -1), set the tracker and reset the per-state flags epstrans = seentrans = 0.
> Fail (clear the flag, stop scanning) if sigtable[insym] == current state (duplicate input symbol at this state) or epstrans == 1 (an arc after an epsilon arc). If insym == EPSILON(0): fail if epstrans or seentrans is already 1, else set epstrans = 1. Then set sigtable[insym] = current state and seentrans = 1.
> Frees sigtable. On failure prints `fails at state %i\n` (the offending line's state_no) to stdout. net is not modified.

> [spec:foma:def:structures.fsm-isunambiguous-fn]
> int fsm_isunambiguous(struct fsm *net)

> [spec:foma:sem:structures.fsm-isunambiguous-fn]
> Tests whether transducer net is unambiguous (no input word has two distinct accepting paths).
> L = fsm_lowerdet(fsm_copy(net)) — outputs relabeled per-state-uniquely so distinct paths get distinct outputs; testnet = fsm_minimize(fsm_compose(fsm_invert(fsm_copy(L)), fsm_copy(L))); ret = fsm_isidentity(testnet).
> Destroys L and testnet; net is not consumed. Returns ret (1 = unambiguous).

> [spec:foma:def:structures.fsm-isuniversal-fn]
> int fsm_isuniversal(struct fsm *net)

> [spec:foma:sem:structures.fsm-isuniversal-fn+1]
> Tests whether net is the universal language ?*. Destructive: net = fsm_minimize(net) (consumes/replaces the argument), then fsm_compact(net); the compacted net is dropped (neither returned nor destroyed).
> The C condition ANDed `line1.state_no == 0` with `line1.state_no == -1` (mutually exclusive → returned 0 for every input). Implement the evident universality test instead: return 1 iff the compacted state table is the lone state 0 with an IDENTITY:IDENTITY self-loop — line 0 has target == 0, final_state == 1, in == IDENTITY(2), out == IDENTITY(2) — followed immediately by the -1 sentinel (line 1's state_no == -1), over an alphabet of only reserved symbols (sigma_max(net->sigma) < 3). Otherwise returns 0.

> [spec:foma:def:structures.fsm-logical-eq-fn]
> struct fsm *fsm_logical_eq(char *string1, char *string2)

> [spec:foma:sem:structures.fsm-logical-eq-fn]
> Builds the regex `?* [x y | y x]/Q ?* [x y | y x]/Q ?*` where x = fsm_symbol(string1), y = fsm_symbol(string2), Q = union_quantifiers() (reads the global quantifier list), and `/Q` is fsm_ignore(..., Q, OP_IGNORE_ALL): the language in which the two variable symbols delimit the same span (logical equivalence).
> Exact construction: fsm_concat(fsm_universal(), fsm_concat(IGN, fsm_concat(fsm_universal(), fsm_concat(IGN, fsm_universal())))) where each IGN is a freshly built fsm_ignore(fsm_union(fsm_concat(fsm_symbol(string1), fsm_symbol(string2)), fsm_concat(fsm_symbol(string2), fsm_symbol(string1))), union_quantifiers(), OP_IGNORE_ALL).
> Every sub-net is constructed fresh (fsm_symbol/fsm_universal/union_quantifiers per occurrence) and consumed by the combinators; returns a caller-owned net.

> [spec:foma:def:structures.fsm-logical-precedence-fn]
> struct fsm *fsm_logical_precedence(char *string1, char *string2)

> [spec:foma:sem:structures.fsm-logical-precedence-fn]
> Builds the regex `\y* x \y* [x | y Q* x] ?*` (x = fsm_symbol(string1), y = fsm_symbol(string2), \y = fsm_term_negation(fsm_symbol(string2)), Q = union_quantifiers()): "x precedes y" over quantifier-marked strings.
> Exact construction: fsm_concat(fsm_kleene_star(fsm_term_negation(fsm_symbol(string2))), fsm_concat(fsm_symbol(string1), fsm_concat(fsm_kleene_star(fsm_term_negation(fsm_symbol(string2))), fsm_concat(fsm_union(fsm_symbol(string1), fsm_concat(fsm_symbol(string2), fsm_concat(union_quantifiers(), fsm_symbol(string1)))), fsm_universal())))).
> Every sub-net is built fresh and consumed by the combinators; reads the global quantifier list via union_quantifiers(). Returns a caller-owned net.

> [spec:foma:def:structures.fsm-lowerdet-fn]
> struct fsm *fsm_lowerdet(struct fsm *net)

> [spec:foma:sem:structures.fsm-lowerdet-fn]
> Makes net's lower (output) side deterministic by relabeling every arc's output with a symbol unique among the arcs of its source state. Consumes/replaces net: net = fsm_minimize(net), then fsm_count(net).
> Pass 1 computes maxarc, the maximum number of arc lines (target != -1) in any state block (a block is a run of consecutive lines with equal state_no; the running count resets at each block boundary, detected by comparing line i's state_no with line i+1's, which also handles the final sentinel).
> If maxarc > sigma_max(net->sigma) - 2 (more arcs than available symbol numbers >= 3), adds maxarc - (maxsigma-2) new sigma symbols via sigma_add, each named sprintf("%012X", newsym++) — 12 zero-padded uppercase hex digits from an unsigned counter starting at 8723643 — then sigma_sort(net).
> Pass 2 walks the lines with counter j reset to 3 at every block boundary: for each arc line, set out = j++ and rewrite in from IDENTITY(2) to UNKNOWN(1) (other in values unchanged). Thus a state's k-th arc (0-based, in table order) gets output symbol number 3+k.
> Returns the minimized, relabeled net (same ownership; other flags/counts are whatever fsm_minimize/fsm_count left).

> [spec:foma:def:structures.fsm-lowerdeteps-fn]
> struct fsm *fsm_lowerdeteps(struct fsm *net)

> [spec:foma:sem:structures.fsm-lowerdeteps-fn]
> Identical to fsm_lowerdet (same minimize+count, same maxarc computed over ALL arcs, same conditional addition of "%012X"-named sigma symbols from unsigned counter 8723643, same per-state counter j starting at 3 and resetting at block boundaries) with one difference in pass 2:
> only arcs with out != EPSILON(0) are relabeled (out = j++, in rewritten IDENTITY->UNKNOWN); arcs with epsilon output are left completely untouched and do not consume a j value.
> Consumes net; returns the relabeled net.

> [spec:foma:def:structures.fsm-markallfinal-fn]
> struct fsm *fsm_markallfinal(struct fsm *net)

> [spec:foma:sem:structures.fsm-markallfinal-fn]
> Iterates net->states until the sentinel (state_no == -1) and sets final_state = YES(1) on every line (all lines of every state, arc lines included).
> Does not update finalcount, pathcount, or any flags. Returns the same net pointer, modified in place.

> [spec:foma:def:structures.fsm-quantifier-fn]
> struct fsm *fsm_quantifier(char *string)

> [spec:foma:sem:structures.fsm-quantifier-fn]
> Builds the regex `\x* x \x* x \x*` where x = fsm_symbol(string) and \x = fsm_term_negation(fsm_symbol(string)): strings containing exactly two occurrences of the quantifier symbol.
> Exact construction: fsm_concat(fsm_kleene_star(fsm_term_negation(fsm_symbol(string))), fsm_concat(fsm_symbol(string), fsm_concat(fsm_kleene_star(fsm_term_negation(fsm_symbol(string))), fsm_concat(fsm_symbol(string), fsm_kleene_star(fsm_term_negation(fsm_symbol(string))))))).
> Each occurrence builds a fresh fsm_symbol net; the combinators consume their operands. Returns a caller-owned net.

> [spec:foma:def:structures.fsm-set-option-fn]
> _Bool fsm_set_option(unsigned long long option, void *value)

> [spec:foma:sem:structures.fsm-set-option-fn]
> If option == FSMO_SKIP_WORD_BOUNDARY_MARKER (enum value 0): dereferences value as `_Bool *` and stores the result into the global `fsm_options.skip_word_boundary_marker`, returning 1 (true).
> Any other option value does nothing and returns 0. `value` is not NULL-checked.

> [spec:foma:def:structures.fsm-sigma-destroy-fn]
> int fsm_sigma_destroy(struct sigma *sigma)

> [spec:foma:sem:structures.fsm-sigma-destroy-fn+1]
> Frees an entire sigma linked list: for each node (saving its next pointer first), frees node->symbol if non-NULL (nulling the field before the node itself is freed), then frees the node.
> Safe on a NULL list (loop body never runs). Returns nothing (the C `int` return, always 1, carries no information).

> [spec:foma:def:structures.fsm-sigma-net-fn]
> struct fsm *fsm_sigma_net(struct fsm *net)

> [spec:foma:sem:structures.fsm-sigma-net-fn]
> Rebuilds net in place as the machine accepting exactly one occurrence of each single alphabet symbol: one arc per sigma symbol from start state 0 to final state 1.
> If sigma_size(net->sigma) == 0: fsm_destroy(net) and return a fresh fsm_empty_set().
> Otherwise, via the dynarray builder: fsm_state_init(sigma_max(net->sigma)); begin state 0 (non-final, start); for each sigma node in list order whose number >= 3 or number == IDENTITY(2) (EPSILON 0 and UNKNOWN 1 are skipped), fsm_state_add_arc(0, number, number, 1, 0, 1) and increment a pathcount; end state 0; begin state 1 (final, non-start) and end it.
> free(net->states); fsm_state_close(net) installs the newly built table and its counts into net (sigma is kept).
> Sets net->is_minimized = YES, net->is_loop_free = YES, net->pathcount = pathcount; sigma_cleanup(net, 1) drops sigma entries no longer used; returns the same net pointer.

> [spec:foma:def:structures.fsm-sigma-pairs-net-fn]
> struct fsm *fsm_sigma_pairs_net(struct fsm *net)

> [spec:foma:sem:structures.fsm-sigma-pairs-net-fn]
> Rebuilds net in place as the machine of attested label pairs: one arc from start state 0 to final state 1 for each distinct (in, out) pair occurring on any arc of net (epsilon labels included; deduplicated).
> Allocates a dedup table `pairs` = calloc(smax*smax, 1) bytes, smax = sigma_max(net->sigma)+1, indexed pairs[smax*in + out] (in/out read as short ints).
> fsm_state_init(sigma_max(net->sigma)); begin state 0 (non-final, start); scan net->states until the sentinel, skipping lines with target == -1; for each unseen (in,out) pair call fsm_state_add_arc(0, in, out, 1, 0, 1), mark the table, increment pathcount. End state 0; begin state 1 (final, non-start) and end it.
> free(pairs); free(net->states); fsm_state_close(net) installs the new table. If pathcount == 0 (source had no arcs): fsm_destroy(net) and return a fresh fsm_empty_set().
> Otherwise set is_minimized = YES, is_loop_free = YES, pathcount; sigma_cleanup(net, 1); return the same net pointer.

> [spec:foma:def:structures.fsm-sort-arcs-fn]
> void fsm_sort_arcs(struct fsm *net, int direction)

> [spec:foma:sem:structures.fsm-sort-arcs-fn]
> Sorts, per source state and in place, the arc lines of net->states in ascending order of the `in` field when direction == 1, otherwise of the `out` field (canonical value 2), using qsort (unstable) with comparators linesortcompin/linesortcompout.
> Scan the line array until the sentinel, tracking the current block's start index (lasthead, initially 0) and a running line count. A block ends at line i when line i+1 has a different state_no OR line i has target == -1. On block end: count line i, but exclude it again if its target == -1 (arcless marker lines are never sorted); if the resulting count > 1, qsort the count lines starting at lasthead; then reset the count to 0 and set lasthead = i+1. Lines not at a block end just increment the count.
> Flag updates afterwards: if net->arity == 1, set both arcs_sorted_in and arcs_sorted_out to 1 and return; else direction 1 sets arcs_sorted_in=1 and arcs_sorted_out=0; direction 2 sets arcs_sorted_out=1 and arcs_sorted_in=0; any other direction value leaves both flags untouched even though the arcs were sorted by out.
> No allocation; assumes lines are grouped by state_no.

> [spec:foma:def:structures.fsm-state-copy-fn]
> struct fsm_state *fsm_state_copy(struct fsm_state *fsm_state, int linecount)

> [spec:foma:sem:structures.fsm-state-copy-fn]
> Mallocs a new array of `linecount` fsm_state entries and memcpy's exactly linecount lines from the source array into it; returns the new caller-owned array.
> For the copy to be a complete table, linecount must include the trailing -1 sentinel line (fsm_count's linecount convention does). No validation is performed.

> [spec:foma:def:structures.linesortcompin-fn]
> int linesortcompin(const void *_a, const void *_b)

> [spec:foma:sem:structures.linesortcompin-fn]
> qsort comparator over `struct fsm_state`: casts both const void pointers to const struct fsm_state * and returns `a->in - b->in` (int subtraction of the short `in` fields; negative/zero/positive gives ascending order of input symbol number).

> [spec:foma:def:structures.linesortcompout-fn]
> int linesortcompout(const void *_a, const void *_b)

> [spec:foma:sem:structures.linesortcompout-fn]
> Same as linesortcompin but compares the `out` fields: returns `a->out - b->out` (ascending order of output symbol number).

> [spec:foma:def:structures.map-firstlines-fn]
> struct state_array *map_firstlines(struct fsm *net)

> [spec:foma:sem:structures.map-firstlines-fn]
> Builds an index from state number to that state's first line in net->states. Mallocs an array of net->statecount + 1 `struct state_array` entries (each holding one `struct fsm_state *transitions` pointer).
> Scans the line array until the sentinel; whenever a line's state_no differs from the previously seen one (tracker starts at -1), stores a pointer to that line at array index state_no.
> Requires lines grouped by state_no and net->statecount up to date (via fsm_count). Entries for state numbers that never appear stay uninitialized (malloc, not calloc).
> Returns the caller-owned array; the stored pointers alias net->states and must not outlive it.

> [spec:foma:def:structures.purge-quantifier-fn]
> void purge_quantifier (char *string)

> [spec:foma:sem:structures.purge-quantifier-fn+1]
> Walks the global static `quantifiers` list and unlinks EVERY node whose name strcmp-equals string. Removed nodes and their names are dropped (the C leaked them).
> The C walked with a trailing prev pointer that advanced onto the node it had just unlinked, so of two CONSECUTIVE matching nodes only the first left the live list (the second unlink wrote into the already-removed node). This removes all matching nodes, consecutive or not — the evident intent.

> [spec:foma:def:structures.union-quantifiers-fn]
> struct fsm *union_quantifiers()

> [spec:foma:sem:structures.union-quantifiers-fn+1]
> Builds a one-state FSM over all currently defined quantifier symbols (global `quantifiers` list): state 0 is both start and final with one self-loop (target 0) labeled in=out=s per quantifier symbol, so it accepts any sequence (Kleene closure) of quantifier symbols.
> net = fsm_create(""); fsm_update_flags(net, YES,YES,YES,YES,NO,NO) — deterministic/pruned/minimized/epsilon-free YES, loop-free NO, completed NO.
> For each quantifier name in list order: s = sigma_add(name, net->sigma); the first assigned number is captured as symlo (taken while the tracker is still 0); syms counts the names. Arc i (i = 0..syms-1) is labeled symlo+i, relying on sigma_add assigning consecutive numbers.
> states = malloc((syms+1) lines): line i = {state_no=0, in=out=symlo+i, target=0, final_state=1, start_state=1} for i < syms; line syms = all-(-1) sentinel. Sets arccount=syms, statecount=finalcount=1; pathcount left unset.
> Linecount = syms+1, INCLUDING the sentinel line to match fsm_count's convention (was: syms, excluding it). Every caller recounts via fsm_count before reading linecount, so no downstream value changed.
> With an empty quantifier list the table is just the sentinel (no state 0 despite statecount=1) and linecount = 1. Returns a caller-owned net.


# foma/spelling.c

> [spec:foma:def:spelling.apply-med-clear-fn]
> void apply_med_clear(struct apply_med_handle *medh)

> [spec:foma:sem:spelling.apply-med-clear-fn]
> Destructor for a MED handle. If `medh` is NULL, return immediately. Otherwise free each
> of these members if (and only if) non-NULL: `agenda`, `instring`, `outstring`, `heap`,
> `state_array`, `align_symbol`, `letterbits`, `nletterbits`, `intword`; if `sigmahash` is
> non-NULL dispose it with `sh_done`. Finally free the handle struct itself. Does not free
> `medh->net` or `medh->cm`: the network and its confusion matrix are owned by the
> `struct fsm`, not the handle. After this call any strings previously returned by
> `apply_med`/the getters are dangling.

> [spec:foma:def:spelling.apply-med-fn]
> char *apply_med(struct apply_med_handle *medh, char *word)

> [spec:foma:sem:spelling.apply-med-fn]
> Incremental A* search for minimum-edit-distance matches of `word` against `medh->net`.
> Non-NULL `word` starts a new search; NULL jumps straight into the middle of the expansion
> loop (label `resume`) to continue the previous search and yield the next-best match —
> calling with NULL before any word was ever searched is undefined behavior (resume state in
> the handle is uninitialized). Returns `medh->outstring` (the matched dictionary-side word;
> the aligned input word is left in `medh->instring`, its cost in `medh->cost`) on each
> match; returns NULL when the heap is exhausted, the cheapest node's f exceeds
> `med_cutoff`, `med_limit` matches have been returned, or the agenda hit
> `med_max_heap_size`.
> Setup (word non-NULL): store the word pointer; reset nodes_expanded=0, astarcount=1,
> heapcount=0 (agenda slot 0, the heap sentinel, is retained); wordlen = strlen (bytes),
> utf8len = codepoint count (utf8strlen); free any previous `intword` and allocate
> utf8len+1 ints; convert each UTF-8 codepoint (utf8skip(p)+1 bytes, copied through a
> 5-byte temp buffer) to its sigma number via `sigmahash`, or IDENTITY (2) when not in the
> alphabet; terminate intword with sentinel -1. Local edit costs delcost=subscost=inscost=1
> are fixed each call and used only when `hascm` is false; otherwise the confusion matrix
> supplies all costs. Insert the root node (wordpos 0, state 0, g 0,
> h = `[spec:foma:sem:spelling.calculate-h-fn]` at (0,0), in 0, out 0, parent -1); if
> node_insert fails return NULL; nummatches = 0.
> Main loop: pop the cheapest node via `[spec:foma:sem:spelling.node-delete-min-fn]`;
> immediately save curr_agenda_offset = node - agenda (node_insert may realloc the agenda,
> so the node must be re-addressed by index; note the subtraction is computed before the
> NULL check — benign UB when the heap is empty); if NULL return NULL. A leftover
> conditional on final_state/wordpos has an empty body (dead code). nodes_expanded++; if
> node->f > med_cutoff return NULL (min-heap ordering guarantees no cheaper node remains).
> Copy wordpos/fsmstate/g into curr_pos/curr_state/curr_g; lines = 0;
> curr_node_has_match = 0.
> Expansion: iterate the state's contiguous transition lines starting at
> state_array[curr_state].transitions (a line with state_no == -1 terminates); `lines`
> counts lines seen (1-based). If the line's final_state flag is set, curr_pos == utf8len,
> and curr_node_has_match == 0: set the flag, call print_match on
> agenda+curr_agenda_offset, nummatches++, and return outstring — the search suspends here;
> all loop state lives in the handle (curr_ptr, curr_pos, curr_g, curr_state, lines,
> nummatches, curr_agenda_offset), which is what makes NULL-resume work. `resume` re-enters
> immediately after this point: first, if nummatches == med_limit return NULL. Dead-end
> lines (target == -1): with curr_pos == utf8len stop expanding this node; with lines == 1
> jump directly to the insertion step (a state with no arcs still allows insertions);
> otherwise stop.
> Per arc (dictionary-side label in = curr_ptr->in) up to three successors are generated,
> each inserted only if g+h <= med_cutoff, and the whole search aborts (return NULL) if
> node_insert reports the size cap: (1) deletion — consume the arc, no word symbol:
> node (curr_pos, target), in = arc label, out = 0,
> g = curr_g + (hascm ? cm[in*maxsigma + 0] : 1), h at (curr_pos, target); (2) unless
> curr_pos == utf8len, match/substitution: out = intword[curr_pos],
> g = curr_g when in == out, else curr_g + (hascm ? cm[in*maxsigma + out] : 1), h at
> (curr_pos+1, target), node (curr_pos+1, target); (3) insertion — consume a word symbol,
> stay in the same state; executed only when lines == 1 (hence at most once per expanded
> node) and skipped when curr_pos == utf8len: in = 0, out = intword[curr_pos],
> g = curr_g + (hascm ? cm[out] : 1), h at (curr_pos+1, curr_state), node
> (curr_pos+1, curr_state). When the word is exhausted only deletion applies. Node in/out
> fields therefore hold (dictionary symbol, word symbol), 0 meaning epsilon. After the
> insertion step, target == -1 stops this node. Advance curr_ptr to the next line while it
> shares state_no, else fall back to popping the next node.

> [spec:foma:def:spelling.apply-med-get-cost-fn]
> int apply_med_get_cost(struct apply_med_handle *medh)

> [spec:foma:sem:spelling.apply-med-get-cost-fn]
> Returns `medh->cost`, the accumulated edit cost (the matched node's g) of the most recent
> match produced by `apply_med` (set by print_match). No NULL check on `medh` (unlike the
> setters). Value is meaningless before the first match.

> [spec:foma:def:spelling.apply-med-get-instring-fn]
> char *apply_med_get_instring(struct apply_med_handle *medh)

> [spec:foma:sem:spelling.apply-med-get-instring-fn]
> Returns `medh->instring`, the handle-owned buffer holding the aligned input word from the
> most recent match (built by print_match). Not a copy: the pointer may be invalidated by a
> later `apply_med` call (realloc) and is freed by apply_med_clear; the caller must not
> free it. No NULL check on `medh`.

> [spec:foma:def:spelling.apply-med-get-outstring-fn]
> char *apply_med_get_outstring(struct apply_med_handle *medh)

> [spec:foma:sem:spelling.apply-med-get-outstring-fn]
> Returns `medh->outstring`, the handle-owned buffer holding the matched dictionary-side
> word from the most recent match (the same pointer `apply_med` returns). Not a copy: may
> be invalidated by a later `apply_med` call (realloc) and is freed by apply_med_clear; the
> caller must not free it. No NULL check on `medh`.

> [spec:foma:def:spelling.apply-med-init-fn]
> struct apply_med_handle *apply_med_init(struct fsm *net)

> [spec:foma:sem:spelling.apply-med-init-fn]
> Allocates (calloc, so all fields start zero/NULL) and returns a MED handle bound to `net`
> (not owned; net's arcs must be sorted so each state's lines are contiguous). Agenda:
> malloc INITIAL_AGENDA_SIZE = 256 astarnodes; set agenda[0].f = -1 — slot 0 is the
> permanent heap sentinel (its other fields stay uninitialized); agenda_size = 256;
> astarcount = 1 (slot 0 reserved). Heap: malloc 256 ints, heap_size = 256, heap[0] = 0
> (index of the sentinel node), heapcount = 0. state_array = map_firstlines(net), an array
> giving each state's first transition line. If net->medlookup and its confusion_matrix are
> both non-NULL: hascm = 1 and cm points at the matrix (borrowed). maxsigma =
> sigma_max(net->sigma) + 1. sigmahash = sh_init(); every sigma entry with
> number > IDENTITY (i.e. >= 3 — EPSILON 0, UNKNOWN 1, IDENTITY 2 are excluded; the scan
> also stops at a number == -1 entry) is added mapping symbol string to number. Then calls
> `[spec:foma:sem:spelling.fsm-create-letter-lookup-fn]` to build letterbits/nletterbits
> and set maxdepth. instring and outstring: malloc INITIAL_STRING_SIZE = 256 bytes each,
> with instring_length = outstring_length = 256. Defaults: med_limit = 4 (max matches
> yielded per word), med_cutoff = 15 (max admissible total cost f = g+h),
> med_max_heap_size = 262145 (agenda growth cap in node_insert). Caller releases with
> apply_med_clear.

> [spec:foma:def:spelling.apply-med-set-align-symbol-fn]
> void apply_med_set_align_symbol(struct apply_med_handle *medh, char *align)

> [spec:foma:sem:spelling.apply-med-set-align-symbol-fn]
> If `medh` is non-NULL, sets medh->align_symbol = strdup(align). The align symbol is what
> print_match emits (on both instring and outstring) for epsilon positions in the
> alignment; when unset (NULL), epsilon positions produce nothing. Latent leak: a previous
> align_symbol is not freed before being overwritten (apply_med_clear frees only the last
> one). Passing align == NULL crashes strdup.

> [spec:foma:def:spelling.apply-med-set-heap-max-fn]
> void apply_med_set_heap_max(struct apply_med_handle *medh, int max)

> [spec:foma:sem:spelling.apply-med-set-heap-max-fn]
> If `medh` is non-NULL, sets medh->med_max_heap_size = max: the cap consulted by
> `[spec:foma:sem:spelling.node-insert-fn]` — an agenda whose doubled size would be >= this
> value refuses to grow, which makes apply_med abort the search. NULL `medh` is a no-op.

> [spec:foma:def:spelling.apply-med-set-med-cutoff-fn]
> void apply_med_set_med_cutoff(struct apply_med_handle *medh, int max)

> [spec:foma:sem:spelling.apply-med-set-med-cutoff-fn]
> If `medh` is non-NULL, sets medh->med_cutoff = max: the maximum admissible total cost
> f = g+h. apply_med neither enqueues successors with g+h above it nor expands a popped
> node whose f exceeds it (terminating the search). Default 15. NULL `medh` is a no-op.

> [spec:foma:def:spelling.apply-med-set-med-limit-fn]
> void apply_med_set_med_limit(struct apply_med_handle *medh, int max)

> [spec:foma:sem:spelling.apply-med-set-med-limit-fn]
> If `medh` is non-NULL, sets medh->med_limit = max: the maximum number of matches
> apply_med will yield per word (checked at the resume point; once nummatches equals it,
> resumption returns NULL). Default 4. NULL `medh` is a no-op.

> [spec:foma:def:spelling.calculate-h-fn]
> int calculate_h(struct apply_med_handle *medh, int *intword, int currpos, int state)

> [spec:foma:sem:spelling.calculate-h-fn]
> A* heuristic: a lower bound on the remaining cost of matching the word suffix
> intword[currpos..] from `state`, assuming every unmatched symbol costs at least 1. If
> intword[currpos] == -1 (the end sentinel) return 0. Locate the two bitsets for `state`
> at offset state * bytes_per_letter_array into medh->letterbits and medh->nletterbits
> (built by `[spec:foma:sem:spelling.fsm-create-letter-lookup-fn]`). Compute hinf = the
> number of suffix symbols (scanning until the -1 sentinel) whose bit is NOT set in the
> state's letterbits, i.e. symbols that occur nowhere on any path leaving `state` — each
> necessarily costs an edit. Compute hn = the number of symbols among the first
> medh->maxdepth (= 2) suffix symbols whose bit is NOT set in nletterbits (symbols not
> matchable within maxdepth transitions of `state`). Bits are tested with
> BITTEST(a,b) = a[b>>3] & (1 << (b & 7)). Return max(hinf, hn). Every occurrence counts
> (a symbol missing from letterbits contributes once per repetition in the suffix). A
> duplicate of the sentinel check appears between the two loops (dead code). The heuristic
> is admissible when all edit costs are >= 1; a confusion matrix with 0-cost cells can make
> it inadmissible (search may then return non-optimal matches first).

> [spec:foma:def:spelling.cmatrix-default-delete-fn]
> void cmatrix_default_delete(struct fsm *net, int cost)

> [spec:foma:sem:spelling.cmatrix-default-delete-fn]
> Sets the default deletion cost: with maxsigma = sigma_max(net->sigma)+1 and
> cm = net->medlookup->confusion_matrix, writes cm[i*maxsigma + 0] = cost for every
> i in 0..maxsigma-1, i.e. all of column 0 (dictionary symbol i paired with epsilon —
> the cost apply_med charges to traverse an arc labeled i without consuming input). Also
> overwrites cm[0][0] (epsilon:epsilon, never consulted) and the reserved rows 1-2. No
> NULL check: requires cmatrix_init to have run first.

> [spec:foma:def:spelling.cmatrix-default-insert-fn]
> void cmatrix_default_insert(struct fsm *net, int cost)

> [spec:foma:sem:spelling.cmatrix-default-insert-fn]
> Sets the default insertion cost: with maxsigma = sigma_max(net->sigma)+1 and
> cm = net->medlookup->confusion_matrix, writes cm[j] = cost for every j in
> 0..maxsigma-1, i.e. all of row 0 (epsilon paired with word symbol j — the cost apply_med
> charges to consume a word symbol without moving in the FSM, looked up as cm[out]). Also
> overwrites cm[0][0] and the reserved columns 1-2. No NULL check: requires cmatrix_init
> to have run first.

> [spec:foma:def:spelling.cmatrix-default-substitute-fn]
> void cmatrix_default_substitute(struct fsm *net, int cost)

> [spec:foma:sem:spelling.cmatrix-default-substitute-fn]
> Sets the default substitution cost: with maxsigma = sigma_max(net->sigma)+1 and
> cm = net->medlookup->confusion_matrix, for all i, j in 1..maxsigma-1 writes
> cm[i*maxsigma + j] = 0 when i == j (identity match is free) and cost otherwise. Row 0
> and column 0 (insertions/deletions) are untouched; reserved indices 1-2 are included in
> the sweep. No NULL check: requires cmatrix_init to have run first.

> [spec:foma:def:spelling.cmatrix-init-fn]
> void cmatrix_init(struct fsm *net)

> [spec:foma:sem:spelling.cmatrix-init-fn]
> Creates and installs a confusion matrix on the network. If net->medlookup is NULL,
> calloc a `struct medlookup` for it. maxsigma = sigma_max(net->sigma) + 1. Allocate
> cm = calloc(maxsigma * maxsigma ints) and store it in
> net->medlookup->confusion_matrix (any previous matrix pointer is overwritten without
> being freed — latent leak on re-init). Initialize every cell: cm[i*maxsigma + j] = 0
> when i == j, else 1. Layout convention used throughout: the row index is the
> dictionary-side (FSM input) symbol number, the column index the input-word symbol
> number; row 0 holds insertion costs, column 0 deletion costs, other cells substitution
> costs; symbol numbers 1 (UNKNOWN) and 2 (IDENTITY) also get rows/columns. Once
> installed, apply_med_init will set hascm and use these costs instead of uniform 1.

> [spec:foma:def:spelling.cmatrix-print-att-fn]
> void cmatrix_print_att(struct fsm *net, FILE *outfile)

> [spec:foma:sem:spelling.cmatrix-print-att-fn]
> Writes the confusion matrix to `outfile` as an AT&T-format weighted transducer with a
> single state 0 and one self-loop per cell. With maxsigma = sigma_max(net->sigma)+1 and
> cm = net->medlookup->confusion_matrix, loop i (row) and j (column) over 0..maxsigma-1,
> skipping any pair where i is 1 or 2 or j is 1 or 2 (reserved UNKNOWN/IDENTITY). For
> i == 0, j != 0 print "0\t0\t@0@\t<sym_j>\t<cost>\n" (epsilon rendered as literal "@0@");
> for j == 0, i != 0 print "0\t0\t<sym_i>\t@0@\t<cost>\n"; for both nonzero print
> "0\t0\t<sym_i>\t<sym_j>\t<cost>\n"; the (0,0) cell emits nothing. cost is
> cm[i*maxsigma + j], symbols come from sigma_string. Finish with the final-state line
> "0\n". No NULL checks on medlookup/cm.

> [spec:foma:def:spelling.cmatrix-print-fn]
> void cmatrix_print(struct fsm *net)

> [spec:foma:sem:spelling.cmatrix-print-fn]
> Pretty-prints the confusion matrix to stdout as a table. maxsigma =
> sigma_max(net->sigma)+1, cm = net->medlookup->confusion_matrix. First compute lsymbol =
> the longest symbol-string length among sigma entries with number >= 3 (entries < 3 are
> skipped). Header line: lsymbol+2 spaces, then "0 " (the epsilon/deletion column), then
> each symbol from number 3 upward via sigma_string, each followed by a space, stopping at
> the first number for which sigma_string returns NULL (a gap in the numbering truncates
> the header). Data rows for i = 0..maxsigma-1, with rows 1 and 2 skipped (after printing
> row 0, i is incremented twice extra). Each row starts with its label right-aligned in
> width lsymbol+1 — the literal "0" for row 0, else sigma_string(i) — followed by the
> column-0 cell right-aligned in width 2: "*" for row 0 (the unused epsilon:epsilon cell),
> else cm[i*maxsigma] (the deletion cost). Columns 1 and 2 are then skipped (j is bumped
> from 0 to 3). For each remaining column j: on the diagonal (i == j) print "*" via
> printf("%.*s", strlen(sym_j)+1, "*") — %s precision truncates but does not pad, so
> exactly one character is emitted; off-diagonal print the cost via
> printf("%.*d", strlen(sym_j)+1, cm[i*maxsigma+j]) — %d precision zero-pads, so the cost
> occupies exactly strlen(sym_j)+1 characters, matching the header column width. Latent
> formatting bug: the 1-character diagonal "*" under-fills its column, misaligning the
> remainder of that row for any symbol longer than... (any symbol at all, since the column
> width is strlen+1 >= 2). Newline after each row. No NULL checks on medlookup/cm.

> [spec:foma:def:spelling.cmatrix-set-cost-fn]
> void cmatrix_set_cost(struct fsm *net, char *in, char *out, int cost)

> [spec:foma:sem:spelling.cmatrix-set-cost-fn]
> Sets one confusion-matrix cell. maxsigma = sigma_max(net->sigma)+1, cm =
> net->medlookup->confusion_matrix (no NULL check). Resolve i: 0 if `in` is NULL (epsilon,
> i.e. insertion row), else sigma_find(in, net->sigma); resolve o likewise from `out`
> (0 = deletion column). If i == -1, print "Warning, symbol '%s' not in alphabet\n" with
> `in` to stdout and return without modifying anything; likewise for o == -1 with `out`
> (the `in` warning takes precedence when both are unknown). Otherwise set
> cm[i*maxsigma + o] = cost. So (NULL, s) sets the insertion cost of s, (s, NULL) the
> deletion cost of s, (s, t) the substitution cost s -> t.

> [spec:foma:def:spelling.fsm-create-letter-lookup-fn]
> void fsm_create_letter_lookup(struct apply_med_handle *medh, struct fsm *net)

> [spec:foma:sem:spelling.fsm-create-letter-lookup-fn]
> Builds the two per-state symbol bitsets consumed by
> `[spec:foma:sem:spelling.calculate-h-fn]`, and sets medh->maxdepth = 2. With
> num_states = net->statecount and num_symbols = sigma_max(net->sigma):
> medh->bytes_per_letter_array = ceil((num_symbols+1)/8) (BITNSLOTS), and
> medh->letterbits / medh->nletterbits are each calloc'd to
> bytes_per_letter_array * num_states bytes (owned by the handle, freed by
> apply_med_clear). A local per-state array of sccinfo {index, lowlink, on_t_stack} is
> calloc'd and freed at the end.
> Phase 1 — letterbits[v] = the set of arc input labels occurring anywhere on any path
> from v (the n = infinity case): an iterative version (via gotos) of Tarjan's SCC DFS,
> using the global ptr_stack to hold suspended edge pointers and the global int_stack as
> the Tarjan stack. The DFS index counter starts at 1 (0 in sccinfo.index means
> unvisited). Start from net->states (the first transition line, assumed to belong to
> state 0). Visiting state v: index[v] = lowlink[v] = counter++, push v on int_stack,
> on_t_stack[v] = 1; if the current line has target == -1 (arcless state) go straight to
> the SCC-root check. For each transition line (v, in, vp): set bit `in` in letterbits[v];
> if index[vp] == 0, push the line on ptr_stack and descend to vp's first line (recursive
> call); else, if vp is on the Tarjan stack, lowlink[v] = min(lowlink[v], lowlink[vp])
> (Tarjan's original uses index[vp] here — the code literally uses lowlink; results are
> equivalent for SCC roots); then OR letterbits[vp] into letterbits[v]. Advance to the
> next line while it shares v's state_no. When v's lines are exhausted and
> lowlink[v] == index[v], v is an SCC root: pop int_stack until v appears, and for each
> popped state clear on_t_stack and overwrite its letterbits with a copy of v's (all SCC
> members share the root's set); finally clear on_t_stack[v]. Returning from a descent
> (outer loop pops edge v -> vp from ptr_stack): OR letterbits[vp] into letterbits[v], set
> the edge label's bit in letterbits[v], lowlink[v] = min(lowlink[v], lowlink[vp]), then
> continue with v's next line, or the root check if that was the last. Only states
> reachable from state 0 are visited; unreachable states keep all-zero letterbits.
> int_stack_clear() when done.
> Phase 2 — nletterbits[v] = the set of labels reachable within maxdepth (= 2) transitions
> of v: for each state v independently, a depth-bounded DFS with explicit stacks: push v's
> first line with depth 0; repeatedly pop (line, depth); if depth == maxdepth abandon the
> branch; if line->in != -1 set its bit in nletterbits[v]; if line->target != -1, first
> push the next sibling line (when it shares state_no) at the same depth, then descend
> into the target's first line with depth+1 and repeat. (Two commented-out debug loops
> over the bitsets have no effect.)

> [spec:foma:def:spelling.letterbits-add-fn]
> void letterbits_add(int v, int symbol, uint8_t *ptr, int bytes_per_letter_array)

> [spec:foma:sem:spelling.letterbits-add-fn]
> Sets bit `symbol` in state v's bit array: with base = ptr + v*bytes_per_letter_array,
> perform base[symbol >> 3] |= (1 << (symbol & 7)). No bounds checking; `symbol` must be
> in 0..(8*bytes_per_letter_array - 1) (in practice 0..sigma_max).

> [spec:foma:def:spelling.letterbits-copy-fn]
> void letterbits_copy(int source, int target, uint8_t *ptr, int bytes_per_letter_array)

> [spec:foma:sem:spelling.letterbits-copy-fn]
> Overwrites state `target`'s bit array with state `source`'s: copies
> bytes_per_letter_array bytes from ptr + source*bytes_per_letter_array to
> ptr + target*bytes_per_letter_array, byte by byte. Destination's previous bits are lost.

> [spec:foma:def:spelling.letterbits-union-fn]
> void letterbits_union(int v, int vp, uint8_t *ptr, int bytes_per_letter_array)

> [spec:foma:sem:spelling.letterbits-union-fn]
> In-place bitwise union: for each of the bytes_per_letter_array bytes, OR state vp's
> byte (at ptr + vp*bytes_per_letter_array) into state v's byte (at
> ptr + v*bytes_per_letter_array). v's set becomes v ∪ vp; vp is unchanged.

> [spec:foma:def:spelling.node-delete-min-fn]
> struct astarnode *node_delete_min(struct apply_med_handle *medh)

> [spec:foma:sem:spelling.node-delete-min-fn]
> Pops the highest-priority node from the binary heap and returns a pointer to it in the
> agenda; returns NULL when heapcount == 0. The heap (medh->heap) stores agenda indices in
> slots 1..heapcount (slot 0 is the sentinel index 0). Priority: smaller f first; among
> equal f, larger wordpos first (prefers nodes deeper into the word). The result is
> agenda + heap[1]; the popped node stays in the agenda (the agenda is append-only —
> ancestors must survive for print_match's parent-chain walk), but the returned pointer is
> only valid until the next agenda realloc in node_insert. Re-heapify: remember the last
> element lastptr = agenda + heap[heapcount], decrement heapcount, then sift down from
> i = 1: child = 2i while 2i <= heapcount; if a right child exists (child != heapcount)
> and beats the left child — right.f < left.f, or (right.f <= left.f and
> right.wordpos > left.wordpos) — select it; if the selected child beats lastptr —
> child.f < last.f, or (child.f <= last.f and child.wordpos > last.wordpos) — move it up
> (heap[i] = heap[child]) and continue from the child slot, else stop. Finally store the
> last element's agenda index at heap[i]. (The <= alternatives combined with the preceding
> strict < make the second clauses effective only at equal f.)

> [spec:foma:def:spelling.node-insert-fn]
> int node_insert(struct apply_med_handle *medh, int wordpos, int fsmstate, int g, int h, int in, int out, int parent)

> [spec:foma:sem:spelling.node-insert-fn]
> Appends a search node to the agenda and pushes its index onto the min-heap. Returns 1 on
> success, 0 to signal that the agenda refuses to grow (caller aborts the search). Agenda:
> the new node goes in slot i = astarcount; if i >= agenda_size - 1, then if
> agenda_size*2 >= med_max_heap_size return 0 without inserting, else double agenda_size
> and realloc the agenda (this invalidates every outstanding astarnode pointer — callers
> must address nodes by index across insertions). Fill the node: wordpos, fsmstate,
> f = g+h, g, h (f/g/h and wordpos are short ints — costs beyond SHRT_MAX silently wrap),
> in (dictionary-side symbol, 0 = epsilon), out (word-side symbol, 0 = epsilon), parent
> (agenda index of the predecessor node, -1 for the root); astarcount++. Heap: increment
> heapcount; if heapcount == heap_size - 1, realloc the heap to double size. Sift up from
> j = heapcount: while the parent slot's node (agenda[heap[j>>1]]) has f > the new f, OR
> has f >= the new f and wordpos <= the new wordpos, copy the parent index down
> (heap[j] = heap[j>>1]) and set j >>= 1; then heap[j] = i. Ordering achieved: min-f, ties
> preferring larger wordpos, and among equal (f >= new f, wordpos <= new wordpos) the new
> node is placed above equal entries (LIFO for exact ties). Termination relies on the
> sentinel: heap[0] == 0 refers to agenda slot 0 whose f was set to -1 at init, and since
> f = g+h >= 0 for nonnegative costs both clauses fail there. Latent hazard: a confusion
> matrix with negative costs can produce f < 0 and walk the sift past the sentinel
> (undefined behavior).

> [spec:foma:def:spelling.print-match-fn]
> void print_match(struct apply_med_handle *medh, struct astarnode *node, struct sigma *sigma, char *word)

> [spec:foma:sem:spelling.print-match-fn]
> Reconstructs the alignment for the accepted search node into medh->outstring
> (dictionary-side string) and medh->instring (input-word-side string), and sets
> medh->cost = node->g. Uses the global int_stack (cleared first) to reverse the parent
> chain. Chain walk (both passes): n = node, then n = medh->agenda + n->parent; stop
> before pushing when n->in == 0 && n->out == 0 (intended to detect the root node, whose
> in/out are 0/0) or when n->parent == -1. Latent bug: a non-root node representing an
> epsilon-labeled deletion arc also has in == 0 && out == 0 and silently truncates the
> printed alignment at that point (cost is still the full node->g).
> Pass 1: push each n->in; if medh->outstring_length < 2*wordlen (wordlen = medh->wordlen,
> the byte length), double outstring_length once and realloc — latent bug: a single
> doubling may still be too small for a much longer word, and multi-byte symbols /
> align_symbol expansion can exceed the 2*wordlen estimate anyway (possible buffer
> overflow). Pop in path order, appending via sprintf at a running offset: sym > 2 emits
> print_sym(sym, sigma); sym == 0 emits medh->align_symbol if set, else nothing; sym == 2
> (IDENTITY) emits literal "@"; sym == 1 emits nothing.
> Pass 2: identical walk pushing n->out; same conditional single doubling of
> instring_length; maintain a byte cursor i into `word` starting at 0. Pop in order:
> sym > 2 emits the sigma symbol and advances i by the codepoint length
> utf8skip(word+i)+1; sym == 0 emits align_symbol if set (cursor unchanged); sym == 2
> emits "*" if i > wordlen, otherwise copies the next UTF-8 codepoint verbatim from
> word+i and advances i (this is how out-of-alphabet input characters survive
> round-trip). Both buffers end NUL-terminated by the final sprintf.

> [spec:foma:def:spelling.print-sym-fn]
> char *print_sym(int sym, struct sigma *sigma)

> [spec:foma:sem:spelling.print-sym-fn]
> Linear scan of the sigma linked list: return the `symbol` string of the first entry
> whose `number` equals sym, following `next` pointers; return NULL if no entry matches.
> The returned pointer aliases the sigma entry's storage (no copy). Callers in this file
> do not check for NULL (print_match would pass it to sprintf and crash on a symbol
> number absent from sigma).

> [spec:foma:def:spelling.sccinfo]
> struct sccinfo {
>   int index;
>   int lowlink;
>   int on_t_stack;
> }

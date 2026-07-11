# foma/fomalib.h

> [spec:foma:def:fomalib.add-defined-fn]
> FEXPORT int add_defined(struct defined_networks *def, struct fsm *net, char *string)

> [spec:foma:sem:fomalib.add-defined-fn]
> Registers `net` under the name `string` in the defined-networks list `def` (a list with a
> permanent dummy head node so the head pointer never changes). If `net` is NULL, returns 0
> immediately. If `strlen(string) > FSM_NAME_LEN` (40), returns -1 without taking ownership.
> Otherwise calls `fsm_count(net)` (refreshes the net's statecount/arccount/linecount/finalcount
> fields), then scans the list for an entry whose non-NULL name strcmp-equals `string`: if found,
> destroys that entry's old net with fsm_destroy, frees its old name, stores `net` and a fresh
> `strdup(string)` in the entry, and returns 1 (redefinition). If not found: if the head node is
> unused (`def->name == NULL`) it is filled in place, else a new node is malloc'ed and spliced
> immediately after the head (`d->next = def->next; def->next = d`); sets `d->name =
> strdup(string)`, `d->net = net`, returns 0. Ownership: the list takes ownership of `net`;
> `string` is always copied.

> [spec:foma:def:fomalib.add-defined-function-fn]
> FEXPORT int add_defined_function (struct defined_functions *deff, char *name, char *regex, int numargs)

> [spec:foma:sem:fomalib.add-defined-function-fn]
> Registers a regex "function" (name, regex source text, arity) in the defined-functions list
> `deff` (dummy-head list; head pointer stays stable). Scans the list for an entry with the same
> name AND the same `numargs`: if found, frees the old `regex`, stores `strdup(regex)`, prints
> "redefined %s@%i)\n" to stderr (note the stray `)` in the format) when the global `g_verbose`
> is nonzero, and returns 1. Otherwise, if `deff->name == NULL` the head node is filled in place,
> else a new node is malloc'ed and inserted right after the head (`d->next = deff->next;
> deff->next = d`); sets `name = strdup(name)`, `regex = strdup(regex)`, `numargs = numargs`;
> returns 0. Both strings are copied (caller keeps its arguments). Entries with the same name but
> different arities coexist.

> [spec:foma:def:fomalib.apply-clear-fn]
> FEXPORT void apply_clear(struct apply_handle *h)

> [spec:foma:sem:fomalib.apply-clear-fn]
> Destroys an apply handle created by apply_init and everything it owns. Frees, in order: every
> sigma-trie node array recorded in h->sigma_trie_arrays (both each ->arr and the list node
> itself), then — each only if non-NULL, resetting the field to NULL — statemap, numlines, marks,
> searchstack, sigs, flag_lookup, sigmatch_array, flagstates; then apply_clear_index(h) (frees the
> index_in/index_out structures, if built); sets last_net = NULL and iterator = 0; frees
> outstring, separator, epsilon_symbol; finally frees `h` itself. Does NOT destroy the underlying
> fsm (borrowed by the handle), and leaks h->flag_list nodes and h->space_symbol (never freed —
> latent leak). `h` must not be used afterwards.

> [spec:foma:def:fomalib.apply-down-fn]
> FEXPORT char *apply_down(struct apply_handle *h, char *word)

> [spec:foma:sem:fomalib.apply-down-fn]
> One step of "apply down": `word` is matched against the upper/input side and the corresponding
> lower/output-side string is returned. Sets h->mode = DOWN (16); h->indexed = 1 iff an
> input-side index exists (h->index_in != NULL, built by apply_index with APPLY_INDEX_INPUT);
> h->binsearch = 1 iff h->last_net->arcs_sorted_in == 1 (arcs sorted for binary search); then
> delegates to apply_updown(h, word), a depth-first traversal of the network consuming `word` on
> the input side. Protocol: call with non-NULL `word` to start a new lookup (tokenizes the word
> against sigma, clears stack/marks, returns the first result); call again with NULL to resume
> and get the next result; NULL return means no (more) results. Returns NULL immediately if the
> net has no final states. The result points at h->outstring, an internal buffer valid only until
> the next apply call — caller must not free it; `word` is borrowed, not copied. Note
> h->last_net->arcs_sorted_in is dereferenced before any NULL check, so a handle with a NULL net
> crashes here.

> [spec:foma:def:fomalib.apply-index-fn]
> FEXPORT void apply_index(struct apply_handle *h, int inout, int densitycutoff, int mem_limit, int flags_only)

> [spec:foma:sem:fomalib.apply-index-fn]
> Builds a per-state, per-symbol transition index to speed up apply, storing it in h->index_in
> when inout == APPLY_INDEX_INPUT (1) or h->index_out when APPLY_INDEX_OUTPUT (2). If flags_only
> is nonzero and the net has no flag diacritics, returns immediately. Steps: (1) scan the state
> array to compute maxtrans, the largest per-state count of real arcs (lines with target != -1).
> (2) Build pre_index: an array of maxtrans+1 buckets, bucket k holding a linked list of the
> state numbers that have exactly k arcs (empty buckets have state_no -1). The fold into a bucket
> happens only when the scan sees the state number change, so the last state in the array is
> never bucketed and thus never indexed (latent bug). (3) Memory accounting: a running counter
> cnt is charged round_up_to_power_of_two(bytes) per allocation and compared against mem_limit
> (bytes); if the initial charge for the statecount-sized pointer array exceeds it, no index is
> built at all. Otherwise indexptr = calloc(statecount) pointers. If flags_only, ensure the
> h->flagstates bitvector exists (apply_mark_flagstates). (4) Visit buckets densest-first
> (maxtrans down to 0). Skip states with fewer than densitycutoff arcs unless flags_only and the
> state carries a flag arc. For each selected state allocate an array of h->sigma_size entries
> {fsmptr = -1; next = NULL for slot EPSILON (0), else next = &slot[EPSILON]} — every non-epsilon
> chain's tail points at the EPSILON slot so epsilon arcs are traversed automatically; stop
> indexing further states when cnt would exceed mem_limit. (5) One pass over all transition
> lines: for each line i with target != -1 whose source state got an index, sym = the line's `in`
> (input indexing) or `out` label; flag-diacritic symbols are filed under EPSILON, and UNKNOWN
> (1) is filed under IDENTITY (2) since they match the same inputs. First line for a slot goes
> into the slot's fsmptr; later lines are pushed as fresh calloc'ed nodes onto slot->next (they
> end up between the head and the epsilon tail, in reverse scan order). (6) Free the pre_index
> scaffolding and store indexptr on the handle (NULL if the limit was hit in step 3).

> [spec:foma:def:fomalib.apply-init-fn]
> apply_handle *apply_init(struct fsm *net)

> [spec:foma:sem:fomalib.apply-init-fn]
> Creates an apply handle for `net`. Seeds the C PRNG via srand(time(NULL)). calloc's the handle,
> then sets: iterate_old = 0, iterator = 0, instring = NULL, flag_list = NULL, flag_lookup =
> NULL, obey_flags = 1, show_flags = 0, print_space = 0, print_pairs = 0, separator =
> strdup(":"), epsilon_symbol = strdup("0"), last_net = net (borrowed pointer — the net is not
> copied and not owned), outstring = malloc(4096) with outstring[0] = '\0' and outstringtop =
> 4096, gstates = net->states, gsigma = net->sigma, printcount = 1. Then builds per-state tables
> via apply_create_statemap (statemap[s] = index of state s's first line in the state array or
> -1; numlines[s] = number of lines for s; marks[s] = 0), allocates the DFS searchstack with 128
> entries (apply_stack_top = 128, stack pointer cleared), and calls apply_create_sigarray: sigs
> symbol table indexed by sigma number (with sigs[EPSILON] = epsilon_symbol, sigs[UNKNOWN] = "?",
> sigs[IDENTITY] = "@" when maxsigma >= IDENTITY), a 1024-entry sigmatch_array, a byte-trie over
> all sigma symbols with number > IDENTITY for longest-match tokenization, and — if any sigma
> symbol is a flag diacritic — has_flags = 1 plus the flag_lookup table (type/name/value per
> symbol number) and the flagstates bitvector. Returns the handle; free with apply_clear. The net
> must outlive the handle and must not be modified while in use.

> [spec:foma:def:fomalib.apply-lower-words-fn]
> FEXPORT char *apply_lower_words(struct apply_handle *h)

> [spec:foma:sem:fomalib.apply-lower-words-fn]
> Sets h->mode = DOWN|ENUMERATE|LOWER (16+2+32) and returns apply_enumerate(h): enumerates the
> words of the lower (output) side of the network by DFS, one accepted path per call, resuming
> the traversal where the previous call stopped (first call when h->iterator == 0 clears the
> stack and marks and starts fresh, then increments the iterator; later calls resume). Returns
> h->outstring (internal buffer, do not free; valid until the next call) or NULL when all paths
> are exhausted or the net has no final states / h->last_net is NULL. EPSILON contributes nothing
> to the printed string; flag-diacritic symbols are omitted unless show_flags is set (and still
> constrain paths when obey_flags is set). Epsilon loops are entered at most once per position,
> so cyclic nets enumerate (possibly infinitely) without hanging on epsilon cycles. Use
> apply_reset_enumerator to restart from the beginning.

> [spec:foma:def:fomalib.apply-med-clear-fn]
> FEXPORT void apply_med_clear(struct apply_med_handle *h)

> [spec:foma:sem:fomalib.apply-med-clear-fn]
> Destroys an MED handle created by apply_med_init. If `medh` is NULL, does nothing. Frees each
> of agenda, instring, outstring, heap, state_array, align_symbol, letterbits, nletterbits and
> intword if non-NULL; calls sh_done(medh->sigmahash) if non-NULL; then frees the handle itself.
> Does not destroy the underlying fsm (borrowed).

> [spec:foma:def:fomalib.apply-med-fn]
> FEXPORT char *apply_med(struct apply_med_handle *medh, char *word)

> [spec:foma:sem:fomalib.apply-med-fn]
> Finds words accepted by the network that are closest to `word` in edit distance, via A* search;
> one match per call. Call with non-NULL `word` to start a search; call with NULL to resume and
> get the next-best match. Returns medh->outstring (internal buffer, do not free) on a match, or
> NULL when the heap is exhausted, the cheapest node's f exceeds med_cutoff, the agenda would
> exceed med_max_heap_size, or med_limit matches have already been returned. Costs: delete =
> substitute = insert = 1, unless the net carries a confusion matrix (medh->hascm), in which case
> cost(arc-symbol i, word-symbol o) = cm[i*maxsigma+o] (row 0 = insertions cm[o], column 0 =
> deletions cm[i*maxsigma]).
> Initialization (word != NULL): store the word pointer; reset nodes_expanded, astarcount = 1,
> heapcount = 0; wordlen = strlen(word), utf8len = utf8strlen(word); build intword = array of
> utf8len sigma numbers (each UTF-8 character looked up in medh->sigmahash; characters not in
> sigma become IDENTITY = 2), terminated by -1; compute h = calculate_h(intword, 0, 0) and insert
> the root node (wordpos 0, fsm state 0, g = 0, in = out = 0, parent = -1); nummatches = 0.
> Main loop: pop the min-f node from the binary heap (ties prefer larger wordpos); NULL heap →
> return NULL; if node->f > med_cutoff → return NULL. Record the node's agenda offset (the agenda
> may be realloc'ed). Set curr_pos = node->wordpos, curr_state = node->fsmstate, curr_g =
> node->g. Walk curr_state's transition lines in order (counting `lines` from 1): if the line is
> final and curr_pos == utf8len and this node has not yet emitted, emit: print_match()
> reconstructs the path by parent pointers and writes the net-side string into medh->outstring
> (symbols > 2 via their sigma strings, 0 as medh->align_symbol if set, 2 as "@") and the aligned
> input word into medh->instring (out-symbols > 2 as sigma strings, 2 as the actual input
> character, 0 as align_symbol), sets medh->cost = node->g; nummatches++ and return outstring. On
> resume (word == NULL) execution continues exactly after that point: first checks nummatches ==
> med_limit (→ NULL), then keeps expanding the interrupted node.
> Expansion per line with target != -1: (1) deletion — in = arc input, out = 0, g += delcost (or
> cm), h = calculate_h(intword, curr_pos, target); enqueue (same wordpos, state target) if g+h <=
> med_cutoff. (2) unless curr_pos == utf8len: match/substitute — in = arc input, out =
> intword[curr_pos]; g unchanged if in == out else += subscost (or cm); h at (curr_pos+1,
> target); enqueue if within cutoff. (3) insertion, only from the state's first line (lines ==
> 1): in = 0, out = intword[curr_pos], stay in curr_state; g += inscost (or cm[out]); enqueue
> (curr_pos+1, curr_state) if within cutoff. Dummy lines (target == -1): stop the line walk if
> curr_pos == utf8len or lines > 1, else jump straight to the insertion step. Any failed enqueue
> (node_insert returns 0 because doubling the agenda would reach med_max_heap_size) aborts the
> whole search with NULL. The admissible heuristic calculate_h(intword, pos, state) = max(number
> of word symbols from pos onward absent from the state's letterbits — symbols occurring anywhere
> in paths from that state — and number of the next maxdepth (2) word symbols absent from its
> nletterbits — symbols occurring within 2 arcs); 0 at end of word.

> [spec:foma:def:fomalib.apply-med-get-cost-fn]
> FEXPORT int apply_med_get_cost(struct apply_med_handle *medh)

> [spec:foma:sem:fomalib.apply-med-get-cost-fn]
> Returns medh->cost, the accumulated edit cost (the g value) of the most recent match produced
> by apply_med (set by its match-printing step). Zero/stale before any match. No side effects; no
> NULL check on medh.

> [spec:foma:def:fomalib.apply-med-get-instring-fn]
> FEXPORT char *apply_med_get_instring(struct apply_med_handle *medh)

> [spec:foma:sem:fomalib.apply-med-get-instring-fn]
> Returns medh->instring: the aligned rendition of the caller's input word from the most recent
> apply_med match (unknown characters printed verbatim, positions where a symbol was inserted
> into the input shown as the align symbol if one is set). Internal buffer owned by the handle —
> caller must not free it; overwritten by the next match. No NULL check on medh.

> [spec:foma:def:fomalib.apply-med-get-outstring-fn]
> FEXPORT char *apply_med_get_outstring(struct apply_med_handle *medh)

> [spec:foma:sem:fomalib.apply-med-get-outstring-fn]
> Returns medh->outstring: the matched network-side (dictionary) word from the most recent
> apply_med match — the same pointer apply_med itself returned (IDENTITY arcs shown as "@",
> deletions shown as the align symbol if set). Internal buffer owned by the handle — caller must
> not free it; overwritten by the next match. No NULL check on medh.

> [spec:foma:def:fomalib.apply-med-init-fn]
> apply_med_handle *apply_med_init(struct fsm *net)

> [spec:foma:sem:fomalib.apply-med-init-fn]
> Creates an MED (minimum-edit-distance lookup) handle for `net`. calloc's the handle and sets:
> net (borrowed); agenda = malloc of 256 astarnodes with agenda[0].f = -1 and agenda_size = 256;
> heap = malloc of 256 ints with heap[0] = 0 as sentinel, heap_size = 256; astarcount = 1;
> heapcount = 0; state_array = map_firstlines(net) (array indexed by state number pointing at
> each state's first transition line). If net->medlookup and its confusion_matrix exist, sets
> hascm = 1 and cm to the matrix. maxsigma = sigma_max(net->sigma)+1. Builds sigmahash (sh_init),
> a string hash mapping every sigma symbol with number > IDENTITY (2) to its number, used to
> tokenize input words. Calls fsm_create_letter_lookup(medh, net): with maxdepth = 2, computes
> per-state bitvectors letterbits (the set of arc symbols occurring anywhere on paths from the
> state, computed with an iterative Tarjan-SCC DFS so cyclic nets work: all states of an SCC get
> the root's bits) and nletterbits (symbols reachable within maxdepth arcs), sized
> bytes_per_letter_array = BITNSLOTS(sigma_max+1) per state; these feed the A* heuristic.
> Allocates instring and outstring buffers of 256 bytes each. Defaults: med_limit = 4 (max
> matches per word), med_cutoff = 15 (max edit cost), med_max_heap_size = 262145 (agenda growth
> cap). Returns the handle; free with apply_med_clear. The net is not modified and must outlive
> the handle.

> [spec:foma:def:fomalib.apply-med-set-align-symbol-fn]
> FEXPORT void apply_med_set_align_symbol(struct apply_med_handle *medh, char *align)

> [spec:foma:sem:fomalib.apply-med-set-align-symbol-fn]
> Sets medh->align_symbol = strdup(align). When set, apply_med's result strings render
> insertion/deletion positions (EPSILON on one side of the alignment) as this symbol, producing
> aligned instring/outstring output; when unset, such positions print nothing. No-op if medh is
> NULL. A previously set symbol is not freed (leaks if called twice).

> [spec:foma:def:fomalib.apply-med-set-heap-max-fn]
> FEXPORT void apply_med_set_heap_max(struct apply_med_handle *medh, int max)

> [spec:foma:sem:fomalib.apply-med-set-heap-max-fn]
> Sets medh->med_max_heap_size = max: the cap on A* agenda growth (in nodes). apply_med's node
> insertion fails — aborting the search with NULL — when doubling the agenda size would reach
> this value. Default 262145. No-op if medh is NULL.

> [spec:foma:def:fomalib.apply-med-set-med-cutoff-fn]
> FEXPORT void apply_med_set_med_cutoff(struct apply_med_handle *medh, int max)

> [spec:foma:sem:fomalib.apply-med-set-med-cutoff-fn]
> Sets medh->med_cutoff = max: the maximum edit cost explored by apply_med. Candidate nodes with
> g+h above the cutoff are not enqueued, and the search stops (returns NULL) when the cheapest
> remaining node's f exceeds it. Default 15. No-op if medh is NULL.

> [spec:foma:def:fomalib.apply-med-set-med-limit-fn]
> FEXPORT void apply_med_set_med_limit(struct apply_med_handle *medh, int max)

> [spec:foma:sem:fomalib.apply-med-set-med-limit-fn]
> Sets medh->med_limit = max: the maximum number of matches apply_med will return for one word;
> once reached, resumed calls return NULL. Default 4. No-op if medh is NULL.

> [spec:foma:def:fomalib.apply-random-lower-fn]
> FEXPORT char *apply_random_lower(struct apply_handle *h)

> [spec:foma:sem:fomalib.apply-random-lower-fn]
> Returns one random word from the lower (output) side of the network. Clears all runtime flag
> values (apply_clear_flags), sets h->mode = DOWN|ENUMERATE|LOWER|RANDOM (16+2+32+1), and calls
> apply_enumerate: performs a random walk from the initial state — at each state one outgoing arc
> is chosen uniformly with rand() — accumulating lower-side symbols; on entering a final state a
> fair coin (rand() % 2 == 0) decides whether to stop and return the accumulated h->outstring. If
> the walk exhausts the search space the current outstring contents are returned as-is (quirk:
> its terminator stems from the last final-state visit, so the result may be stale/partial).
> Returns NULL only when h->last_net is NULL or the net has no final states. State marks are not
> used in RANDOM mode and the enumerator's iterator is not advanced, so each call is an
> independent walk; results may repeat. Returned buffer is internal — do not free.

> [spec:foma:def:fomalib.apply-random-upper-fn]
> FEXPORT char *apply_random_upper(struct apply_handle *h)

> [spec:foma:sem:fomalib.apply-random-upper-fn]
> Identical to apply_random_lower except the mode is DOWN|ENUMERATE|UPPER|RANDOM (16+2+64+1):
> clears runtime flag values, then performs one random walk (uniform arc choice, coin flip at
> final states to stop) printing the upper (input) side symbols. Returns the internal
> h->outstring, or NULL if the net is NULL or has no final states.

> [spec:foma:def:fomalib.apply-random-words-fn]
> FEXPORT char *apply_random_words(struct apply_handle *h)

> [spec:foma:sem:fomalib.apply-random-words-fn]
> Identical to apply_random_lower except the mode is DOWN|ENUMERATE|LOWER|UPPER|RANDOM
> (16+2+32+64+1): one random walk printing both sides of each arc — a single symbol when both
> sides are the same sigma symbol, otherwise "in<separator>out" (separator defaults to ":").
> Returns the internal h->outstring, or NULL if the net is NULL or has no final states.

> [spec:foma:def:fomalib.apply-reset-enumerator-fn]
> FEXPORT void apply_reset_enumerator(struct apply_handle *h)

> [spec:foma:sem:fomalib.apply-reset-enumerator-fn]
> Restarts word enumeration on the handle: zeroes marks[i] for every i in [0,
> h->last_net->statecount) and sets h->iterator = 0 and h->iterate_old = 0, so the next
> apply_words/apply_upper_words/apply_lower_words call starts a fresh DFS from the initial state.
> Does not clear the search stack (the next fresh enumerate call does that itself).

> [spec:foma:def:fomalib.apply-set-epsilon-fn]
> FEXPORT void apply_set_epsilon(struct apply_handle *h, char *symbol)

> [spec:foma:sem:fomalib.apply-set-epsilon-fn]
> Sets the string used to display EPSILON (symbol 0) in printed output. Frees the old
> h->epsilon_symbol, sets it to strdup(symbol), and updates h->sigs[EPSILON].symbol/.length to
> point at the new string. Default is "0" (set by apply_init). Requires h->sigs to exist
> (allocated by apply_init); crashes on a NULL handle or before init.

> [spec:foma:def:fomalib.apply-set-obey-flags-fn]
> FEXPORT void apply_set_obey_flags(struct apply_handle *h, int value)

> [spec:foma:sem:fomalib.apply-set-obey-flags-fn]
> Sets h->obey_flags = value. Nonzero (the default, 1): flag diacritics are checked for
> consistency during traversal and inconsistent paths are pruned. Zero: flag arcs are traversed
> unconditionally (consuming no input, like epsilon). No other side effects.

> [spec:foma:def:fomalib.apply-set-print-pairs-fn]
> FEXPORT void apply_set_print_pairs(struct apply_handle *h, int value)

> [spec:foma:sem:fomalib.apply-set-print-pairs-fn]
> Sets h->print_pairs = value. When nonzero, up/down application prints, for every arc whose two
> sides differ, the pair "<in<separator>out>" (with UNKNOWN replaced by the actual matched input
> character) instead of only the result-side symbol. Default 0. No other side effects.

> [spec:foma:def:fomalib.apply-set-print-space-fn]
> FEXPORT void apply_set_print_space(struct apply_handle *h, int value)

> [spec:foma:sem:fomalib.apply-set-print-space-fn]
> Sets h->print_space = value and unconditionally sets h->space_symbol = strdup(" ") — even when
> value is 0, and leaking any previously set space symbol. When print_space is nonzero, the space
> symbol is appended after every nonempty symbol printed to the output string.

> [spec:foma:def:fomalib.apply-set-separator-fn]
> FEXPORT void apply_set_separator(struct apply_handle *h, char *symbol)

> [spec:foma:sem:fomalib.apply-set-separator-fn]
> Sets h->separator = strdup(symbol): the string printed between the two sides of an arc when
> pair output is produced (word enumeration with differing sides, or print_pairs mode). Default
> ":" (set by apply_init). The old separator is not freed (leaks).

> [spec:foma:def:fomalib.apply-set-show-flags-fn]
> FEXPORT void apply_set_show_flags(struct apply_handle *h, int value)

> [spec:foma:sem:fomalib.apply-set-show-flags-fn]
> Sets h->show_flags = value. Nonzero: flag-diacritic symbols (e.g. "@U.F.V@") are included in
> printed output strings. Zero (the default): they are suppressed (printed as empty). Independent
> of obey_flags, which controls whether flags constrain paths.

> [spec:foma:def:fomalib.apply-set-space-symbol-fn]
> FEXPORT void apply_set_space_symbol(struct apply_handle *h, char *space)

> [spec:foma:sem:fomalib.apply-set-space-symbol-fn]
> Sets h->space_symbol = strdup(space) and turns h->print_space on (1), so the given string is
> appended after every nonempty printed symbol. A previously set space symbol is not freed
> (leaks).

> [spec:foma:def:fomalib.apply-up-fn]
> FEXPORT char *apply_up(struct apply_handle *h, char *word)

> [spec:foma:sem:fomalib.apply-up-fn]
> Mirror image of apply_down: `word` is matched against the lower/output side and the
> corresponding upper/input-side string is returned. Sets h->mode = UP (8); h->indexed = 1 iff
> h->index_out != NULL (built by apply_index with APPLY_INDEX_OUTPUT); h->binsearch = 1 iff
> h->last_net->arcs_sorted_out == 1; then delegates to apply_updown(h, word). Same protocol and
> ownership as apply_down: non-NULL word starts a lookup, NULL resumes for the next result, NULL
> return means exhausted; result is the internal h->outstring buffer (do not free); dereferences
> h->last_net before any NULL check.

> [spec:foma:def:fomalib.apply-upper-words-fn]
> FEXPORT char *apply_upper_words(struct apply_handle *h)

> [spec:foma:sem:fomalib.apply-upper-words-fn]
> Sets h->mode = DOWN|ENUMERATE|UPPER (16+2+64) and returns apply_enumerate(h): enumerates the
> words of the upper (input) side of the network, one accepted path per call, with the same
> iterator/resume semantics, return-buffer ownership, epsilon/flag suppression, and NULL-on-
> exhaustion behavior as apply_lower_words.

> [spec:foma:def:fomalib.apply-words-fn]
> FEXPORT char *apply_words(struct apply_handle *h)

> [spec:foma:sem:fomalib.apply-words-fn]
> Sets h->mode = DOWN|ENUMERATE|LOWER|UPPER (16+2+32+64) and returns apply_enumerate(h):
> enumerates whole paths of the network, one per call, printing both sides of each arc — a single
> symbol when both sides are the same sigma symbol, otherwise "in<separator>out" (separator
> defaults to ":"). Same iterator/resume semantics, buffer ownership, flag handling and
> NULL-on-exhaustion behavior as apply_lower_words.

> [spec:foma:def:fomalib.cmatrix-default-delete-fn]
> FEXPORT void cmatrix_default_delete(struct fsm *net, int cost)

> [spec:foma:sem:fomalib.cmatrix-default-delete-fn]
> Sets the default deletion cost in the net's confusion matrix: with maxsigma =
> sigma_max(net->sigma)+1, writes cm[i*maxsigma + 0] = cost for every i in [0, maxsigma) — column
> 0 (network symbol i realized as nothing). Note this includes the [0][0] (epsilon,epsilon) cell.
> Requires cmatrix_init to have been called (dereferences net->medlookup->confusion_matrix
> unchecked). Modifies the matrix in place; no return value.

> [spec:foma:def:fomalib.cmatrix-default-insert-fn]
> FEXPORT void cmatrix_default_insert(struct fsm *net, int cost)

> [spec:foma:sem:fomalib.cmatrix-default-insert-fn]
> Sets the default insertion cost in the net's confusion matrix: with maxsigma =
> sigma_max(net->sigma)+1, writes cm[j] = cost for every j in [0, maxsigma) — row 0 (nothing
> realized as symbol j). Note this includes the [0][0] cell. Requires cmatrix_init to have been
> called (dereferences net->medlookup->confusion_matrix unchecked). In place; no return value.

> [spec:foma:def:fomalib.cmatrix-default-substitute-fn]
> FEXPORT void cmatrix_default_substitute(struct fsm *net, int cost)

> [spec:foma:sem:fomalib.cmatrix-default-substitute-fn]
> Sets the default substitution cost in the net's confusion matrix: with maxsigma =
> sigma_max(net->sigma)+1, for all i,j in [1, maxsigma) writes cm[i*maxsigma+j] = 0 when i == j,
> else cost. Row 0 and column 0 (insertions/deletions) are left untouched; UNKNOWN (1) and
> IDENTITY (2) rows/columns are included. Requires cmatrix_init to have been called (dereferences
> net->medlookup->confusion_matrix unchecked). In place; no return value.

> [spec:foma:def:fomalib.cmatrix-init-fn]
> FEXPORT void cmatrix_init(struct fsm *net)

> [spec:foma:sem:fomalib.cmatrix-init-fn]
> Attaches a fresh confusion matrix to `net` for MED lookup. If net->medlookup is NULL, calloc's
> it. With maxsigma = sigma_max(net->sigma)+1, allocates cm = calloc(maxsigma*maxsigma ints),
> stores it in net->medlookup->confusion_matrix (any previous matrix pointer is overwritten and
> leaks), and initializes cm[i*maxsigma+j] = 0 when i == j, else 1 — i.e. unit
> insert/delete/substitute costs, zero for identity. The matrix is indexed [input-symbol ×
> output-symbol] by sigma number, row/column 0 being epsilon.

> [spec:foma:def:fomalib.cmatrix-print-att-fn]
> FEXPORT void cmatrix_print_att(struct fsm *net, FILE *outfile)

> [spec:foma:sem:fomalib.cmatrix-print-att-fn]
> Writes the net's confusion matrix to `outfile` in AT&T-transducer-like text form. With maxsigma
> = sigma_max(net->sigma)+1, iterates all ordered pairs (i,j) in [0,maxsigma)^2, skipping any
> pair where either index is 1 or 2 (UNKNOWN/IDENTITY; condition `(i!=0 && i<3) || (j!=0 &&
> j<3)`), and skipping (0,0): prints one line "0\t0\tIN\tOUT\tCOST\n" per pair, where IN/OUT are
> sigma_string() of the number, with 0 rendered as "@0@", and COST = cm[i*maxsigma+j]. Row i=0
> lines are insertions, column j=0 lines deletions. Ends with the final-state line "0\n".
> Requires the confusion matrix to exist (dereferenced unchecked).

> [spec:foma:def:fomalib.cmatrix-print-fn]
> FEXPORT void cmatrix_print(struct fsm *net)

> [spec:foma:sem:fomalib.cmatrix-print-fn+1]
> Pretty-prints the net's confusion matrix to stdout as a table. lsymbol = length of the longest
> sigma symbol with number >= 3. Header row: lsymbol+2 spaces, then "0 ", then each symbol from
> number 3 upward (stopping when sigma_string returns NULL), each followed by a space. Data rows:
> i = 0, then 3..maxsigma-1 (after printing row 0, i is incremented twice so rows 1 and 2 are
> skipped). Each row starts with the row label right-aligned in lsymbol+1 columns ("0" for row 0)
> followed by the deletion cost cm[i*maxsigma] right-aligned in 2 columns ("*" for row 0), then
> skips columns 1 and 2. Remaining cells for column j: on the diagonal (i == j) "*" right-aligned
> to width strlen(symbol j)+1 (the C source used that count as a "%.*s" precision, which truncated
> "*" to one char and misaligned the row); off-diagonal the cost cm[i*maxsigma+j] printed with
> "%.*d" using precision strlen(symbol j)+1 — i.e. zero-padded to the width of the header symbol
> plus one, so e.g. cost 1 under symbol "a" prints "01". Rendering is factored into a
> writer-generic `cmatrix_print_to` (cmatrix_print writes to stdout). Requires the confusion
> matrix and sigma to exist (dereferenced unchecked).

> [spec:foma:def:fomalib.cmatrix-set-cost-fn]
> FEXPORT void cmatrix_set_cost(struct fsm *net, char *in, char *out, int cost)

> [spec:foma:sem:fomalib.cmatrix-set-cost-fn]
> Sets a single cell of the net's confusion matrix. i = 0 if `in` is NULL (epsilon), else
> sigma_find(in, net->sigma); o likewise for `out`. If either lookup returned -1, prints
> "Warning, symbol '%s' not in alphabet\n" to stdout and returns without modifying anything.
> Otherwise cm[i*maxsigma+o] = cost, where maxsigma = sigma_max(net->sigma)+1. NULL `in` sets an
> insertion cost, NULL `out` a deletion cost. Requires cmatrix_init to have been called
> (dereferences the matrix unchecked).

> [spec:foma:def:fomalib.defined-functions]
> struct defined_functions {
>   char *name;
>   char *regex;
>   int numargs;
>   struct defined_functions *next;
> }

> [spec:foma:def:fomalib.defined-functions-init-fn]
> defined_functions *defined_functions_init(void)

> [spec:foma:sem:fomalib.defined-functions-init-fn]
> Allocates (calloc) and returns a single zeroed defined_functions node (name = regex = NULL,
> numargs = 0, next = NULL) that serves as a permanent dummy head: add/find operations fill this
> node first or splice new nodes after it, so the returned pointer remains valid for the list's
> lifetime. Caller owns the node.

> [spec:foma:def:fomalib.defined-networks]
> struct defined_networks {
>   char *name;
>   struct fsm *net;
>   struct defined_networks *next;
> }

> [spec:foma:def:fomalib.defined-networks-init-fn]
> defined_networks *defined_networks_init(void)

> [spec:foma:sem:fomalib.defined-networks-init-fn]
> Allocates (calloc) and returns a single zeroed defined_networks node (name = NULL, net = NULL,
> next = NULL) serving as a permanent dummy head, so the head pointer handed to callers never
> changes as definitions are added or removed. Caller owns the node.

> [spec:foma:def:fomalib.defined-quantifiers]
> struct defined_quantifiers {
>   char *name;
>   struct defined_quantifiers *next;
> }

> [spec:foma:def:fomalib.file-to-mem-fn]
> FEXPORT char *file_to_mem(char *name)

> [spec:foma:sem:fomalib.file-to-mem-fn]
> Reads the entire file `name` into a fresh NUL-terminated buffer. Steps: fopen(name, "r") — on
> failure prints "Error opening file '%s'\n" to stdout and returns NULL. Size = fseek to end +
> ftell, then rewind. malloc(size+1) — on failure prints "Error reading file '%s'\n" and returns
> NULL (file handle leaks). fread of exactly `size` bytes — on short read prints the same error
> and returns NULL (buffer and handle leak). Then checks the buffer start against known BOM
> byte sequences (UTF-8 EF BB BF, UTF-32LE/BE, UTF-16LE/BE): if one matches, prints "<encoding>
> BOM mark is detected in file '%s'.\n" and returns NULL (buffer and handle leak; note the check
> runs before NUL-termination and may compare against uninitialized bytes for files shorter than
> 4 bytes). Otherwise closes the file, writes '\0' at buffer[size], and returns the buffer, which
> the caller owns and must free.

> [spec:foma:def:fomalib.find-defined-fn]
> struct fsm *find_defined(struct defined_networks *def, char *string)

> [spec:foma:sem:fomalib.find-defined-fn]
> Linear scan of the defined-networks list starting at `def`: returns the `net` field of the
> first entry whose name is non-NULL and strcmp-equal to `string`, or NULL if no entry matches
> (or def is NULL). The returned fsm is borrowed from the list — the caller must not destroy it
> (use fsm_copy to take an independent copy).

> [spec:foma:def:fomalib.find-defined-function-fn]
> char *find_defined_function(struct defined_functions *deff, char *name, int numargs)

> [spec:foma:sem:fomalib.find-defined-function-fn]
> Linear scan of the defined-functions list starting at `deff`: returns the `regex` field of the
> first entry whose name is non-NULL, strcmp-equal to `name`, and whose numargs equals `numargs`;
> NULL if none matches. The returned string is borrowed from the list — do not free.

> [spec:foma:def:fomalib.flag-build-fn]
> FEXPORT int flag_build(int ftype, char *fname, char *fvalue, int fftype, char *ffname, char *ffvalue)

> [spec:foma:sem:fomalib.flag-build-fn]
> Pure decision table used when building flag-elimination filters: given the flag occurrence f =
> (ftype, fname, fvalue) being eliminated and another flag occurrence ff = (fftype, ffname,
> ffvalue) that may precede it on a path, decides whether ff makes f succeed or fail. Returns 1
> (FAIL), 2 (SUCCEED) or 3 (NONE/irrelevant) — these constants are private to flags.c, not the
> public FLAG_* bits. If strcmp(fname, ffname) != 0 → NONE. A NULL fvalue is treated as "" and
> marks the flag "selfnull" (valueless, e.g. @R.A@); a NULL ffvalue is treated as "". Let eq mean
> fvalue strcmp-equals ffvalue. Rules (first match wins):
> U flag: vs P eq → SUCCEED; vs C → SUCCEED; vs U not-eq → FAIL; vs P not-eq → FAIL; vs N eq →
> FAIL.
> R valueless: vs U, P or N → SUCCEED; vs C → FAIL.
> R with value: vs P eq or U eq → SUCCEED; vs P not-eq, U not-eq, N (any), or C → FAIL.
> D valueless: vs C → SUCCEED; vs P, U or N → FAIL.
> D with value: vs P not-eq → SUCCEED; vs C → SUCCEED; vs N eq → SUCCEED; vs P eq → FAIL; vs U eq
> → FAIL; vs N not-eq → FAIL.
> Anything else → NONE (notably D-with-value vs U-not-eq, and every combination where ftype is C,
> N, P or E).

> [spec:foma:def:fomalib.flag-eliminate-fn]
> fsm *flag_eliminate(struct fsm *net, char *name)

> [spec:foma:sem:fomalib.flag-eliminate-fn+1]
> Eliminates the flag diacritic with attribute `name` (or ALL flags when name == NULL) from `net`
> while preserving the flag semantics, by composing filter automata on both sides and then
> replacing flag arcs with EPSILON. Steps: (1) If net->pathcount == 0 (the stored field, not
> recomputed), return net unchanged (stderr note if g_verbose). (2) Extract the list of all flag
> symbols in sigma as (type, attribute, value) triples. (3) If name != NULL and no extracted flag
> has that attribute, return net unchanged (verbose note). (4) For each extracted flag f whose
> attribute matches (or all if name == NULL) — the type restriction is written `f->type &
> (FLAG_UNIFY|FLAG_REQUIRE|FLAG_DISALLOW|FLAG_EQUAL)` — the C used bitwise OR, which is
> always true, so filters were also attempted for C/P/N flags; `&` restricts the body to U/R/D/E as
> intended and changes nothing observable (flag_build classifies pairs only for U/R/D f):
> build succeed_flags and
> fail_flags as minimized unions of the single-symbol FSMs of every extracted flag ff for which
> flag_build(f, ff) returns SUCCEED resp. FAIL (flag symbol strings are reconstructed as
> "@T.name@" or "@T.name.value@"), plus self = the single-symbol FSM of f. If at least one ff
> contributed, build newfilter: for f of type FLAG_REQUIRE, newfilter = fsm_complement(
> fsm_concat( fsm_optionality(fsm_concat(fsm_universal(), fail_flags)),
> fsm_concat(fsm_complement(fsm_contains(succeed_flags)), fsm_concat(self, fsm_universal())))) —
> i.e. ~[(?* FAIL)^<2 ~$SUCCEED SELF ?*]; for all other types newfilter =
> fsm_complement(fsm_contains(fsm_concat(fail_flags,
> fsm_concat(fsm_complement(fsm_contains(succeed_flags)), self)))) — i.e. ~$[FAIL ~$SUCCEED
> SELF]. The overall filter is the fsm_intersect of all newfilters. (5) If a filter was built:
> with the global g_flag_is_epsilon temporarily forced to 0 (restored afterwards), newnet =
> fsm_compose(fsm_copy(filter), fsm_compose(net, fsm_copy(filter))) — applied on both tapes
> because the net may be a transducer; else newnet = net. (6) flag_purge(newnet, name): every arc
> label that is a matching flag symbol is rewritten to EPSILON (both in and out checked
> independently), the symbols are removed from sigma, and
> is_deterministic/is_minimized/is_epsilon_free are set NO. (7) newnet = fsm_minimize(newnet);
> sigma_cleanup(newnet, 0); sigma_sort(newnet); free(flags) (frees only the list head — the other
> nodes and their name/value strings leak, as does the filter fsm). Returns fsm_topsort(newnet).
> Ownership: consumes `net` except when returned unchanged at steps 1/3.

> [spec:foma:def:fomalib.flag-twosided-fn]
> fsm *flag_twosided(struct fsm *net)

> [spec:foma:sem:fomalib.flag-twosided-fn]
> Enforces two-sided flag diacritics: every arc carrying a flag ends up with identical flag
> labels on both tapes. Pass 1 (in place): build isflag[] over sigma numbers with flag_check;
> track maxstate; for each real arc (target != -1): if `in` is a flag and out == EPSILON, set out
> = in (record change); else if `out` is a flag and in == EPSILON, set in = out. Count newarcs =
> arcs where a flag appears on either side and in != out after this. If newarcs == 0: when a
> change was made, set is_deterministic/is_minimized/is_pruned = UNK and return
> fsm_topsort(fsm_minimize(net)); otherwise return net untouched. Pass 2: realloc net->states to
> i+newarcs lines (quirk: sized with sizeof(struct fsm) instead of sizeof(struct fsm_state) —
> harmless over-allocation), then split each offending arc through a fresh state (numbered
> maxstate+1 upward, one per split; new lines appended with add_fsm_arc, final_state = 0,
> start_state = 0): flag-in/plain-out → original arc becomes in:in targeting the new state, new
> arc EPSILON:out to the old target; plain-in/flag-out → original becomes in:EPSILON to the new
> state, new arc out:out to the old target; both-flags (differing) → original becomes in:in to
> the new state, new arc out:out to the old target. Append the -1 sentinel line, set
> is_deterministic/is_minimized = UNK, and return fsm_topsort(fsm_minimize(net)). Consumes/
> modifies `net`; the returned pointer supersedes it.

> [spec:foma:def:fomalib.foma-net-print-fn]
> FEXPORT int foma_net_print(struct fsm *net, gzFile outfile)

> [spec:foma:sem:fomalib.foma-net-print-fn+1]
> Serializes `net` to the already-open sink `outfile` in the textual foma binary format. Layout,
> in order: header line "##foma-net 1.0##\n"; "##props##\n"
> followed by one line of space-separated properties: arity arccount statecount linecount
> finalcount pathcount(%lld) is_deterministic is_pruned is_minimized is_epsilon_free is_loop_free
> extras name, where extras = is_completed | (arcs_sorted_in << 2) | (arcs_sorted_out << 4);
> "##sigma##\n" followed by "number symbol\n" for every sigma entry with number != -1, in list
> order; "##states##\n" followed by one line per transition line in array order, compressed: on a
> new state number the line has 5 fields "state in out target final" if in != out, else 4 fields
> "state in target final"; on a repeated state number 3 fields "in out target" if in != out, else
> 2 fields "in target"; then the sentinel line "-1 -1 -1 -1 -1\n". If net->medlookup and its
> confusion_matrix exist: "##cmatrix##\n" followed by maxsigma*maxsigma lines (maxsigma =
> sigma_max+1), one int per line, row-major. Finally "##end##\n". Returns `Ok(())` on success and
> propagates the first write failure as its `io::Error` (the C returned a vestigial `1` always);
> does not close the file and does not modify the net.

> [spec:foma:def:fomalib.foma-write-prolog-fn]
> FEXPORT int foma_write_prolog(struct fsm *net, char *filename)

> [spec:foma:sem:fomalib.foma-write-prolog-fn]
> Writes `net` as Prolog facts. Output goes to stdout when filename == NULL or fopen(filename,
> "w") fails (then prints an error note and falls back); otherwise to the file, announcing
> "Writing prolog to file '%s'.\n" on stdout. Calls fsm_count(net) first (refreshes counts).
> identifier = net->name (copied into a 100-byte buffer with strcpy). Emits, in order: (1)
> "network(ID).\n". (2) One pass over the state array recording finals[state] from each line's
> final_state and marking used_symbols for every in/out label != -1. (3) For every sigma number i
> from 3 to sigma_max that is NOT used on any arc: `symbol(ID, "SYM").\n` — declaring
> otherwise-lost alphabet symbols; a symbol whose text is "0" is written as "%0"; embedded `"`
> are escaped as `\"`. (4) For every line with target != -1: `arc(ID, source, target, LABEL).\n`
> where in/out strings are: number 0 → "0", 1 or 2 → "?", else the sigma text; then literal text
> "0" (for a non-0 number) becomes "%0" and literal text "?" (for a number > 2) becomes "%?" —
> note the out-side "?" escape erroneously tests stateptr->in instead of ->out (latent bug).
> LABEL forms for arity 2: in == out == IDENTITY → `"?"`; in == out and in != UNKNOWN → `"SYM"`;
> otherwise `"IN":"OUT"` (so an UNKNOWN:UNKNOWN arc prints `"?":"?"`, meaning unequal unknowns).
> Arity 1: `"IN"`. All symbol texts printed with `"` escaping. (5) `final(ID, state).\n` for
> every final state in numeric order. Closes the file (if not stdout), frees temporaries, returns
> 1 always.

> [spec:foma:def:fomalib.fsm]
> struct fsm {
>   char name[FSM_NAME_LEN];
>   int arity;
>   int arccount;
>   int statecount;
>   int linecount;
>   int finalcount;
>   long long pathcount;
>   int is_deterministic;
>   int is_pruned;
>   int is_minimized;
>   int is_epsilon_free;
>   int is_loop_free;
>   int is_completed;
>   int arcs_sorted_in;
>   int arcs_sorted_out;
>   struct fsm_state *states;
>   struct sigma *sigma;
>   struct medlookup *medlookup;
> }

> [spec:foma:def:fomalib.fsm-add-loop-fn]
> fsm *fsm_add_loop(struct fsm *net, struct fsm *marker, int finals)

> [spec:foma:sem:fomalib.fsm-add-loop-fn]
> Returns a copy of `net` with the arcs of the (typically single-state) `marker` transducer added
> as self-loops at selected states: finals == 1 → at every final state; finals == 0 → at every
> non-final state; finals == 2 → at every state. Implementation: open read handles on net and
> marker, open a construction handle named net->name, copy net's sigma, and copy every arc of net
> by symbol number. Then for each selected state i, reset the marker read handle and add, for
> every arc of marker, an arc (i, i, in-symbol, out-symbol) BY SYMBOL STRING — so marker symbols
> absent from net's sigma are added to the result's sigma. (For finals == 1 the selected states
> are produced by iterating net's finals; for 0/2 by looping i over 0..statecount-1 and testing
> finality.) All of net's final states are then marked final, the initial state is 0, and the
> construction is finished (which recomputes counts and sorts). Frees both read handles, destroys
> `net` (consumed), and returns the new fsm. `marker` is NOT destroyed — caller keeps ownership.

> [spec:foma:def:fomalib.fsm-add-sink-fn]
> fsm *fsm_add_sink(struct fsm *net, int final)

> [spec:foma:sem:fomalib.fsm-add-sink-fn]
> Completes `net` with an explicit sink state: returns a copy where every state has an outgoing
> arc for every sigma symbol number in [2, sigma_max] (IDENTITY and all real symbols; EPSILON 0
> and UNKNOWN 1 excluded), missing ones going to a new sink state numbered
> statecount-of-original. Implementation: read handle on net; construction handle named
> net->name; copy sigma; sigmatable = maxsigma ints initialized to -1. For each state (in state
> order): copy each of its arcs by symbol numbers and record sigmatable[arc-in-symbol] =
> current-state; then for i in [2, maxsigma): if sigmatable[i] != current-state, add arc
> (state, sink, i, i). (The table needs no per-state reset because it stores the state number;
> note only INPUT-side symbols count as present.) Then add self-loops (sink, sink, i, i) for all
> i in [2, maxsigma). Original final states are preserved; the sink is made final iff `final` ==
> 1. Initial state is 0. Finishes construction, destroys `net` (consumed), returns the new fsm.

> [spec:foma:def:fomalib.fsm-bimachine-fn]
> fsm *fsm_bimachine(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-bimachine-fn]
> Unimplemented placeholder: prints "implementation pending\n" (exactly that lowercase text) to
> stdout and returns `net` unchanged. The argument is not destroyed and no fields are modified.

> [spec:foma:def:fomalib.fsm-boolean-fn]
> fsm *fsm_boolean(int value)

> [spec:foma:sem:fomalib.fsm-boolean-fn]
> Maps a truth value to a network: returns fsm_empty_set() when `value` == 0 (the empty language),
> otherwise fsm_empty_string() (the language containing only the empty string). Allocates a fresh
> network owned by the caller; no global state.

> [spec:foma:def:fomalib.fsm-clear-contexts-fn]
> FEXPORT void fsm_clear_contexts(struct fsmcontexts *contexts)

> [spec:foma:sem:fomalib.fsm-clear-contexts-fn]
> Destroys a linked list of context pairs. For each node starting at `contexts`: calls fsm_destroy
> on the four member networks left, right, cpleft and cpright (fsm_destroy is NULL-safe), saves the
> next pointer, frees the node, continues. NULL input is a no-op. The caller's head pointer is left
> dangling (not cleared).

> [spec:foma:def:fomalib.fsm-close-sigma-fn]
> fsm *fsm_close_sigma(struct fsm *net, int mode)

> [spec:foma:sem:fomalib.fsm-close-sigma-fn]
> Removes transitions carrying wildcard symbols. Opens a read handle on `net` and a construction
> handle named net->name whose alphabet is copied verbatim from net->sigma. Every arc of net is
> re-added by symbol numbers (same source/target/in/out) iff: mode == 0 and none of in/out equals
> UNKNOWN (1) or IDENTITY (2); or mode == 1 and neither in nor out equals UNKNOWN (IDENTITY arcs are
> kept). All final and initial state markings are copied. Finishes the construction, destroys `net`
> (consumed), returns fsm_minimize of the new network. Because sigma is copied wholesale, the
> wildcard symbols may remain listed in the alphabet even when all their arcs were dropped.

> [spec:foma:def:fomalib.fsm-coaccessible-fn]
> fsm *fsm_coaccessible(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-coaccessible-fn+2]
> Prunes every state from which no final state is reachable; works in place on net->states and
> returns the same net. Steps:
> 1. Build an inverse-transition table: for each line with source s, target t, t != -1 and t != s,
> prepend s to t's inverse list (heads stored inline in a statecount-sized array initialized to
> state -1, extra entries as malloc'd nodes).
> 2. Mark coaccessible states: push each final state once onto the int stack, marking it (markcount
> counts marks); then repeatedly pop a state and mark/push every unmarked state in its inverse list.
> If markcount reaches statecount, the net is already pruned: clear the stack and skip step 3.
> 3. Otherwise, if the incoming coacc array is empty (statecount 0 — an already-empty machine) or the
> start state (0) is itself not coaccessible, then no path reaches a final, so L = ∅: replace the
> state array with fsm_empty() and produce the canonical empty machine in the well-formed
> fsm_empty_set shape (one non-final start state, statecount 1, linecount 2, arccount 0), destroy
> sigma and replace it with a fresh empty sigma, set is_pruned, and return. The empty-coacc guard
> makes the function idempotent (re-pruning an already-empty machine returns here instead of
> indexing coacc[0] out of bounds). This subsumes the markcount == 0 (no finals) case, whose start is also not coaccessible.
> The C source instead set mapping[0] = 0 unconditionally and renumbered the surviving (orphaned)
> coaccessible component from 1, producing a net with states but no start state; a disconnected
> component can never make L non-empty when the start is pruned, so the empty machine is correct.
> 4. Otherwise (state 0 coaccessible) renumber and rewrite in place. mapping[0] = 0; every other
> coaccessible state gets the next number 1,2,... in ascending old order. Iterate the old lines: a
> line is kept iff its state is marked and its target is -1 or marked, with state/target renumbered
> and in/out/final/start preserved; whenever a state's lines end with none kept and the state was
> final, a final sentinel line (mapped state, in/out/target -1, final 1, original start flag) is
> emitted instead (tracked with an `added` array; the same fixup runs once after the loop for the
> last state). If nothing at all was kept, write a line (0,-1,-1,-1,-1,-1). Append the -1 sentinel;
> the array is not shrunk. Update linecount, arccount, and statecount (= markcount).
> Frees all temporaries, sets is_pruned = YES, returns net.

> [spec:foma:def:fomalib.fsm-compact-fn]
> FEXPORT void fsm_compact(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-compact-fn]
> Alphabet compaction, in place (void): removes from sigma every ordinary symbol (number > 2) whose
> transition distribution is identical to IDENTITY's, since @/? already cover it.
> A per-symbol `potential` flag starts at 1; symbols whose sigma string is longer than one UTF-8
> character are cleared up front (@ and ? only match single characters). One pass over the line
> array keeps checktable[sym] = (state,target) of the last arc seen with in == out == sym (sym > 2)
> or with in == IDENTITY. At every state boundary (and once at the sentinel), for each sym >= 3: if
> neither sym nor IDENTITY was recorded at the finished state, sym is unaffected; otherwise
> potential[sym] is cleared unless sym's recorded (state,target) equals IDENTITY's entry exactly.
> Any arc with in != out clears the potential of whichever of in/out is > 2. (Only the last arc per
> symbol per state is compared, so the analysis presumes a deterministic machine.)
> If no symbol >= 3 survives, temporaries are freed and the net is untouched. Otherwise the line
> array is compacted in place, dropping every line whose in symbol is > 2 with potential still set
> (lines with in == -1 are kept) and re-appending the sentinel; the removable sigma nodes are
> unlinked and freed (sigprev would be NULL if the very first sigma entry were removable — safe only
> because special symbols 0-2 sort first); finally sigma_cleanup(net,0) renumbers the remaining
> symbols. net->linecount/arccount are left stale (not recomputed).

> [spec:foma:def:fomalib.fsm-complement-fn]
> fsm *fsm_complement(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-complement-fn]
> Complements an acceptor via the shared routine fsm_completes(net, COMPLEMENT); in place, returns
> net. Setup: minimize net if not already minimized; remove UNKNOWN from sigma if present; if
> IDENTITY is absent add it and remember the machine was incomplete; sigsize = number of sigma
> entries, minus one if EPSILON is present; last_sigma = sigma_max; fsm_count.
> One scan over all lines toggles every final_state bit in place (this is the complementation),
> records per-state start/final flags, counts real arcs, and computes sink candidacy: a state is a
> candidate iff it has no arc to a different state and its toggled final flag is 1. Sets
> is_loop_free = NO and pathcount = PATHCOUNT_CYCLIC.
> If IDENTITY was already present and arccount == sigsize*statecount, the machine was already
> complete: set is_completed = YES, is_minimized = YES, is_pruned = NO, is_deterministic = YES and
> return (finals stay toggled). Otherwise use the lowest-numbered sink candidate, or append a new
> state numbered statecount (non-start, final because complementing) and bump statecount.
> Build a dense table target[state][in-symbol] (row stride sigsize+2) from the existing arcs indexed
> by input symbol; give the sink a self-loop on every symbol in [2..last_sigma]; route every missing
> (state,symbol) entry to the sink. Emit a fresh line array containing, for every state i and every
> symbol j in [2..last_sigma], the arc (i, j, j, target, final[i], start[i]) — output forced equal
> to input, so the operation assumes arity 1 — plus the -1 sentinel, replacing net->states (old
> freed). Set is_minimized = NO, is_pruned = NO, is_completed = YES and the new statecount.
> Complement is relative to Sigma* with IDENTITY covering unlisted symbols; correct only for
> epsilon-free deterministic acceptors.

> [spec:foma:def:fomalib.fsm-complete-fn]
> fsm *fsm_complete(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-complete-fn]
> fsm_completes(net, COMPLETE): the identical completion algorithm described under
> `[spec:foma:sem:fomalib.fsm-complement-fn]`, except final flags are NOT toggled and the sink is
> non-final: a candidate is an existing non-final state with no arcs to other states, else a new
> non-final state is appended. Result accepts the same language but has, for every state, an
> outgoing arc for every sigma symbol >= 2 (IDENTITY added to sigma if missing, UNKNOWN removed).
> In place; returns net with is_completed = YES.

> [spec:foma:def:fomalib.fsm-compose-fn]
> fsm *fsm_compose(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:fomalib.fsm-compose-fn]
> Composes two transducers (net1 upper, net2 lower). Both are minimized first; if either is empty,
> both are destroyed and fsm_empty_set() returned.
> If the global g_flag_is_epsilon is on, each net's flag-diacritic symbols are first added to the
> other net's sigma (so ?/@ never match flags), a per-symbol is_flag table is built after merging,
> and a warning is printed if both nets contain flags. Sigmas are then merged/harmonized with
> fsm_merge_sigma(net1, net2).
> Result states are triples (a,b,mode) allocated on demand via a triplet hash, seeded with (0,0,0)
> as state 0; work items live on the int stack. A result state is a start state iff both a and b are
> start states and mode == 0; final iff both a and b are final (any mode).
> For each popped (a,b,mode), b's real arcs are indexed by input symbol into per-symbol lists valid
> only for this iteration (mainloop counter); IDENTITY is indexed under UNKNOWN so both are found
> together. Then three arc groups are emitted:
> 1. Pair match: for each arc of a with output aout (searched under UNKNOWN when IDENTITY) and each
> indexed arc of b with input bin: if aout == IDENTITY and bin == UNKNOWN the emitted pair becomes
> UNKNOWN:UNKNOWN; if aout == UNKNOWN and bin == IDENTITY, bin/bout become UNKNOWN; then if
> bin == aout and (bin != EPSILON or mode == 0), emit arc ain:bout to state (a_target,b_target,0).
> (An upper x:0 arc can thus combine with a lower 0:y arc only in mode 0.)
> 2. Upper epsilon outputs (aout == EPSILON): bistate variant (g_compose_tristate == 0): only in
> mode 0, emit ain:EPSILON to (a_target,b,0). Tristate: unless mode == 2, emit ain:EPSILON to
> (a_target,b,1). With flag-is-epsilon, an a-arc whose output symbol is a flag is instead passed
> through unchanged as ain:aout to (a_target,b,0), only from mode 0.
> 3. Lower epsilon inputs (bin == EPSILON): bistate: always allowed, emit EPSILON:bout to
> (a,b_target,1). Tristate: unless mode == 1, emit EPSILON:bout to (a,b_target,2). With
> flag-is-epsilon, a b-arc whose input is a flag is passed through as bin:bout to (a,b_target,1),
> from any mode.
> The new machine replaces net1's states; net2 is destroyed; net1 keeps the merged sigma. Returns
> fsm_coaccessible(fsm_topsort(fsm_coaccessible(net1))). Both inputs are consumed.

> [spec:foma:def:fomalib.fsm-concat-fn]
> fsm *fsm_concat(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:fomalib.fsm-concat-fn]
> Concatenation L(net1)·L(net2). First fsm_merge_sigma(net1,net2) and fsm_count on both. If either
> net has no final state, both are destroyed and fsm_empty_set() is returned.
> net2's state numbers and targets are shifted up by net1->statecount (fsm_add_to_states). A new
> line array of net1->linecount + net2->linecount + net1->finalcount + 2 entries is filled: net1's
> lines are copied in order, but before the first line of each final state an epsilon arc
> (state, EPSILON, EPSILON, target = net1->statecount — net2's shifted initial state 0 — final 0,
> original start flag) is inserted; every copied net1 line gets final forced to 0, and pure final
> sentinel lines (target == -1 && final == 1) are dropped. Then all net2 lines are copied with
> start_state forced to 0 (finals preserved), and the -1 sentinel appended.
> net1's old array is freed and replaced; net2 is destroyed (both inputs consumed). EPSILON is added
> to the sigma if absent; fsm_count; is_epsilon_free/is_deterministic/is_minimized/is_pruned = NO.
> Returns fsm_minimize(net1). Relies on the invariant that a network's initial state is state 0.

> [spec:foma:def:fomalib.fsm-concat-m-n-fn]
> fsm *fsm_concat_m_n(struct fsm *net1, int m, int n)

> [spec:foma:sem:fomalib.fsm-concat-m-n-fn]
> Bounded iteration net1{m,n}. acc starts as fsm_empty_string(); for i = 1..n,
> acc = fsm_concat(acc, X) where X = fsm_copy(net1) when i <= m (mandatory copy) and
> fsm_optionality(fsm_copy(net1)) when i > m. net1 is then destroyed and acc returned. n < 1 yields
> the empty-string language regardless of m; m >= n makes all n copies mandatory. The result is
> minimized (each fsm_concat minimizes).

> [spec:foma:def:fomalib.fsm-concat-n-fn]
> fsm *fsm_concat_n(struct fsm *net1, int n)

> [spec:foma:sem:fomalib.fsm-concat-n-fn]
> net1^n: simply fsm_concat_m_n(net1, n, n) — exactly n mandatory concatenated copies. n < 1 yields
> the empty-string language. net1 is consumed.

> [spec:foma:def:fomalib.fsm-construct-add-arc-fn]
> FEXPORT void fsm_construct_add_arc(struct fsm_construct_handle *handle, int source, int target, char *in, char *out)

> [spec:foma:sem:fomalib.fsm-construct-add-arc-fn]
> Adds one arc with string labels to a construction in progress. Grows the state list to cover
> `source` and `target` (fsm_construct_check_size: realloc to the smallest power of two > state_no,
> zero-initializing the new slots), raises handle->maxstate to max(source,target), and marks both
> states used. A new transition node is prepended (LIFO) to source's transition list. Each label is
> resolved with fsm_construct_check_symbol; on miss (-1) it is added via fsm_construct_add_symbol.
> The node stores the resolved in/out numbers and the target. No duplicate detection here (exact
> duplicates are dropped later by the state builder in fsm_construct_done).

> [spec:foma:def:fomalib.fsm-construct-add-arc-nums-fn]
> FEXPORT void fsm_construct_add_arc_nums(struct fsm_construct_handle *handle, int source, int target, int in, int out)

> [spec:foma:sem:fomalib.fsm-construct-add-arc-nums-fn]
> Same as fomalib.fsm-construct-add-arc-fn but the in/out symbol numbers are supplied directly and
> stored verbatim: grows the state list to cover source and target, updates maxstate, marks both
> states used, prepends a transition node (in, out, target) to source's list. No sigma lookup or
> insertion happens — the caller must guarantee the numbers have symbol entries (e.g. via
> fsm_construct_copy_sigma), otherwise the number silently disappears from the final sigma.

> [spec:foma:def:fomalib.fsm-construct-add-symbol-fn]
> FEXPORT int fsm_construct_add_symbol(struct fsm_construct_handle *handle, char *symbol)

> [spec:foma:sem:fomalib.fsm-construct-add-symbol-fn]
> Unconditionally adds `symbol` to the construction's alphabet and returns its assigned number. If
> the string equals a reserved name (@_EPSILON_SYMBOL_@, @_UNKNOWN_SYMBOL_@, @_IDENTITY_SYMBOL_@) it
> gets the fixed number 0/1/2 respectively, raising handle->maxsigma to that number if lower.
> Otherwise it gets maxsigma+1, but never below 3 (MINSIGMA), and maxsigma is updated. If the number
> does not fit, fsm_sigma_list is realloc'd to the next power of two above its current size — the
> grown region is NOT zero-initialized, so gap slots may later feed garbage to the sigma conversion.
> The symbol is strdup'd into slot [number] and (symbol,number) is inserted into the chained hash
> table (hash = sum of the string's char values mod 1021; chars are signed, so non-ASCII bytes hash
> platform-dependently). Never checks for duplicates: calling it for an existing symbol allocates a
> fresh number — callers must probe with fsm_construct_check_symbol first.

> [spec:foma:def:fomalib.fsm-construct-check-symbol-fn]
> FEXPORT int fsm_construct_check_symbol(struct fsm_construct_handle *handle, char *symbol)

> [spec:foma:sem:fomalib.fsm-construct-check-symbol-fn]
> Looks up `symbol` in the construction handle's hash table (hash = byte-sum of the string mod
> 1021). If the head bucket is empty, returns -1; otherwise walks the chain comparing with strcmp
> and returns the stored symbol number on match, or -1 if the chain is exhausted.

> [spec:foma:def:fomalib.fsm-construct-copy-sigma-fn]
> FEXPORT void fsm_construct_copy_sigma(struct fsm_construct_handle *handle, struct sigma *sigma)

> [spec:foma:sem:fomalib.fsm-construct-copy-sigma-fn+1]
> Copies an existing sigma into a construction handle. For each sigma node (stopping at NULL or a
> node with number -1): raises handle->maxsigma to the node's number; if the number >=
> fsm_sigma_list_size, reallocs the list, growing (next_power_of_two) repeatedly until the number
> fits. The C source grew only one doubling per symbol, so a number more than twice the size
> overflowed the array (latent, unreachable while sizes start at 1024); the loop guarantees the
> slot fits. The grown region is not zeroed. Strdups the symbol string into slot [number] and
> inserts (symbol,number) into the chained hash table. Symbol numbers are preserved exactly,
> including the reserved 0/1/2.

> [spec:foma:def:fomalib.fsm-construct-done-fn]
> fsm *fsm_construct_done(struct fsm_construct_handle *handle)

> [spec:foma:sem:fomalib.fsm-construct-done-fn+1]
> Finalizes a construction handle into a struct fsm. If maxstate == -1, or no final state was ever
> set, or no initial state was set, returns fsm_empty_set() immediately — in this path the handle
> and its contents are leaked (not freed).
> Otherwise runs the dynamic state builder (fsm_state_init with maxsigma+1): for each state
> 0..maxstate in order it emits the state's transitions by walking its transition list (LIFO —
> reverse order of addition) with the state's final/initial flags; arc-less states get a sentinel
> line. During the walk it tracks "emptiness": the machine counts as empty unless some state is both
> initial and final, or some initial state has an outgoing arc.
> Creates a shell with fsm_create(""), frees the shell's fresh sigma, and closes the builder into it
> (fsm_state_close fills arity, state/arc/line/final counts, pathcount = unknown, and heuristic
> determinism/epsilon-freeness flags). Attaches a sigma converted from the handle's sigma list
> (entries in ascending number order; NULL-symbol slots skipped). net->name is set to the handle's
> name if non-NULL, capped at FSM_NAME_LEN (40) bytes, else to a random hex string. The C strncpy'd
> exactly 40 bytes, which can split a UTF-8 codepoint; the cap is now rounded down to the nearest
> character boundary so the name stays valid UTF-8.
> Frees all transition nodes, hash-chain nodes, the sigma list, hash table, state list, and the
> handle itself; then sigma_sort(net). If the emptiness tracking never fired, the built net is
> destroyed and fsm_empty_set() returned; otherwise the net is returned (caller owns).

> [spec:foma:def:fomalib.fsm-construct-init-fn]
> fsm_construct_handle *fsm_construct_init(char *name)

> [spec:foma:sem:fomalib.fsm-construct-init-fn]
> Allocates and returns a fresh construction handle: fsm_state_list = calloc'd 1024 entries (size
> recorded as 1024), fsm_sigma_list = calloc'd 1024 entries (size 1024), fsm_sigma_hash = calloc'd
> 1021 (SIGMA_HASH_SIZE) buckets; maxstate = -1, maxsigma = -1, numfinals = 0, hasinitial = 0;
> name = strdup(name), or NULL when name is NULL. The handle is later consumed by
> fsm_construct_done.

> [spec:foma:def:fomalib.fsm-construct-set-final-fn]
> FEXPORT void fsm_construct_set_final(struct fsm_construct_handle *handle, int state_no)

> [spec:foma:sem:fomalib.fsm-construct-set-final-fn]
> Marks state_no final in a construction: grows the state list if needed (next power of two >
> state_no, new slots zeroed), raises handle->maxstate to state_no if higher, and — only if the
> state was not already final — sets its is_final flag and increments handle->numfinals.

> [spec:foma:def:fomalib.fsm-construct-set-initial-fn]
> FEXPORT void fsm_construct_set_initial(struct fsm_construct_handle *handle, int state_no)

> [spec:foma:sem:fomalib.fsm-construct-set-initial-fn]
> Marks state_no initial: grows the state list if needed, raises maxstate if needed, sets the
> state's is_initial flag (idempotent, no counter) and handle->hasinitial = 1. Several states may be
> marked initial; the built machine is then flagged nondeterministic by the state builder.

> [spec:foma:def:fomalib.fsm-contains-fn]
> fsm *fsm_contains(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-contains-fn]
> Returns [?* A ?*]: fsm_concat(fsm_concat(fsm_universal(), net), fsm_universal()), where
> fsm_universal() is the one-state IDENTITY self-loop machine. net is consumed (fsm_concat consumes
> both arguments); the result is minimized.

> [spec:foma:def:fomalib.fsm-contains-one-fn]
> fsm *fsm_contains_one(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-contains-one-fn]
> "Contains exactly one" $.A. Computes fsm_minus($X, $Y) where $ is fsm_contains, X is a copy of
> net, and Y — the strings with multiple/overlapping occurrences — is built literally as
> fsm_union(fsm_intersect(fsm_concat(fsm_kleene_plus(fsm_identity()), fsm_concat(copy,
> fsm_universal())), fsm_concat(copy, fsm_universal())), fsm_intersect(fsm_concat(copy,
> fsm_kleene_plus(fsm_identity())), copy)), i.e. $[[?+ A ?* & A ?*] | [A ?+ & A]]. Four copies of
> net are consumed by the formula; net itself is then destroyed. Returns the difference.

> [spec:foma:def:fomalib.fsm-contains-opt-one-fn]
> fsm *fsm_contains_opt_one(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-contains-opt-one-fn]
> "Contains at most one" occurrence: returns fsm_union(fsm_contains_one(fsm_copy(net)),
> fsm_complement(fsm_contains(fsm_copy(net)))) — strings with exactly one occurrence of net or none
> at all ($.A | ~$A). net is destroyed after the copies are taken.

> [spec:foma:def:fomalib.fsm-context-restrict-fn]
> fsm *fsm_context_restrict(struct fsm *X, struct fsmcontexts *LR)

> [spec:foma:sem:fomalib.fsm-context-restrict-fn]
> Compiles context restriction X => L1 _ R1, ..., Ln _ Rn.
> Var = the single-symbol net "@VARX@"; Notvar = kleene-star of the term negation of "@VARX@".
> "@VARX@" is added to X's sigma (then sorted) so ? cannot match it. For every context pair in LR: a
> NULL left/right is replaced by fsm_empty_string(); otherwise "@VARX@" is added to its sigma and
> any ".#." sigma symbol is renamed to "@#@" (sigma_substitute), then sorted — note the caller's
> context networks are mutated in place.
> UnionP starts as fsm_empty_set() and accumulates, per pair, minimize(union(minimize(concat(
> copy(left), Var, Notvar, Var, copy(right))), UnionP)). UnionL = minimize(Notvar · Var · copy(X) ·
> Var · Notvar). Result = intersect(UnionL, complement(Notvar · minimize(UnionP · Notvar))).
> If "@VARX@" remains in Result's sigma, Result = complement(fsm_substitute_symbol(Result,
> "@VARX@" -> epsilon)); else Result = complement(Result). If "@#@" is in Result's sigma (a context
> used the word boundary): Word = "@#@" (not-"@#@")* "@#@"; Result = intersect(Word, Result), then
> substitute "@#@" -> epsilon.
> Destroys UnionP, Var, Notvar and X. Finally calls fsm_clear_contexts(pairs) — but `pairs` is the
> exhausted loop cursor, i.e. NULL, so the LR list is in fact never freed (latent leak; LR survives,
> mutated). Returns Result.

> [spec:foma:def:fomalib.fsm-copy-fn]
> fsm *fsm_copy(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-copy-fn+1]
> Copies a network. A &mut borrow is never NULL; NULL-able callers check at the call site.
> Calls fsm_count(net) on the SOURCE FIRST to refresh its counts, THEN captures the
> now-fresh name/counts/flags into a new struct fsm — the C memcpy'd the whole struct before
> fsm_count ran, leaving the copy's counts stale. The copy gets its own sigma (sigma_copy) and its
> own state array (malloc + memcpy of linecount lines, sentinel included); medlookup is deep-cloned
> rather than aliased (the C shared it — a double-free hazard). The source is otherwise untouched;
> caller owns the copy.

> [spec:foma:def:fomalib.fsm-create-fn]
> fsm *fsm_create(char *name)

> [spec:foma:sem:fomalib.fsm-create-fn+1]
> Allocates a new empty network shell. The in-memory name is stored in full. C printed a warning
> when strlen(name) > FSM_NAME_LEN (40) and strncpy'd the name into a fixed 40-byte field (no NUL
> terminator guaranteed at 40+ chars), truncating longer names; the binary file format still caps
> names at 40 bytes on read/write. Sets arity = 1, arccount = 0; is_deterministic, is_pruned,
> is_minimized, is_epsilon_free, is_loop_free, arcs_sorted_in, arcs_sorted_out all NO; sigma =
> sigma_create() (fresh empty sigma); states = NULL; medlookup = NULL. Remaining numeric fields
> (statecount, linecount, finalcount, pathcount, is_completed) are left uninitialized malloc
> garbage. Caller owns the result.

> [spec:foma:def:fomalib.fsm-create-letter-lookup-fn]
> FEXPORT void fsm_create_letter_lookup(struct apply_med_handle *medh, struct fsm *net)

> [spec:foma:sem:fomalib.fsm-create-letter-lookup-fn]
> Precomputes two per-state input-symbol bitvectors on the med (approximate-match) handle, used as
> the A*-search heuristic. Sets medh->maxdepth = 2; bytes_per_letter_array = BITNSLOTS(sigma_max+1)
> bytes; letterbits and nletterbits are calloc'd statecount x bytes_per_letter_array.
> letterbits[v] = set of input-symbol numbers occurring anywhere on any path from state v (infinite
> horizon). Computed with an iterative (goto-converted) Tarjan SCC DFS starting from the first line
> of net->states: for each edge v -> v', v's bits gain the edge's in symbol and all of v''s bits;
> lowlink/index bookkeeping uses the global int stack; when an SCC root closes, its bits are copied
> to every other SCC member (making the computation correct on cyclic graphs). Only states reachable
> from the initial state receive bits.
> nletterbits[v] = set of input symbols reachable from v in fewer than medh->maxdepth (2) arcs: for
> each state v a bounded DFS over medh->state_array adds each arc's in symbol (if != -1) to v's
> bits, descending into targets with depth+1 and visiting sibling arcs at the same depth, cutting
> off at depth == maxdepth.
> Requires medh->state_array to be built beforehand; uses and clears the global int/ptr stacks;
> frees its SCC bookkeeping. `net` is neither modified nor destroyed. No return value.

> [spec:foma:def:fomalib.fsm-cross-product-fn]
> fsm *fsm_cross_product(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:fomalib.fsm-cross-product-fn]
> Cross product A .x. B of two acceptors: relates every string of net1 to every string of net2. Both
> nets are minimized, sigmas merged, both counted. Runs a parallel construction over state pairs
> (a,b) from (0,0) (triplet hash assigns result state numbers; int stack holds pending pairs). A
> result state is final iff both members final, start iff both start. For each pair, every line of a
> is crossed with every line of b, including final sentinel lines:
> - both lines real arcs: emit (in_a : in_b) to the pair of targets; if exactly one side is IDENTITY
> it is downgraded to UNKNOWN; if both sides are IDENTITY, an additional UNKNOWN:UNKNOWN arc to the
> same target is emitted (@:@ plus ?:?). Only the in symbols are used (arguments are acceptors).
> - a's line is a final sentinel (target -1, final 1) and b's a real arc: emit (EPSILON : in_b),
> IDENTITY downgraded to UNKNOWN, to (a stays, b's target) — A waits at a final state (0:b arcs).
> - symmetrically, b final sentinel and a real arc: emit (in_a : EPSILON) to (a's target, b stays).
> - non-final sentinel lines produce nothing.
> After closing, one scan of the result adds EPSILON and/or UNKNOWN to the sigma if any arc uses
> them. net1's state array is replaced in place, net2 destroyed (both inputs consumed); returns
> fsm_coaccessible(net1).

> [spec:foma:def:fomalib.fsm-destroy-fn]
> FEXPORT int fsm_destroy(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-destroy-fn]
> Frees a network and everything it owns. Returns 0 if net is NULL. Otherwise frees
> medlookup->confusion_matrix and medlookup when present, destroys the sigma list (freeing each node
> and its symbol string), frees the state array if non-NULL, frees the struct itself, returns 1.

> [spec:foma:def:fomalib.fsm-determinize-fn]
> fsm *fsm_determinize(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-determinize-fn]
> Subset construction: fsm_subset(net, DETERMINIZE), in place on net. If net->is_deterministic ==
> YES the net is returned unchanged immediately.
> The construction alphabet is the set of distinct in:out label PAIRS on arcs, each mapped to a
> dense integer via a maxsigma x maxsigma table (maxsigma = sigma_max+1); the pair EPSILON:EPSILON
> (0:0) is the only epsilon. net->arity is recomputed during this mapping (2 iff some in != out or
> UNKNOWN occurs). Each state's real arcs (epsilon pairs and sentinel lines excluded) are collected
> into a per-state array sorted by pair symbol; the sort pass also detects whether the machine is
> already deterministic (duplicate symbol at a state).
> Epsilon closure: per-state target lists of EPSILON:EPSILON arcs are memoized (self-loops and
> duplicate targets skipped); closures are computed by a mark-based DFS. Subsets of states are
> canonicalized in a hash table keyed order-insensitively (size equality plus membership marks);
> each new subset receives the next state number and a final flag = true iff any member is final.
> Subset 0 is the epsilon closure of all start states. Fast path: if no epsilon pair exists, no
> state had duplicate outgoing symbols, there is exactly one start state, and that start state is
> state 0 (checked via a _Bool truncation of the state number), the machine is already
> deterministic: is_deterministic and is_epsilon_free are set to YES and net is returned unchanged.
> Main loop over a worklist of unprocessed subsets: for subset T, sweep its members' sorted arc
> lists in ascending pair-symbol order; for the current minimal symbol collect the union of all
> member targets (deduplicated with a mark table), epsilon-close it, look up/create the target
> subset U, and emit one arc T -(in:out)-> U carrying T's final flag; T is a start state iff T == 0.
> Subsets without outgoing arcs still emit their (possibly final) sentinel line.
> The new line array replaces net->states (old freed); counts and flags are refreshed by the builder
> close; sigma is unchanged; returns the same net pointer. The result is deterministic over label
> pairs (a:b treated as one symbol).

> [spec:foma:def:fomalib.fsm-empty-fn]
> fsm_state *fsm_empty()

> [spec:foma:sem:fomalib.fsm-empty-fn]
> Allocates and returns a bare 2-line fsm_state array (not a struct fsm): line 0 = state 0 with
> in/out/target all -1, final 0, start 1 (a single non-final initial state with no arcs); line 1 =
> the all -1 terminator. Used as the state array of the empty language.

> [spec:foma:def:fomalib.fsm-empty-set-fn]
> fsm *fsm_empty_set()

> [spec:foma:sem:fomalib.fsm-empty-set-fn]
> Returns a new network accepting nothing: fsm_create("") with states = fsm_empty() (one non-final
> start state, no arcs). Flags: deterministic/pruned/minimized/epsilon-free/loop-free = YES,
> completed = NO. statecount 1, finalcount 0, arccount 0, linecount 2, pathcount 0. Sigma empty.

> [spec:foma:def:fomalib.fsm-empty-string-fn]
> fsm *fsm_empty_string()

> [spec:foma:sem:fomalib.fsm-empty-string-fn]
> Returns a new network accepting exactly the empty string: fsm_create("") plus a 2-line state
> array: state 0 with no arcs (in/out/target -1), final 1, start 1; then the -1 sentinel. Flags
> det/pruned/min/eps-free/loop-free = YES, completed NO; statecount 1, finalcount 1, arccount 0,
> linecount 2, pathcount 1. Sigma empty.

> [spec:foma:def:fomalib.fsm-epsilon-remove-fn]
> fsm *fsm_epsilon_remove(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-epsilon-remove-fn]
> fsm_subset(net, EPSILON_REMOVE): shares the entire machinery of
> `[spec:foma:sem:fomalib.fsm-determinize-fn]` (same pair alphabet, epsilon memoization, subset
> hashing, in-place result) with two differences. (1) Additional fast path: if the machine contains
> no EPSILON:EPSILON arc, is_epsilon_free is set to YES and net returned untouched (the
> is_deterministic == YES early return and the already-deterministic fast path also apply). (2) In
> the main loop, each individual target reached on a symbol is epsilon-closed and mapped to its own
> subset immediately, one arc emitted per target — nondeterminism is preserved; only epsilon
> transitions are eliminated. Returns the same net pointer.

> [spec:foma:def:fomalib.fsm-equal-substrings-fn]
> fsm *fsm_equal_substrings(struct fsm *net, struct fsm *left, struct fsm *right)

> [spec:foma:sem:fomalib.fsm-equal-substrings-fn]
> _eq(net, left, right): keeps only the paths of `net` whose lower side has all its
> left...right-delimited substrings identical. Caveat: no reliable termination condition — if
> delimited substrings can be unboundedly long and differ, the loop below never terminates (e.g.
> _eq(l a* r l a* r, l, r)); it terminates whenever the possible identical substrings are bounded in
> length.
> Uses auxiliary symbols "@<eq<@" (LB) and "@>eq>@" (RB), added to net's sigma. Building blocks:
> NOLB = (not LB)*, NORB = (not RB)*, NOBR = ~$[LB|RB].
> 1. InsertBrackets = [~$[left|right] [left 0:LB | 0:RB right]]* ~$[left|right];
> Lbracketed = copy(net) .o. InsertBrackets.
> 2. BracketFilter = NOBR LB NOBR RB NOBR [LB NOBR RB NOBR]+ (proper nesting, at least two bracket
> pairs). Lbypass = lower(Lbracketed .o. ~BracketFilter .o. [LB:0|RB:0|NOBR]*) (brackets removed);
> Leq = Lbracketed .o. BracketFilter.
> 3. Labels = fsm_sigma_pairs_net of the lower projection of Leq composed with
> [[NOLB:0]* LB:0 NORB* RB:0]* [NOLB:0]* — the attested symbols occurring between LB and RB.
> 4. Move = minimized union, over every label symbol X with number >= 3, of
> [NOLB* LB:0 X 0:LB]* NOLB* (moves each LB rightwards across one X). If there are no such symbols,
> net is destroyed and an untouched copy of the original net (made at entry) is returned.
> 5. Cleanup = NOLB-star [LB:0 RB:0 NOLB*]* | ~$[LB RB]. Loop: Leq = Leq .o. copy(Cleanup); if
> "@<eq<@" no longer occurs on Leq's lower side, stop; else Leq = Leq .o. copy(Move), repeat.
> 6. Result = minimize(net .o. [lower(Leq) | Lbypass]); both auxiliary symbols removed from Result's
> sigma; fsm_compact(Result); sigma_sort(Result). net is consumed; left and right are only copied,
> never destroyed; several intermediates (LB, RB, NOLB, NORB, NOBR, Cleanup, Move, entry copy) are
> leaked. Returns Result.

> [spec:foma:def:fomalib.fsm-equivalent-fn]
> FEXPORT int fsm_equivalent(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:fomalib.fsm-equivalent-fn]
> Structural path-equivalence test by parallel traversal; equals language equivalence only when both
> machines are deterministic, epsilon-free and in minimal canonical form. Merges sigmas (mutating
> both nets) and counts both. Worklist of state pairs starting at (0,0), each pair visited once
> (triplet hash). For a pair (a,b): fail immediately if the states' final flags differ; every real
> arc of a must have some arc of b with identical in AND out numbers (first match wins; the target
> pair is pushed if unseen); symmetrically every real arc of b must have a matching arc of a (no
> push). Arc scans stop at a state's first sentinel line. Returns 1 if the worklist empties with no
> mismatch, else 0. Both net1 and net2 are destroyed in all cases.

> [spec:foma:def:fomalib.fsm-escape-fn]
> fsm *fsm_escape(char *symbol)

> [spec:foma:sem:fomalib.fsm-escape-fn]
> Returns fsm_symbol(symbol+1): the single-symbol network for the input string minus its first
> character, i.e. strips one leading escape character (e.g. "%a" -> "a"). No validation of length;
> the string is caller-owned and not freed.

> [spec:foma:def:fomalib.fsm-explode-fn]
> fsm *fsm_explode(char *symbol)

> [spec:foma:sem:fomalib.fsm-explode-fn]
> Builds a linear chain acceptor from a quoted multicharacter string. The first and last characters
> of `symbol` are treated as delimiters and dropped (content = symbol[1..strlen-2]). The content is
> split into single UTF-8 characters; the k-th character (k from 1) becomes an arc from state k-1 to
> state k with that character on both sides (each one-character substring is duplicated, added via
> fsm_construct_add_arc, then freed). State 0 is initial; the last state is final. Empty content
> yields one state that is both initial and final (empty-string language). Built with the
> construction API and returned via fsm_construct_done; the input string is caller-owned.

> [spec:foma:def:fomalib.fsm-extract-ambiguous-domain-fn]
> fsm *fsm_extract_ambiguous_domain(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-extract-ambiguous-domain-fn]
> Returns the set of input (upper-side) words of transducer `net` that map to more than one output.
> Implements [_loweruniq(T) .o. _notid(_loweruniq(T).i .o. _loweruniq(T))].u:
> loweruniq = fsm_lowerdet(net) (consumes net): minimizes, then rewrites every arc's output to a
> per-state-unique symbol number (>= 3, filler symbols named as 12-digit hex strings are added to
> sigma when out-degree exceeds available symbols) and downgrades IDENTITY inputs to UNKNOWN, so the
> lower side uniquely encodes the path.
> result = fsm_topsort(fsm_minimize(fsm_upper(fsm_compose(copy(loweruniq),
> fsm_extract_nonidentity(fsm_compose(fsm_invert(copy(loweruniq)), copy(loweruniq))))))): inverse
> composed with self is an identity relation exactly on unambiguous inputs, so extracting the
> non-identity part isolates ambiguous ones; composing back and taking the upper projection yields
> the ambiguous input words. loweruniq is destroyed; sigma_cleanup(result,1), fsm_compact(result)
> and sigma_sort(result) applied; returns result. Input consumed.

> [spec:foma:def:fomalib.fsm-extract-ambiguous-fn]
> fsm *fsm_extract_ambiguous(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-extract-ambiguous-fn]
> Returns net restricted to its ambiguous inputs:
> fsm_topsort(fsm_minimize(fsm_compose(fsm_extract_ambiguous_domain(fsm_copy(net)), net))). The
> compose consumes both arguments, so net itself is consumed; a new network is returned.

> [spec:foma:def:fomalib.fsm-extract-nonidentity-fn]
> fsm *fsm_extract_nonidentity(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-extract-nonidentity-fn]
> Experimental helper: returns the upper-side language of all paths of transducer `net` that pass
> through an arc where the relation stops being an identity.
> Runs the discrepancy-tracking DFS of the identity test: net is minimized in place (fsm_minimize's
> return value is discarded — relies on in-place mutation) and counted; a fresh sigma symbol
> "@KILL@" is added (number remembered). Each state stores a discrepancy string: the pending surplus
> of upper (length > 0) or lower (length < 0) symbols along the path so far. Traversing arc in:out
> from a state fails if: in or out is UNKNOWN; in is IDENTITY with nonzero discrepancy; with
> positive discrepancy, out is neither EPSILON nor the first pending symbol (mirrored for negative
> discrepancy and in); with empty discrepancy, in != out and neither is EPSILON; the target state is
> final and the updated discrepancy is nonzero; or the target was already visited with a different
> stored discrepancy. Unlike the pure test, failure does not abort: the offending arc's OUT symbol
> is overwritten with the "@KILL@" number and traversal continues with the state's remaining arcs.
> Afterwards: sigma_sort(net); net2 = fsm_upper(fsm_compose(net, fsm_contains(
> fsm_symbol("@KILL@")))) — keeps exactly the paths through a marked arc and projects the upper
> side; "@KILL@" is removed from net2's sigma, which is re-sorted. net is consumed by the compose;
> the per-state discrepancy strings are leaked. Returns net2.

> [spec:foma:def:fomalib.fsm-extract-unambiguous-fn]
> fsm *fsm_extract_unambiguous(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-extract-unambiguous-fn]
> Returns net restricted to inputs with exactly one output:
> fsm_topsort(fsm_minimize(fsm_compose(fsm_complement(fsm_extract_ambiguous_domain(fsm_copy(net))),
> net))). net is consumed; a new network is returned.

> [spec:foma:def:fomalib.fsm-find-ambiguous-fn]
> fsm *fsm_find_ambiguous(struct fsm *net, int **extras)

> [spec:foma:sem:fomalib.fsm-find-ambiguous-fn]
> Declared in fomalib.h but never defined: no implementation exists anywhere in the foma sources,
> so any call fails at link time. A port need only reserve the signature (a net plus an int**
> out-parameter, evidently intended to accompany the ambiguity-extraction family); there is no
> behavior to reproduce.

> [spec:foma:def:fomalib.fsm-flatten-fn]
> fsm *fsm_flatten(struct fsm *net, struct fsm *epsilon)

> [spec:foma:sem:fomalib.fsm-flatten-fn+1]
> Rewrites transducer `net` as an equal-length acceptor over a flattened alphabet: each arc a:b
> becomes two identity arcs a then b through a fresh intermediate state, with EPSILON replaced by a
> concrete stand-in symbol.
> net is minimized first. The stand-in string is the input label of the FIRST arc of `epsilon`
> (strdup'd from its sigma). An arc-less epsilon net (fsm_get_next_arc == 0, end-of-arcs) destroys
> both nets and returns None. C compared fsm_get_next_arc's result against -1, which it never
> returns, so the guard never fired and an arc-less epsilon net left the cursor on the sentinel line
> (in == -1), causing an out-of-bounds sigma read.
> Construction handle named net->name with sigma copied verbatim from net. For the k-th arc (k from
> 0; source s, target t, labels in:out) the intermediate state is m = net->statecount + k: if in or
> out is EPSILON, string-based arcs (s,m,I,I) and (m,t,O,O) are added where I/O are the arc's symbol
> strings with EPSILON replaced by the stand-in; otherwise numeric arcs (s,m,in,in) and
> (m,t,out,out). All finals and initials are copied. Destroys net and epsilon, frees the stand-in
> string, returns the new network.

> [spec:foma:def:fomalib.fsm-follows-fn]
> fsm *fsm_follows(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:fomalib.fsm-follows-fn]
> Returns ~$[net1 ?* net2], built literally as fsm_complement(fsm_minimize(fsm_contains(
> fsm_minimize(fsm_concat(fsm_minimize(fsm_copy(net1)), fsm_concat(fsm_universal(),
> fsm_minimize(fsm_copy(net2))))))))): the strings containing no occurrence of net1 followed
> (anywhere later) by an occurrence of net2. Only copies of the arguments are consumed; net1 and
> net2 themselves are NOT destroyed — a deviation from the usual consuming convention (callers that
> assume consumption leak them).

> [spec:foma:def:fomalib.fsm-get-arc-in-fn]
> FEXPORT char *fsm_get_arc_in(struct fsm_read_handle *handle)

> [spec:foma:sem:fomalib.fsm-get-arc-in-fn]
> Returns the input-symbol string of the arc under the read handle's arc cursor: NULL if the cursor
> is unset (no successful fsm_get_next_arc/fsm_get_next_state yet), else the symbol string stored at
> handle->fsm_sigma_list[cursor->in]. The pointer is borrowed from the handle's sigma list (valid
> until fsm_read_done; caller must not free). No bounds check: calling it while the cursor sits on a
> sentinel line (in == -1) reads out of bounds.

> [spec:foma:def:fomalib.fsm-get-arc-num-in-fn]
> FEXPORT int fsm_get_arc_num_in(struct fsm_read_handle *handle)

> [spec:foma:sem:fomalib.fsm-get-arc-num-in-fn]
> Returns the input-symbol number of the arc under the read handle's arc cursor, or -1 if the cursor
> is unset. On a sentinel line this returns the stored -1.

> [spec:foma:def:fomalib.fsm-get-arc-num-out-fn]
> FEXPORT int fsm_get_arc_num_out(struct fsm_read_handle *handle)

> [spec:foma:sem:fomalib.fsm-get-arc-num-out-fn]
> Returns the output-symbol number of the arc under the read handle's arc cursor, or -1 if the
> cursor is unset. On a sentinel line this returns the stored -1.

> [spec:foma:def:fomalib.fsm-get-arc-out-fn]
> FEXPORT char *fsm_get_arc_out(struct fsm_read_handle *handle)

> [spec:foma:sem:fomalib.fsm-get-arc-out-fn]
> Output-side counterpart of `[spec:foma:sem:fomalib.fsm-get-arc-in-fn]`: NULL if the cursor is unset,
> else the borrowed sigma-list string for cursor->out; no bounds check for out == -1.

> [spec:foma:def:fomalib.fsm-get-arc-source-fn]
> FEXPORT int fsm_get_arc_source(struct fsm_read_handle *handle)

> [spec:foma:sem:fomalib.fsm-get-arc-source-fn]
> Returns the source-state number of the arc under the read handle's arc cursor: -1 if the cursor
> is unset (no successful fsm_get_next_arc/fsm_get_next_state yet), else the cursor line's stored
> state_no verbatim — so if the cursor is parked on the terminating sentinel line after iteration
> is exhausted, this returns the stored -1. No side effects.

> [spec:foma:def:fomalib.fsm-get-arc-target-fn]
> FEXPORT int fsm_get_arc_target(struct fsm_read_handle *handle)

> [spec:foma:sem:fomalib.fsm-get-arc-target-fn]
> Returns the target-state number of the arc under the read handle's arc cursor, or -1 if the
> cursor is unset. Returns the stored target field verbatim, so on a sentinel or arcless dummy
> line it yields the stored -1. No side effects.

> [spec:foma:def:fomalib.fsm-get-has-unknowns-fn]
> FEXPORT int fsm_get_has_unknowns(struct fsm_read_handle *handle)

> [spec:foma:sem:fomalib.fsm-get-has-unknowns-fn]
> Returns the handle's has_unknowns flag, computed once at fsm_read_init: 1 if any line of the
> wrapped net had in or out equal to UNKNOWN (1) or IDENTITY (2), else 0. Pure field read.

> [spec:foma:def:fomalib.fsm-get-library-version-string-fn]
> FEXPORT char *fsm_get_library_version_string()

> [spec:foma:sem:fomalib.fsm-get-library-version-string-fn]
> sprintf's "%i.%i.%i%s" of MAJOR_VERSION (0), MINOR_VERSION (10), BUILD_VERSION (0) and
> STATUS_VERSION ("alpha") — yielding "0.10.0alpha" — into a function-local static 20-byte
> buffer and returns a pointer to it. The buffer is rewritten on every call and shared by all
> callers (not thread-safe); the caller must not free it.

> [spec:foma:def:fomalib.fsm-get-next-arc-fn]
> FEXPORT int fsm_get_next_arc(struct fsm_read_handle *handle)

> [spec:foma:sem:fomalib.fsm-get-next-arc-fn]
> Advances the handle's arc cursor to the next real arc and returns 1, or 0 at end. If the cursor
> is unset, it starts at the first line of the state array; if it already sits on the terminating
> sentinel (state_no == -1) it returns 0 immediately; otherwise it moves one line forward. In
> either starting case it then skips lines whose target is -1 (arcless dummy lines) until reaching
> a real arc (return 1, cursor on it, readable via the fsm_get_arc_* accessors) or the sentinel
> (return 0, cursor parked there so all further calls return 0). Enumerates arcs in the net's line
> order: grouped by source state, ascending. fsm_read_reset restarts the iteration.

> [spec:foma:def:fomalib.fsm-get-next-final-fn]
> FEXPORT int fsm_get_next_final(struct fsm_read_handle *handle)

> [spec:foma:sem:fomalib.fsm-get-next-final-fn]
> Enumerates the net's final-state numbers in ascending order from the handle's finals array
> (built at fsm_read_init, -1-terminated). First call (cursor unset) points the cursor at the
> array head and returns its value; each later call returns -1 without moving if the cursor
> already sits on the -1 terminator, otherwise advances one slot and returns that value. So -1
> signals exhaustion (and is returned by every call thereafter). fsm_read_reset restarts it.

> [spec:foma:def:fomalib.fsm-get-next-initial-fn]
> FEXPORT int fsm_get_next_initial(struct fsm_read_handle *handle)

> [spec:foma:sem:fomalib.fsm-get-next-initial-fn]
> Identical mechanism to `[spec:foma:sem:fomalib.fsm-get-next-final-fn]` but over the handle's
> initials array: enumerates the net's start-state numbers in ascending order, returning -1 at
> exhaustion (and on every call thereafter); own cursor, reset by fsm_read_reset.

> [spec:foma:def:fomalib.fsm-get-next-state-arc-fn]
> FEXPORT int fsm_get_next_state_arc(struct fsm_read_handle *handle)

> [spec:foma:sem:fomalib.fsm-get-next-state-arc-fn]
> Iterates the outgoing arcs of the state most recently returned by fsm_get_next_state.
> Pre-increments the arc cursor; if the new line's state_no differs from handle->current_state or
> its target is -1 (dummy line of an arcless state), decrements the cursor back and returns 0
> (iteration done); else returns 1 with the cursor on the arc, readable via the fsm_get_arc_*
> accessors. Only valid after fsm_get_next_state, which parks the arc cursor one line before the
> state's first line; there is no NULL-cursor guard, so calling it before any fsm_get_next_state
> dereferences a NULL-adjacent pointer (undefined behavior).

> [spec:foma:def:fomalib.fsm-get-next-state-fn]
> FEXPORT int fsm_get_next_state(struct fsm_read_handle *handle)

> [spec:foma:sem:fomalib.fsm-get-next-state-fn]
> Enumerates the net's states in ascending state-number order via the handle's states_head array
> (built at init: entry i points at state i's first line). First call sets the state cursor to
> the array head; later calls increment it. When the cursor index reaches
> fsm_get_num_states(handle), returns -1 (exhausted). Otherwise stores the state's number in
> handle->current_state, sets the shared arc cursor to one line BEFORE the state's first line (so
> fsm_get_next_state_arc's pre-increment lands on it), and returns the state number. Note this
> clobbers the same arc cursor that fsm_get_next_arc uses; fsm_read_reset restarts everything.

> [spec:foma:def:fomalib.fsm-get-num-states-fn]
> FEXPORT int fsm_get_num_states(struct fsm_read_handle *handle)

> [spec:foma:sem:fomalib.fsm-get-num-states-fn]
> Returns handle->net->statecount — the state count of the network the read handle wraps (a live
> read of the net's field, normally unchanged while a handle is open). No side effects.

> [spec:foma:def:fomalib.fsm-get-option-fn]
> FEXPORT void *fsm_get_option(unsigned long long option)

> [spec:foma:sem:fomalib.fsm-get-option-fn]
> Getter for library-global options held in the global struct fsm_options: for option ==
> FSMO_SKIP_WORD_BOUNDARY_MARKER returns a pointer to the global _Bool
> fsm_options.skip_word_boundary_marker (a live pointer — writes through it change behavior,
> e.g. fsm_merge_sigma's ".#." propagation); any other option value returns NULL.

> [spec:foma:def:fomalib.fsm-get-symbol-number-fn]
> FEXPORT int fsm_get_symbol_number(struct fsm_read_handle *handle, char *symbol)

> [spec:foma:sem:fomalib.fsm-get-symbol-number-fn]
> Looks up a symbol string in the handle's sigma list: linear scan of indices 0 ..
> sigma_list_size-1 (sigma_list_size = sigma_max+1 at init) over fsm_sigma_list, a dense array
> indexed by symbol number whose unused slots hold NULL symbols (skipped); returns the first
> index whose symbol strcmp-equals `symbol`, or -1 if absent. `symbol` is only read.

> [spec:foma:def:fomalib.fsm-identity-fn]
> fsm *fsm_identity()

> [spec:foma:sem:fomalib.fsm-identity-fn]
> Constructs a fresh network for `?` — the identity relation over any single symbol. Creates a
> net via fsm_create(""), frees the initial empty sigma node, and hand-builds a 3-line state
> array: line 0 = state 0 (start, non-final) with an IDENTITY:IDENTITY (2:2) arc to state 1;
> line 1 = state 1 as an arcless final line; line 2 = the -1 sentinel. Sigma is a single node
> {number IDENTITY (2), symbol "@_IDENTITY_SYMBOL_@"}. Flags: deterministic/pruned/minimized/
> epsilon-free/loop-free = YES, completed = NO. Counts: statecount 2, finalcount 1, arccount 1,
> linecount 3, pathcount 1. Caller owns the result.

> [spec:foma:def:fomalib.fsm-ignore-fn]
> fsm *fsm_ignore(struct fsm *net1, struct fsm *net2, int operation)

> [spec:foma:sem:fomalib.fsm-ignore-fn+1]
> Ignore: net1 with net2-material freely interspersed. Both inputs are minimized first
> (consumed). If net2 is then empty: destroy net2 and return net1 unchanged. Otherwise merge
> sigmas (fsm_merge_sigma) and recount. For operation == OP_IGNORE_INTERNAL (2), compute by
> formula: result = lower(compose(ignore(copy(net1), symbol("@i<@"), OP_IGNORE_ALL),
> compose(complement(union(concat(symbol("@i<@"), universal), concat(universal,
> symbol("@i<@")))), simple_replace(symbol("@i<@"), copy(net2))))) — i.e. insertion is barred at
> the very start and end of the string; then remove "@i<@" from the result sigma, destroy net1
> and net2, return the new net. For OP_IGNORE_ALL (1), splice in place: for each state s of
> net1 (at its first line), emit an EPSILON:EPSILON arc from s (keeping s's final/start flags)
> to the start state of a private copy of net2, then s's original arc lines; arcless dummy
> lines are replaced by just the splice arc. The net2 copies are appended after net1's states
> in blocks of net2->statecount states (numbered from net1->statecount up, in state-encounter
> order; net2's start state is state 0 since it is minimized); every copy line is non-final and
> non-start, and each final state of a copy gets one EPSILON:EPSILON arc back to s. Because the
> splice can be re-entered from s, this inserts [net2]* at every position. Result reuses the
> net1 struct (state array replaced, all flags cleared, counts recomputed via fsm_count); net2
> is destroyed. Result is not determinized or minimized.

> [spec:foma:def:fomalib.fsm-intersect-fn]
> fsm *fsm_intersect(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:fomalib.fsm-intersect-fn]
> Intersection by running the machines in parallel. Minimizes both inputs; if either is then
> empty, destroys both and returns fsm_empty_set(). Merges sigmas (fsm_merge_sigma, which
> expands UNKNOWN/IDENTITY arcs over the union alphabet). Explores state pairs from (0,0) with
> a worklist stack and a pair-to-new-state-number hash (pair (0,0) is new state 0): a pair is
> start iff both components are start, final iff both are final. For each pair, index net2's
> arcs of state b by exact (in,out) label (a sigma2size^2 table stamped with a generation
> counter so it needn't be cleared between pairs); for each arc of a whose exact (in,out) label
> also leaves b, emit an arc with that label to the pair (a.target, b.target), allocating a new
> state number and pushing the pair if unseen. Labels must match literally on both tapes — no
> epsilon-closure (minimized nets are epsilon-free, but e.g. a:0 only matches a:0). The result
> is a fresh fsm struct that takes net1's merged sigma; both inputs are destroyed; the result
> is passed through fsm_coaccessible (dead-end pruning) and returned, not minimized.

> [spec:foma:def:fomalib.fsm-invert-fn]
> fsm *fsm_invert(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-invert-fn]
> Inverts the transducer in place: swaps the in and out fields of every line (including dummy
> and sentinel lines, where both are -1) and swaps the arcs_sorted_in/arcs_sorted_out flags.
> Sigma is untouched (it is shared by both tapes); no other flags or counts change. Returns the
> same net pointer. O(linecount).

> [spec:foma:def:fomalib.fsm-isempty-fn]
> FEXPORT int fsm_isempty(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-isempty-fn]
> Tests whether the language/relation is empty. Minimizes a COPY of net (the input is not
> modified or consumed) and checks whether the result is the canonical empty machine: first
> line has target == -1 and final_state == 0 and the second line is the -1 sentinel (one
> non-final state, no arcs). Destroys the copy; returns 1 if empty, else 0.

> [spec:foma:def:fomalib.fsm-isfunctional-fn]
> FEXPORT int fsm_isfunctional(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-isfunctional-fn]
> Tests whether the transducer is functional (maps each input to at most one output): computes
> tmp = fsm_minimize(fsm_compose(fsm_invert(fsm_copy(net)), fsm_copy(net))) — i.e. net.i .o.
> net — and returns fsm_isidentity(tmp), destroying tmp before returning. The input is not
> consumed. Returns 1/0.

> [spec:foma:def:fomalib.fsm-isidentity-fn]
> FEXPORT int fsm_isidentity(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-isidentity-fn]
> Tests whether the transducer encodes only identity relations (every accepted pair has equal
> upper and lower strings). Operates on a minimized copy (destroyed before return; the input is
> untouched). Performs a DFS from the initial state using the global pointer stack (cleared on
> entry and on failure), carrying per state a memoized "discrepancy": the queue of symbols one
> tape is ahead by (length > 0 means the input side is ahead and the output side owes those
> symbols; length < 0 the converse; 0 means balanced). Per arc: fail if in or out is UNKNOWN
> (1); fail if in == IDENTITY (2) with a non-empty discrepancy; with a non-empty discrepancy,
> the owing side's symbol must be EPSILON (0) or equal the queue's front symbol; with an empty
> discrepancy, fail if in != out and neither is EPSILON. The new discrepancy for the target is
> the old queue with the front consumed when the owing side matched it, and with the
> non-epsilon symbol of an x:0 / 0:x arc appended when that lengthens the debt. Fail if the
> target is a final state and the new discrepancy is non-empty, or if the target was already
> visited with a different stored discrepancy (compared by length and contents); otherwise
> store it and continue depth-first. Returns 1 if the whole reachable graph passes, 0 on any
> failure.

> [spec:foma:def:fomalib.fsm-issequential-fn]
> FEXPORT int fsm_issequential(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-issequential-fn]
> Tests whether the net is sequential on the input tape: in every state, no input symbol labels
> two arcs, and an EPSILON (0) input arc may only occur as the state's sole arc. Implementation:
> allocate an int table indexed by symbol number (size sigma_max+1, entries initialized to -2)
> recording the last state in which each input symbol was seen; scan lines in order, skipping
> lines with in < 0; per-state seen-any-arc/seen-epsilon flags reset when state_no changes.
> Fails when: the current input symbol's table entry equals the current state (duplicate), or
> any arc follows an EPSILON-input arc in the same state, or an EPSILON-input arc follows any
> other arc. Returns 1 if the scan completes, else 0 — and in the failure case prints "fails at
> state %i\n" (the offending state) to stdout. The net is not modified or consumed.

> [spec:foma:def:fomalib.fsm-isunambiguous-fn]
> FEXPORT int fsm_isunambiguous(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-isunambiguous-fn]
> Tests whether no input string has two distinct successful paths. Steps: loweruniq =
> fsm_lowerdet(fsm_copy(net)) — relabels every arc's output with a per-state-unique marker
> symbol so distinct paths become distinguishable on the lower side; testnet =
> fsm_minimize(fsm_compose(fsm_invert(fsm_copy(loweruniq)), fsm_copy(loweruniq))); result =
> fsm_isidentity(testnet). Destroys loweruniq and testnet; the input is not consumed. Returns
> 1/0.

> [spec:foma:def:fomalib.fsm-isuniversal-fn]
> FEXPORT int fsm_isuniversal(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-isuniversal-fn+1]
> Tests whether the language is ?* (universal). Minimizes net (the compacted result is dropped —
> neither returned nor destroyed) and runs fsm_compact on it, then pattern-matches the state array
> against the canonical universal machine: line 0 a final state with an IDENTITY:IDENTITY (2:2)
> self-loop to state 0, line 1 the -1 sentinel, and sigma_max < 3 (no real symbols); returns 1 on a
> match, else 0.
> The C condition conjoined (fsm+1)->state_no == 0 with (fsm+1)->state_no == -1
> (unsatisfiable → always returned 0); the erroneous == 0 conjunct is dropped, implementing the
> evident universality test.

> [spec:foma:def:fomalib.fsm-kleene-plus-fn]
> fsm *fsm_kleene_plus(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-kleene-plus-fn]
> Kleene plus A+: identical construction to `[spec:foma:sem:fomalib.fsm-kleene-star-fn]` except
> the new start state 0 is NOT final (so the empty string is accepted only if A accepts it) and
> finalcount is left unchanged. Consumes net.

> [spec:foma:def:fomalib.fsm-kleene-star-fn]
> fsm *fsm_kleene_star(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-kleene-star-fn]
> Kleene star A*. Minimizes net (consumed), then rebuilds the state array with all old states
> shifted up by 1: new state 0 is the unique start state, marked final, with an
> EPSILON:EPSILON arc to state 1 (the old initial state — state 0 after minimization). Old
> lines are copied with state_no/target incremented and start flags cleared; when a final
> state's lines begin, an EPSILON:EPSILON arc from it back to state 0 (final flag set) is
> emitted first, and an arcless final state's dummy line is replaced by just that arc. Adds
> EPSILON to sigma if absent. Updates counts (statecount+1, finalcount+1, linecount, arccount
> recomputed, pathcount unknown), clears the deterministic/pruned/minimized/epsilon-free
> flags, frees the old array, and returns the same net struct.

> [spec:foma:def:fomalib.fsm-left-rewr-fn]
> fsm *fsm_left_rewr(struct fsm *net, struct fsm *rewr)

> [spec:foma:sem:fomalib.fsm-left-rewr-fn]
> Fast single-symbol left-context rewrite: _leftrewr(L, a:b) computes a -> b || .#. L _ (and
> with net = [?* L], the context becomes L _). `rewr` must be a single-arc a:b machine; after
> fsm_merge_sigma(net, rewr), a = the in and b = the out of rewr's first state line. Rebuilds
> net through read/construct handles: every original state becomes final in the output; each
> arc is copied with its numeric labels, except that an arc whose input is a leaving an
> ORIGINALLY-final state (finality in net marks "context L just completed") has its output
> replaced by b. The machine is then completed into a sink state (number = original
> statecount): for every symbol i in 2..sigma_max (so never EPSILON 0 or UNKNOWN 1) with no
> arc from the current state and i != a, add arc state -> sink over i:i; if the state had no
> arc with input a at all, add state -> sink over a:b if the state was originally final, else
> a:a. If any sink arc was added, the sink gets i:i self-loops for all i in 2..sigma_max and
> is final. Initial state is 0; output sigma is copied from net. Destroys both net and rewr;
> returns the newly constructed net.

> [spec:foma:def:fomalib.fsm-lenient-compose-fn]
> fsm *fsm_lenient_compose(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:fomalib.fsm-lenient-compose-fn]
> Lenient composition, literally fsm_priority_union_upper(fsm_compose(fsm_copy(net1), net2),
> fsm_copy(net1)) followed by fsm_destroy(net1) — i.e. [A .o. B] .P. A: inputs that survive
> the composition keep their composed mappings; inputs rejected by B fall back to their A
> mappings. (The source comment claims "[A .o. B] .P. B" but the code passes A as the
> fallback.) net2 is consumed by the compose; net1 is copied twice, then destroyed. Returns
> the new net.

> [spec:foma:def:fomalib.fsm-letter-machine-fn]
> fsm *fsm_letter_machine(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-letter-machine-fn+1]
> Converts a net whose sigma may contain multicharacter symbols into an equivalent machine
> whose arcs carry only single UTF-8 letters. Minimizes net and iterates its arcs via a read
> handle, building the output with a construct handle (named literally "name"). Arcs where
> both sides are special (symbol number <= IDENTITY) or single-letter strings are copied
> verbatim (by symbol string). An arc where either side is a real symbol (number > IDENTITY)
> of utf8-length > 1 is split into steps = max(inlen, outlen) consecutive arcs (special-symbol
> sides count as length 1) threaded through fresh intermediate states numbered from
> net->statecount upward: step i takes the next UTF-8 letter of each multichar side; a side
> whose letters run out contributes "@_EPSILON_SYMBOL_@"; a special-symbol side repeats its
> special symbol string on EVERY step (it is never epsilon-padded). The first step leaves the
> original source, the last step enters the original target. Finals and initials are copied
> through. Returns the newly constructed net. Latent bugs to preserve/flag: (1) statecount is
> read via the old `net` pointer after fsm_minimize(net) may have returned a different
> pointer (potential use-after-free); (2) the minimized input net is never destroyed (the
> read handle does not own it) — it leaks; (3) letters are staged through fixed 128-byte
> buffers.
> The C sized the output-side copy by utf8skip(in) instead of utf8skip(out),
> corrupting the copied letter when the two sides' current letters had different UTF-8
> widths; the port sizes the output copy by utf8skip(out)+1, so each step copies exactly one
> UTF-8 output character.

> [spec:foma:def:fomalib.fsm-lexc-parse-file-fn]
> fsm *fsm_lexc_parse_file(char *myfile, int verbose)

> [spec:foma:sem:fomalib.fsm-lexc-parse-file-fn]
> Reads the whole file into memory with file_to_mem (see
> `[spec:foma:sem:fomalib.file-to-mem-fn]`) and delegates to fsm_lexc_parse_string with the same
> verbose flag, returning its result. The file buffer is never freed (leak). There is no NULL
> check: if file_to_mem fails, NULL is handed to the lexc scanner (undefined behavior).

> [spec:foma:def:fomalib.fsm-lexc-parse-string-fn]
> fsm *fsm_lexc_parse_string(char *mystring, int verbose)

> [spec:foma:sem:fomalib.fsm-lexc-parse-string-fn]
> Compiles a lexc grammar supplied as a string. Saves the global g_defines (restored before
> returning), creates a flex scan buffer over the string (YY_BUF_SIZE 16 MB), resets the
> static entry counter to -1 and lexclineno to 1, calls lexc_init() to reset the lexc
> compiler's state, then runs the lexc scanner (lexclex()), whose rule actions drive the lexc
> compiler (defining lexicons, entries, continuation classes). While scanning, the entry count
> of each finished LEXICON is printed to stdout ("%i, ", with "%i..." progress every 10000
> entries); if the scanner returns anything but 1 (its error code) and entries were counted,
> the final lexicon's count is printed with a newline. Deletes the scan buffer and returns
> lexc_to_fsm() — the network compiled from all parsed lexicons starting at the root (NULL if
> nothing valid was parsed). The `verbose` parameter is ignored. The input string is not
> freed.

> [spec:foma:def:fomalib.fsm-logical-eq-fn]
> fsm *fsm_logical_eq(char *string1, char *string2)

> [spec:foma:sem:fomalib.fsm-logical-eq-fn]
> Logical equivalence of two quantifier variables — the language where x = string1 and y =
> string2 span the same substring: ?* [x y | y x]/Q ?* [x y | y x]/Q ?*, where x and y are
> single-symbol machines (fsm_symbol) and /Q is fsm_ignore(…, union_quantifiers(),
> OP_IGNORE_ALL) with union_quantifiers() = the one-state machine accepting any symbol from
> the global list of currently defined quantifiers. Built literally with
> fsm_concat/fsm_union/fsm_universal from those pieces (the [x y | y x]/Q block appears
> twice, built independently each time). Reads the global quantifier list; the string
> arguments are only read. Returns a new net.

> [spec:foma:def:fomalib.fsm-logical-precedence-fn]
> fsm *fsm_logical_precedence(char *string1, char *string2)

> [spec:foma:sem:fomalib.fsm-logical-precedence-fn]
> Logical precedence x < y for quantifier variables x = string1, y = string2: the language
> \y* x \y* [x | y Q* x] ?*, where x/y are single-symbol machines (fsm_symbol), \y =
> fsm_term_negation(fsm_symbol(string2)) (any one other symbol), Q = union_quantifiers() (a
> one-state machine accepting any currently defined quantifier symbol; reads the global
> quantifier list) and ?* = fsm_universal(). Assembled literally with
> fsm_concat/fsm_kleene_star/fsm_union. String arguments are only read; returns a new net.

> [spec:foma:def:fomalib.fsm-lower-fn]
> fsm *fsm_lower(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-lower-fn]
> Lower (output-side) projection, in place. Rebuilds the state array with the fsm_state_*
> dynarray builder, walking lines in order: state numbers, finality and start flags are
> preserved (arcless states kept as arcless); each real arc's label becomes out:out, with out
> == UNKNOWN (1) mapped to IDENTITY (2). The old state array is freed and replaced; the
> deterministic/pruned/minimized flags are cleared (others set to UNK) and sigma_cleanup(net,
> 0) drops symbols no longer used by any arc. Returns the same (consumed/modified) net. Not
> determinized: distinct upper symbols over the same lower symbol yield duplicate arcs.

> [spec:foma:def:fomalib.fsm-lowerdet-fn]
> fsm *fsm_lowerdet(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-lowerdet-fn]
> Ambiguity-test helper: relabels the output side of every arc with a symbol unique within its
> source state. Minimizes net (consumed, in place), computes maxarc = the maximum out-degree
> over all states; the relabeling needs symbol numbers 3..maxarc+2, so if maxarc >
> sigma_max-2, adds (maxarc - (sigma_max-2)) fresh dummy symbols to sigma — each named by
> sprintf "%012X" of an unsigned counter starting at 8723643, incremented per symbol — and
> re-sorts sigma. Then for each state's arcs in order, sets out = 3, 4, 5, ... (counter
> resetting to 3 at every new source state) and replaces in == IDENTITY (2) with UNKNOWN (1).
> Returns the modified net; counts/flags are not updated beyond what fsm_minimize set.

> [spec:foma:def:fomalib.fsm-lowerdeteps-fn]
> fsm *fsm_lowerdeteps(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-lowerdeteps-fn]
> Same as `[spec:foma:sem:fomalib.fsm-lowerdet-fn]` except arcs whose output is EPSILON (0) are
> left completely untouched (neither out relabeled nor the in IDENTITY->UNKNOWN replacement
> applied); the per-state counter still starts at 3 and only advances on relabeled arcs. The
> dummy-symbol padding computation still counts epsilon-output arcs in maxarc.

> [spec:foma:def:fomalib.fsm-mark-ambiguous-fn]
> fsm *fsm_mark_ambiguous(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-mark-ambiguous-fn]
> Declared in fomalib.h (FEXPORT prototype) but never defined anywhere in the library or
> tools: any use fails at link time. There is no behavior to port; the port need only decide
> whether to omit the declaration or supply an implementation intentionally.

> [spec:foma:def:fomalib.fsm-mark-fsm-tail-fn]
> fsm *fsm_mark_fsm_tail(struct fsm *net, struct fsm *marker)

> [spec:foma:sem:fomalib.fsm-mark-fsm-tail-fn]
> Rewrite-compiler helper: _marktail(?* R.r, 0:x).r implements ~$x .o. [..] -> x || _ R.
> Rebuilds net (via read/construct handles, sigma copied from net) so that every arc entering
> an originally-final state t is rerouted through an interposed state m: for each distinct
> such t, allocate m (numbered from net->statecount upward, memoized in a calloc'd mappings
> array where 0 means unset — safe since new numbers start at statecount >= 1) and, for every
> arc of `marker`, add one arc m -> t labeled with that arc's in/out symbol strings (marker's
> own state structure is ignored — only its arc labels matter — and the labels are added by
> string, so they are auto-added to the output sigma). The incoming arc is then emitted as
> source -> m with its original numeric labels; arcs into non-final states are copied
> unchanged. All original states 0..statecount-1 are marked final (the interposed states are
> not); initial state is 0. Destroys net; marker is NOT destroyed (caller keeps it); returns
> the new net.

> [spec:foma:def:fomalib.fsm-markallfinal-fn]
> fsm *fsm_markallfinal(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-markallfinal-fn]
> Sets final_state = YES on every line of the state array (all states become final), in place,
> and returns the same net. finalcount, pathcount and the is_* flags are NOT updated.

> [spec:foma:def:fomalib.fsm-merge-sigma-fn]
> FEXPORT void fsm_merge_sigma(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:fomalib.fsm-merge-sigma-fn]
> Harmonizes two networks' alphabets in place (neither net destroyed; no return value). First,
> unless the global option fsm_options.skip_word_boundary_marker is set, if exactly one net's
> sigma contains ".#." it is added to the other (sigma_add + sigma_sort). Then builds the
> merged sigma as a sorted union walk over both sigma lists (assumed sorted): special symbols
> (number <= IDENTITY = 2) merge by number; ordinary symbols merge by strcmp on the symbol
> string, receiving consecutive new numbers; per-symbol provenance is recorded (net1-only,
> net2-only, or both), as are old->new number mappings for each net and whether the sigmas
> were identical. Every arc label > 2 in both nets is rewritten through its mapping; each net
> gets its own fresh copy of the merged sigma (old sigmas freed). Finally, if a net contains
> UNKNOWN (1) or IDENTITY (2) and the sigmas were not identical, its state array is rebuilt
> with unknown-expansion over the symbols present only in the OTHER net: every @:@ (IDENTITY)
> arc additionally gets an s:s arc for each such symbol s; every ?:x arc gets s:x; every x:?
> gets x:s; every ?:? arc gets an s:t arc for each ordered pair of distinct labels where each
> of s,t is either UNKNOWN or an other-net-only symbol (the original ?:?, meaning a pair of
> distinct symbols both outside sigma, is kept). Ordinary and dummy lines are copied through;
> the new array (sized exactly by a pre-count) replaces the old. Counts and flags are not
> updated by this function.

> [spec:foma:def:fomalib.fsm-minimize-fn]
> fsm *fsm_minimize(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-minimize-fn]
> Returns NULL for NULL input. Ensures the net is deterministic (calls fsm_determinize if
> is_deterministic != YES) and trim (fsm_coaccessible if is_pruned != YES). Then, if
> is_minimized != YES and the global g_minimal is 1, minimizes: Hopcroft's algorithm
> (fsm_minimize_hop, which returns fsm_empty_set() if there are no final states) when the
> global g_minimize_hopcroft != 0 (the default), else Brzozowski's
> determinize(reverse(determinize(reverse(net)))); afterwards sets the
> deterministic/pruned/minimized/epsilon-free flags to YES. With g_minimal == 0 only the
> determinize/prune steps run. Consumes its argument and may return a different pointer.

> [spec:foma:def:fomalib.fsm-minus-fn]
> fsm *fsm_minus(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:fomalib.fsm-minus-fn]
> Difference net1 - net2 by a parallel product with a dead sentinel for net2. Minimizes both
> inputs, merges sigmas (fsm_merge_sigma), recounts. Explores pairs (a, b) from (0,0) with the
> int worklist stack (cleared first) and a pair hash; pairs are stored 1-based, with b == 0
> encoding "net2 is dead" (has fallen off). A pair is start iff a == 0 and b == 0 (both live
> initial); final iff a is final and (b is dead or b is non-final). For each arc of state a
> (all of a's arcs are kept, labels unchanged; no epsilon-closure — labels must match
> exactly): b's arc with the identical (in,out) label, if any, determines the target pair
> (a.target, b.target), otherwise the target pair is (a.target, dead); unseen pairs get new
> state numbers and are pushed. The new state array replaces net1's (net1's struct and merged
> sigma are reused); net2 is destroyed; returns fsm_minimize of the result.

> [spec:foma:def:fomalib.fsm-network-to-char-fn]
> FEXPORT char *fsm_network_to_char(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-network-to-char-fn]
> Returns a freshly strdup'ed copy of the LAST symbol in the net's sigma list — the
> highest-numbered symbol, as sigma is kept sorted by number — or NULL if the first sigma
> entry has number -1 (the empty-sigma placeholder). Dereferences net->sigma unconditionally
> (crashes on a NULL sigma). The net is unmodified; the caller owns and must free the string.
> Used to fetch the symbol of a single-symbol machine.

> [spec:foma:def:fomalib.fsm-optionality-fn]
> fsm *fsm_optionality(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-optionality-fn]
> (A) — optional A: literally returns fsm_union(net, fsm_empty_string()), the union of net
> with the empty-string language. net is consumed by the union; returns the new net. (Routed
> through the shared kleene-closure helper with the OPTIONALITY operation, which short-circuits
> to this union before any of the star/plus construction runs.)

> [spec:foma:def:fomalib.fsm-options]
> typedef enum

> [spec:foma:def:fomalib.fsm-parse-regex-fn]
> fsm *fsm_parse_regex(char *regex, struct defined_networks *defined_nets, struct defined_functions *defined_funcs)

> [spec:foma:sem:fomalib.fsm-parse-regex-fn]
> Compiles a foma regular expression to a network. Clears the global current_parse to NULL,
> allocates a copy of `regex` with ";" appended (the parser's statement terminator), and runs
> the reentrant regex parser (my_yyparse) at line 1 with the given defined-network and
> defined-function tables (used to resolve named definitions; may be NULL). On success (parser
> returns 0): frees the copy and returns fsm_minimize(current_parse) — the net the parser
> deposited in the global. On failure: frees the copy and returns NULL (syntax errors are
> printed to stderr by the parser). The input string is not modified or freed; nested/recursive
> parses are supported via a global parser-state stack (depth-limited).

> [spec:foma:def:fomalib.fsm-precedes-fn]
> fsm *fsm_precedes(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:fomalib.fsm-precedes-fn]
> Mirror of `[spec:foma:sem:fomalib.fsm-follows-fn]`: returns ~$[net2 ?* net1], built literally
> as fsm_complement(fsm_minimize(fsm_contains(fsm_minimize(fsm_concat(fsm_minimize(
> fsm_copy(net2)), fsm_concat(fsm_universal(), fsm_minimize(fsm_copy(net1)))))))) — the
> strings containing no occurrence of net2 followed (anywhere later) by an occurrence of net1.
> Only copies are consumed; net1 and net2 are NOT destroyed (callers assuming the usual
> consuming convention leak them).

> [spec:foma:def:fomalib.fsm-priority-union-lower-fn]
> fsm *fsm_priority_union_lower(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:fomalib.fsm-priority-union-lower-fn]
> Lower-side priority union A .p. B = A | [B .o. ~[A.l]]: literally ret =
> fsm_union(fsm_copy(net1), fsm_compose(net2, fsm_complement(fsm_lower(fsm_copy(net1)))));
> then fsm_destroy(net1). B contributes only pairs whose LOWER string is outside A's lower
> language (A's mappings win on shared lower strings). net2 is consumed by the compose; net1
> is copied twice, then destroyed. Returns the new net.

> [spec:foma:def:fomalib.fsm-priority-union-upper-fn]
> fsm *fsm_priority_union_upper(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:fomalib.fsm-priority-union-upper-fn]
> Upper-side priority union A .P. B = A | [~[A.u] .o. B]: literally ret =
> fsm_union(fsm_copy(net1), fsm_compose(fsm_complement(fsm_upper(fsm_copy(net1))), net2));
> then fsm_destroy(net1). B contributes only pairs whose UPPER string is outside A's upper
> language (A's mappings take priority). net2 is consumed by the compose; net1 is copied
> twice, then destroyed. Returns the new net.

> [spec:foma:def:fomalib.fsm-quantifier-fn]
> fsm *fsm_quantifier(char *string)

> [spec:foma:sem:fomalib.fsm-quantifier-fn]
> Base language for a quantifier variable: strings containing exactly two occurrences of the
> symbol `string` — \x* x \x* x \x* with x = fsm_symbol(string) and \x = fsm_term_negation(x)
> (any single symbol other than x), assembled literally with fsm_concat and fsm_kleene_star
> (a fresh fsm_symbol/fsm_term_negation net is built for each of the five factors). `string`
> is only read; returns a new net.

> [spec:foma:def:fomalib.fsm-quotient-interleave-fn]
> fsm *fsm_quotient_interleave(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:fomalib.fsm-quotient-interleave-fn]
> Interleaving quotient A/\/B: the set of strings that can be interleaved into (a string of) B
> to yield a string of A. Built with the marker symbol "@>@" (call it x) as
> [B/[x \x* x] & A/x .o. [[\x]:0* (x:0 \x* x:0)]*].l — literally:
> fsm_lower(fsm_compose(fsm_intersect(fsm_ignore(net2, concat(symbol(x),
> concat(kleene_star(term_negation(symbol(x))), symbol(x))), OP_IGNORE_ALL),
> fsm_ignore(net1, symbol(x), OP_IGNORE_ALL)),
> fsm_kleene_star(concat(kleene_star(cross_product(term_negation(symbol(x)), empty_string())),
> optionality(concat(cross_product(symbol(x), empty_string()),
> concat(kleene_star(term_negation(symbol(x))), cross_product(symbol(x), empty_string())))))))).
> B is decorated with marker-delimited insertion slots, A with free markers; the intersection
> keeps consistent decorations and the final composition/projection deletes everything except
> the material between marker pairs. Afterwards "@>@" is removed from the result's sigma
> (sigma_remove; no other sigma cleanup). Both inputs are consumed by the constructors; returns
> the new net.

> [spec:foma:def:fomalib.fsm-quotient-left-fn]
> fsm *fsm_quotient_left(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:fomalib.fsm-quotient-left-fn]
> Left quotient A\\\B: the set of suffixes one can append to a string of A to obtain a string
> of B. Literally [B .o. A:0 ?*].l — fsm_lower(fsm_compose(net2,
> fsm_concat(fsm_cross_product(net1, fsm_empty_string()), fsm_universal()))): the composed
> transducer deletes a leading A-string and keeps the remainder; the lower projection yields
> the suffix language. Both inputs are consumed by the constructors; returns the new net (not
> minimized).

> [spec:foma:def:fomalib.fsm-quotient-right-fn]
> fsm *fsm_quotient_right(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:fomalib.fsm-quotient-right-fn]
> Right quotient A///B: the set of prefixes one can prepend to a string of B to obtain a
> string of A. Literally [A .o. ?* B:0].l — fsm_lower(fsm_compose(net1,
> fsm_concat(fsm_universal(), fsm_cross_product(net2, fsm_empty_string())))): the composition
> keeps an arbitrary prefix and deletes a trailing B-string; the lower projection yields the
> prefix language. Both inputs are consumed by the constructors; returns the new net (not
> minimized).

> [spec:foma:def:fomalib.fsm-read-binary-file-fn]
> fsm *fsm_read_binary_file(char *filename)

> [spec:foma:sem:fomalib.fsm-read-binary-file-fn]
> Loads the first network from a foma binary file. The file is zlib/gzip-compressed text (but
> uncompressed files read fine, since gzread passes them through). Step 1: slurp the whole
> uncompressed contents into a NUL-terminated malloc'd buffer: the size is taken from the last
> 4 bytes of the file (little-endian uncompressed size, gzip trailer — only size mod 2^32) if
> the file is gzip, else the plain byte size; returns NULL if the file can't be opened or the
> size is 0. Step 2: parse one network from the buffer, line by line ('\n'-separated; each
> line is copied into a 4096-byte stack buffer with no bounds check — longer lines overflow).
> Format, in order: line "##foma-net 1.0##" (exactly, else error/NULL); line "##props##"; a
> props line scanned as "%i %i %i %i %i %lld %i %i %i %i %i %i %s" = arity arccount statecount
> linecount finalcount pathcount is_deterministic is_pruned is_minimized is_epsilon_free
> is_loop_free extras name — extras bit-packs is_completed (bits 0-1), arcs_sorted_in (bits
> 2-3), arcs_sorted_out (bits 4-5); name is copied into net->name (strncpy, 40 bytes) and also
> strdup'd (and leaked by this function). Then lines are skipped until "##sigma##" (an empty
> line first is a format error → NULL). Sigma lines are "number symbol" split at the first
> space; an empty symbol part means the symbol is a literal newline "\n"; each is appended to
> the sigma list verbatim (no sorting). A line starting with '#' ends the section and must be
> "##states##". State lines are space-separated ints with 2-5 fields filling a
> linecount-sized fsm_state array (the count INCLUDES the sentinel line): 2 fields = in target
> (state_no/final repeat the last explicit ones, out = in); 3 = in out target; 4 = state_no in
> target final (out = in); 5 = state_no in out target final; 4/5-field lines update the
> "last state"/"last final" memory (initially -1 / '1'). Each line's start_state is 1 if the
> remembered state is 0, 0 if > 0, -1 if -1 (the "-1 -1 -1 -1 -1" sentinel). A '#' line ends
> the section; if it is "##cmatrix##", a confusion matrix of (sigma_max+1)^2 ints (one per
> line, row-major) is allocated and filled until the next '#' line. The final line must be
> "##end##", else error/NULL (partial nets leak on mid-parse errors). Returns the parsed net;
> the buffer is freed. Several networks may be concatenated in a file; this reads the first.

> [spec:foma:def:fomalib.fsm-read-binary-file-multiple-fn]
> fsm *fsm_read_binary_file_multiple(fsm_read_binary_handle fsrh)

> [spec:foma:sem:fomalib.fsm-read-binary-file-multiple-fn]
> Iterator over a multi-network foma binary file. fsrh is an opaque handle from
> fsm_read_binary_file_multiple_init(filename), which slurps the whole (possibly gzipped) file
> into memory with a read cursor. Each call parses the next concatenated network from the
> buffer using the same wire format as `[spec:foma:sem:fomalib.fsm-read-binary-file-fn]` and
> returns it (the parsed net name string is freed, not leaked). When parsing fails or the
> buffer is exhausted, returns NULL and frees the buffer and handle — the handle must not be
> used again after a NULL return, and there is no way to abandon iteration early without
> leaking the handle.

> [spec:foma:def:fomalib.fsm-read-done-fn]
> FEXPORT void fsm_read_done(struct fsm_read_handle *handle)

> [spec:foma:sem:fomalib.fsm-read-done-fn]
> Disposes of a read handle: frees its lookuptable, sigma list, finals array, initials array,
> states_head array, and the handle struct itself. The underlying net is NOT destroyed — the
> handle never owned it. No NULL check: passing NULL crashes.

> [spec:foma:def:fomalib.fsm-read-handle]
> struct fsm_read_handle {
>   struct fsm_state *arcs_head;
>   struct fsm_state **states_head;
>   struct fsm_state *arcs_cursor;
>   int *finals_head;
>   int *finals_cursor;
>   struct fsm_state **states_cursor;
>   int *initials_head;
>   int *initials_cursor;
>   int current_state;
>   struct fsm_sigma_list *fsm_sigma_list;
>   int sigma_list_size;
>   struct fsm *net;
>   unsigned char *lookuptable;
>   _Bool has_unknowns;
> }

> [spec:foma:def:fomalib.fsm-read-init-fn]
> fsm_read_handle *fsm_read_init(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-read-init-fn]
> Creates an iteration handle over an existing net (net is borrowed, not owned, and must
> already have an accurate statecount — fsm_count is NOT called). NULL net → NULL. One pass
> over the state array builds: a calloc'd per-state byte lookuptable where bit 0 = initial and
> bit 1 = final (set from the first line seen for each state; also used to count distinct
> initials/finals); a states_head array (statecount+1 pointers) mapping each state number to
> its first line (set whenever state_no differs from the previous line's); and has_unknowns,
> set if any arc label (in or out) is UNKNOWN (1) or IDENTITY (2). Then initials_head and
> finals_head are allocated as -1-terminated int arrays listing initial/final state numbers in
> ascending order (scan of the lookuptable). The handle also stores the sigma as an indexable
> symbol list (sigma_to_list), sigma_list_size = sigma_max+1, arcs_head = net->states, and the
> net pointer; all iteration cursors start NULL (handle is calloc'd). Freed later by
> fsm_read_done.

> [spec:foma:def:fomalib.fsm-read-is-final-fn]
> FEXPORT int fsm_read_is_final(struct fsm_read_handle *h, int state)

> [spec:foma:sem:fomalib.fsm-read-is-final-fn]
> Returns h->lookuptable[state] & 2 — i.e. 2 if the state was final when the handle was built,
> 0 otherwise (truthy, not normalized to 1). No bounds or NULL checks; state must be in
> 0..statecount-1.

> [spec:foma:def:fomalib.fsm-read-is-initial-fn]
> FEXPORT int fsm_read_is_initial(struct fsm_read_handle *h, int state)

> [spec:foma:sem:fomalib.fsm-read-is-initial-fn]
> Returns h->lookuptable[state] & 1 — 1 if the state carried the start flag when the handle
> was built, 0 otherwise. No bounds or NULL checks.

> [spec:foma:def:fomalib.fsm-read-prolog-fn]
> fsm *fsm_read_prolog(char *filename)

> [spec:foma:sem:fomalib.fsm-read-prolog-fn+1]
> Reads a network from a prolog-fact text file (the format written by foma's prolog writer).
> Returns NULL if the file can't be opened or contains no "network(" line. Lines are read
> into a 1024-byte buffer (fgets, so longer lines are silently split) and dispatched by
> prefix, parsed by naive substring scanning (no unescaping of \" inside quotes; a symbol
> containing "). breaks the parse). The C source's substring lookups were unchecked (a missing
> delimiter, or a fact before the first "network(" clause, NULL-derefs); on any missing
> delimiter or absent net handle print "File format error in prolog file.\n" and return NULL
> instead of crashing. "network(NAME)." starts the construction (a second
> network( line prints a warning and stops reading — only the first net is returned);
> "final(N, S)." marks state atoi of the text after the first space final; "symbol(N, \"S\")."
> adds symbol S to the sigma if not present ("%0" is unescaped to "0") — this preserves
> symbols unused by any arc; "arc(N, SRC, TGT, \"IN\")." or "arc(N, SRC, TGT,
> \"IN\":\"OUT\")." adds an arc — arity is 2 iff the line contains "\":\"" and does not
> contain the literal label ", \":\").". Label unescaping, in order: arity-1 "?" →
> "@_IDENTITY_SYMBOL_@"; arity-2 "?" (either side) → "@_UNKNOWN_SYMBOL_@"; "0" →
> "@_EPSILON_SYMBOL_@"; "%0" → "0"; "%?" → "?". Arity-1 arcs are added as IN:IN. Initial
> state is 0; the built net is topologically sorted (fsm_topsort, computing pathcount) and
> returned.

> [spec:foma:def:fomalib.fsm-read-reset-fn]
> FEXPORT void fsm_read_reset(struct fsm_read_handle *handle)

> [spec:foma:sem:fomalib.fsm-read-reset-fn]
> Rewinds all four iterators of a read handle by setting arcs_cursor, initials_cursor,
> finals_cursor and states_cursor to NULL (the "not started" sentinel used by the
> fsm_get_next_* functions). NULL handle is tolerated (no-op).

> [spec:foma:def:fomalib.fsm-read-spaced-text-file-fn]
> fsm *fsm_read_spaced_text_file(char *filename)

> [spec:foma:sem:fomalib.fsm-read-spaced-text-file-fn]
> Builds a network from a text file of space-separated symbol lines using the trie API
> (`[spec:foma:sem:fomalib.fsm-trie-init-fn]` etc.). Reads the whole file with file_to_mem
> (NULL on open error or BOM). Loop: skip blank lines, take one line t1; if the immediately
> following line t2 is missing or empty, t1 is a one-tape word — each space-separated token
> becomes a sym:sym trie step, where token "0" means EPSILON:EPSILON and "%0" means the
> literal symbol "0"; otherwise (t1, t2) is an upper/lower pair — tokens are consumed
> pairwise from both lines, the shorter line padding with "@_EPSILON_SYMBOL_@" ("0" → epsilon,
> "%0" → literal "0" on each side independently), each pair becoming one in:out trie step.
> After each word/pair, fsm_trie_end_word. Frees the buffer and returns fsm_trie_done (the
> determinized-by-construction trie net). Duplicate words merge; the result is an acyclic
> trie-shaped machine.

> [spec:foma:def:fomalib.fsm-read-text-file-fn]
> fsm *fsm_read_text_file(char *filename)

> [spec:foma:sem:fomalib.fsm-read-text-file-fn]
> Builds an acceptor from a word list: reads the whole file with file_to_mem (NULL on open
> error or BOM), splits it destructively at every '\n', and for each nonempty line calls
> fsm_trie_add_word (`[spec:foma:sem:fomalib.fsm-trie-add-word-fn]`) — each word contributes a
> path of sym:sym arcs, one per UTF-8 character. A final line without trailing newline is
> included. Frees the buffer and returns fsm_trie_done(th), the trie of all words.

> [spec:foma:def:fomalib.fsm-reverse-fn]
> fsm *fsm_reverse(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-reverse-fn]
> Reversal A.r via read/construct handles. The output keeps net's name and a copy of its
> sigma. All original state numbers are shifted up by 1 and a fresh initial state 0 is added.
> Every arc s→t (in:out) becomes t+1 → s+1 with the same numeric labels; for every final
> state f of the input, an EPSILON:EPSILON arc 0 → f+1 is added; every initial state i of the
> input becomes final as i+1; state 0 is the only initial state (and is not final, so the
> empty string is accepted only if the input accepted it). The result has is_deterministic =
> 0 and is_epsilon_free = 0. Destroys the input net; returns the new net.

> [spec:foma:def:fomalib.fsm-rewrite-fn]
> fsm *fsm_rewrite(struct rewrite_set *all_rules)

> [spec:foma:sem:fomalib.fsm-rewrite-fn]
> Compiles a whole set of (possibly parallel) rewrite rules into one transducer, using a
> flattened 4-tape string encoding: each tape cell of a candidate derivation is 4 consecutive
> symbols — tape 1: position type (@O@ outside a rewrite; @I[@ / @I@ / @I]@ / @I[]@ begin /
> inside / end / begin+end of a rewrite); tape 2: the rule-number symbol "@#%04i@" (or @0@
> outside); tape 3: input symbol or @0@; tape 4: output symbol, @0@, or @ID@ (= "repeat tape
> 3"). Every 4-tape word is delimited by the Boundary word @O@ @0@ @#@ @ID@ on each end.
> Steps: (1) count all rules across the ruleset list, allocate a batch struct holding the
> rule-name strings, ISyms = the union of the four @I...@ symbols, Rulenames = union of all
> rule-name symbols, and ANY = fsm_identity(); add all special symbols (@0@ @O@ @I@ @I[@ @I[]@
> @I]@ @ID@ @#@) plus every rule-name symbol to the sigma of every rule part and context net
> (in place). (2) For each rule build its flattened center cross-product CP — rewrite_cp(L,R)
> for plain rules, rewrite_cp_transducer for T(x)-type rules (whose left field is then split:
> right := lower projection, left := upper projection), rewrite_cp_markup(L,R1,R2) for A -> B
> ... C markup rules — store a copy in rules->cross_product and union everything into RuleCP.
> (3) Base = Boundary [RuleCP | Outside]* Boundary, with Outside = @O@ @0@ ANY @ID@. (4)
> Compile each context pair into the flattened domain according to the ruleset's direction:
> cpleft/cpright = rewrite_upper for OP_UPWARD_REPLACE (both), rewrite_lower for
> OP_DOWNWARD_REPLACE (both), mixed for OP_RIGHTWARD (lower left/upper right) and OP_LEFTWARD
> (upper left/lower right), rewrite_two_level for OP_TWO_LEVEL_REPLACE. (5) Per rule, in
> order: for dotted rules ([..] epsilon-insertion) intersect Base with constraints allowing
> the empty center only adjacent to an "extension point"; if the ruleset has contexts,
> intersect Base with the context-restriction language of the rule's cross_product; build the
> violation language C as the union of — unrewritten nonempty left side (obligatory ->-type),
> unrewritten nonempty right side (obligatory <--type), not-leftmost/not-longest matches
> (ARROW_LONGEST_MATCH, on upper for ->, lower for <-), not-leftmost/not-shortest
> (ARROW_SHORTEST_MATCH likewise); for context-free rulesets subtract contains(C) from Base
> (obligatory dotted rules instead subtract contains(Epextend Epextend)); for each context
> subtract contains(cpleft C cpright) (obligatory dotted rules subtract a left/right-extended
> variant). (6) Decode: Base = lower side of Base composed with the regex [?:0]^4 [?:0 ?:0 ?
> ?]* [?:0]^4 (keeps only tapes 3-4 and strips the boundary blocks), then fsm_unflatten(Base,
> "@0@", "@ID@") pairs consecutive symbols into transducer arcs. (7) Remove all special and
> rule-name symbols from the sigma, fsm_compact + sigma_sort, free the batch nets, return
> Base. The rule/context nets inside all_rules gain new cross_product/cpleft/cpright members
> but all_rules itself is not freed.

> [spec:foma:def:fomalib.fsm-sequentialize-fn]
> fsm *fsm_sequentialize(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-sequentialize-fn]
> Unimplemented stub: prints "Implementation pending\n" to stdout and returns net unchanged.
> No sequentialization is performed; the port need only preserve (or explicitly reject) this
> placeholder behavior.

> [spec:foma:def:fomalib.fsm-set-option-fn]
> FEXPORT _Bool fsm_set_option(unsigned long long option, void *value)

> [spec:foma:sem:fomalib.fsm-set-option-fn]
> Sets a global library option. The only recognized option is
> FSMO_SKIP_WORD_BOUNDARY_MARKER (enum value 0): value is dereferenced as a _Bool* and copied
> into the global fsm_options.skip_word_boundary_marker (consulted by fsm_merge_sigma to
> suppress ".#." propagation), returning 1. Any other option value returns 0 without touching
> anything.

> [spec:foma:def:fomalib.fsm-shuffle-fn]
> fsm *fsm_shuffle(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:fomalib.fsm-shuffle-fn]
> Shuffle (interleaving) product A ∥ B: at each step either machine moves while the other
> stays. Calls fsm_minimize on both inputs but DISCARDS the returned pointers (latent
> use-after-free if minimization rebuilds the net — document and preserve or fix
> deliberately), then fsm_merge_sigma and fsm_count. Explores pairs (a,b) from (0,0) using
> the shared int-stack worklist and pair hash: pair state is start iff both components are
> start, final iff both are final; for every arc a→a' (in:out) add an arc to pair (a',b), and
> for every arc b→b' add an arc to (a,b'), numbering and pushing unseen pairs. The new state
> array is built with the fsm_state_* builder and replaces net1's (net1's struct and merged
> sigma reused); net2 is destroyed. Returns net1. No flags or counts are updated after the
> rebuild (statecount etc. come from the builder; is_* flags are stale from minimization).

> [spec:foma:def:fomalib.fsm-sigma-destroy-fn]
> FEXPORT int fsm_sigma_destroy(struct sigma *sigma)

> [spec:foma:sem:fomalib.fsm-sigma-destroy-fn]
> Frees an entire sigma linked list: for each node, frees the symbol string if non-NULL (and
> NULLs it) then frees the node. NULL input is a safe no-op. Always returns 1.

> [spec:foma:def:fomalib.fsm-sigma-net-fn]
> fsm *fsm_sigma_net(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-sigma-net-fn]
> Replaces net (in place, reusing its struct and sigma) with the two-state machine accepting
> exactly the single-symbol strings of its alphabet. If sigma_size is 0, destroys net and
> returns fsm_empty_set(). Otherwise builds (fsm_state_* builder): state 0 = initial,
> non-final, with one s:s arc to state 1 for every sigma entry whose number is >= 3 or ==
> IDENTITY (2) — EPSILON (0) and UNKNOWN (1) are skipped; the IDENTITY arc makes the language
> include "?"; state 1 = final, arcless. Old state array freed. Sets is_minimized = YES,
> is_loop_free = YES, pathcount = number of arcs added, then sigma_cleanup(net, 1) (forced:
> drops sigma symbols >= 3 not used on any arc and renumbers densely — here all are used, but
> stray EPSILON/UNKNOWN entries stay in sigma). Returns net.

> [spec:foma:def:fomalib.fsm-sigma-pairs-net-fn]
> fsm *fsm_sigma_pairs_net(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-sigma-pairs-net-fn]
> Builds (in place) the two-state machine whose arcs are the distinct label pairs actually
> attested on net's arcs: scans all lines with target != -1, and for each (in,out) pair not
> yet seen (deduplicated via a calloc'd (sigma_max+1)^2 byte matrix indexed in*smax+out) adds
> one in:out arc from initial state 0 to final state 1. Special numbers (EPSILON, UNKNOWN,
> IDENTITY) are kept as-is. If the net had no arcs at all, destroys it and returns
> fsm_empty_set(). Otherwise frees the old state array, sets is_minimized = YES, is_loop_free
> = YES, pathcount = number of distinct pairs, runs sigma_cleanup(net, 1) (drop unused
> ordinary symbols, renumber densely), and returns net.

> [spec:foma:def:fomalib.fsm-simple-replace-fn]
> fsm *fsm_simple_replace(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:fomalib.fsm-simple-replace-fn]
> Unconditional replacement A -> B, built purely from combinators as
> [~[?* [A & ?+] ?*] [A .x. B]]* ~[?* [A & ?+] ?*]: alternate blocks that contain no nonempty
> A-substring with cross-product rewrites of A into B. Literally: UPlus =
> minimize(kleene_plus(identity)); NotContainA = complement(concat(concat(universal,
> minimize(intersect(copy(A), copy(UPlus)))), universal)) (the A & ?+ intersection excludes
> the empty string so containment is of nonempty A only); result =
> concat(minimize(kleene_star(minimize(concat(NotContainA,
> minimize(cross_product(copy(A), copy(B))))))), minimize(complement(...same NotContainA
> built a second time...))). Destroys net1, net2 and UPlus; returns the new net (outer concat
> not minimized).

> [spec:foma:def:fomalib.fsm-sort-arcs-fn]
> FEXPORT void fsm_sort_arcs(struct fsm *net, int direction)

> [spec:foma:sem:fomalib.fsm-sort-arcs-fn]
> In-place qsort of each state's contiguous block of arc lines by numeric label: direction 1
> sorts by the in field, direction 2 by the out field (comparator returns a->field -
> b->field). Blocks are found in one scan: a block ends when the next line has a different
> state_no or the current line is an arcless dummy (target == -1, which is excluded from
> sorting); only blocks with more than one arc are sorted. Then sets the sortedness flags: if
> net->arity == 1, both arcs_sorted_in and arcs_sorted_out = 1 (in == out for automata);
> otherwise direction 1 sets arcs_sorted_in = 1 / arcs_sorted_out = 0, direction 2 the
> converse. No return value; sigma, counts and other flags untouched.

> [spec:foma:def:fomalib.fsm-state]
> struct fsm_state {
>   int state_no;
>   short int in;
>   short int out;
>   int target;
>   char final_state;
>   char start_state;
> }

> [spec:foma:def:fomalib.fsm-substitute-label-fn]
> fsm *fsm_substitute_label(struct fsm *net, char *original, struct fsm *substitute)

> [spec:foma:sem:fomalib.fsm-substitute-label-fn]
> Splices the network `substitute` in place of every arc of net labeled with symbol
> `original`. First fsm_merge_sigma(net, substitute) (both sigmas mutated in place); fresh
> state numbers start at addstate1 = net->statecount. Opens read handles on both nets and
> resolves repsym = the numeric sigma index of `original` in the merged sigma; if absent,
> returns net unchanged (the substitute read handle leaks and substitute is not destroyed).
> Otherwise builds the output (net's name, net's sigma copied) by iterating net's arcs: (a)
> arc repsym:repsym — emit source→addstate1 EPSILON:EPSILON, copy every substitute arc
> shifted by addstate1 (labels added by symbol string), then for each final state f of
> substitute emit addstate1+f→(the spliced arc's target) EPSILON:EPSILON; addstate1 +=
> substitute->statecount. (b) arc with repsym on exactly one side — build subnet2 =
> minimize(cross_product(copy(substitute), fsm_symbol(out-symbol-string))) when in == repsym,
> or minimize(cross_product(fsm_symbol(in-symbol-string), copy(substitute))) when out ==
> repsym, and splice subnet2 the same way (entry epsilon arc, shifted copy, exit epsilon arcs
> from its finals to the arc's target); addstate1 += subnet2->statecount; destroy subnet2.
> (c) any other arc is copied verbatim by numeric labels. Finals and initials of net are
> copied through. Returns the newly constructed net (contains epsilons, not minimized).
> Neither net nor substitute is destroyed — the caller still owns both (with merged sigmas).

> [spec:foma:def:fomalib.fsm-substitute-symbol-fn]
> fsm *fsm_substitute_symbol(struct fsm *net, char *original, char *substitute)

> [spec:foma:sem:fomalib.fsm-substitute-symbol-fn]
> Renames (or epsilon-removes) a symbol, in place. If original and substitute are equal
> strings, or original is not in net's sigma, returns net unchanged. Determines the
> replacement number s: the literal string "0" means EPSILON (0); otherwise s = the symbol's
> existing sigma number, adding it to sigma if absent. Rewrites every line's in and out that
> equal original's number o to s, removes original from the sigma, and sigma_sort(net)
> (renumbers ordinary symbols densely in sorted order and remaps all arc labels accordingly).
> Clears all is_* flags (fsm_update_flags all NO), runs sigma_cleanup(net, 0) (no-op when
> UNKNOWN/IDENTITY present), sets is_minimized = NO, and returns fsm_determinize(net) —
> needed because substituting to epsilon or onto an existing symbol can introduce
> nondeterminism. Consumes net; may return a different pointer.

> [spec:foma:def:fomalib.fsm-symbol-fn]
> fsm *fsm_symbol(char *symbol)

> [spec:foma:sem:fomalib.fsm-symbol-fn]
> Builds the single-symbol machine for a symbol string. For "@_EPSILON_SYMBOL_@": a one-state
> machine (state 0 final+initial, one arcless dummy line then the sentinel), EPSILON (0) added
> to sigma, counts arccount 0 / statecount 1 / linecount 1 / finalcount 1, and
> is_deterministic, is_minimized and is_epsilon_free forced to NO (the empty-string machine is
> treated as epsilon-containing). Otherwise: the symbol number is IDENTITY (2) for
> "@_IDENTITY_SYMBOL_@", else sigma_add of the string (first ordinary symbol gets 3); two
> states — 0 (initial) --sym:sym--> 1 (final) — with counts arity 1, pathcount 1, arccount 1,
> statecount 2, linecount 2, finalcount 1, arcs_sorted_in/out YES, and
> deterministic/minimized/epsilon-free YES. In both cases the net is freshly created (name
> ""). Note "@_UNKNOWN_SYMBOL_@" is NOT special-cased: it is added as an ordinary symbol.

> [spec:foma:def:fomalib.fsm-symbol-occurs-fn]
> FEXPORT int fsm_symbol_occurs(struct fsm *net, char *symbol, int side)

> [spec:foma:sem:fomalib.fsm-symbol-occurs-fn]
> Tests whether a symbol is actually used on an arc. Resolves the symbol string to its sigma
> number (0 = "does not occur" if the symbol isn't in sigma at all), then scans all lines:
> returns 1 at the first line whose in matches (side == M_UPPER, 1), whose out matches (side
> == M_LOWER, 2), or where either side matches (side == M_UPPER+M_LOWER, 3). Returns 0
> otherwise — including for any other side value. Net is not modified. (Arcless dummy lines
> have in/out == -1 and can never match.)

> [spec:foma:def:fomalib.fsm-term-negation-fn]
> fsm *fsm_term_negation(struct fsm *net1)

> [spec:foma:sem:fomalib.fsm-term-negation-fn]
> Term negation \A — the single-symbol strings not in A: literally
> fsm_intersect(fsm_identity(), fsm_complement(net1)), where fsm_identity() is the fresh
> two-state ?-machine (one IDENTITY:IDENTITY arc). net1 is consumed by the complement;
> returns the new net.

> [spec:foma:def:fomalib.fsm-topsort-fn]
> fsm *fsm_topsort(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-topsort-fn+1]
> Topologically sorts an acyclic net's states (renumbering them in topological order) and
> counts its paths; detects cycles. NULL → NULL. After fsm_count: one pass computes each
> state's in-degree (invcount) and each state's first line index; a self-loop found here
> immediately marks the net cyclic. Invcount is a plain `i32` (was an UNSIGNED
> SHORT that silently overflowed past 65535 incoming arcs) — a latent overflow widened per
> the conventions; unchanged for ≤65535 in-arcs into any single state.
> Kahn's algorithm on the shared int stack (cleared first), seeded with state 0 only and
> pathcount[0] = 1: pop a state, mark it treated, record it as the i-th state in topological
> `order` and newnum[state] = i; for each of its arcs, decrement the target's invcount, add
> the source's pathcount into the target's (a negative sum sets a sticky overflow flag), and
> if the target is already treated the net is cyclic; push targets whose invcount reaches 0.
> If any state was never treated, the net is cyclic. Cyclic outcome: set pathcount =
> PATHCOUNT_CYCLIC (-1) and is_loop_free = 0, leave the state array UNCHANGED, and return
> net. Acyclic outcome: build a new state array by visiting states in topological order,
> copying each state's lines with state_no and target mapped through newnum (arcless dummy
> lines kept, -1 targets preserved), appending the sentinel; sum pathcount over final states
> into net->pathcount (PATHCOUNT_OVERFLOW, -2, if the overflow flag tripped), set is_loop_free
> = 1, free the old array and return net (same struct, in place). All scratch arrays are
> freed; the int stack is cleared on exit.

> [spec:foma:def:fomalib.fsm-trie-add-word-fn]
> FEXPORT void fsm_trie_add_word(struct fsm_trie_handle *th, char *word)

> [spec:foma:sem:fomalib.fsm-trie-add-word-fn]
> Adds a word to a trie as an identity path: segments word into UTF-8 characters (utf8skip
> gives continuation-byte count from the lead byte) and calls fsm_trie_symbol(th, c, c) for
> each character (each character is copied into a strdup'd scratch copy of the word, used as
> a staging buffer, freed at the end), then fsm_trie_end_word(th). The loop also guards on a
> byte-length counter, so malformed UTF-8 cannot run past the end. word itself is not
> modified or freed.

> [spec:foma:def:fomalib.fsm-trie-done-fn]
> fsm *fsm_trie_done(struct fsm_trie_handle *th)

> [spec:foma:sem:fomalib.fsm-trie-done-fn]
> Converts a finished trie to a network and frees the trie. Creates a construct handle (net
> name literally "name"); walks all 1,048,573 hash buckets and, for each occupied bucket,
> every chained transition entry, adding an arc sourcestate→targetstate labeled insym:outsym
> (by string, so symbols are added to the output sigma as encountered); an unoccupied bucket
> head (insym NULL) contributes nothing. Marks every trie state 0..used_states with is_final
> == 1 as final; initial state is 0. Then frees all chained hash nodes, the string-intern
> hash (sh_done), the state array, the bucket array, and the handle. Returns the constructed
> net (deterministic by construction since trie insertion never duplicates a
> (source,insym,outsym) transition). An empty trie yields the empty-set machine.

> [spec:foma:def:fomalib.fsm-trie-end-word-fn]
> FEXPORT void fsm_trie_end_word(struct fsm_trie_handle *th)

> [spec:foma:sem:fomalib.fsm-trie-end-word-fn]
> Ends the current word: sets is_final = 1 on the trie state under the cursor
> (trie_states[trie_cursor]) and resets the cursor to the root state 0, ready for the next
> word.

> [spec:foma:def:fomalib.fsm-trie-handle]
> struct fsm_trie_handle {
>   struct trie_states *trie_states;
>   unsigned int trie_cursor;
>   struct trie_hash *trie_hash;
>   unsigned int used_states;
>   unsigned int statesize;
>   struct sh_handle *sh_hash;
> }

> [spec:foma:def:fomalib.fsm-trie-init-fn]
> fsm_trie_handle *fsm_trie_init()

> [spec:foma:sem:fomalib.fsm-trie-init-fn]
> Allocates a trie handle (calloc'd, so used_states = 0 and cursor at root state 0): a
> transition hash table of THASH_TABLESIZE = 1,048,573 calloc'd bucket heads (entries are
> stored in the head when free, else chained), a state array of TRIE_STATESIZE = 32,768
> trie_states (statesize records the capacity; grown on demand), and a string-intern hash
> (sh_init). Returns the handle; the caller finishes with fsm_trie_done.

> [spec:foma:def:fomalib.fsm-trie-symbol-fn]
> FEXPORT void fsm_trie_symbol(struct fsm_trie_handle *th, char *insym, char *outsym)

> [spec:foma:sem:fomalib.fsm-trie-symbol-fn]
> Advances the trie cursor over one insym:outsym transition, creating it if new. Hash h =
> fold of (each byte of insym, then each byte of outsym) as hash = hash*101 + byte, then
> hash*101 + sourcestate, mod 1,048,573 (bytes go through plain char, signed on most
> platforms). If bucket h's head is occupied, walk the chain; an entry matching insym,
> outsym (strcmp) AND sourcestate == cursor means the transition exists: cursor = its
> targetstate, return. Otherwise increment used_states (the new target state number), store
> the transition (source = old cursor, target = used_states, symbols interned via
> sh_find_add_string with value 1) in the bucket head if it was free, else in a fresh
> calloc'd node inserted directly after the head; cursor = used_states. If used_states >=
> statesize, grow the state array to the next power of two above statesize (2^(floor(log2 n)
> + 1), i.e. doubling) via realloc; finally the new state's is_final is set to 0.

> [spec:foma:def:fomalib.fsm-unflatten-fn]
> fsm *fsm_unflatten(struct fsm *net, char *epsilon_sym, char *repeat_sym)

> [spec:foma:sem:fomalib.fsm-unflatten-fn]
> Converts a "flattened" automaton over an even-length alphabet stream into a transducer by
> pairing consecutive arcs: an even-position arc supplies the input symbol, the following
> odd-position arc the output symbol. Calls fsm_minimize(net) DISCARDING the returned pointer
> (latent use-after-free if minimization rebuilds — flag/decide in the port), then fsm_count.
> Looks up the numeric sigma codes of epsilon_sym and repeat_sym (-1 if absent, matching
> nothing). Explores even states from 0 with the shared int stack and pair hash (pairs are
> (s,s), pushed/popped doubled): the pair state inherits final/start from the even state; for
> every arc a→b (skip arcless) and every arc b→c of the odd state b (skip arcless), emit an
> arc to the (renumbered) pair of c with labels in = a-arc's in, out = b-arc's in, mapped as
> follows, in order: if out == repeat_sym's code, out := in; else if either is IDENTITY (2),
> map IDENTITY → UNKNOWN (1) on whichever sides are IDENTITY; then in == epsilon_sym's code →
> EPSILON (0), and likewise out. The new state array (fsm_state_* builder) replaces net's;
> returns net (in place). Counts/flags are whatever the builder and earlier minimize left;
> sigma still contains epsilon_sym/repeat_sym (callers remove them).

> [spec:foma:def:fomalib.fsm-union-fn]
> fsm *fsm_union(struct fsm *net_1, struct fsm *net_2)

> [spec:foma:sem:fomalib.fsm-union-fn]
> Union by epsilon-linking: fsm_merge_sigma(net1, net2), fsm_count both, then build a new
> line array of net1->linecount + net2->linecount + 2 entries: a fresh initial, non-final
> state 0 with EPSILON:EPSILON arcs to offset1 = 1 (net1's old state 0) and offset2 =
> net1->statecount + 1 (net2's old state 0); all net1 lines copied with state_no/target + 1,
> then all net2 lines with state_no/target + offset2, start flags cleared everywhere,
> finality preserved; sentinel appended. Updates statecount (s1+s2+1), linecount, arccount
> (2 + real arcs), finalcount (sum); adds EPSILON to sigma if absent. net1's old array is
> freed and its struct reused (merged sigma kept); net2 is destroyed. Flags: everything NO
> except is_loop_free = UNK. Returns net1 unminimized.

> [spec:foma:def:fomalib.fsm-universal-fn]
> fsm *fsm_universal()

> [spec:foma:sem:fomalib.fsm-universal-fn]
> The universal language ?*: a fresh net (fsm_create "") with a single state 0 that is both
> initial and final and carries one IDENTITY:IDENTITY (2:2) self-loop; sigma = {IDENTITY}.
> Flags deterministic/pruned/minimized/epsilon-free = YES, loop-free and completed = NO;
> counts arccount 1, statecount 1, linecount 2 (arc + sentinel), finalcount 1, pathcount =
> PATHCOUNT_CYCLIC (-1). Returns the new net.

> [spec:foma:def:fomalib.fsm-upper-fn]
> fsm *fsm_upper(struct fsm *net)

> [spec:foma:sem:fomalib.fsm-upper-fn]
> Upper (input-side) projection, in place — exact mirror of
> `[spec:foma:sem:fomalib.fsm-lower-fn]`: rebuilds the state array preserving state
> numbers/finality/start flags, relabeling every real arc to in:in with in == UNKNOWN (1)
> mapped to IDENTITY (2). Old array freed; deterministic/pruned/minimized flags cleared
> (others UNK); sigma_cleanup(net, 0); returns the same net, not determinized.

> [spec:foma:def:fomalib.fsm-write-binary-file-fn]
> FEXPORT int fsm_write_binary_file(struct fsm *net, char *filename)

> [spec:foma:sem:fomalib.fsm-write-binary-file-fn]
> Serializes net to a gzip-compressed text file in the wire format of
> `[spec:foma:sem:fomalib.fsm-read-binary-file-fn]`. Returns 1 if gzopen(filename, "wb") fails,
> else 0 after writing and closing (note: 0 = success). Emission: "##foma-net 1.0##\n",
> "##props##\n", the 13-field props line (extras = is_completed | arcs_sorted_in<<2 |
> arcs_sorted_out<<4; pathcount as %lld; net->name last — a name containing spaces corrupts
> the format since the reader scans %s), "##sigma##\n" then one "number symbol\n" line per
> sigma entry (skipping a leading number == -1 placeholder), "##states##\n" then one line per
> state line using the shortest of the 2/3/4/5-field encodings: a line for a new state_no
> uses 5 fields "state in out target final" when in != out, else 4 fields "state in target
> final"; a continuation line for the same state uses 3 fields "in out target" when in !=
> out, else 2 fields "in target". Then the sentinel line "-1 -1 -1 -1 -1\n". If the net has
> a confusion matrix (medlookup), "##cmatrix##\n" followed by (sigma_max+1)^2 integers, one
> per line, row-major. Finally "##end##\n". net is not modified or destroyed.

> [spec:foma:def:fomalib.fsmcontexts]
> struct fsmcontexts {
>   struct fsm *left;
>   struct fsm *right;
>   struct fsmcontexts *next;
>   struct fsm *cpleft;
>   struct fsm *cpright;
> }

> [spec:foma:def:fomalib.fsmrules]
> struct fsmrules {
>   struct fsm *left;
>   struct fsm *right;
>   struct fsm *right2;
>   struct fsm *cross_product;
>   struct fsmrules *next;
>   int arrow_type;
>   int dotted;
> }

> [spec:foma:def:fomalib.load-defined-fn]
> FEXPORT int load_defined(struct defined_networks *def, char *filename)

> [spec:foma:sem:fomalib.load-defined-fn]
> Loads every network from a (gzipped) multi-net foma binary file into the defined-networks
> list. Prints "Loading definitions from %s." to stdout; if the file can't be slurped
> (io_gz_file_to_mem returns 0) prints "File error." to stderr and returns 0. Otherwise
> repeatedly parses networks (wire format of `[spec:foma:sem:fomalib.fsm-read-binary-file-fn]`)
> until NULL, calling add_defined(def, net, name) for each: an existing definition with the
> same name is replaced (its old net destroyed), otherwise the net is appended (the list's
> dummy head entry filled first); names longer than 40 bytes (FSM_NAME_LEN) are rejected by
> add_defined, silently leaking that net. The strdup'd name returned by the parser is never
> freed (leak per network). Frees the file buffer and returns 1.

> [spec:foma:def:fomalib.medlookup]
> struct medlookup {
>   int *confusion_matrix;
> }

> [spec:foma:def:fomalib.net-print-att-fn]
> FEXPORT int net_print_att(struct fsm *net, FILE *outfile)

> [spec:foma:sem:fomalib.net-print-att-fn+1]
> Writes net to an already-open sink in AT&T tabular format (no weights). Builds an
> indexable symbol array from the sigma; if sigma_max >= 0, slot 0 (EPSILON) is pointed at
> the global g_att_epsilon (settable variable "att-epsilon", default "@0@") — UNKNOWN and
> IDENTITY print as whatever their sigma strings are (normally "@_UNKNOWN_SYMBOL_@" /
> "@_IDENTITY_SYMBOL_@"). Pass 1: for every line with target != -1, print
> "source\ttarget\tinsym\toutsym\n". Pass 2: for the first line of each state block with
> final_state == 1, print "state\n" (each final state exactly once, in state-array order).
> Frees the symbol array and returns `Ok(())` on success, propagating the first write failure as
> its `io::Error` (the C returned a vestigial `1`). net is unmodified; the stream is not closed.

> [spec:foma:def:fomalib.read-att-fn]
> fsm *read_att(char *filename)

> [spec:foma:sem:fomalib.read-att-fn]
> Reads an AT&T-format tab-separated text file into a network. NULL if the file can't be
> opened. Each line (fgets into a 1024-byte buffer — longer lines are split; trailing '\n'
> stripped) is tokenized by strtok on '\t' (consecutive tabs collapse, so empty fields are
> skipped), collecting at most 6 tokens: 4 or more tokens → an arc from atoi(token0) to
> atoi(token1) labeled token2:token3 (token 5, the weight, is ignored); 1-3 tokens → state
> atoi(token0) is final (weights on final-state lines are thus tolerated); empty lines
> skipped. An in/out string equal to the global g_att_epsilon ("@0@" by default) is replaced
> by "@_EPSILON_SYMBOL_@". The construct handle is named after the filename; the initial
> state is 0. After fsm_construct_done: fsm_count, then fsm_topsort (renumbers topologically
> and computes pathcount, or marks the net cyclic) and its result is returned.

> [spec:foma:def:fomalib.remove-defined-fn]
> int remove_defined (struct defined_networks *def, char *string)

> [spec:foma:sem:fomalib.remove-defined-fn]
> Removes a named definition from the defined-networks list, keeping the head node's address
> stable. string == NULL undefines everything: every entry's net is fsm_destroy'ed and name
> freed, but the list NODES are not freed and the name/net fields are left DANGLING (latent
> bug: the list head still points at freed payloads; document literal behavior); returns 0.
> Otherwise finds the entry whose name strcmp-matches; returns 1 if none exists. If the
> match is the head node: with a successor, destroy the head's net/name, move the successor's
> name/net into the head and unlink/free the successor node; without one, destroy the payload
> and NULL out name/net/next (empty list, head preserved). A non-head match has its payload
> destroyed and its node unlinked and freed. Returns 0 on success.

> [spec:foma:def:fomalib.rewrite-set]
> struct rewrite_set {
>   struct fsmrules *rewrite_rules;
>   struct fsmcontexts *rewrite_contexts;
>   struct rewrite_set *next;
>   int rule_direction;
> }

> [spec:foma:def:fomalib.save-defined-fn]
> FEXPORT int save_defined(struct defined_networks *def, char *filename)

> [spec:foma:sem:fomalib.save-defined-fn]
> Writes all defined networks to one gzip file (the format of
> `[spec:foma:sem:fomalib.fsm-write-binary-file-fn]`, networks concatenated). def == NULL →
> "No defined networks." on stderr, return 0. gzopen failure → error printf, return -1.
> Prints "Writing definitions to file %s."; for each list entry: entries with a NULL net are
> skipped with a message; otherwise the definition name is strncpy'd into net->name
> (FSM_NAME_LEN = 40 bytes — a name of exactly 40 bytes leaves the field unterminated, latent
> bug) so the name round-trips through the props line, then the net is serialized. Closes
> the file and returns 1. The nets themselves are not modified otherwise nor destroyed.

> [spec:foma:def:fomalib.save-stack-att-fn]
> FEXPORT int save_stack_att()

> [spec:foma:sem:fomalib.save-stack-att-fn]
> Declared in fomalib.h (FEXPORT prototype) but never defined anywhere in the library or
> tools: any use fails at link time. There is no behavior to port; the port need only decide
> whether to omit the declaration or supply an implementation intentionally.

> [spec:foma:def:fomalib.sh-add-string-fn]
> char *sh_add_string(struct sh_handle *sh, char *string, int value)

> [spec:foma:sem:fomalib.sh-add-string-fn]
> Unconditionally inserts a string into the intern hash (no duplicate check — callers wanting
> dedup use sh_find_add_string). Bucket index = fold hash = hash*101 + byte over the string's
> chars (plain char, so bytes >= 0x80 are typically sign-extended negative — must be
> reproduced for hash compatibility), mod STRING_HASH_SIZE = 8191. If the bucket head's
> string is NULL, the strdup'd string and value are stored in the head; otherwise a new
> malloc'd node is inserted immediately after the head. Returns the interned (strdup'd) copy,
> owned by the hash. sh->lastvalue is NOT updated.

> [spec:foma:def:fomalib.sh-done-fn]
> void sh_done(struct sh_handle *sh)

> [spec:foma:sem:fomalib.sh-done-fn]
> Destroys a string-intern hash: for each of the 8191 buckets, frees the head's string (if
> any), then walks the chain freeing each node's string and the node itself; finally frees
> the bucket array and the handle. All pointers previously returned by
> sh_add_string/sh_find_add_string are invalidated.

> [spec:foma:def:fomalib.sh-find-add-string-fn]
> char *sh_find_add_string(struct sh_handle *sh, char *string, int value)

> [spec:foma:sem:fomalib.sh-find-add-string-fn]
> Find-or-insert: sh_find_string first; if found, returns the existing interned pointer (the
> value argument is IGNORED — an existing entry's value is not updated, though sh->lastvalue
> was just set by the find); on a miss, sh_add_string(string, value) inserts and returns the
> new interned copy (lastvalue NOT set on this path). Equal strings therefore always map to
> one canonical pointer, enabling pointer-comparison by callers.

> [spec:foma:def:fomalib.sh-find-string-fn]
> char *sh_find_string(struct sh_handle *sh, char *string)

> [spec:foma:sem:fomalib.sh-find-string-fn]
> Looks up a string in the intern hash: walks the bucket chain for sh_hashf(string) (see
> `[spec:foma:sem:fomalib.sh-add-string-fn]` for the hash); a node whose stored string is NULL
> (only possible for an unused bucket head) ends the search with NULL; a strcmp match sets
> sh->lastvalue to the entry's stored value (retrievable via sh_get_value) and returns the
> interned string pointer; chain exhaustion returns NULL.

> [spec:foma:def:fomalib.sh-get-value-fn]
> int sh_get_value(struct sh_handle *sh)

> [spec:foma:sem:fomalib.sh-get-value-fn]
> Returns sh->lastvalue: the integer stored with the string most recently found by
> sh_find_string (directly or via sh_find_add_string's hit path). Uninitialized garbage
> before the first successful lookup (the handle is malloc'd and lastvalue never cleared) and
> unchanged by plain insertions.

> [spec:foma:def:fomalib.sh-handle]
> struct sh_handle {
>   struct sh_hashtable *hash;
>   int lastvalue;
> }

> [spec:foma:def:fomalib.sh-hashtable]
> struct sh_hashtable {
>   char *string;
>   int value;
>   struct sh_hashtable *next;
> }

> [spec:foma:def:fomalib.sh-init-fn]
> struct sh_handle *sh_init()

> [spec:foma:sem:fomalib.sh-init-fn]
> Allocates a string-intern hash handle: the handle itself is malloc'd (so lastvalue starts
> uninitialized) and the bucket array is calloc(STRING_HASH_SIZE = 8191) sh_hashtable
> structs — bucket heads are in-line (string NULL = empty), collisions are chained. Returns
> the handle; freed with sh_done.

> [spec:foma:def:fomalib.sigma+1]
> One sigma alphabet entry `{ int number; symbol }`. The alphabet as a whole is
> a `Vec<Sigma>` in insertion order; number < IDENTITY is reserved for special
> symbols and an entry's number is an independent, possibly-sparse id (not the
> Vec index). An empty alphabet is an empty Vec — there is no sentinel node.

> [spec:foma:def:fomalib.sigma-copy-fn]
> sigma *sigma_copy(struct sigma *sigma)

> [spec:foma:sem:fomalib.sigma-copy-fn+1]
> Deep-copies a sigma alphabet: an empty alphabet copies to an empty alphabet; otherwise
> every entry is copied in order (number and symbol). Returns the new alphabet; the source
> is untouched.

> [spec:foma:def:fomalib.trie-hash]
> struct trie_hash {
>   char *insym;
>   char *outsym;
>   unsigned int sourcestate;
>   unsigned int targetstate;
>   struct trie_hash *next;
> }

> [spec:foma:def:fomalib.trie-states]
> struct trie_states {
>   _Bool is_final;
> }


# foma/determinize.c

> [spec:foma:def:determinize.add-fsm-arc-fn]
> extern int add_fsm_arc(struct fsm_state *fsm, int offset, int state_no, int in, int out, int target, int final_state, int start_state)

> [spec:foma:sem:determinize.add-fsm-arc-fn]
> This is only an extern declaration in determinize.c; the definition lives in
> constructions.c (see `[spec:foma:sem:constructions.add-fsm-arc-fn]`). Behavior: write one
> line of the flat fsm_state table at index `offset` with the six given field values
> (state_no, in, out, target, final_state, start_state) and return offset+1. Note that
> despite being declared here, determinize.c never calls it; nothing else is required of a
> port beyond making the shared definition visible.

> [spec:foma:def:determinize.add-t-ptr-fn]
> void add_T_ptr(int setnum, int setsize, unsigned int theset, int fs)

> [spec:foma:sem:determinize.add-t-ptr-fn]
> Registers a newly numbered subset in the file-static `T_ptr` table (indexed by subset
> number) and schedules it for processing. Steps: (1) if setnum >= T_limit, double T_limit,
> realloc T_ptr to the new size, and set `.size = 0` for every entry from index setnum
> through the new T_limit-1 (only the size field is cleared; finalstart/set_offset of the
> grown region are left as realloc garbage — size==0 is the "unused" sentinel). Since
> subset numbers are assigned consecutively, one doubling always suffices. (2) Store into
> T_ptr[setnum]: size = setsize, set_offset = theset (an offset into the global set_table
> pool where the member list lives), finalstart = fs (1 if any member state is final).
> (3) Push setnum onto the global int stack — the LIFO agenda of unprocessed subsets that
> `[spec:foma:sem:determinize.next-unmarked-fn]` pops. Note the function has external linkage
> (not static) even though it is internal to the module.

> [spec:foma:def:determinize.e-closure-fn]
> INLINE static int e_closure(int states)

> [spec:foma:sem:determinize.e-closure-fn]
> Expands the state set held in temp_move[0..states-1] to its epsilon closure in place,
> then canonicalizes it to a subset number via set_lookup. Caller protocol: the caller has
> collected `states` distinct states into temp_move, marked each with
> e_table[s] = v (where v was `mainloop` at collection time), then incremented mainloop to
> v+1 before calling. Steps: (1) if epsilon_symbol == -1 (no epsilon arcs anywhere), skip
> closure entirely and return set_lookup(temp_move, states). (2) If states == 0, return -1
> (the only way -1 is produced). (3) Decrement mainloop (back to v so the caller's e_table
> marks are current). (4) For each initial member s: let n = e_closure_memo[s] (the head
> node of the memoized epsilon graph, see `[spec:foma:sem:determinize.memoize-e-closure-fn]`);
> if n->target == NULL (no epsilon successors) skip; else push n on the global pointer
> stack and run a DFS: pop node n; if marktable[n->state] == mainloop, skip; set
> n->mark = mainloop and marktable[n->state] = mainloop; if e_table[n->state] != mainloop,
> append n->state at temp_move[set_size++] and set e_table[n->state] = mainloop (initial
> members are already marked, so they are not duplicated); if n->target == NULL continue;
> otherwise walk the chain n, n->next, ... and for each chain node c with
> c->target->mark != mainloop, set c->target->mark = mainloop and push c->target. Three
> mark stores cooperate: node->mark set at push time prevents duplicate stack pushes,
> marktable prevents re-expansion at pop time, and e_table governs set membership.
> (5) Increment mainloop back to v+1 and return set_lookup(temp_move, set_size), i.e. the
> canonical subset number (existing or freshly assigned, in which case the subset was also
> pushed on the agenda).

> [spec:foma:def:determinize.e-closure-free-fn]
> static void e_closure_free()

> [spec:foma:sem:determinize.e-closure-free-fn]
> Tears down the epsilon-closure memoization built by
> `[spec:foma:sem:determinize.memoize-e-closure-fn]`. Free marktable; then for each of the
> num_states head nodes, walk its ->next chain and free every chain node (the head nodes
> themselves live inside the e_closure_memo array and are not individually freed); finally
> free the e_closure_memo array. Called from fsm_subset wrapup only when
> epsilon_symbol != -1 (the memo is only built in that case).

> [spec:foma:def:determinize.e-closure-memo]
> struct e_closure_memo {
>   int state;
>   int mark;
>   struct e_closure_memo *target;
>   struct e_closure_memo *next;
> }

> [spec:foma:def:determinize.fsm-determinize-fn]
> struct fsm *fsm_determinize(struct fsm *net)

> [spec:foma:sem:determinize.fsm-determinize-fn]
> Thin wrapper: return fsm_subset(net, SUBSET_DETERMINIZE) (operation code 2). Consumes
> net and returns the same struct fsm pointer mutated in place (its states array is
> rebuilt unless already deterministic, in which case net is returned untouched with its
> flags possibly updated). See `[spec:foma:sem:determinize.fsm-subset-fn]`. Callers throughout
> foma rely on the in-place/consuming semantics.

> [spec:foma:def:determinize.fsm-epsilon-remove-fn]
> struct fsm *fsm_epsilon_remove(struct fsm *net)

> [spec:foma:sem:determinize.fsm-epsilon-remove-fn]
> Thin wrapper: return fsm_subset(net, SUBSET_EPSILON_REMOVE) (operation code 1). Removes
> all (EPSILON:EPSILON) arcs by replacing each move target with the canonical subset for
> its epsilon closure, emitting one arc per distinct target-closure so nondeterminism on
> real symbols is preserved. In-place/consuming like determinize. If the input has no
> epsilon arcs it is returned unmodified with is_epsilon_free = YES.

> [spec:foma:def:determinize.fsm-subset-fn]
> static struct fsm *fsm_subset(struct fsm *net, int operation)

> [spec:foma:sem:determinize.fsm-subset-fn]
> Core subset construction serving three operations: SUBSET_EPSILON_REMOVE (1),
> SUBSET_DETERMINIZE (2), SUBSET_TEST_STAR_FREE (3). Steps: (1) if
> net->is_deterministic == YES and operation != SUBSET_TEST_STAR_FREE, return net
> unchanged. (2) Save operation in file-static `op`; fsm_count(net) to refresh counts;
> num_states = statecount; set file-static flag deterministic = 1. (3) init(net) (scratch
> allocation, sigma_to_pairs, init_trans_array); nhash_init(num_states < 12 ? 6 :
> num_states/2). (4) T = initial_e_closure(net): registers subset 0 = epsilon closure of
> the start-state set (all states for STAR_FREE), fills finals/epsilon_symbol/
> num_start_states/numss and memoizes the epsilon graph; then int_stack_clear() empties
> the agenda (subset 0 was pushed there by insertion but is instead used directly as the
> loop seed). (5) Already-deterministic shortcut: if deterministic is still 1 (no state
> had two arcs on the same composite symbol and no epsilon fanout was memoized) and
> epsilon_symbol == -1 and num_start_states == 1 and numss == 0, set
> net->is_deterministic = net->is_epsilon_free = YES, free all scratch, and return net
> unmodified. numss is a C _Bool assigned the raw state number of the last-seen start
> state, so it collapses any nonzero number to 1 — the test really means "the single
> start state is state 0". (6) If operation == SUBSET_EPSILON_REMOVE and
> epsilon_symbol == -1, set is_epsilon_free = YES, free scratch, return net unmodified.
> (7) Begin output via the dynarray builder: fsm_state_init(sigma_max(net->sigma))
> (STAR_FREE: sigma_max+1, and star_free_mark = 0); for non-STAR_FREE, free(net->states)
> now — the old line table is consumed. (8) Do-loop over agenda subsets, seeded with
> T = 0: fsm_state_set_current_state(T, T_ptr[T].finalstart, T == 0 ? 1 : 0) — output
> state number = subset number, final iff any member final, start iff subset 0. Fetch the
> member list (set_table + T_ptr[T].set_offset, length T_ptr[T].size); reset each
> member's trans_array tail to 0; minsym = smallest composite symbol heading any member's
> sorted transition list; if none, fsm_state_end_state() and continue. Then an ascending
> symbol sweep: for the current minsym, scan every member and consume its sorted
> transition entries equal to minsym, collecting distinct targets into temp_move[j++]
> guarded by the e_table[target] == mainloop marker; each member's next unconsumed entry
> lowers next_minsym. Per-target for EPSILON_REMOVE: immediately do mainloop++,
> U = e_closure(j) (j is 1, so the singleton target expands to its epsilon closure),
> map minsym back through single_symbol_to_symbol_pair, fsm_state_add_arc(T, in, out, U,
> finalstart, T==0), and reset j = 0 — one arc per distinct target closure. (Because
> mainloop advances per target, the same raw target can be re-collected later in the same
> sweep; the dynarray layer silently drops the resulting identical duplicate arc.) For
> DETERMINIZE and TEST_STAR_FREE: after the full member scan for minsym, mainloop++,
> U = e_closure(j) of the whole move set, and emit the single arc (T, in, out, U).
> TEST_STAR_FREE additionally clears star_free_mark to 0 if
> `[spec:foma:sem:determinize.nhash-find-insert-fn]` set it (the arc it would add under the
> mark is commented out — vestigial). Repeat with minsym = next_minsym until no symbols
> remain; fsm_state_end_state(); T = next_unmarked() until it returns -1 (agenda empty;
> LIFO order makes exploration depth-first; subsets are numbered 0,1,2,... in discovery
> order so the output is densely numbered with start state 0). (9) Wrapup: free the hash
> table, set_table, T_ptr, temp_move, e_table, trans list/array; if epsilon_symbol != -1
> call e_closure_free(); free the sigma pair arrays and finals; fsm_state_close(net)
> installs the newly built line table into net and recomputes linecount/arccount/
> statecount/finalcount and the is_deterministic/is_epsilon_free flags tracked by the
> dynarray layer (is_pruned/is_minimized/is_loop_free become UNK). Return net.

> [spec:foma:def:determinize.hashf-fn]
> INLINE static int hashf(int *set, int setsize)

> [spec:foma:sem:determinize.hashf-fn]
> Hash a set of setsize ints so that all permutations of the same elements hash equal
> (required because move sets are produced in varying orders). Using unsigned 32-bit
> wrapping arithmetic: hashval starts at 6703271; for each element e_i (i = 0..setsize-1):
> hashval = (e_i + 1103*setsize) * hashval, and sum += e_i + i. Then
> hashval += 31 * sum, and finally hashval %= nhash_tablesize. Return as int. Permutation
> invariance holds because the product of the (e_i + 1103*setsize) factors is commutative
> and sum(e_i + i) = sum(e_i) + setsize*(setsize-1)/2 is order-independent.

> [spec:foma:def:determinize.init-fn]
> static void init(struct fsm *net)

> [spec:foma:sem:determinize.init-fn]
> Allocates all fsm_subset scratch state. Steps: e_table = calloc(statecount ints) — the
> zeroed dedup-marker table keyed by mainloop values; mainloop = 1; temp_move =
> malloc((statecount+1) ints) — the working buffer for move sets and closures;
> limit = next_power_of_two(linecount) and fsm_linecount = 0 (both vestigial: written here
> and never read anywhere); sigma_to_pairs(net); T_last_unmarked = 0 (only read by dead
> code in next_unmarked); T_limit = next_power_of_two(num_states) and T_ptr =
> calloc(T_limit struct T_memo) — the subset metadata table; set_table_size =
> next_power_of_two(num_states), set_table = malloc(set_table_size ints),
> set_table_offset = 0 — the pool where all subset member lists are stored consecutively;
> finally init_trans_array(net). Note foma's next_power_of_two(v) returns 1 << bitlen(v),
> i.e. strictly greater than v (4 -> 8, 0 -> 1).

> [spec:foma:def:determinize.init-trans-array-fn]
> static void init_trans_array(struct fsm *net)

> [spec:foma:sem:determinize.init-trans-array-fn]
> Builds a per-state sorted outgoing-transition index over composite symbols, excluding
> epsilon arcs, and detects whether the input is already deterministic. Allocate
> trans_list_determinize = malloc(linecount struct trans_list) (the shared entry pool) and
> trans_array_determinize = calloc(statecount struct trans_array). Walk the line table in
> order (foma lines are grouped by ascending state_no): when state_no changes, store the
> accumulated size into the previous state's trans_array entry and point the new state's
> `transitions` at the current pool position; skip lines with target == -1 (arc-less state
> markers); compute inout = symbol_pair_to_single_symbol(in, out) and skip the arc if
> inout == epsilon_symbol (epsilon arcs are handled by closure, not by moves); otherwise
> append {inout, target} to the pool and bump size. After the loop, flush the final
> state's size. Then for every state with size > 1: qsort its slice ascending by inout
> (trans_sort_cmp), and scan the sorted slice — any two adjacent entries with equal inout
> mean two arcs on the same composite symbol from one state, so set the file-static
> `deterministic` flag to 0. States keep tail fields at 0 (calloc) for later cursor use.

> [spec:foma:def:determinize.initial-e-closure-fn]
> static int initial_e_closure(struct fsm *net)

> [spec:foma:sem:determinize.initial-e-closure-fn]
> Builds the finals table and subset 0. Steps: finals = calloc(num_states _Bool); scan the
> whole line table: set finals[state_no] = 1 for lines flagged final_state; a state is
> initial if its lines have start_state set — or unconditionally when
> op == SUBSET_TEST_STAR_FREE (every state seeds the construction) — and each initial
> state is collected once into temp_move[j++] guarded by marking e_table[state] with the
> current mainloop; count them in num_start_states; assign numss = state_no, but numss is
> a _Bool so it only records whether the last-collected start state is nonzero (used by
> the already-deterministic shortcut in `[spec:foma:sem:determinize.fsm-subset-fn]`). Then
> mainloop++; if epsilon_symbol != -1, memoize_e_closure(fsm) builds the epsilon graph.
> Return e_closure(j): registers the epsilon closure of the initial set as the first
> inserted subset and returns its number, 0 (current_setnum starts at -1).

> [spec:foma:def:determinize.memoize-e-closure-fn]
> static void memoize_e_closure(struct fsm_state *fsm)

> [spec:foma:sem:determinize.memoize-e-closure-fn]
> Builds the per-state epsilon adjacency graph consumed by
> `[spec:foma:sem:determinize.e-closure-fn]`. Allocate e_closure_memo = calloc(num_states
> nodes) — head node i gets state = i, target = NULL (mark 0, next NULL from calloc);
> marktable = calloc(num_states ints); redcheck = malloc(num_states ints) all set to -1.
> Scan the line table (grouped by source state): a line is an epsilon arc iff
> in == EPSILON and out == EPSILON (both 0) and target != -1; skip self-loops
> (target == state_no) and duplicate (source,target) pairs — redcheck[target] holds the
> last source that recorded target, so record only when redcheck[target] != state_no,
> then set redcheck[target] = state_no; push each kept target's number on the global int
> stack and set laststate = state (laststate is only updated on epsilon-arc lines). When
> the scanned state_no differs from laststate (including at the terminating
> state_no == -1 sentinel) and the stack is non-empty, flush: set the file-static
> `deterministic` flag to 0 (epsilon fanout defeats the already-deterministic shortcut);
> pop one target and set head(laststate)->target = &e_closure_memo[popped]; for each
> further pop, append a malloc'd chain node {state = laststate, target = head of popped
> state, next = NULL} to head(laststate)'s ->next chain. Break out at the sentinel after
> flushing; free redcheck. Result: state s has epsilon successors iff
> e_closure_memo[s].target != NULL, and its successors are the ->target head-node
> pointers found by walking the node and its ->next chain.

> [spec:foma:def:determinize.move-set-fn]
> static unsigned int move_set(int *set, int setsize)

> [spec:foma:sem:determinize.move-set-fn]
> Appends a set's member list into the global set_table pool and returns its offset. If
> set_table_offset + setsize >= set_table_size (note >=: growth also triggers on an exact
> fit), repeatedly double set_table_size until it fits, then realloc set_table. memcpy
> setsize ints from set to set_table + set_table_offset; save the old offset, advance
> set_table_offset by setsize, and return the old offset. Stored sets are immutable and
> live for the whole construction; T_memo/nhash entries reference them by
> (offset, size).

> [spec:foma:def:determinize.next-unmarked-fn]
> static int next_unmarked(void)

> [spec:foma:sem:determinize.next-unmarked-fn]
> Agenda pop: if the global int stack is empty return -1, else return int_stack_pop() —
> the number of an unprocessed subset previously pushed by
> `[spec:foma:sem:determinize.add-t-ptr-fn]` (LIFO, so the subset loop is depth-first).
> Everything after the return statement (a sequential scan advancing T_last_unmarked and
> terminating on T_limit or a zero-size T_ptr entry) is unreachable dead code left over
> from an earlier FIFO design; do not implement it.

> [spec:foma:def:determinize.nhash-find-insert-fn]
> static int nhash_find_insert(int *set, int setsize)

> [spec:foma:sem:determinize.nhash-find-insert-fn]
> Canonicalizes a state set to its subset number, inserting if unseen. Precondition: `set`
> holds setsize distinct states and every member s currently has
> e_table[s] == mainloop - 1 (established by the caller's marking pass). Steps: hashval =
> hashf(set, setsize). If the bucket head at table[hashval] is empty (size == 0), return
> nhash_insert(hashval, set, setsize). Otherwise walk the bucket chain: skip entries whose
> size differs; for equal-size candidates, test set equality by checking that every
> element of the stored member list (set_table + entry->set_offset) has
> e_table == mainloop - 1 — valid as a set-equality test because sizes match and the
> probe is duplicate-free. When op == SUBSET_TEST_STAR_FREE and a match was found, also
> compare stored vs probe element-by-element; any positional mismatch (same set, different
> order) sets the file-static star_free_mark = 1. On a match return the stored setnum. If
> the chain has no match: if nhash_load / NHASH_LOAD_LIMIT > nhash_tablesize (load limit
> is 2, i.e. more than two entries per bucket on average), call nhash_rebuild_table() and
> recompute hashval against the new table size; then return nhash_insert(...). Note the
> growth check only runs on the collision-miss path — inserting into an empty bucket
> never triggers a rebuild.

> [spec:foma:def:determinize.nhash-free-fn]
> static void nhash_free(struct nhash_list *nptr, int size)

> [spec:foma:sem:determinize.nhash-free-fn]
> Frees a hash table of `size` buckets: for each bucket, free every chained node reachable
> from ->next (the bucket heads are elements of the array itself, not separately
> allocated), then free the bucket array. Does not free the referenced set_table storage.

> [spec:foma:def:determinize.nhash-init-fn]
> static void nhash_init (int initial_size)

> [spec:foma:sem:determinize.nhash-init-fn]
> Creates the subset hash table. Table sizes are always drawn from the static primes table
> {61, 127, 251, 509, 1021, 2039, 4093, 8191, 16381, 32749, 65521, 131071, 262139, 524287,
> 1048573, 2097143, 4194301, 8388593, 16777213, 33554393, 67108859, 134217689, 268435399,
> 536870909, 1073741789, 2147483647}: pick the smallest entry >= initial_size (so the
> minimum size is 61). Set nhash_load = 0, table = calloc(nhash_tablesize bucket heads;
> zeroed so size == 0 marks an empty bucket), and current_setnum = -1 so the first
> inserted subset gets number 0.

> [spec:foma:def:determinize.nhash-insert-fn]
> static int nhash_insert(int hashval, int *set, int setsize)

> [spec:foma:sem:determinize.nhash-insert-fn]
> Inserts a new subset at bucket hashval and assigns it the next number. Steps: increment
> current_setnum; increment nhash_load; compute fs = 1 iff any member state is final per
> the finals[] bitmap, else 0. If the bucket head is empty (size == 0): copy the member
> list into the set_table pool via move_set, then record set_offset, size = setsize, and
> setnum = current_setnum in the head. Otherwise malloc a fresh nhash_list node, splice it
> in as the second chain element (new->next = head->next; head->next = new), and fill it
> the same way. In both cases call add_T_ptr(current_setnum, setsize, set_offset, fs) —
> which records the T_memo entry and pushes the subset on the agenda — and return
> current_setnum.

> [spec:foma:def:determinize.nhash-list]
> struct nhash_list {
>   int setnum;
>   unsigned int size;
>   unsigned int set_offset;
>   struct nhash_list *next;
> }

> [spec:foma:def:determinize.nhash-rebuild-table-fn]
> static void nhash_rebuild_table ()

> [spec:foma:sem:determinize.nhash-rebuild-table-fn]
> Grows the hash table to the next prime and rehashes every entry. Steps: save the old
> table pointer and size; nhash_load = 0; scan the primes table for the first entry >=
> the current nhash_tablesize (which is itself always one of the table primes, so this
> lands exactly on it) and set nhash_tablesize to the following entry; calloc the new
> bucket array. For every entry in every non-empty old bucket chain: recompute hashval =
> hashf over its stored member list (set_table + set_offset, size) with the new table
> size, and reinsert — directly into the new bucket head if empty (this is the only place
> nhash_load is incremented), otherwise as a freshly malloc'd node spliced in after the
> head. Finally nhash_free the old chains and array (stored sets in set_table are shared
> and untouched). Two latent quirks to preserve/flag: (1) nhash_load ends up counting
> only occupied buckets rather than total entries, understating the load factor for
> subsequent growth checks; (2) if the table is already at the last prime (2147483647),
> primes[i+1] reads past the end of the array — practically unreachable.

> [spec:foma:def:determinize.set-lookup-fn]
> INLINE static int set_lookup (int *lookup_table, int size)

> [spec:foma:sem:determinize.set-lookup-fn]
> Pure alias: return nhash_find_insert(lookup_table, size). Maps a state set to its
> canonical subset number, assigning a fresh number (plus T_memo entry and agenda push)
> if the set has not been seen before. Same caller marking precondition as
> `[spec:foma:sem:determinize.nhash-find-insert-fn]`.

> [spec:foma:def:determinize.sigma-to-pairs-fn]
> static void sigma_to_pairs(struct fsm *net)

> [spec:foma:sem:determinize.sigma-to-pairs-fn]
> Builds a bijection between (in,out) label pairs and dense composite symbol numbers so
> the construction can treat transducer labels as single symbols. Steps: epsilon_symbol =
> -1; maxsigma = sigma_max(net->sigma) + 1 (exclusive upper bound on alphabet symbol
> numbers); allocate single_sigma_array (2*maxsigma*maxsigma ints — deliberately
> oversized; only the first 2*num_symbols slots are used) and double_sigma_array
> (maxsigma*maxsigma ints) initialized to -1. Scan every line of the state table: skip
> lines with in == -1 or out == -1 (non-arcs); if in != out, or either equals UNKNOWN
> (constant 1), set net->arity = 2 (side effect: marks the net a transducer); on the
> first occurrence of a pair, assign it the next composite number x (0, 1, 2, ... in
> first-appearance order): double_sigma_array[maxsigma*in + out] = x (forward map),
> single_sigma_array[2x] = in and single_sigma_array[2x+1] = out (back map); if the pair
> is (EPSILON, EPSILON), i.e. (0,0), record epsilon_symbol = x. Set num_symbols to the
> number of distinct pairs. Composite numbering therefore follows first appearance in the
> line table, not numeric symbol order; sorted transition lists sort by this numbering.

> [spec:foma:def:determinize.single-symbol-to-symbol-pair-fn]
> static void single_symbol_to_symbol_pair(int symbol, int *symbol_in, int *symbol_out)

> [spec:foma:sem:determinize.single-symbol-to-symbol-pair-fn]
> Back-maps a composite symbol to its label pair using the table built by
> `[spec:foma:sem:determinize.sigma-to-pairs-fn]`: *symbol_in = single_sigma_array[2*symbol];
> *symbol_out = single_sigma_array[2*symbol + 1]. Inverse of
> symbol_pair_to_single_symbol for registered pairs.

> [spec:foma:def:determinize.symbol-pair-to-single-symbol-fn]
> static int symbol_pair_to_single_symbol(int in, int out)

> [spec:foma:sem:determinize.symbol-pair-to-single-symbol-fn]
> Forward map: return double_sigma_array[maxsigma*in + out], the dense composite symbol
> assigned to the (in,out) pair by sigma_to_pairs, or -1 if the pair was never registered
> (callers only ever pass pairs that occur in the machine, so -1 is not seen in
> practice).

> [spec:foma:def:determinize.t-memo]
> struct T_memo {
>   unsigned char finalstart;
>   unsigned int size;
>   unsigned int set_offset;
> }

> [spec:foma:def:determinize.trans-array]
> struct trans_array {
>   struct trans_list *transitions;
>   unsigned int size;
>   unsigned int tail;
> }

> [spec:foma:def:determinize.trans-list]
> struct trans_list {
>   int inout;
>   int target;
> }

> [spec:foma:def:determinize.trans-sort-cmp-fn]
> static int trans_sort_cmp(const void *a, const void *b)

> [spec:foma:sem:determinize.trans-sort-cmp-fn]
> qsort comparator over struct trans_list: returns a->inout - b->inout, i.e. ascending
> order by composite symbol; entries with equal inout keep an unspecified relative order.
> The subtraction idiom would overflow only for symbol values beyond INT_MAX/2, which
> cannot occur since composite symbols are dense small nonnegative ints.


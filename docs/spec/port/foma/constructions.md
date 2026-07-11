# foma/constructions.c

> [spec:foma:def:constructions.add-fsm-arc-fn]
> int add_fsm_arc(struct fsm_state *fsm, int offset, int state_no, int in, int out, int target, int final_state, int start_state)

> [spec:foma:sem:constructions.add-fsm-arc-fn]
> Writes one transition line into a caller-allocated `fsm_state` array at index
> `offset`: sets that line's `state_no`, `in`, `out`, `target`, `final_state` and
> `start_state` fields to the given arguments verbatim, then returns `offset + 1`
> (the next free index). Performs no allocation, no bounds checking, and no
> validation. Callers use it for three kinds of lines: real arcs, final-state-only
> marker lines (`in`/`out`/`target` = -1 with a real `state_no`), and the
> terminating sentinel line (all six fields -1).

> [spec:foma:def:constructions.add-to-mergesigma-fn]
> struct mergesigma *add_to_mergesigma(struct mergesigma *msigma, struct sigma *sigma, short presence)

> [spec:foma:sem:constructions.add-to-mergesigma-fn]
> Helper of `[spec:foma:sem:constructions.fsm-merge-sigma-fn]`. Appends one entry
> describing the single sigma node `sigma` to the merged-alphabet linked list whose
> current tail is `msigma`, and returns the new tail.
> - Node selection and numbering seed: if `msigma->number == -1` (the dummy head node
>   that fsm_merge_sigma allocates before the merge loop), the head node itself is
>   overwritten in place and the running counter `number` is set to 2. Otherwise a new
>   node is malloc'd and linked as `msigma->next` (its own `next` set to NULL), and the
>   counter is read from the old tail's `number` before advancing to the new node.
> - New entry's `number`: if `sigma->number < 3` (a special symbol: EPSILON = 0,
>   UNKNOWN = 1, IDENTITY = 2), the special number is copied unchanged. Otherwise, the
>   counter is first clamped up to 2 if it is below 3 (i.e. if the previous entry was a
>   special or this is the first entry), and the entry receives counter + 1. Thus the
>   first ordinary symbol always gets number 3 and consecutive ordinary symbols get
>   consecutive numbers, regardless of their numbers in the source sigmas.
> - The entry's `symbol` pointer aliases `sigma->symbol` (no string copy is made), and
>   `presence` is stored verbatim (1 = symbol occurs only in net1's sigma, 2 = only in
>   net2's, 3 = in both).

> [spec:foma:def:constructions.copy-mergesigma-fn]
> struct sigma *copy_mergesigma(struct mergesigma *mergesigma)

> [spec:foma:sem:constructions.copy-mergesigma-fn]
> Converts a mergesigma linked list into a freshly allocated `struct sigma` linked
> list of the same length and order. For each node it copies `number` verbatim and
> deep-copies `symbol` with strdup (a NULL symbol stays NULL); the `presence` field is
> dropped. Returns the head of the new list, or NULL if the input list is NULL.
> Note: if the list still consists only of the never-overwritten dummy head
> (`number == -1`, `symbol == NULL`, possible only when both merged sigmas were
> empty), that dummy is copied too, yielding a one-node sigma with number -1.

> [spec:foma:def:constructions.fsm-add-loop-fn]
> struct fsm *fsm_add_loop(struct fsm *net, struct fsm *marker, int finals)

> [spec:foma:sem:constructions.fsm-add-loop-fn]
> Adds every arc of `marker` as a self-loop at selected states of `net` (used by rule
> compilation as `_addfinalloop(L, "#":0)` etc.; the marker is typically a single-arc
> machine). `finals` selects the states: 1 = final states only, 0 = non-final states
> only, 2 = all states; any other value adds no loops at all.
> Steps: open read handles on `net` and `marker`; create a construct handle named
> `net->name`, copying `net`'s sigma verbatim (symbol numbers preserved). First copy
> every arc of `net` unchanged, by symbol numbers. Then, at each selected state i,
> replay every arc of `marker` as an arc from i to i, added by symbol *name*
> (in/out strings), so marker labels missing from `net`'s sigma are added to the
> result's sigma automatically. Only `marker`'s arc labels matter; its state
> structure, finality and initial state are ignored. For `finals == 1` the selected
> states are enumerated via the read handle's final-state iterator (and each is also
> marked final at that point); for `finals` 0 or 2 the loop runs over state numbers
> 0..`net->statecount`-1, testing finality with fsm_read_is_final for `finals == 0`.
> Afterwards all of `net`'s final states are marked final (again, harmlessly, in the
> `finals == 1` case), the initial state is set to 0, and the machine is built with
> fsm_construct_done (which recomputes counts/flags and sorts the sigma).
> Ownership: `net` is destroyed; `marker` is NOT destroyed (caller keeps it). Returns
> the newly constructed net.

> [spec:foma:def:constructions.fsm-add-sink-fn]
> struct fsm *fsm_add_sink(struct fsm *net, int final)

> [spec:foma:sem:constructions.fsm-add-sink-fn]
> Completes `net` with one fresh sink state, directing every missing transition to
> it. The sink's state number is the current number of states (fsm_get_num_states on
> the read handle). Steps: create a construct handle named `net->name` with `net`'s
> sigma copied verbatim; let maxsigma = sigma_max(net->sigma) + 1 and allocate an int
> table `sigmatable` of that size, initialized to -1. Iterate states with
> fsm_get_next_state; for each state, copy all its outgoing arcs unchanged (by
> numbers) and record `sigmatable[in] = currstate` for each arc's input symbol. Then
> for every symbol number i with 2 <= i < maxsigma (i.e. IDENTITY and all ordinary
> symbols; EPSILON = 0 and UNKNOWN = 1 are never used as completion labels) whose
> table entry does not equal currstate, add an arc currstate -> sink labeled i:i.
> "Seen" is judged by input symbol only: an arc a:b marks only a as seen. After all
> states, unconditionally add i:i self-loops on the sink for every i in
> 2..maxsigma-1 (the sink and its loops are added even if the machine was already
> complete). Original final states are preserved; the sink is made final iff the
> `final` argument is 1; the initial state is set to 0.
> Ownership: `net` is destroyed; the newly constructed net is returned (counts and
> flags recomputed by fsm_construct_done; not minimized).

> [spec:foma:def:constructions.fsm-add-to-states-fn]
> static void fsm_add_to_states(struct fsm *net, int add)

> [spec:foma:sem:constructions.fsm-add-to-states-fn]
> Renumbers all states of `net` in place by a constant offset: walks the state-line
> array up to the sentinel (state_no == -1), adding `add` to every line's `state_no`
> and to every `target` that is not -1 (final-marker lines keep target -1). No
> counts, flags or sigma are touched. Used by fsm_concat to shift net2's state
> numbers past net1's.

> [spec:foma:def:constructions.fsm-bimachine-fn]
> struct fsm *fsm_bimachine(struct fsm *net)

> [spec:foma:sem:constructions.fsm-bimachine-fn]
> Not implemented in foma: prints "implementation pending\n" to stdout and returns
> the input `net` unchanged. A port must reproduce this no-op behavior (or reject
> the operation), not invent a bimachine construction.

> [spec:foma:def:constructions.fsm-close-sigma-fn]
> struct fsm *fsm_close_sigma(struct fsm *net, int mode)

> [spec:foma:sem:constructions.fsm-close-sigma-fn]
> Removes arcs carrying wildcard labels, "closing" the alphabet. Rebuilds the
> machine with a construct handle (sigma copied verbatim, name preserved), copying
> an arc (by numbers) only if it passes the filter:
> - mode == 0: keep the arc iff neither `in` nor `out` is UNKNOWN (1) or IDENTITY (2);
> - mode == 1: additionally keep arcs where neither side is UNKNOWN (IDENTITY arcs
>   survive). Literally, an arc is kept if (in and out are both not in {1,2}) OR
>   (mode == 1 and in != 1 and out != 1).
> Final states and initial states are copied unchanged. The UNKNOWN/IDENTITY entries
> are NOT removed from the sigma here (fsm_construct_done/sigma handling keeps the
> copied sigma). Ownership: `net` is destroyed; returns fsm_minimize of the newly
> constructed machine.

> [spec:foma:def:constructions.fsm-compact-fn]
> void fsm_compact(struct fsm *net)

> [spec:foma:sem:constructions.fsm-compact-fn]
> In-place optimization (void): removes from the sigma every ordinary symbol whose
> distribution in the machine is exactly the same as IDENTITY's, deleting its arcs
> (such symbols are then subsumed by @/? wildcards). Requires lines grouped by state.
> Steps:
> - Let numsymbols = sigma_max. Allocate `potential[0..numsymbols]`, all 1, and
>   `checktable[0..numsymbols]` of {state_no, target} pairs, all {-1,-1}.
> - Any sigma symbol whose UTF-8 length exceeds 1 is marked non-potential (since @
>   and ? only match single UTF-8 characters, removing a multichar symbol would
>   change semantics).
> - One pass over the lines with `prevstate` initialized to 0. Whenever the current
>   line's state_no differs from prevstate (including on reaching the sentinel,
>   after which the pass breaks): for each symbol j in 3..numsymbols, clear
>   potential[j] unless either (a) neither checktable[j] nor checktable[IDENTITY]
>   was recorded for prevstate, or (b) checktable[j] and checktable[IDENTITY] have
>   identical state_no and target (i.e. at prevstate, j:j and @:@ both exist and go
>   to the same target).
> - For each non-sentinel line with in != -1 and out != -1: if (in == out and
>   in > 2) or in == IDENTITY, record checktable[in] = {state_no, target}; if
>   in != out, clear potential[in] when in > 2 and potential[out] when out > 2
>   (symbols used in non-identity pairs are never removable). Set prevstate to the
>   line's state_no.
> - If no symbol in 3..numsymbols remains potential, free the tables and return
>   without changes.
> - Otherwise compact the state array in place: copy lines downward, skipping
>   (dropping) every line whose `in` is a potential symbol > 2; lines with in == -1
>   and all other lines are kept; finally copy the sentinel. The array is not
>   reallocated.
> - Then unlink from the sigma list every entry with number > 2 and potential set,
>   freeing the node and its symbol string. Latent bug: this uses `sigprev->next`
>   without a NULL check, so if the very first sigma entry were removable (a sigma
>   that starts with an ordinary symbol, e.g. a symbol that never occurs in any
>   arc), it dereferences NULL; in practice sigmas begin with a special (<= 2)
>   entry.
> - Free the tables and call sigma_cleanup(net, 0). Counts and flags are not
>   recomputed. Note: if the machine has no IDENTITY arcs at all, any symbol that
>   occurs on an arc is cleared (its target can never equal the {-1,-1} IDENTITY
>   record), so only completely unused symbols get removed.

> [spec:foma:def:constructions.fsm-complement-fn]
> struct fsm *fsm_complement(struct fsm *net)

> [spec:foma:sem:constructions.fsm-complement-fn]
> Complements an automaton: returns fsm_completes(net, COMPLEMENT) where
> COMPLEMENT = 0 (see `[spec:foma:sem:constructions.fsm-completes-fn]`): the machine
> is completed over its (IDENTITY-extended) alphabet with final and non-final states
> swapped. Only meaningful for automata (identity label pairs); consumes/rewrites
> `net` in place and returns it.

> [spec:foma:def:constructions.fsm-complete-fn]
> struct fsm *fsm_complete(struct fsm *net)

> [spec:foma:sem:constructions.fsm-complete-fn]
> Completes an automaton without changing its language: returns
> fsm_completes(net, COMPLETE) where COMPLETE = 1 (see
> `[spec:foma:sem:constructions.fsm-completes-fn]`). Consumes/rewrites `net` in
> place and returns it.

> [spec:foma:def:constructions.fsm-completes-fn]
> struct fsm *fsm_completes(struct fsm *net, int operation)

> [spec:foma:sem:constructions.fsm-completes-fn]
> Shared implementation of completion (`operation` == COMPLETE == 1) and
> complementation (`operation` == COMPLEMENT == 0) for automata. Rewrites `net` in
> place and returns it.
> - If `net->is_minimized != YES`, first replace net with fsm_minimize(net).
> - Sigma normalization: if UNKNOWN is in the sigma, remove "@_UNKNOWN_SYMBOL_@"
>   from it; if IDENTITY is absent, add it and set a local flag incomplete = 1.
> - Let sigsize = sigma_size (number of entries) and last_sigma = sigma_max; if
>   EPSILON is present, decrement sigsize (epsilon is not a completion label).
> - fsm_count(net); statecount = net->statecount. Allocate short arrays `starts`,
>   `finals`, `sinks` of statecount+1 entries; for i < statecount initialize
>   sinks[i] = 1, finals[i] = starts[i] = 0.
> - One pass over the lines: for COMPLEMENT, toggle each line's final_state in
>   place (1<->0) as it is visited. Count arcs (target != -1). Record
>   starts[state_no] = start_state and finals[state_no] = (post-toggle)
>   final_state. Disqualify a state as reusable sink (sinks[state] = 0) if:
>   its (post-toggle) final_state is 1 under COMPLETE, or 0 under COMPLEMENT
>   (i.e. a sink must be non-accepting in the completed machine's original sense —
>   for COMPLEMENT the sink must end up final), or if the line has a real target
>   different from its own state (a sink may only have self-loops or no arcs).
> - Set net->is_loop_free = NO and net->pathcount = PATHCOUNT_CYCLIC (-1).
> - Early exit ("already complete"): if incomplete == 0 and
>   arccount == sigsize * statecount, free the three arrays, set
>   is_completed = YES, is_minimized = YES, is_pruned = NO, is_deterministic = YES
>   and return net (for COMPLEMENT the finals were already toggled in the pass).
>   This test assumes a deterministic machine with gap-free sigma numbering.
> - Choose the sink: the lowest-numbered state with sinks[i] == 1; if none, invent
>   state number statecount with starts = 0 and finals = 1 for COMPLEMENT, 0 for
>   COMPLETE, then statecount++.
> - Set sigsize += 2, then build an int table state_table[statecount][sigsize],
>   initialized to -1, filling state_table[state_no][in] = target for every line
>   with a real target. (TODO noted in source: this indexing relies on gap-free
>   sigma numbering; with gaps, in > sigsize-1 would write out of its row.) Add
>   self-loops state_table[sink][j] = sink and then, for every state, point every
>   still-missing entry for j in 2..last_sigma at the sink.
> - Emit the new state array (sigsize*statecount+1 lines allocated): for each
>   state i in 0..statecount-1 and each symbol j in 2..last_sigma in order, one
>   line (i, j, j, state_table[i][j] (or sink if -1), finals[i], starts[i]);
>   then the sentinel. Note labels are always j:j, and EPSILON/UNKNOWN columns
>   (0,1) are never emitted, so any epsilon arcs are silently dropped; the
>   function is only correct for epsilon-free automata where in == out on every
>   arc (a transducer's out labels are discarded).
> - Replace net->states (old array freed), free the temp arrays, set
>   is_minimized = NO, is_pruned = NO, is_completed = YES, and
>   net->statecount = statecount. Sigma keeps the added IDENTITY entry.

> [spec:foma:def:constructions.fsm-compose-fn]
> struct fsm *fsm_compose(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:constructions.fsm-compose-fn]
> Transducer composition net1 .o. net2 by lazy product construction over state
> triples (p, q, mode), where mode is an epsilon-filter mode (bistate: 0/1;
> tristate: 0/1/2, selected by the global `g_compose_tristate`, default off).
> Setup:
> - Minimize both inputs. If either is empty, destroy both and return
>   fsm_empty_set().
> - If global `g_flag_is_epsilon` is set: before merging sigmas, add every flag
>   diacritic symbol (flag_check) of net1's sigma missing from net2's sigma to
>   net2, and vice versa, then sigma_sort both; print a warning if both nets
>   contained flags (composition may be incorrect). This stops UNKNOWN/IDENTITY
>   from matching flags, which must behave like epsilons.
> - fsm_merge_sigma(net1, net2) (`[spec:foma:sem:constructions.fsm-merge-sigma-fn]`).
>   If g_flag_is_epsilon, build a boolean table is_flag[symbol number] over the
>   merged sigma.
> - fsm_update_flags(net1, YES, NO, UNK, YES, UNK, UNK).
> - Index structure for net2: let max2sigma = sigma_max(net2->sigma). Allocate
>   `index` (max2sigma+1 row-tail pointers) and `outarray`, a
>   (max2sigma+1)-row table with row width max2sigma+2 of entries
>   {symin, symout, target, mainloop}; row i initially has tail = row start. At
>   each popped state, for every arc of q with a real target, append
>   {in, out, target} to the row for its input symbol, where IDENTITY is indexed
>   under UNKNOWN (they share matching semantics); a `mainloop` counter stamped on
>   every entry (incremented once per popped state) marks entries as fresh so rows
>   never need clearing: if a row's tail entry is stale the row restarts at its
>   beginning, else the tail advances by one.
> - Worklist: an int stack seeded with the triple (mode=0, q=0, p=0), pushed in
>   the order mode, q, p so pops yield p, q, mode. A triplet hash
>   (`[spec:foma:sem:constructions.triplet-hash-insert-fn]`) maps each discovered
>   (p, q, mode) to a dense new-state number, seeded with (0,0,0) -> 0. Output is
>   accumulated with the dynarray builder (fsm_state_init(sigma_max(net1->sigma)),
>   fsm_state_set_current_state / fsm_state_add_arc / fsm_state_end_state).
> Per popped (p, q, mode): current state number = hash lookup of (p,q,mode); it is
> a start state iff p and q are both start states and mode == 0; final iff p and q
> are both final. Then three arc-generation phases (targets are looked up in the
> hash; if absent they are inserted, assigned the next number, and pushed):
> 1. Symbol matches: for each arc of p with aout = out >= 0, scan the net2 index
>    row for aout (IDENTITY searched under UNKNOWN) from the row start through its
>    current tail while entries are fresh. For each entry (bin, bout, btarget):
>    first apply wildcard adjustment — if aout == IDENTITY and bin == UNKNOWN, set
>    ain = aout = UNKNOWN; else if aout == UNKNOWN and bin == IDENTITY, set
>    bin = bout = UNKNOWN. Then if bin == aout and bin != -1 and
>    (bin != EPSILON or mode == 0), emit arc ain:bout to state
>    (ptarget, btarget, 0). (The bin == aout == EPSILON case is the joint x:0 0:y
>    step, permitted only in mode 0.) Note: the source has separate
>    bistate/tristate branches here but their code is identical.
> 2. Epsilon (and flag) outputs of p: for each arc of p (skipped entirely unless
>    aout == EPSILON or g_flag_is_epsilon): if g_flag_is_epsilon and aout is a
>    flag and mode == 0, emit arc ain:aout to (ptarget, q, mode 0). If
>    aout == EPSILON: bistate — only when mode == 0, emit ain:EPSILON to
>    (ptarget, q, mode 0); tristate — when mode != 2, emit ain:EPSILON to
>    (ptarget, q, mode 1).
> 3. Epsilon (and flag) inputs of q: for each arc of q (skipped unless
>    bin == EPSILON or g_flag_is_epsilon): if g_flag_is_epsilon and bin is a flag
>    (any mode), emit bin:bout to (p, qtarget, mode 1). If bin == EPSILON:
>    bistate — in any mode, emit EPSILON:bout to (p, qtarget, mode 1); tristate —
>    only when mode != 1, emit EPSILON:bout to (p, qtarget, mode 2).
> The bistate filter thus permits x:0 steps only in mode 0 and 0:y steps in any
> mode (entering mode 1), preventing duplicate interleavings of epsilons.
> Teardown: free net1's old state array, destroy net2, fsm_state_close(net1)
> (installs the built states into net1, recomputing counts/flags; the merged sigma
> stays on net1), free the index, outarray, state-pointer arrays, is_flag and the
> triplet hash. Finally net1 = fsm_topsort(fsm_coaccessible(net1)) and the result
> returned is fsm_coaccessible(net1) (coaccessibility applied twice — literal
> behavior). No minimization is performed.

> [spec:foma:def:constructions.fsm-concat-fn]
> struct fsm *fsm_concat(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:constructions.fsm-concat-fn]
> Concatenation net1 net2 via an epsilon splice.
> - fsm_merge_sigma(net1, net2); fsm_count on both.
> - If either machine has no final state, destroy both and return fsm_empty_set()
>   (concatenation with the empty language is empty).
> - Shift net2's state numbers up by net1->statecount (fsm_add_to_states); net2's
>   start state is assumed to be state 0, so its shifted start is exactly
>   net1->statecount.
> - Allocate a new line array of net1->linecount + net2->linecount +
>   net1->finalcount + 2 lines. Walk net1's lines (grouped by state): at the first
>   line of each final state (tracked with a current_final variable holding the
>   last final state emitted), first emit an EPSILON:EPSILON arc from that state
>   to state net1->statecount (net2's shifted start), with final_state 0 and the
>   line's start_state preserved. Then copy the line itself with final_state
>   forced to 0 — except pure final-marker lines (target == -1 and
>   final_state == 1), which are dropped (replaced by the epsilon arc).
> - Copy all of net2's (shifted) lines with start_state forced to 0, finality
>   preserved. Append the sentinel.
> - Free net1's old array, install the new one on net1, destroy net2. Add EPSILON
>   to net1's sigma if missing. fsm_count(net1); clear is_epsilon_free,
>   is_deterministic, is_minimized, is_pruned (set to NO). Return
>   fsm_minimize(net1). Both inputs are consumed.

> [spec:foma:def:constructions.fsm-concat-m-n-fn]
> struct fsm *fsm_concat_m_n(struct fsm *net1, int m, int n)

> [spec:foma:sem:constructions.fsm-concat-m-n-fn]
> Bounded repetition A^{m,n} (at least m, at most n copies). Starts with
> acc = fsm_empty_string(); for i = 1..n, acc = fsm_concat(acc, X) where X is
> fsm_copy(net1) when i <= m and fsm_optionality(fsm_copy(net1)) when i > m (each
> fsm_concat minimizes its result). Destroys net1 and returns acc. Edge cases:
> n < 1 yields the empty-string language regardless of m; if m > n all n copies
> are mandatory (A^n).

> [spec:foma:def:constructions.fsm-concat-n-fn]
> struct fsm *fsm_concat_n(struct fsm *net1, int n)

> [spec:foma:sem:constructions.fsm-concat-n-fn]
> Exact repetition A^n: returns fsm_concat_m_n(net1, n, n)
> (`[spec:foma:sem:constructions.fsm-concat-m-n-fn]`). Consumes net1; n < 1 yields
> the empty-string language.

> [spec:foma:def:constructions.fsm-contains-fn]
> struct fsm *fsm_contains(struct fsm *net)

> [spec:foma:sem:constructions.fsm-contains-fn]
> The "contains" operator $A = [?* A ?*]: returns
> fsm_concat(fsm_concat(fsm_universal(), net), fsm_universal()), i.e. all strings
> having a substring in A (with fsm_universal being the IDENTITY self-loop
> machine). Consumes net; result is minimized by fsm_concat.

> [spec:foma:def:constructions.fsm-contains-one-fn]
> struct fsm *fsm_contains_one(struct fsm *net)

> [spec:foma:sem:constructions.fsm-contains-one-fn]
> The "contains exactly one" operator $.A, computed as
> $A - $[[?+ A ?* & A ?*] | [A ?+ & A]] over copies of the input. Literally:
> ret = fsm_minus(fsm_contains(fsm_copy(net)), fsm_contains(fsm_union(
> fsm_intersect(fsm_concat(fsm_kleene_plus(fsm_identity()),
> fsm_concat(fsm_copy(net), fsm_universal())),
> fsm_concat(fsm_copy(net), fsm_universal())),
> fsm_intersect(fsm_concat(fsm_copy(net), fsm_kleene_plus(fsm_identity())),
> fsm_copy(net))))), where fsm_identity() is the single-IDENTITY-arc machine
> (? as a language). The subtracted term describes strings containing overlapping
> or multiple occurrences of A. Destroys net and returns ret.

> [spec:foma:def:constructions.fsm-contains-opt-one-fn]
> struct fsm *fsm_contains_opt_one(struct fsm *net)

> [spec:foma:sem:constructions.fsm-contains-opt-one-fn]
> The "contains at most one" operator $?A = $.A | ~$A: returns
> fsm_union(fsm_contains_one(fsm_copy(net)),
> fsm_complement(fsm_contains(fsm_copy(net)))) — strings that contain exactly one
> occurrence of A, or none at all. Destroys net and returns the union.

> [spec:foma:def:constructions.fsm-context-restrict-fn]
> struct fsm *fsm_context_restrict(struct fsm *X, struct fsmcontexts *LR)

> [spec:foma:sem:constructions.fsm-context-restrict-fn]
> Context restriction X => L1 _ R1, ..., Ln _ Rn over the context list LR,
> implemented with the auxiliary variable symbol "@VARX@" and word-boundary
> stand-in "@#@", following the formula
> `[[(?) \.#.* (?)] - subst([[\X* X C X \X*] - [\X* [L1 X \X* X R1|...|Ln X \X* X Rn] \X*]], X, 0)]`
> where X is "@VARX@". Steps:
> - Var = fsm_symbol("@VARX@"); Notvar = minimize(([the term negation of
>   "@VARX@"])*) i.e. [\@VARX@]*.
> - Add "@VARX@" to X's sigma and sigma_sort(X), so ? / @ in X cannot match the
>   variable symbol (avoids spurious nondeterminism).
> - For each context pair in LR: a NULL left or right is replaced by
>   fsm_empty_string(); otherwise "@VARX@" is added to its sigma, every ".#." in
>   its sigma is renamed to "@#@" (sigma_substitute), and the sigma re-sorted.
> - UnionP = union over all pairs of minimize(L_i Var Notvar Var R_i), built by
>   repeated fsm_union starting from fsm_empty_set(), minimized at each step.
> - UnionL = minimize(Notvar Var X Var Notvar) (X consumed via fsm_copy; the
>   original X object destroyed at the end).
> - Result = fsm_intersect(UnionL, complement(Notvar (minimize(UnionP Notvar)))).
> - If "@VARX@" survives in Result's sigma, Result =
>   complement(fsm_substitute_symbol(Result, "@VARX@", "@_EPSILON_SYMBOL_@"));
>   otherwise Result = complement(Result).
> - If "@#@" is in Result's sigma (i.e. some context referenced the word
>   boundary): Word = minimize("@#@" [\"@#@"]* "@#@"); Result =
>   fsm_intersect(Word, Result), then substitute "@#@" with epsilon.
> - Cleanup: destroy UnionP, Var, Notvar and X, and call fsm_clear_contexts on
>   the loop cursor — which is NULL after the loops, so literally the LR context
>   list and its networks are never freed (latent leak/bug: fsm_clear_contexts(LR)
>   was clearly intended).
> - Returns Result. Ownership: X is consumed; the nets inside LR are only read
>   (copied), but LR is (intended to be) consumed by the callee.

> [spec:foma:def:constructions.fsm-count-fn]
> void fsm_count(struct fsm *net)

> [spec:foma:sem:constructions.fsm-count-fn]
> Recomputes the bookkeeping counts of `net` from its state-line array in one
> pass up to the sentinel:
> - statecount = (highest state_no seen) + 1 (states are assumed densely
>   numbered from 0; an empty array yields statecount 1);
> - linecount = number of lines including the sentinel (lines before it + 1);
> - arccount = number of lines with target != -1;
> - finalcount = number of state-runs whose line has final_state set, counted
>   only at the first line of each run of equal consecutive state_no values (so
>   lines must be grouped by state for this to equal the number of final states).
> Results are stored into net->statecount/linecount/arccount/finalcount; flags,
> arity and pathcount are not touched.

> [spec:foma:def:constructions.fsm-count-states-fn]
> int fsm_count_states(struct fsm_state *fsm)

> [spec:foma:sem:constructions.fsm-count-states-fn]
> Counts the states of a raw state-line array: walks lines until the sentinel
> (state_no == -1), incrementing a counter each time a line's state_no differs
> from the previous line's (initial previous value -1). This equals the number of
> distinct states only when lines are grouped by state; a state whose lines
> appear in two non-adjacent runs would be counted twice. Returns the count
> (0 for an array that starts with the sentinel).

> [spec:foma:def:constructions.fsm-cross-product-fn]
> struct fsm *fsm_cross_product(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:constructions.fsm-cross-product-fn]
> Cross product A .x. B of two automata by running them in parallel; used for
> explicit cross products (a:b, A .x. B) — rewrite-rule compilation uses
> rewrite_cp() instead. A machine may "stay" (epsilon on its side) only when it is
> in a final state, postponing unmatched material to the end.
> Setup: minimize both inputs; fsm_merge_sigma(net1, net2); fsm_count both. Seed
> an int-stack worklist with the pair (0,0) and a triplet hash mapping
> (0,0,0) -> 0; fsm_state_init(sigma_max(net1->sigma)); build state-pointer
> arrays (`[spec:foma:sem:constructions.init-state-pointers-fn]`) for both.
> Per popped pair (a,b): new-state number = hash find (a,b,0); start iff both are
> start states, final iff both final; then iterate ALL pairs of lines (one from
> state a, one from state b), including target == -1 final-marker lines:
> - Skip the pair if both targets are -1; skip if a's line is a marker line
>   (target -1) and not final; likewise for b.
> - Both move: if both lines have real targets, look up / create state
>   (atarget, btarget) (on insert, push the pair), take symbol1 = a-line's `in`,
>   symbol2 = b-line's `in` (input sides only — the nets are automata), convert
>   IDENTITY to UNKNOWN on whichever side is IDENTITY when the other side is not,
>   and emit arc symbol1:symbol2. If BOTH inputs are IDENTITY, additionally emit
>   an UNKNOWN:UNKNOWN arc to the same target (@:@ yields both @:@ and ?:?).
> - A stays: if a's line has final_state == 1 and b's line has a real target,
>   look up / create state (a, btarget) and emit arc EPSILON:symbol2 where
>   symbol2 is b's input with IDENTITY replaced by UNKNOWN (@ cannot pair with
>   epsilon). Duplicate emissions from multiple a-lines are suppressed by the
>   dynarray builder's per-state arc dedup.
> - B stays: symmetrically, if b's line is final and a's line has a real target,
>   state (atarget, b), arc symbol1:EPSILON with IDENTITY -> UNKNOWN.
> After the worklist empties: free net1's old lines, fsm_state_close(net1). Scan
> the resulting lines and add EPSILON (resp. UNKNOWN) to the sigma if any arc
> label uses it and it is missing. Free the pointer arrays and hash, destroy
> net2, and return fsm_coaccessible(net1) (not minimized; may be nondeterministic).

> [spec:foma:def:constructions.fsm-equal-substrings-fn]
> struct fsm *fsm_equal_substrings(struct fsm *net, struct fsm *left, struct fsm *right)

> [spec:foma:sem:constructions.fsm-equal-substrings-fn]
> The _eq(L, left, right) operator: restricts the (lower side of) `net` to those
> strings in which every substring delimited as left X right has the SAME X in all
> its occurrences. Auxiliary bracket symbols LB = "@<eq<@" and RB = "@>eq>@" are
> used. Caveat (documented in source): the extraction loop has no reliable
> termination condition — if the possible identical delimited substrings are
> unbounded in length (e.g. _eq(l a* r l a* r, l, r)) it loops forever.
> Steps (all regex terms built literally with the construction functions):
> - oldnet = fsm_copy(net) (kept as fallback return value).
> - LB, RB = fsm_symbol machines; NOLB/NORB = minimized term negations \LB, \RB;
>   NOBR = minimize(~$[LB|RB]). Add "@<eq<@" and "@>eq>@" to net's sigma, sort.
> - InsertBrackets = minimize([~$[left|right] [left 0:LB | 0:RB right]]* ~$[left|right]);
>   Lbracketed = fsm_compose(fsm_copy(net), InsertBrackets) — inserts LB after
>   each `left` and RB before each `right`.
> - BracketFilter = NOBR LB NOBR RB NOBR [LB NOBR RB NOBR]+ (proper nesting, at
>   least two bracketed instances).
> - RemoveBrackets = [LB:0 | RB:0 | NOBR]*.
> - Lbypass = lower(compose(copy(Lbracketed), compose(complement(copy(BracketFilter)),
>   RemoveBrackets))) — strings whose bracketings are improper or have fewer than
>   two bracketed substrings, with brackets erased; these bypass the equality
>   check. Leq = compose(Lbracketed, BracketFilter).
> - Labels = fsm_sigma_pairs_net(lower(compose(copy(Leq),
>   [[\LB:0]* LB:0 \RB* RB:0]* [\LB:0]*))) — a net whose sigma holds exactly the
>   symbols occurring between LB and RB.
> - Cleanup = minimize(\LB* [LB:0 RB:0 \LB*]* | ~$[LB RB]) — deletes adjacent
>   LB RB pairs while filtering out strings containing both LB RB and LB X RB
>   with nonempty X.
> - Move = minimize(union over every sigma entry of Labels with number >= 3 of
>   ThisMove(sym)), starting from fsm_empty_string(), where ThisMove(X) =
>   [\LB* LB:0 X 0:LB]* \LB* — shifts each bracket rightward past one X, e.g.
>   rewriting "LB a b RB LB a b RB" to "a LB b RB a LB b RB". If no such symbol
>   exists (syms == 0), destroy net and return oldnet unchanged.
> - Main loop: repeatedly Leq = compose(Leq, copy(Cleanup)); if "@<eq<@" no
>   longer occurs on Leq's lower side (fsm_symbol_occurs, M_LOWER), exit the
>   loop; otherwise Leq = compose(Leq, copy(Move)) and repeat.
> - Result = minimize(compose(net, union(lower(Leq), Lbypass))); remove "@<eq<@"
>   and "@>eq>@" from Result's sigma; fsm_compact(Result); sigma_sort(Result);
>   destroy oldnet; return Result.
> Ownership: net is consumed; left and right are only fsm_copy'd, never destroyed
> here (caller keeps them). Intermediate machines are consumed by the operations
> that combine them.

> [spec:foma:def:constructions.fsm-equivalent-fn]
> int fsm_equivalent(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:constructions.fsm-equivalent-fn]
> Tests structural path equivalence of two machines by parallel traversal,
> returning 1 (equivalent) or 0. Correct only for deterministic, minimized,
> epsilon-free machines whose lines are grouped by state (it matches arcs by
> exact in/out label pairs and follows only the first match).
> - fsm_merge_sigma(net1, net2) (harmonizes numbering and expands wildcards
>   against each other's alphabets); fsm_count both.
> - Worklist of state pairs seeded with (0,0); a triplet hash records visited
>   pairs, seeded with (0,0,0).
> - Per popped pair (a,b): if a's and b's finality differ, the machines are not
>   equivalent. For each arc line of a (stopping at a line with target == -1):
>   scan b's lines for one with equal `in` and equal `out`; if none, not
>   equivalent; if found, and the target pair (atarget, btarget) is not yet in
>   the hash, insert it and push it. Then, symmetrically, verify every arc of b
>   has a label-matching arc in a (no pushing in this direction).
> - If the worklist empties without a mismatch, the result is 1.
> Cleanup in all cases: BOTH net1 and net2 are destroyed, the state-pointer
> arrays freed, and the hash freed. Returns the 0/1 verdict.

> [spec:foma:def:constructions.fsm-escape-fn]
> struct fsm *fsm_escape(char *symbol)

> [spec:foma:sem:constructions.fsm-escape-fn]
> Builds the single-symbol machine for an escaped symbol: returns
> fsm_symbol(symbol + 1), i.e. simply skips the first byte of the string (the
> escape character, e.g. "%") and delegates to
> `[spec:foma:sem:constructions.fsm-symbol-fn]` with the remainder. No validation
> is performed; an empty remainder is passed through as the empty string symbol.

> [spec:foma:def:constructions.fsm-explode-fn]
> struct fsm *fsm_explode(char *symbol)

> [spec:foma:sem:constructions.fsm-explode-fn+1]
> Builds a linear chain automaton spelling out the characters of `symbol`.
> Iterates over `symbol` one UTF-8 character at a time; the k-th character
> becomes an arc from state k-1 to state k labeled c:c, added by name via the
> construct API (each distinct character enters the sigma). State 0 is initial;
> the state after the last character is final. An empty input yields the
> single-state empty-string machine (state 0 initial and final). Returns the
> constructed net.
> The parameter is the payload itself; C received the brace-enclosed form
> ({abc}) and stripped the first and last byte, so callers had to re-wrap
> already-stripped text only for it to be unwrapped again — and a short or
> non-delimited string indexed out of bounds.

> [spec:foma:def:constructions.fsm-flatten-fn]
> struct fsm *fsm_flatten(struct fsm *net, struct fsm *epsilon)

> [spec:foma:sem:constructions.fsm-flatten-fn+1]
> Flattens a transducer into an automaton over single symbols by splitting every
> arc a:b into two consecutive arcs a:a then b:b through a fresh intermediate
> state, with EPSILON replaced by a visible symbol taken from the `epsilon`
> machine (the input-side string of its first arc).
> Steps: net = fsm_minimize(net). Read `epsilon`'s first arc; if it has none
> (fsm_get_next_arc == 0, end-of-arcs) return None. epssym = strdup of its input
> symbol string. C compared fsm_get_next_arc's result to -1, which it never
> returns, so the "return NULL" branch for an arc-less epsilon machine was dead
> and an arc-less `epsilon` led to reading an invalid arc. Create a construct handle
> (name and sigma copied from net); maxstate starts at net->statecount and each
> processed arc allocates one fresh intermediate state (maxstate++).
> For each arc of net (source s, target t, labels in:out): if either side is
> EPSILON, add by strings: s -> mid labeled i:i and mid -> t labeled o:o, where i
> is the arc's input string (or epssym if in == EPSILON) and o the output string
> (or epssym if out == EPSILON); otherwise add numerically s -> mid (in:in) and
> mid -> t (out:out). Every output arc is thus an identity pair. Finals and
> initials are copied from net.
> Ownership: destroys net and epsilon, frees epssym, returns the newly
> constructed net (not minimized).

> [spec:foma:def:constructions.fsm-follows-fn]
> struct fsm *fsm_follows(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:constructions.fsm-follows-fn]
> The "follows" restriction operator: returns
> fsm_complement(fsm_minimize(fsm_contains(fsm_minimize(fsm_concat(
> fsm_minimize(fsm_copy(net1)), fsm_concat(fsm_universal(),
> fsm_minimize(fsm_copy(net2)))))))) — i.e. ~$[net1 ?* net2], the strings that do
> NOT contain an occurrence of net1 followed (with any material in between) by an
> occurrence of net2. Operates on copies: neither net1 nor net2 is destroyed
> (caller keeps ownership). Compare `[spec:foma:sem:constructions.fsm-precedes-fn]`,
> which is the same with net1/net2 swapped.

> [spec:foma:def:constructions.fsm-ignore-fn]
> struct fsm *fsm_ignore(struct fsm *net1, struct fsm *net2, int operation)

> [spec:foma:sem:constructions.fsm-ignore-fn+1]
> The ignore operator: net1 with freely interspersed strings from net2.
> `operation` is OP_IGNORE_ALL (1: ignore anywhere, A/B) or OP_IGNORE_INTERNAL
> (2: ignore only strictly inside A, A./.B).
> - Minimize both inputs. If net2 is empty, destroy net2 and return net1
>   unchanged. Then fsm_merge_sigma(net1, net2) and fsm_count both.
> - OP_IGNORE_INTERNAL is rewritten with the marker "@i<@" as: Result =
>   lower(compose(ignore(copy(net1), fsm_symbol("@i<@"), OP_IGNORE_ALL),
>   compose(complement(union("@i<@" ?*, ?* "@i<@")),
>   fsm_simple_replace(fsm_symbol("@i<@"), copy(net2))))); then "@i<@" is removed
>   from Result's sigma, both inputs destroyed, Result returned. (Markers may go
>   anywhere; the filter bans them at the very start/end; each marker is then
>   replaced by net2.)
> - Otherwise (any other operation value behaves like OP_IGNORE_ALL) a direct
>   splice is built into a new array of lines1 + states1*(lines2 + finalcount2 + 1)
>   + 1 lines: walking net1's lines, at the FIRST line of each state (tracked via
>   a handled_states1 bitmap) emit an EPSILON:EPSILON arc from that state to the
>   entry state of a per-state private copy of net2, placed at block offset
>   states1 + spliceIndex*states2 (net2's start is assumed to be its state 0);
>   record return_state[spliceIndex] = the state; then copy the line itself if it
>   carries a label (in != -1). All other lines are copied verbatim.
> - Then, for each splice in creation order, append net2's lines shifted by the
>   block offset, with final_state and start_state forced to 0. At the first line
>   of each FINAL state of net2 (tracked via handled_states2, reset per splice),
>   emit instead an EPSILON:EPSILON arc from the shifted state back to
>   return_state[spliceIndex] (non-final), followed by the line's own arc if it
>   has a real target. In the plain-copy branch a target of -1 (a final marker) is
>   preserved; only a real target is shifted by the offset. C shifted the target
>   unconditionally (`target + offset`), so a -1 would become a bogus state number
>   — this cannot occur for a minimized net2 (every arc-less state is final and
>   handled by the first branch) but would corrupt a non-minimized net2.
> - Append the sentinel; free the bitmaps and return table; free net1's old
>   lines; destroy net2; install the new array on net1; fsm_update_flags(net1,
>   NO,NO,NO,NO,NO,NO); fsm_count(net1). Returns net1 (not minimized). Both
>   inputs are consumed (except net2-empty early return, which keeps net1).

> [spec:foma:def:constructions.fsm-intersect-fn]
> struct fsm *fsm_intersect(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:constructions.fsm-intersect-fn]
> Intersection by the running-in-parallel product construction.
> - Minimize both inputs; if either is then empty, destroy both and return
>   fsm_empty_set().
> - fsm_merge_sigma(net1, net2) — harmonizes numbering and expands
>   UNKNOWN/IDENTITY over each other's new symbols, so afterwards arcs match
>   iff their (in,out) number pairs are equal (IDENTITY only matches IDENTITY).
> - fsm_update_flags(net1, YES, NO, UNK, YES, UNK, UNK).
> - Per-state lookup index for net2: sigma2size = sigma_max(net2->sigma)+1; a
>   calloc'd sigma2size x sigma2size table of {mainloop, target} indexed by
>   [in][out]. A counter `mainloop`, bumped once per popped state pair, marks
>   entries fresh so the table is never cleared.
> - Worklist: int stack seeded with pair (0,0); triplet hash seeded
>   (0,0,0) -> 0 maps discovered pairs (a,b) (third component always 0) to dense
>   new-state numbers. Output built with the dynarray builder
>   (fsm_state_init(sigma_max(net1->sigma))); state-pointer arrays for both nets.
> - Per popped pair (a,b): current state = hash find; start iff both start,
>   final iff both final. Fill the index from b's lines (skipping in < 0 marker
>   lines): table[in][out] = {mainloop, target}. Then for each line of a with
>   in >= 0 and out >= 0: if table[in][out] is fresh, resolve the target pair
>   (atarget, btarget) via the hash (inserting and pushing if new) and emit the
>   arc (in, out, target). fsm_state_end_state() after each pair.
> - Result assembly: create a fresh net with fsm_create(""), destroy its default
>   sigma and steal net1's merged sigma (net1->sigma = NULL); destroy net2 and
>   net1; fsm_state_close(new_net); free the index, state-pointer arrays and
>   hash. Return fsm_coaccessible(new_net) (not minimized). Both inputs consumed.

> [spec:foma:def:constructions.fsm-invert-fn]
> struct fsm *fsm_invert(struct fsm *net)

> [spec:foma:sem:constructions.fsm-invert-fn]
> Inverts a transducer in place: swaps `in` and `out` on every line up to the
> sentinel, and swaps the `arcs_sorted_in`/`arcs_sorted_out` flags. Nothing else
> (sigma, counts, other flags) changes; returns the same net object.

> [spec:foma:def:constructions.fsm-kleene-closure-fn]
> struct fsm *fsm_kleene_closure(struct fsm *net, int operation)

> [spec:foma:sem:constructions.fsm-kleene-closure-fn]
> Shared implementation of Kleene star (operation KLEENE_STAR = 0), Kleene plus
> (KLEENE_PLUS = 1) and optionality (OPTIONALITY = 2).
> - OPTIONALITY short-circuits: return fsm_union(net, fsm_empty_string()).
> - Otherwise: net = fsm_minimize(net); fsm_count(net). Allocate
>   linecount + finalcount + 1 new lines.
> - A new state 0 is prepended (all original states shift up by 1): emit line
>   (0, EPSILON, EPSILON, target 1, final_state 1 for star / 0 for plus,
>   start_state 1). The original start state is assumed to be state 0 (target 1
>   after shifting); the new state 0 is the only start state, and for star it is
>   final so that the empty string is accepted.
> - Copy the machine with all state/target numbers incremented by 1
>   (start_state forced to 0), inserting closure arcs at final states: a pure
>   final-marker line (target == -1, final 1) is replaced by the arc
>   (state, EPSILON, EPSILON, target 0, final 1, start 0), i.e. an epsilon arc
>   back to the new state 0; at the first line of a final state that has real
>   arcs (detected by comparing the shifted state number with the previous
>   line's, tracked in `laststate`), the same epsilon-to-0 arc is emitted before
>   the copied arcs. Append the sentinel.
> - Update counts in place: statecount += 1; linecount = number of lines
>   written; finalcount += 1 for star only; arccount recomputed while emitting
>   (initialized to 1 for the initial epsilon arc); pathcount =
>   PATHCOUNT_UNKNOWN (-3). Replace net->states, add EPSILON to the sigma if
>   missing, and fsm_update_flags(net, NO,NO,NO,NO,UNK,NO). Returns the same net
>   object, NOT minimized. (Note the loop back to state 0, rather than to the old
>   start, makes A+ = A A* correct because state 0 leads into the old start by
>   the initial epsilon arc.)

> [spec:foma:def:constructions.fsm-kleene-plus-fn]
> struct fsm *fsm_kleene_plus(struct fsm *net)

> [spec:foma:sem:constructions.fsm-kleene-plus-fn]
> Kleene plus A+: returns fsm_kleene_closure(net, KLEENE_PLUS) with
> KLEENE_PLUS = 1 (see `[spec:foma:sem:constructions.fsm-kleene-closure-fn]`);
> the new prepended start state is not final, so the empty string is accepted
> only if A itself accepts it. Consumes net.

> [spec:foma:def:constructions.fsm-kleene-star-fn]
> struct fsm *fsm_kleene_star(struct fsm *net)

> [spec:foma:sem:constructions.fsm-kleene-star-fn]
> Kleene star A*: returns fsm_kleene_closure(net, KLEENE_STAR) with
> KLEENE_STAR = 0 (see `[spec:foma:sem:constructions.fsm-kleene-closure-fn]`);
> the new prepended start state is also final, so the empty string is always
> accepted. Consumes net.

> [spec:foma:def:constructions.fsm-left-rewr-fn]
> struct fsm *fsm_left_rewr(struct fsm *net, struct fsm *rewr)

> [spec:foma:sem:constructions.fsm-left-rewr-fn]
> Fast single-symbol left-context rewrite: _leftrewr(L, a:b) builds the
> transducer for a -> b || .#. L _ (and with net = ?* L, for a -> b || L _).
> `net` is a machine whose final states mark "the left context has just been
> matched"; `rewr` must be a single-arc a:b machine — only the FIRST line of
> rewr->states is consulted (relabelin = its in, relabelout = its out), read
> AFTER fsm_merge_sigma(net, rewr) so the numbers live in the merged sigma.
> Steps: open a read handle on net; sinkstate = number of states; construct
> handle with net's name and (merged) sigma; maxsigma = sigma_max + 1; int table
> sigmatable[maxsigma] initialized to -1; addedsink = 0.
> For each state currstate of net:
> - Mark currstate final in the output (every original state becomes final — the
>   result accepts/transduces all inputs).
> - Copy each outgoing arc, recording sigmatable[in] = currstate; if the arc's
>   input equals relabelin, set seensource = 1, and if currstate is final in the
>   INPUT net (left context just matched), replace the output label with
>   relabelout (so a:b is applied exactly after L); otherwise the arc is copied
>   unchanged.
> - Completion: for every symbol i in 2..maxsigma-1 (IDENTITY and ordinary
>   symbols; EPSILON/UNKNOWN skipped) not seen leaving currstate and not equal to
>   relabelin, add arc currstate -> sinkstate labeled i:i, setting addedsink.
> - If relabelin was never seen at currstate: add currstate -> sinkstate labeled
>   relabelin:relabelout if currstate is input-final, else relabelin:relabelin;
>   set addedsink.
> If any sink arc was added, give sinkstate i:i self-loops for all i in
> 2..maxsigma-1 and mark it final. Set initial state 0.
> Ownership: destroys net and rewr; frees the table; returns the constructed net
> (counts/flags from fsm_construct_done; not minimized). The sink never
> rewrites: it is the catch-all state reached when the input has diverged from
> net's context tracking, where every symbol, including the rewrite source, maps
> to itself.

> [spec:foma:def:constructions.fsm-lenient-compose-fn]
> struct fsm *fsm_lenient_compose(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:constructions.fsm-lenient-compose-fn]
> Lenient composition A .O. B, implemented as
> fsm_priority_union_upper(fsm_compose(fsm_copy(net1), net2), fsm_copy(net1)),
> i.e. [A .o. B] .P. A: compose A with B, and for those inputs (upper-side
> strings) that the composition does not cover, fall back to A's own mappings.
> NOTE: the comment in the C source says "A .O. B = [A .o. B] .P. B", but the
> code passes a COPY OF NET1 (A) as the fallback — port the code, not the
> comment. See `[spec:foma:sem:constructions.fsm-priority-union-upper-fn]` and
> `[spec:foma:sem:constructions.fsm-compose-fn]`.
> Ownership: net2 is consumed by the inner compose; net1 is copied twice and
> then destroyed with fsm_destroy before returning. The result is whatever
> fsm_priority_union_upper returns (a union result: not minimized).

> [spec:foma:def:constructions.fsm-letter-machine-fn]
> struct fsm *fsm_letter_machine(struct fsm *net)

> [spec:foma:sem:constructions.fsm-letter-machine-fn+1]
> Converts a machine whose labels may be multi-character strings into an
> equivalent "letter machine" where every arc carries a single UTF-8 character,
> by splitting each multi-character arc into a chain of arcs through fresh
> states.
> Setup: inh = fsm_read_init(fsm_minimize(net)); outh = fsm_construct_init with
> the literal name "name" (the input's name is NOT preserved); addstate =
> net->statecount, the next fresh state number (read through the original `net`
> pointer AFTER minimizing — safe only because fsm_minimize normally returns
> the same object; under Brzozowski minimization this is a use-after-free).
> For each arc (source, target, labels as both numbers and strings):
> - If neither side is a multi-char ordinary symbol (i.e. for both sides,
>   number <= IDENTITY (2) or utf8strlen(label) <= 1), copy the arc unchanged
>   (added by label strings).
> - Otherwise let inlen = 1 if innum <= IDENTITY else utf8strlen(in), same for
>   outlen; steps = max(inlen, outlen) (always >= 2 here). Emit `steps` arcs
>   forming a chain: step 0 goes source -> addstate (addstate++); intermediate
>   steps go addstate-1 -> addstate (addstate++); the final step goes
>   addstate-1 -> the original target. Labels per step: on the input side, if
>   innum <= IDENTITY the ORIGINAL special label string is repeated at every
>   step (e.g. @:xyz becomes @:x @:y @:z); if ordinary, the next single UTF-8
>   character of `in` is consumed (copied into a 128-byte buffer), and once
>   exhausted (inlen reaches 0) "@_EPSILON_SYMBOL_@" is used. Same for the
>   output side: the next single UTF-8 character of `out` is consumed (copy
>   sized by utf8skip(out)+1) and once exhausted "@_EPSILON_SYMBOL_@" is used.
> The C sized the output-side copy by the byte length of the
> current INPUT character (strncpy(tmpout, out, utf8skip(in)+1), with `in`
> possibly already advanced) while NUL-terminating at utf8skip(out)+1, so a
> multibyte output character following a shorter input character was garbled.
> The port sizes the output copy by utf8skip(out)+1, consuming exactly one
> UTF-8 output character per step.
> Finals and initials are copied from the input. Returns the machine built by
> fsm_construct_done (sigma rebuilt from the label strings actually used).
> Ownership: `net` is consumed by fsm_minimize; the minimized net itself is
> never fsm_destroy'ed (leaked). Result is a fresh network, not minimized.

> [spec:foma:def:constructions.fsm-mark-fsm-tail-fn]
> struct fsm *fsm_mark_fsm_tail(struct fsm *net, struct fsm *marker)

> [spec:foma:sem:constructions.fsm-mark-fsm-tail-fn]
> _marktail(?* L, 0:x): rule-compilation helper implementing
> ~$x .o. [..] -> x || L _ (and, via reversal, the right-context variant).
> Every arc of `net` that enters a FINAL state (finality of `net` marks "the
> context L has just been matched") is rerouted through a fresh intermediate
> state, from which the arcs of `marker` (typically the single arc 0:x) lead to
> the original target — inserting the marker just before each context match
> completes.
> Steps: open read handles on net (inh) and marker (minh); construct handle
> with net's name and net's sigma copied verbatim; mappings =
> calloc(net->statecount, sizeof(int)) caches target -> fresh state;
> maxstate = net->statecount is the next fresh state number. For each arc of
> net (source s, target t, labels by number): if t is final and mappings[t] is
> 0, allocate newtarget = maxstate++, record mappings[t] = newtarget, and
> replay every arc of marker (by label STRINGS, so marker symbols missing from
> net's sigma get added) as an arc newtarget -> t; if mappings[t] was already
> set, reuse it; then emit s -> newtarget with the arc's original labels. Arcs
> whose target is not final are copied unchanged. Afterwards EVERY original
> state 0..net->statecount-1 is marked final (the fresh marker states are not),
> the initial state is set to 0, and the net is built with fsm_construct_done
> (counts/flags recomputed).
> Notes: marker's own state structure, finality and initial state are ignored —
> only its arc labels matter (a multi-state marker is flattened into parallel
> newtarget -> t arcs). A final state with no incoming arcs (e.g. a final start
> state) gets no marker inserted. The mappings cache uses 0 for "unset", which
> is unambiguous because fresh numbers start at statecount >= 1.
> Ownership: net is destroyed; marker is NOT destroyed (caller keeps it);
> returns the fresh net, not minimized.

> [spec:foma:def:constructions.fsm-merge-sigma-fn]
> void fsm_merge_sigma(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:constructions.fsm-merge-sigma-fn]
> Core alphabet-harmonization pass run before every binary construction: merges
> the sigmas of net1 and net2 into one shared numbering, rewrites both nets'
> arcs to the new numbers, and expands UNKNOWN (1) / IDENTITY (2) arcs over the
> symbols known only to the other net. Both sigmas must be in sorted order
> (specials ascending by number first, then ordinary symbols in strcmp order),
> as sigma_sort guarantees.
> - Word boundary: unless the global fsm_options.skip_word_boundary_marker is
>   set, if exactly one sigma contains ".#.", add it to the other (sigma_add +
>   sigma_sort) before anything else.
> - Merged list: allocate int arrays mapping_1 and mapping_2 of
>   sigma_max(sigma1)+sigma_max(sigma2)+3 entries, and a mergesigma list seeded
>   with a dummy head (number -1, symbol NULL). Walk both sigma lists in
>   parallel, appending entries with add_to_mergesigma (see
>   `[spec:foma:sem:constructions.add-to-mergesigma-fn]`: ordinary symbols are
>   renumbered densely from 3 in merged order, specials keep their numbers).
>   While both cursors are live: if either current entry is special
>   (number <= IDENTITY), compare by number; otherwise compare symbols with
>   strcmp. Equal entries are appended once with presence 3 and both cursors
>   advance; otherwise the smaller entry is appended with presence 1 (net1-only)
>   or 2 (net2-only) and the flag `equal` (initially 1) is cleared. Once one
>   list is exhausted the rest of the other is appended one-sided (also
>   clearing `equal`). mapping_N[old number] = merged number is recorded for
>   ordinary matches, for one-sided ordinary appends, and for everything in the
>   tail phase; specials never need mapping. While scanning the special region,
>   set unknown_1 / unknown_2 if that side's sigma contains UNKNOWN or IDENTITY.
> - Arc renumbering: in both nets, every line's in/out that is > 2 is replaced
>   by its mapping (specials 0..2 and the -1 fields of final-marker/sentinel
>   lines are untouched).
> - Both old sigmas are destroyed and replaced by two independent deep copies
>   of the merged list (copy_mergesigma), so net1->sigma and net2->sigma are now
>   structurally equal.
> - Unknown expansion (per net, run only if that net had UNKNOWN or IDENTITY in
>   its sigma AND `equal` == 0): let a "new symbol" be a merged entry whose
>   presence equals the OTHER net only (2 when expanding net1, 1 for net2) and
>   whose number > IDENTITY. First net_unk = count of other-net-only entries
>   (literally including specials private to the other side — this only
>   over-allocates), and net_adds is summed per line: +net_unk for in ==
>   IDENTITY; +net_unk for ?:x (in UNKNOWN, out not UNKNOWN); +net_unk for x:?
>   (out UNKNOWN, in not UNKNOWN); +net_unk^2+net_unk for ?:?. A fresh array of
>   old_linecount + net_adds + 1 lines is filled by copying each line with
>   expansions (each added line reuses the original's target/final/start):
>   - in == IDENTITY: copy the line, then add sym:sym for every new symbol
>     (keyed on the input side only; assumes IDENTITY occurs only as @:@).
>   - ?:x (x may be EPSILON or ordinary): copy, then add sym:x per new symbol.
>   - x:?: copy, then add x:sym per new symbol.
>   - ?:?: copy, then add m:m2 for every ordered pair from
>     new-symbols-plus-UNKNOWN with m->number != m2->number and at least one of
>     the two being a new symbol (i.e. a:b for distinct new symbols, plus ?:b
>     and a:? for each new symbol).
>   - Lines whose in and out are both ordinary (> IDENTITY) or EPSILON are
>     copied unchanged, as are lines with in == -1 (final markers). CAUTION: a
>     line with IDENTITY on only the output side matches no case and is
>     silently dropped — well-formed foma nets never contain such arcs.
>   A sentinel is appended, the old array freed, net->states replaced.
> - The mergesigma list and mapping arrays are freed. Neither net's counts or
>   flags are updated (callers run fsm_count / fsm_update_flags afterwards);
>   both nets remain owned by the caller. No return value.

> [spec:foma:def:constructions.fsm-minus-fn]
> struct fsm *fsm_minus(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:constructions.fsm-minus-fn]
> Difference A - B by a parallel product run in which B may go "dead". Both
> nets are first minimized (making them deterministic, epsilon-free and trim),
> then fsm_merge_sigma(net1, net2) (with unknown expansion) and fsm_count on
> both. Pair states are stored in a triplet hash SHIFTED BY +1:
> (a+1, b+1, 0), where a stored second component of 0 means "B is dead" (B has
> fallen off the input and can no longer subtract anything). The worklist is
> the global int stack, explicitly cleared with int_stack_clear() first (unique
> to this function), then seeded with the pair (1,1) — i.e. (state 0, state 0)
> — inserted into the hash as (1,1,0) = new state 0. Per-state indexes come
> from init_state_pointers; output goes through the dynarray layer
> (fsm_state_init(sigma_max(net1->sigma)) etc.).
> For each popped pair: current_state = hash key of the shifted pair; then
> decrement a and b to real state numbers (b == -1 after decrementing means
> dead). If b is dead: current_start = 0, current_final = A-state's final flag;
> otherwise current_start = 1 iff a == 0 && b == 0, and current_final = 1 iff
> the A-state is final AND the B-state is NOT final.
> For each line of A-state a, stopping at final-marker lines (target == -1):
> - if B is dead, the target pair is (a_target+1, 0);
> - otherwise scan B-state b's lines for the first arc with identical in AND
>   out numbers (B is deterministic, so at most one): if found the target pair
>   is (a_target+1, b_target+1), else (a_target+1, 0) (B goes dead).
> Look the pair up in the hash; if absent push it and insert it (assigning the
> next state number); emit the arc with A's labels to that number. End each
> state with fsm_state_end_state.
> Finish: free net1's old state array, fsm_state_close(net1), free indexes,
> destroy net2, free the hash, and return fsm_minimize(net1).
> Notes: matching is by exact label pairs, so on transducers this computes
> path-wise (not relation-wise) difference; correctness relies on minimization
> having made both machines deterministic acceptor-like graphs. B need not be
> complete — missing B transitions simply send B dead, after which the
> remainder of A is accepted verbatim.

> [spec:foma:def:constructions.fsm-network-to-char-fn]
> char *fsm_network_to_char(struct fsm *net)

> [spec:foma:sem:constructions.fsm-network-to-char-fn]
> Returns a freshly strdup'ed copy of the LAST symbol in net's sigma linked
> list — the highest-numbered symbol, since sigmas are kept sorted by number —
> or NULL if the first node has number -1 (the empty-sigma dummy from
> sigma_create). Intended for elementary single-symbol networks, where the last
> entry is the symbol the net was built from. Walks the list keeping a trailing
> pointer, stopping at NULL or at a node with number -1, and duplicates that
> trailing node's symbol. The net is not modified or consumed; the caller owns
> the returned string. Crashes if net->sigma is NULL (cannot happen for nets
> made via fsm_create).

> [spec:foma:def:constructions.fsm-optionality-fn]
> struct fsm *fsm_optionality(struct fsm *net)

> [spec:foma:sem:constructions.fsm-optionality-fn]
> Optionality (A): calls fsm_kleene_closure(net, OPTIONALITY) with
> OPTIONALITY = 2, which short-circuits (before any of the closure copying
> logic runs — see `[spec:foma:sem:constructions.fsm-kleene-closure-fn]`) to
> fsm_union(net, fsm_empty_string()): the union of A with the empty-string
> language, built by the standard union construction
> (`[spec:foma:sem:constructions.fsm-union-fn]`). Consumes net; the result is
> not minimized.

> [spec:foma:def:constructions.fsm-precedes-fn]
> struct fsm *fsm_precedes(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:constructions.fsm-precedes-fn]
> Precedence language A < B = ~$[B ?* A]: the set of strings containing no
> factor from B that is later followed (with arbitrary material in between) by
> a factor from A — i.e. every occurrence of A precedes every occurrence of B.
> Built mechanically as fsm_complement(fsm_minimize(fsm_contains(fsm_minimize(
> fsm_concat(fsm_minimize(fsm_copy(net2)), fsm_concat(fsm_universal(),
> fsm_minimize(fsm_copy(net1)))))))) — note net2 (B) comes FIRST in the
> concatenation. fsm_contains wraps its argument as ?* X ?* and fsm_complement
> negates over the machine's own alphabet plus IDENTITY.
> Ownership: unusual for this module — net1 and net2 are only copied, never
> destroyed; the caller retains both inputs. Returns the complement result
> (complete, not pruned).

> [spec:foma:def:constructions.fsm-priority-union-lower-fn]
> struct fsm *fsm_priority_union_lower(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:constructions.fsm-priority-union-lower-fn]
> Priority union on the lower side, A .p. B = A | [B .o. ~[A.l]]: keeps all of
> A, plus those pairs of B whose LOWER-side (output) string is not a lower-side
> string of A. Built literally as
> fsm_union(fsm_copy(net1), fsm_compose(net2,
> fsm_complement(fsm_lower(fsm_copy(net1))))) — B is composed on top of the
> complement of A's lower projection, so B's outputs surviving are exactly
> those outside A.l; then unioned with A itself.
> Ownership: net2 is consumed by fsm_compose; net1 is copied twice and then
> destroyed with fsm_destroy. Returns the fsm_union result (not minimized,
> nondeterministic epsilon-start construction).

> [spec:foma:def:constructions.fsm-priority-union-upper-fn]
> struct fsm *fsm_priority_union_upper(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:constructions.fsm-priority-union-upper-fn]
> Priority union on the upper side, A .P. B = A | [~[A.u] .o. B]: keeps all of
> A, plus B's behavior on those UPPER-side (input) strings that A does not
> accept on its upper side. Built literally as
> fsm_union(fsm_copy(net1), fsm_compose(fsm_complement(fsm_upper(fsm_copy(net1))),
> net2)) — the complement of A's upper projection is composed above B, filtering
> B down to inputs A cannot handle; then unioned with A itself.
> Ownership: net2 is consumed by fsm_compose; net1 is copied twice and then
> destroyed with fsm_destroy. Returns the fsm_union result (not minimized,
> nondeterministic epsilon-start construction).

> [spec:foma:def:constructions.fsm-quotient-interleave-fn]
> struct fsm *fsm_quotient_interleave(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:constructions.fsm-quotient-interleave-fn]
> Interleaving quotient A/\/B: the set of strings that can be interleaved into
> B (net2) to obtain a string of A (net1). Implements the formula
> [B/[x \x* x] & A/x .o. [[[\x]:0]* (x:0 \x* x:0)]*].l with the auxiliary
> marker x = the symbol "@>@", assembled from primitives exactly as:
> - Left operand of the compose: fsm_intersect of
>   (1) fsm_ignore(net2, X X, OP_IGNORE_ALL) where the ignored language is
>   fsm_concat(fsm_symbol("@>@"), fsm_concat(fsm_kleene_star(
>   fsm_term_negation(fsm_symbol("@>@"))), fsm_symbol("@>@"))) — i.e. B with
>   bracketed segments "@>@ (non-@>@)* @>@" freely inserted; and
>   (2) fsm_ignore(net1, fsm_symbol("@>@"), OP_IGNORE_ALL) — A with single
>   "@>@" markers freely inserted. The intersection forces the inserted
>   bracketed segments of B to line up with material of A.
> - Right operand: fsm_kleene_star(fsm_concat(
>   fsm_kleene_star(fsm_cross_product(fsm_term_negation(fsm_symbol("@>@")),
>   fsm_empty_string())), fsm_optionality(fsm_concat(
>   fsm_cross_product(fsm_symbol("@>@"), fsm_empty_string()),
>   fsm_concat(fsm_kleene_star(fsm_term_negation(fsm_symbol("@>@"))),
>   fsm_cross_product(fsm_symbol("@>@"), fsm_empty_string())))))) — a
>   transducer deleting everything OUTSIDE bracket pairs and deleting the
>   brackets themselves, keeping only the bracketed contents.
> - Result = fsm_lower(fsm_compose(left, right)); finally "@>@" is removed
>   from Result->sigma with sigma_remove (arcs are already free of it; the C
>   comment notes the sigma "could" be cleaned up further but is not).
> Ownership: net1 and net2 are consumed (by the fsm_ignore calls). Returns
> Result, not minimized.

> [spec:foma:def:constructions.fsm-quotient-left-fn]
> struct fsm *fsm_quotient_left(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:constructions.fsm-quotient-left-fn]
> Left quotient A\\\B = [B .o. A:0 ?*].l: the set of suffixes that can be
> appended to a string of A (net1) to obtain a string of B (net2). Built
> literally as fsm_lower(fsm_compose(net2,
> fsm_concat(fsm_cross_product(net1, fsm_empty_string()), fsm_universal()))) —
> the transducer deletes an A-prefix (A crossed with the empty string) and
> passes the rest through ?*; composing B above it and taking the lower
> projection leaves exactly the possible remainders.
> Ownership: both net1 and net2 are consumed. Returns the lower projection
> (not minimized).

> [spec:foma:def:constructions.fsm-quotient-right-fn]
> struct fsm *fsm_quotient_right(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:constructions.fsm-quotient-right-fn]
> Right quotient A///B = [A .o. ?* B:0].l: the set of prefixes that can be
> extended by a string of B (net2) to obtain a string of A (net1). Built
> literally as fsm_lower(fsm_compose(net1, fsm_concat(fsm_universal(),
> fsm_cross_product(net2, fsm_empty_string())))) — the transducer passes an
> arbitrary prefix through ?* and deletes a B-suffix; composing A above it and
> taking the lower projection leaves exactly the prefixes.
> Ownership: both net1 and net2 are consumed. Returns the lower projection
> (not minimized).

> [spec:foma:def:constructions.fsm-sequentialize-fn]
> struct fsm *fsm_sequentialize(struct fsm *net)

> [spec:foma:sem:constructions.fsm-sequentialize-fn]
> Not implemented in foma: prints "Implementation pending\n" to stdout and
> returns the input `net` unchanged (no flags, counts or states touched). A
> port must reproduce this no-op behavior (or reject the operation), not invent
> a sequentialization algorithm.

> [spec:foma:def:constructions.fsm-shuffle-fn]
> struct fsm *fsm_shuffle(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:constructions.fsm-shuffle-fn]
> Shuffle (interleaving) product A || B: accepts every interleaving of a path
> of A with a path of B, built as a pair construction where at each step
> exactly one machine moves and the other stays.
> Setup: fsm_minimize is called on BOTH nets with the result DISCARDED (latent
> bug: safe only because fsm_minimize normally mutates in place and returns the
> same pointer — via determinize/coaccessible/Hopcroft; under Brzozowski
> minimization, g_minimize_hopcroft == 0, the input object is destroyed and the
> local pointers dangle). Then fsm_merge_sigma(net1, net2) (with unknown
> expansion) and fsm_count on both.
> Pair machinery (the standard pattern of this module): a triplet hash keyed
> (a, b, 0) maps state pairs to new state numbers in insertion order, seeded
> with (0,0,0) = state 0; the worklist is the global int stack seeded with the
> pair (0,0) (STACK_2_PUSH pushes b first so a pops first); per-state line
> indexes from init_state_pointers on both machines; output through the
> dynarray layer, initialized with fsm_state_init(sigma_max(net1->sigma))
> (which also deduplicates identical arcs per state).
> For each popped pair (a,b): new state number = hash key of (a,b,0); start
> iff both a and b are start states, final iff both are final. For every real
> arc of a (target != -1): emit its in:out from the current state to pair
> (a_target, b) — A moves, B stays — looking the pair up in the hash and
> pushing/inserting it if new. Symmetrically, for every real arc of b emit its
> in:out to pair (a, b_target). No epsilon, UNKNOWN or IDENTITY special-casing
> beyond what fsm_merge_sigma already did. fsm_state_end_state after each pair.
> Finish: free net1's old state array, fsm_state_close(net1) (installs the new
> lines, recomputes counts/determinism flags, pathcount unknown), free both
> indexes, destroy net2, free the hash. Returns net1: NOT minimized and NOT
> pruned (non-coaccessible pairs may remain, e.g. pairs where one machine can
> no longer reach a final state).

> [spec:foma:def:constructions.fsm-simple-replace-fn]
> struct fsm *fsm_simple_replace(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:constructions.fsm-simple-replace-fn]
> Unconditional obligatory replacement transducer A -> B, per the classic
> formula [~[?* [A-0] ?*] [A .x. B]]* ~[?* [A-0] ?*], where [A-0] (A minus the
> empty string) is realized as [A & ?+]. Assembled exactly as:
> - UPlus = fsm_minimize(fsm_kleene_plus(fsm_identity())) — the language ?+ of
>   all nonempty strings.
> - NotContainsA = fsm_complement(fsm_minimize(fsm_concat(fsm_concat(
>   fsm_universal(), fsm_minimize(fsm_intersect(fsm_copy(net1),
>   fsm_copy(UPlus)))), fsm_universal()))) — ~$[A-0], strings containing no
>   nonempty factor from A (built twice, once per occurrence in the formula).
> - ret = fsm_concat(fsm_minimize(fsm_kleene_star(fsm_minimize(fsm_concat(
>   NotContainsA, fsm_minimize(fsm_cross_product(fsm_copy(net1),
>   fsm_copy(net2))))))), fsm_minimize(NotContainsA')) — i.e. zero or more
>   rounds of "some A-free prefix, then one A rewritten to B via the cross
>   product A .x. B", followed by an A-free tail.
> Nonempty A-matches cannot be skipped (obligatory); the empty string in A is
> effectively ignored on the matching side but still contributes epsilon:B
> pairs through the cross product.
> Ownership: net1, net2 and UPlus are destroyed before returning. Returns ret
> (fsm_concat output: minimized).

> [spec:foma:def:constructions.fsm-sort-lines-fn]
> void fsm_sort_lines(struct fsm *net)

> [spec:foma:sem:constructions.fsm-sort-lines-fn]
> Sorts net's state-line array in place into ascending state_no order using
> qsort with the sort_cmp comparator, over exactly
> find_arccount(net->states) lines (the lines before the -1 sentinel, which
> therefore stays in place at the end). Because qsort is unstable, the relative
> order of lines WITHIN one state may be permuted arbitrarily; only grouping by
> state number is guaranteed. Restores the "grouped by state" invariant that
> init_state_pointers and most algorithms require. Touches no counts, flags or
> sigma (in particular arcs_sorted_in/out are NOT updated).

> [spec:foma:def:constructions.fsm-substitute-label-fn]
> struct fsm *fsm_substitute_label(struct fsm *net, char *original, struct fsm *substitute)

> [spec:foma:sem:constructions.fsm-substitute-label-fn]
> Splices the network `substitute` in place of every arc of `net` labeled with
> the symbol named `original`. Steps:
> - fsm_merge_sigma(net, substitute) first (aligns numbering, may expand
>   unknowns in both). addstate1 = net->statecount is the next fresh state
>   number; addstate2 = substitute->statecount.
> - Open read handles on net (inh) and substitute (subh); repsym =
>   fsm_get_symbol_number(inh, original). If original is not in the (now
>   merged) sigma, free inh only and return net unchanged — note net's sigma
>   has already been merged at this point, and subh plus the substitute net are
>   leaked.
> - Otherwise create a construct handle with net's name and net's sigma copied
>   verbatim, then process each arc of net (source s, target t, numbers
>   in/out):
>   - in == repsym && out == repsym: emit s --EPSILON:EPSILON--> addstate1;
>     copy every arc of substitute (by label STRINGS) shifted by +addstate1
>     (fsm_read_reset(subh) first, so the same handle is replayed per splice);
>     for each final state f of substitute emit
>     addstate1+f --EPSILON:EPSILON--> t; then addstate1 += addstate2.
>   - exactly one side == repsym: build subnet2 =
>     fsm_minimize(fsm_cross_product(fsm_copy(substitute),
>     fsm_symbol(out-label-string))) when in == repsym, or
>     fsm_minimize(fsm_cross_product(fsm_symbol(in-label-string),
>     fsm_copy(substitute))) when out == repsym — pairing the substitute
>     language with the arc's other-side symbol. Splice subnet2 in exactly as
>     above (epsilon in, shifted arc copies via a fresh read handle, epsilon
>     arcs from its finals to t), advance addstate1 by subnet2->statecount, and
>     destroy subnet2.
>   - neither side matches: copy the arc unchanged by numbers.
> - Copy net's finals and initials, free both read handles, and return the net
>   built by fsm_construct_done (counts recomputed, sigma sorted).
> Ownership: NEITHER net nor substitute is destroyed on any path (callers must
> free them); the result is always a fresh network (or `net` itself on the
> repsym-not-found path). Assumes both machines' initial state is 0.

> [spec:foma:def:constructions.fsm-substitute-symbol-fn]
> struct fsm *fsm_substitute_symbol(struct fsm *net, char *original, char *substitute)

> [spec:foma:sem:constructions.fsm-substitute-symbol-fn]
> Replaces every occurrence of the symbol named `original` on either side of
> net's arcs by the symbol named `substitute`, mutating net in place. Rules:
> - If the two names are strcmp-equal, return net untouched.
> - o = sigma_find(original, net->sigma); if absent (-1), return net untouched.
> - Replacement number s: if substitute is the literal string "0", s = EPSILON
>   (0); otherwise s = the existing sigma number of substitute, or the number
>   assigned by sigma_add if it is new.
> - Every line whose in (resp. out) equals o is rewritten to s.
> - Then: sigma_remove(original, ...) drops the old entry; sigma_sort(net)
>   re-sorts ordinary symbols alphabetically, renumbering them densely from 3
>   and remapping all arcs; fsm_update_flags(net, NO,NO,NO,NO,NO,NO);
>   sigma_cleanup(net, 0) removes ordinary symbols no longer used on any arc
>   (a no-op whenever the sigma still contains UNKNOWN or IDENTITY);
>   is_minimized is forced NO.
> - Returns fsm_determinize(net) — required because substituting toward
>   EPSILON or an already-present symbol can create epsilon arcs or
>   nondeterminism. The result is determinized but NOT minimized.
> Ownership: operates on and returns the same net object (no copy).

> [spec:foma:def:constructions.fsm-symbol-fn]
> struct fsm *fsm_symbol(char *symbol)

> [spec:foma:sem:constructions.fsm-symbol-fn]
> Builds the elementary one-symbol network for the label string `symbol`.
> Starts from fsm_create("") and fsm_update_flags(net, YES,YES,YES,YES,YES,NO).
> - symbol == "@_EPSILON_SYMBOL_@": sigma gets the EPSILON (0) special; the
>   state array is a single line for a final start state 0 with no arcs
>   (state 0, in/out/target -1, final 1, start 1) plus the sentinel. arccount
>   0, statecount 1, linecount 1, finalcount 1. Then is_deterministic,
>   is_minimized and is_epsilon_free are overridden to NO — a literal quirk
>   (the machine trivially is all three); ports must not rely on these flags
>   being YES for the epsilon machine.
> - Otherwise: symbol_no = sigma_add_special(IDENTITY, ...) (number 2) if
>   symbol == "@_IDENTITY_SYMBOL_@", else sigma_add(symbol, ...) (number 3 in
>   the fresh sigma). Three lines: start state 0 with one arc
>   symbol_no:symbol_no to state 1; state 1 final with a pure final-marker
>   line; sentinel. arity 1, pathcount 1, arccount 1, statecount 2, linecount
>   2, finalcount 1; arcs_sorted_in and arcs_sorted_out set YES (overriding
>   fsm_update_flags' NO), is_deterministic/is_minimized/is_epsilon_free YES.
> Returns the fresh net. Note: no epsilon entry is added to the sigma in the
> non-epsilon case, and UNKNOWN is never produced here.

> [spec:foma:def:constructions.fsm-symbol-occurs-fn]
> int fsm_symbol_occurs(struct fsm *net, char *symbol, int side)

> [spec:foma:sem:constructions.fsm-symbol-occurs-fn]
> Tests whether `symbol` actually labels an arc of net on the requested
> side(s). sym = sigma_find(symbol, net->sigma); returns 0 immediately if the
> symbol is not in the sigma. Otherwise scans all state lines: with side ==
> M_UPPER (1) it matches the in field, side == M_LOWER (2) the out field, and
> side == M_UPPER+M_LOWER (3) either field; returns 1 on the first hit and 0 if
> the scan completes without one. Any other `side` value returns 0. Purely
> observational — net is neither modified nor consumed. (Used e.g. by
> fsm_equal_substrings to detect leftover marker symbols on the lower side.)

> [spec:foma:def:constructions.fsm-term-negation-fn]
> struct fsm *fsm_term_negation(struct fsm *net1)

> [spec:foma:sem:constructions.fsm-term-negation-fn]
> Term negation \A: the set of SINGLE-symbol strings not in A. Returns
> fsm_intersect(fsm_identity(), fsm_complement(net1)): fsm_identity() is the
> fresh two-state @:@ machine accepting exactly one arbitrary symbol;
> fsm_complement (fsm_completes with COMPLEMENT) negates net1 over its own
> alphabet extended with IDENTITY; intersecting restricts the complement to
> length-1 strings. Consumes net1 (via fsm_complement). Returns the
> fsm_intersect result (coaccessible, not minimized).

> [spec:foma:def:constructions.fsm-unflatten-fn]
> struct fsm *fsm_unflatten(struct fsm *net, char *epsilon_sym, char *repeat_sym)

> [spec:foma:sem:constructions.fsm-unflatten-fn]
> Inverse of fsm_flatten: converts an ACCEPTOR over a "flattened" alphabet — in
> which each transduction pair is spelled as two consecutive symbols, input
> symbol then output symbol — back into a transducer by pairing arcs at even
> and odd path positions. `epsilon_sym` names the symbol that stands for
> EPSILON, `repeat_sym` the symbol meaning "output equals input".
> Setup: fsm_minimize(net) is called with its result DISCARDED (latent bug:
> safe only because minimization normally mutates in place; under Brzozowski
> minimization the old object is freed and `net` dangles); fsm_count(net);
> epsilon = sigma_find(epsilon_sym, net->sigma) and repeat =
> sigma_find(repeat_sym, net->sigma) (either may be -1 if absent, in which case
> it simply never matches). New states correspond to "even" original states,
> stored in a triplet hash as (s, s, 0), seeded with (0,0,0) = state 0; the
> worklist is the global int stack seeded with the pair (0,0). Note both
> int_stack_pop() results are assigned to the same variable `a` (harmless,
> since pairs are always (s,s)). Output goes through the dynarray layer
> (fsm_state_init(sigma_max(net->sigma)); duplicate arcs suppressed).
> For each popped even state a: current_state = hash key of (a,a,0); its start
> and final flags are taken directly from a. For every real arc of a
> (in = that arc's `in` field, b = its target) and, nested, every real arc of b
> (out = THAT arc's `in` field, t = its target — only input labels are read,
> acceptor assumption): look up / insert (t,t,0), pushing (t,t) if new;
> transform the label pair: if out == repeat then out = in; else if either side
> is IDENTITY (2), each IDENTITY side becomes UNKNOWN (1); afterwards any side
> equal to `epsilon` becomes EPSILON (0). Emit the arc in:out from
> current_state to the new number. fsm_state_end_state per state.
> Finish: free net's old state array; fsm_state_close(net) installs the new
> lines (counts/flags recomputed); free the state index and the hash; return
> the same net object. The sigma still contains epsilon_sym and repeat_sym
> entries. Odd-position states' finality is ignored: acceptance is decided by
> even states only, so an odd-length path contributes nothing.

> [spec:foma:def:constructions.fsm-union-fn]
> struct fsm *fsm_union(struct fsm *net1, struct fsm *net2)

> [spec:foma:sem:constructions.fsm-union-fn]
> Union A | B by the epsilon construction. Steps: fsm_merge_sigma(net1, net2)
> (aligning both sigmas, expanding unknowns), then fsm_count on both to refresh
> counts. Allocate a fresh line array of net1->linecount + net2->linecount + 2
> entries. Emit two lines for a new start state 0, both marked start and
> non-final: 0 --EPSILON:EPSILON--> 1 (net1's shifted start) and
> 0 --EPSILON:EPSILON--> net1->statecount+1 (net2's shifted start). Then copy
> all of net1's lines with state_no and target shifted by +1, and all of
> net2's lines shifted by +net1->statecount+1 (targets of -1 stay -1); in every
> copied line start_state is forced to 0 while final_state is preserved.
> Append the sentinel. Update net1 in place: statecount = s1+s2+1; linecount =
> total lines written INCLUDING the sentinel; arccount = 2 + number of copied
> lines with target != -1; finalcount = f1+f2. Free net1's old state array and
> install the new one; fsm_destroy(net2);
> fsm_update_flags(net1, NO,NO,NO,NO,UNK,NO); add EPSILON to net1's sigma if
> missing. Returns net1 — NOT minimized, nondeterministic (two start
> epsilons), pathcount left stale. Assumes both machines use state 0 as their
> start. Empty inputs need no special-casing: a final-state-free operand simply
> contributes an unproductive branch.

> [spec:foma:def:constructions.fsm-universal-fn]
> struct fsm *fsm_universal()

> [spec:foma:sem:constructions.fsm-universal-fn]
> Returns a fresh machine for the universal language ?* : fsm_create("") with
> fsm_update_flags(net, YES,YES,YES,YES,NO,NO) (deterministic, pruned,
> minimized, epsilon-free; loop-free NO; completed NO). The sigma receives only
> the IDENTITY (2) special via sigma_add_special. States: one line
> 0 --IDENTITY:IDENTITY--> 0 marked both final and start, plus the sentinel.
> Counts set explicitly: arccount 1, statecount 1, linecount 2, finalcount 1,
> pathcount = PATHCOUNT_CYCLIC (-1). Takes no arguments and consumes nothing.

> [spec:foma:def:constructions.fsm-update-flags-fn]
> void fsm_update_flags(struct fsm *net, int det, int pru, int min, int eps, int loop, int completed)

> [spec:foma:sem:constructions.fsm-update-flags-fn]
> Bulk flag setter used throughout the library: assigns the six arguments
> verbatim (values are the YES/NO/UNK constants 1/0/2) to
> net->is_deterministic, is_pruned, is_minimized, is_epsilon_free,
> is_loop_free and is_completed, in that argument order, and additionally
> ALWAYS clears arcs_sorted_in and arcs_sorted_out to NO. Touches nothing else
> (counts, pathcount, sigma, states all untouched). No return value.

> [spec:foma:def:constructions.init-state-pointers-fn]
> struct state_arr *init_state_pointers(struct fsm_state *fsm_state)

> [spec:foma:sem:constructions.init-state-pointers-fn]
> Builds the per-state lookup index used by every product construction in this
> module: from a state-line array, returns a malloc'ed array of struct
> state_arr entries indexed by state number, each holding that state's final
> flag, start flag, and a pointer to its FIRST line in the array.
> Steps: states = fsm_count_states(...) (counts distinct consecutive state_no
> runs); allocate states+1 entries (one spare, never initialized); zero the
> final and start fields of entries 0..states-1 (the transitions pointers start
> uninitialized). Then a single pass over the lines up to the sentinel: a line
> with final_state == 1 sets entry[state_no].final = 1; start_state == 1 sets
> .start = 1; and the first line of each state_no run (detected by comparing
> with the previously seen state_no, initialized to -1) sets .transitions to
> point at that line within the ORIGINAL array (aliased, not copied).
> Preconditions: lines must be grouped by state (fsm_sort_lines order) and
> state numbers must be dense 0..n-1 — a state number >= the run count causes
> out-of-bounds writes. Consumers iterate a state's lines as
> `for (p = entry[s].transitions; p->state_no == s; p++)`, which additionally
> relies on final-marker lines carrying the state's number and the sentinel
> terminating the last state. Caller frees the returned array only.

> [spec:foma:def:constructions.mergesigma]
> struct mergesigma {
>   char *symbol;
>   unsigned char presence;
>   int number;
>   struct mergesigma *next;
> }

> [spec:foma:def:constructions.sort-cmp-fn]
> int sort_cmp(const void *a, const void *b)

> [spec:foma:sem:constructions.sort-cmp-fn+1]
> qsort comparator over struct fsm_state lines: orders by
> a->state_no - b->state_no, i.e. ascending state number. Lines of
> the same state compare equal, so qsort may permute them arbitrarily
> (fsm_sort_lines guarantees grouping only, not intra-state order). state_no
> values are small non-negative ints (or -1 for the sentinel, which
> fsm_sort_lines excludes from the sorted range). Returns an Ordering
> (Less/Equal/Greater); the C `int` sign of the subtraction carries the same
> information.

> [spec:foma:def:constructions.state-arr]
> struct state_arr {
>   int final;
>   int start;
>   struct fsm_state *transitions;
> }

> [spec:foma:def:constructions.triplet-hash-find-fn]
> int triplet_hash_find(struct triplethash *th, int a, int b, int c)

> [spec:foma:sem:constructions.triplet-hash-find-fn]
> Looks up the triplet (a,b,c) in the open-addressing table. Starting at slot
> triplethash_hashf(a,b,c) % tablesize, probes linearly (wrapping at
> tablesize) for at most tablesize steps: hitting an EMPTY slot (key == -1)
> returns -1 immediately (the triplet cannot be further along the probe chain,
> since inserts never leave gaps); hitting a slot whose a, b and c all match
> returns that slot's stored key; after tablesize probes without either,
> returns -1. Non-mutating. In the product constructions the returned key is
> the new-state number previously assigned by triplet_hash_insert, and -1
> means "state pair not yet seen".

> [spec:foma:def:constructions.triplet-hash-free-fn]
> void triplet_hash_free(struct triplethash *th)

> [spec:foma:sem:constructions.triplet-hash-free-fn]
> Frees a triplet hash: if th is non-NULL, frees its triplets slot array (if
> non-NULL) and then the struct itself. NULL-tolerant no-op otherwise.

> [spec:foma:def:constructions.triplet-hash-init-fn]
> struct triplethash *triplet_hash_init()

> [spec:foma:sem:constructions.triplet-hash-init-fn]
> Allocates and returns a fresh triplet hash: an open-addressing (linear
> probing) table of int triplets used by the product constructions to number
> state pairs/triples. Initial tablesize is exactly 128 slots, occupancy 0,
> and every slot's key field set to -1 ("empty"); the a/b/c fields of empty
> slots are left uninitialized. Keys assigned by subsequent inserts are the
> consecutive integers 0, 1, 2, ... in insertion order.

> [spec:foma:def:constructions.triplet-hash-insert-fn]
> int triplet_hash_insert(struct triplethash *th, int a, int b, int c)

> [spec:foma:sem:constructions.triplet-hash-insert-fn]
> Inserts the triplet (a,b,c), assigning it the next sequential key, and
> returns that key. Probes linearly from triplethash_hashf(a,b,c) % tablesize
> (wrapping) for the first EMPTY slot (key == -1); stores a, b, c there with
> key = current occupancy; increments occupancy; if occupancy then exceeds
> tablesize/2, grows the table via triplet_hash_rehash (doubling); finally
> returns occupancy - 1 (the key just assigned — note the rehash does not
> change occupancy, so the return value is correct either way).
> IMPORTANT: does NOT check whether (a,b,c) is already present — inserting a
> duplicate triplet silently creates a second entry with a fresh key. Callers
> in this module always call triplet_hash_find first and insert only on -1.
> The keys, being consecutive from 0, serve directly as new state numbers.

> [spec:foma:def:constructions.triplet-hash-insert-with-key-fn]
> void triplet_hash_insert_with_key(struct triplethash *th, int a, int b, int c, int key)

> [spec:foma:sem:constructions.triplet-hash-insert-with-key-fn]
> Raw insertion used only by triplet_hash_rehash: probes linearly from
> triplethash_hashf(a,b,c) % tablesize (wrapping) for the first empty slot
> (key == -1) and stores a, b, c with the CALLER-SUPPLIED key. Does not update
> occupancy, never triggers a rehash, performs no duplicate check, and returns
> nothing. Loops forever if the table has no empty slot (cannot happen given
> the <= 0.5 load factor maintained by triplet_hash_insert).

> [spec:foma:def:constructions.triplet-hash-rehash-fn]
> void triplet_hash_rehash(struct triplethash *th)

> [spec:foma:sem:constructions.triplet-hash-rehash-fn]
> Doubles the table: allocates a new slot array of 2 * tablesize entries with
> all keys set to -1, installs it (updating th->tablesize BEFORE reinserting,
> so probes use the new size), then walks the old array and re-inserts every
> occupied entry (key != -1) via triplet_hash_insert_with_key, preserving each
> entry's existing key. occupancy is unchanged. Frees the old array. Triggered
> by triplet_hash_insert whenever occupancy exceeds half the table size.

> [spec:foma:def:constructions.triplethash]
> struct triplethash {
>   struct triplethash_triplets *triplets;
>   unsigned int tablesize;
>   int occupancy;
> }

> [spec:foma:def:constructions.triplethash-hashf-fn]
> unsigned int triplethash_hashf(int a, int b, int c)

> [spec:foma:sem:constructions.triplethash-hashf-fn]
> Hash function for int triplets: computes a * 7907 + b * 86028157 + c * 7919
> in (signed) int arithmetic and casts the result to unsigned int. The
> multiplications and additions are expected to wrap modulo 2^32 (technically
> signed-overflow UB in C; a port must use explicitly wrapping 32-bit
> arithmetic to reproduce the same slot sequence). The exact prime constants
> 7907, 86028157 and 7919 are load-bearing for hash distribution but any port
> may reproduce them verbatim; callers always reduce the result modulo the
> table size.

> [spec:foma:def:constructions.triplethash-triplets]
> struct triplethash_triplets {
>   int a;
>   int b;
>   int c;
>   int key;
> }


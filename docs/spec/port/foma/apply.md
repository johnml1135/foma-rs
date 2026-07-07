# foma/apply.c

> [spec:foma:def:apply.apply-add-flag-fn]
> void apply_add_flag(struct apply_handle *h, char *name)

> [spec:foma:sem:apply.apply-add-flag-fn]
> Registers a flag-diacritic feature name in the handle's global flag-state list `h->flag_list`
> (singly linked nodes of {name, value, neg, next}). If the list is empty, allocate a node and
> make it the head. Otherwise walk the list comparing names with strcmp; if an equal name exists,
> return without change (duplicates collapse, so all diacritics on one feature share state).
> Otherwise allocate a new node and append it after the last node. The new node stores the `name`
> pointer as-is (not copied; the handle takes over the string allocated by the flag parser),
> value = NULL (unset), neg = 0, next = NULL. Called once per flag symbol found in sigma by
> `[spec:foma:sem:apply.apply-create-sigarray-fn]`.

> [spec:foma:def:apply.apply-add-sigma-trie-fn]
> void apply_add_sigma_trie(struct apply_handle *h, int number, char *symbol, int len)

> [spec:foma:sem:apply.apply-add-sigma-trie-fn]
> Inserts `symbol` (exactly `len` bytes) into the byte-wise sigma trie rooted at h->sigma_trie,
> mapping the byte sequence to sigma number `number`. Each trie level is a 256-cell array of
> {signum, next} indexed by unsigned byte value. For i = 0..len-1: index the current level with
> byte symbol[i]; if i is the last byte, set that cell's signum = number; otherwise, if the cell's
> next level is NULL, calloc a fresh 256-cell level (signum 0 = "no symbol ends here"), link it as
> next, and prepend the new array to the h->sigma_trie_arrays list (kept only so
> `[spec:foma:sem:apply.apply-clear-fn]` can free all levels); then descend into next.
> signum 0 as the absent sentinel is safe because only sigma numbers > IDENTITY (2) are inserted;
> EPSILON (0), UNKNOWN (1), IDENTITY (2) are never in the trie.

> [spec:foma:def:apply.apply-append-fn]
> int apply_append(struct apply_handle *h, int cptr, int sym)

> [spec:foma:sem:apply.apply-append-fn+1]
> Renders the label of arc line `cptr` into h->outstring at byte offset h->opos and returns the
> number of bytes emitted (no NUL terminator is written by the normal paths; the caller terminates
> in `[spec:foma:sem:apply.apply-return-string-fn]`). `sym` is the output-side symbol number for
> the current direction. Reads the raw arc symbols symin = gstates[cptr].in and
> symout = gstates[cptr].out and their display strings/lengths astring/alen, bstring/blen from
> h->sigs. Growth policy: while alen + blen + opos + 2 + strlen(separator) >= h->outstringtop,
> realloc outstring to double size and double outstringtop. If has_flags and show_flags is off,
> any side whose symbol is a flag diacritic is replaced by "" (length 0). Then:
> ENUMERATE mode, both sides (UPPER and LOWER both set): if astring == bstring (pointer equality;
> true when symin == symout) copy astring alone (len = alen); else copy astring + separator +
> bstring (len = alen + blen + strlen(separator)). EPSILON is not suppressed on this path: an
> epsilon:epsilon arc prints the epsilon display symbol.
> ENUMERATE, one side only: an EPSILON side becomes ""; emit astring if the mode's side is UPPER,
> else bstring (memcpy of len bytes).
> Non-ENUMERATE (real word application): (1) if print_pairs is on and symin != symout: emit
> "<" + astring + separator + bstring + ">", len = alen + blen + 2 + strlen(separator); first, if
> symin == UNKNOWN in DOWN mode (resp. symout == UNKNOWN in UP mode), strncpy 1 byte of the input
> at instring+ipos into the sigma display string itself — this writes into the string literal "?"
> installed for UNKNOWN (undefined behavior, latent crash) and truncates multibyte input to one
> byte. (2) else if sym == IDENTITY: copy sigmatch_array[ipos].consumes bytes verbatim from
> instring+ipos (the matched input token) plus one NUL after them; len = consumes. The growth
> check above budgeted only alen + blen bytes (1 + 1 for "@"/"?"), so a token whose consumes
> exceeds that can overflow outstring — latent buffer overflow. (3) else if sym == EPSILON:
> return 0 immediately (nothing emitted). (4) else memcpy the display string of the output side
> (bstring in DOWN mode, astring in UP mode).
> Finally, if print_space is on and len > 0, strcpy h->space_symbol after the emitted bytes and
> increment len by the space symbol's full byte length (strlen(space_symbol)), so a multi-byte
> separator survives intact. The C source incremented len by exactly 1 regardless of the symbol's
> length, overwriting all but the separator's first byte on the next append; single-byte separators
> are unaffected either way.

> [spec:foma:def:apply.apply-at-last-arc-fn]
> int apply_at_last_arc(struct apply_handle *h)

> [spec:foma:sem:apply.apply-at-last-arc-fn]
> Returns 1 if the arc currently addressed by the resumption cursor is the last candidate arc of
> its state (so backtracking must pop further rather than trying siblings), else 0. Three regimes
> mirroring `[spec:foma:sem:apply.apply-follow-next-arc-fn]`:
> (1) h->state_has_index set: last iff h->iptr->next == NULL or h->iptr->next->fsmptr == -1
> (an empty EPSILON head slot terminates the chain).
> (2) else if h->binsearch and the state is flag-free (net has no flags, or the state's bit in
> h->flagstates is clear): last iff the next arc line belongs to a different state_no, or, with
> seeksym = sigmatch_array[ipos].signumber and nextsym = the input-side symbol of line h->ptr
> (in if mode has DOWN, else out), nextsym == -1 or seeksym < nextsym (arcs sorted: no later
> sibling can match). sigmatch_array[ipos] is read without a bounds check; when
> ipos == current_instring_length this reads one entry past the tokenized input — stale data, and
> a true out-of-bounds read when the array was reallocated to exactly the input length (latent bug).
> (3) otherwise: last iff the next arc line has a different state_no.

> [spec:foma:def:apply.apply-binarysearch-fn]
> int apply_binarysearch(struct apply_handle *h)

> [spec:foma:sem:apply.apply-binarysearch-fn]
> For states whose arcs are sorted on the current input side and that carry no flag arcs: finds
> the first arc at or after h->ptr within the current state's block whose input-side symbol
> (in if mode has DOWN, else out) can match the next input token; sets h->curr_ptr to it and
> returns 1, else returns 0. Steps: curr_ptr = ptr; nextsym = symbol at ptr. If nextsym == EPSILON
> return 1 (epsilons sort first; curr_ptr = ptr). If nextsym == -1 return 0 (dummy line of an
> arcless state). If ipos >= current_instring_length return 0 (only epsilon arcs, already handled,
> could apply). seeksym = sigmatch_array[ipos].signumber. If seeksym == nextsym, or
> nextsym == UNKNOWN and seeksym == IDENTITY, return 1 with curr_ptr = ptr. Otherwise let
> lastptr = statemap[state] + numlines[state] - 1 (last line of the block) and start at ptr+1.
> If seeksym == IDENTITY (an UNKNOWN arc also matches, and UNKNOWN=1 sorts before IDENTITY=2, so
> bisection is unsound) or lastptr - thisptr < APPLY_BINSEARCH_THRESHOLD (10), scan linearly:
> succeed on symbol == seeksym or (symbol == UNKNOWN and seeksym == IDENTITY); return 0 as soon
> as symbol > seeksym or symbol == -1. Otherwise bisect the sorted range for seeksym; on a hit,
> rewind to the first line of the run of equal symbols (cannot cross the block start because the
> arc at ptr was already known unequal), set curr_ptr there, return 1; return 0 if the interval
> empties.

> [spec:foma:def:apply.apply-check-flag-fn]
> int apply_check_flag(struct apply_handle *h, int type, char *name, char *value)

> [spec:foma:sem:apply.apply-check-flag-fn]
> Implements flag-diacritic semantics against the global state in h->flag_list; returns
> SUCCEED (1) or FAIL (0). First find the flag_list node whose name strcmp-equals `name` — no NULL
> guard, so an unregistered name dereferences NULL (unreachable in practice: every feature is
> pre-registered by `[spec:foma:sem:apply.apply-create-sigarray-fn]`). Unconditionally save the
> node's current value into h->oldflagvalue and its neg into h->oldflagneg so the caller can push
> them for backtrack restore. Then dispatch on `type`:
> FLAG_UNIFY (1, U.f.v): if stored value is NULL, set value = strdup(value), succeed. Else if
> values strcmp-equal and neg == 0, succeed. Else if values differ and neg == 1 (stored is a
> negative setting for a different value), set value = strdup(value), neg = 0, succeed. Otherwise
> fail (equal-but-negated, or unequal-positive).
> FLAG_CLEAR (2, C.f): set value = NULL, neg = 0; always succeed.
> FLAG_DISALLOW (4, D.f / D.f.v): if stored value is NULL, succeed. If `value` is NULL (bare D.f)
> and stored is set, fail. If values differ: fail if neg == 1, else succeed. If values equal:
> succeed if neg == 1, else fail.
> FLAG_NEGATIVE (8, N.f.v): store the `value` pointer as-is (no strdup — aliases the flag_lookup
> string), neg = 1; succeed.
> FLAG_POSITIVE (16, P.f.v): store the `value` pointer as-is, neg = 0; succeed.
> FLAG_REQUIRE (32, R.f / R.f.v): with `value` NULL (bare R.f): fail iff stored value is NULL.
> With a value: fail if stored is NULL, fail if values differ, fail if equal but neg == 1, else
> succeed.
> FLAG_EQUAL (64, E.f.g): here `value` names another feature; find its node flist2. If flist2 is
> absent: succeed iff this feature's stored value is NULL. If either stored value is NULL: succeed
> iff both are NULL and the neg fields are equal. Otherwise succeed iff the values strcmp-equal
> and the neg fields are equal.
> Any other type prints "***Don't know what do with flag [type][name][value]" to stderr and fails.
> Values strdup'd by U are never freed (leak); P and N make the stored value alias flag_lookup.

> [spec:foma:def:apply.apply-clear-flags-fn]
> void apply_clear_flags(struct apply_handle *h)

> [spec:foma:sem:apply.apply-clear-flags-fn]
> Resets the global flag state: for every node in h->flag_list set value = NULL and neg = 0.
> Previously stored values are not freed (strings strdup'd by FLAG_UNIFY leak). The list
> structure itself is preserved.

> [spec:foma:def:apply.apply-clear-fn]
> void apply_clear(struct apply_handle *h)

> [spec:foma:sem:apply.apply-clear-fn]
> Destroys an apply handle and everything it owns. Frees, in order: every trie level recorded in
> h->sigma_trie_arrays plus the list nodes themselves (releasing the whole sigma trie), setting
> the list head to NULL; then, if non-NULL (NULLing each field afterwards): statemap, numlines,
> marks, searchstack, sigs, flag_lookup, sigmatch_array, flagstates. Calls
> `[spec:foma:sem:apply.apply-clear-index-fn]` to release both state indexes. Sets
> last_net = NULL and iterator = 0, then frees outstring, separator, epsilon_symbol, and finally
> the handle itself. The fsm is not owned and not freed. Leaks: h->flag_list nodes (and their
> name strings), h->space_symbol, and any strdup'd flag values.

> [spec:foma:def:apply.apply-clear-index-fn]
> void apply_clear_index(struct apply_handle *h)

> [spec:foma:sem:apply.apply-clear-index-fn]
> Releases both per-state arc indexes if present: for h->index_in and then h->index_out, if
> non-NULL, call `[spec:foma:sem:apply.apply-clear-index-list-fn]` on it, free the outer
> per-state pointer array, and set the field to NULL.

> [spec:foma:def:apply.apply-clear-index-list-fn]
> void apply_clear_index_list(struct apply_handle *h, struct apply_state_index **index)

> [spec:foma:sem:apply.apply-clear-index-list-fn]
> Frees the per-state index arrays of one index built by `[spec:foma:sem:apply.apply-index-fn]`.
> Returns immediately if `index` is NULL. For each state i in 0..statecount-1 with index[i]
> non-NULL: remember iptr_zero = index[i] (the base sigma_size array, whose element 0 is the
> EPSILON slot); for each symbol slot j from sigma_size-1 down to 0, walk that slot's ->next
> chain freeing each heap-allocated overflow node, stopping at NULL or on reaching iptr_zero
> (the tails of all non-EPSILON chains point back into the base array's EPSILON slot and must
> not be freed as separate nodes); then free the base array index[i]. The outer array itself is
> left for the caller.

> [spec:foma:def:apply.apply-create-sigarray-fn]
> void apply_create_sigarray(struct apply_handle *h, struct fsm *net)

> [spec:foma:sem:apply.apply-create-sigarray-fn]
> Builds the symbol tables used by application from net->sigma. Sets
> h->sigma_size = sigma_max(net->sigma) + 1. Allocates h->sigmatch_array as a calloc'd array of
> 1024 {signumber, consumes} entries with h->sigmatch_array_size = 1024 (resized later by
> `[spec:foma:sem:apply.apply-create-sigmatch-fn]`), and h->sigs as sigma_size slots of
> {symbol, length}. Initializes has_flags = 0, flag_list = NULL. Callocs the 256-cell root level
> of the sigma trie and records it as the first node of h->sigma_trie_arrays. For every sigma
> entry (walking h->gsigma until NULL or number == -1): if flag_check recognizes the symbol as a
> flag diacritic, set has_flags = 1 and register its feature name (from flag_get_name, freshly
> allocated) via `[spec:foma:sem:apply.apply-add-flag-fn]`; store
> sigs[number] = {symbol pointer borrowed from sigma, strlen(symbol)}; if number > IDENTITY,
> insert it into the trie via `[spec:foma:sem:apply.apply-add-sigma-trie-fn]`. If
> maxsigma >= IDENTITY, install the reserved displays: sigs[EPSILON] = {h->epsilon_symbol, its
> strlen}, sigs[UNKNOWN] = {"?", 1}, sigs[IDENTITY] = {"@", 1} (string literals). If has_flags:
> allocate h->flag_lookup with sigma_size entries initialized to {type 0, name NULL, value NULL},
> then walk sigma again (this second walk lacks the number != -1 guard of the first) storing
> {flag_get_type, flag_get_name, flag_get_value} for each flag symbol at its number; finally call
> `[spec:foma:sem:apply.apply-mark-flagstates-fn]`.

> [spec:foma:def:apply.apply-create-sigmatch-fn]
> void apply_create_sigmatch(struct apply_handle *h)

> [spec:foma:sem:apply.apply-create-sigmatch-fn]
> Tokenizes h->instring into h->sigmatch_array so matching is O(1) per arc: entry i (written only
> at token-start byte offsets) holds {signumber, consumes} — the sigma number matched at offset i
> and how many input bytes it spans. No-op when mode has ENUMERATE (no input string). Sets
> h->current_instring_length = strlen(instring). If that length >= sigmatch_array_size, free the
> array and malloc exactly length entries (size = length; note there is no entry for index ==
> length, making later unguarded reads at ipos == length out of bounds — see
> `[spec:foma:sem:apply.apply-at-last-arc-fn]` and `[spec:foma:sem:apply.apply-set-iptr-fn]`).
> For i from 0, advancing by consumes: walk the sigma trie byte by byte from offset i, recording
> in lastmatch the signum of the deepest cell whose signum != 0 (longest-leftmost match; the walk
> stops at the string's NUL, at a matched cell with no deeper level, or at a dead end). If
> lastmatch != 0: signumber = lastmatch and consumes = sigs[lastmatch].length. Otherwise:
> signumber = IDENTITY and consumes = utf8skip(symbol+i) + 1 (the byte length of one UTF-8
> character). Then, while the bytes at i+consumes form a Unicode combining character (ranges
> 0300-036F, 1AB0-1ABE, 1DC0-1DFF, 20D0-20F0, FE20-FE2D, per utf8iscombining), add that
> character's byte length to consumes and force signumber = IDENTITY: a base symbol plus
> combining marks is one unknown symbol even when the base alone is in sigma (rationale: had
> base+mark been a sigma symbol, longest match would already have found it). Store consumes.
> Byte positions inside a token keep stale values from earlier calls; they are never consulted
> because ipos always advances by whole tokens.

> [spec:foma:def:apply.apply-create-statemap-fn]
> void apply_create_statemap(struct apply_handle *h, struct fsm *net)

> [spec:foma:sem:apply.apply-create-statemap-fn]
> Builds per-state line tables from net->states. Allocates h->statemap, h->marks, h->numlines,
> each one int per state, initialized to statemap[s] = -1, marks[s] = 0, numlines[s] = 0. Scans
> the arc-line array up to the state_no == -1 sentinel: numlines[s] counts the lines belonging to
> state s (an arcless state still contributes its single dummy line with in/target == -1), and
> statemap[s] records the index of the first line of state s (left -1 for absent states).
> statemap translates a target state number to its first arc line; numlines bounds binary search.

> [spec:foma:def:apply.apply-down-fn]
> char *apply_down(struct apply_handle *h, char *word)

> [spec:foma:sem:apply.apply-down-fn]
> Applies `word` on the input (upper) side, yielding lower-side strings, with the iterator
> protocol of `[spec:foma:sem:apply.apply-updown-fn]` (pass the word once, then NULL for further
> results). Sets h->mode = DOWN (16); sets h->indexed to 1 iff h->index_in exists (write-only
> bookkeeping — nothing in this module reads h->indexed); sets h->binsearch = 1 iff
> h->last_net->arcs_sorted_in == 1, else 0; delegates to apply_updown. Note h->last_net is
> dereferenced before apply_updown's NULL guard, so a NULL net crashes here.

> [spec:foma:def:apply.apply-enumerate-fn]
> char *apply_enumerate(struct apply_handle *h)

> [spec:foma:sem:apply.apply-enumerate-fn]
> Common driver for the word-enumeration and random entry points. Returns NULL if h->last_net is
> NULL or has no final states (finalcount == 0). Forces h->binsearch = 0 (enumeration never
> bisects). If h->iterator == 0 (fresh enumeration): set iterate_old = 0, clear any leftover
> search stack and marks via `[spec:foma:sem:apply.apply-force-clear-stack-fn]`, run
> `[spec:foma:sem:apply.apply-net-fn]`, and, unless mode has RANDOM, increment h->iterator so the
> next call resumes instead of restarting. Otherwise (iterator != 0): set iterate_old = 1 and call
> apply_net to resume the suspended search. Returns apply_net's result (a pointer into
> h->outstring, or NULL when the enumeration is exhausted).

> [spec:foma:def:apply.apply-follow-next-arc-fn]
> int apply_follow_next_arc(struct apply_handle *h)

> [spec:foma:sem:apply.apply-follow-next-arc-fn]
> Core DFS step: starting from the resumption cursor (h->ptr = current arc line, h->iptr =
> current index node), find the next arc of the current state that matches the input at h->ipos
> and passes flag checks, commit it (append output, push a backtrack frame, move to the target
> state), and return 1; return 0 if no remaining arc works. The input-side symbol symin and
> output-side symout are the arc's (in,out) when mode has DOWN, (out,in) otherwise. Strategy:
> (1) Indexed state (h->state_has_index): iterate h->iptr along the index chain while non-NULL
> and iptr->fsmptr != -1, setting ptr = curr_ptr = iptr->fsmptr per candidate; on rejection
> advance iptr = iptr->next.
> (2) Binary search — when h->binsearch and the state is flag-free (no flags in the net, or its
> h->flagstates bit clear): repeatedly call `[spec:foma:sem:apply.apply-binarysearch-fn]`; when
> a found candidate is rejected, if the following line still belongs to the same state advance
> curr_ptr and ptr to it (returning 0 if that line's target is -1) and search again; otherwise
> return 0. This path never involves flags, so frames are pushed with NULL flag fields.
> (3) Linear scan: iterate curr_ptr from ptr across the state's block (same state_no, in != -1).
> In RANDOM mode each iteration first counts the block's arcs (vcount) and overwrites
> curr_ptr = ptr + rand() % vcount (or ptr if vcount == 0) — a uniform pick with replacement;
> the outer loop then increments from the last pick and re-tests the block condition, so the
> number of retries is nondeterministic, failing arcs can be re-picked, and matching arcs missed.
> Per candidate, all strategies do: marksource = marks[current state]; marktarget = marks[target
> state] (target's first line found via statemap); eatupi =
> `[spec:foma:sem:apply.apply-match-length-fn]`(symin) — a pure lookahead. Epsilon-loop check:
> reject the arc if eatupi == -1 or marktarget == -1 - ipos - eatupi, i.e. the target state was
> already entered twice at the input position we would arrive at (see
> `[spec:foma:sem:apply.apply-mark-state-fn]`). Then eatupi =
> `[spec:foma:sem:apply.apply-match-str-fn]`(symin, ipos), which may mutate flag state; reject on
> -1. On acceptance: eatupo = `[spec:foma:sem:apply.apply-append-fn]`(curr_ptr, symout); if
> obey_flags and has_flags and symin's flag type is one of
> FLAG_UNIFY|FLAG_CLEAR|FLAG_POSITIVE|FLAG_NEGATIVE (the state-mutating flags), capture
> fname = the feature name and fvalue/fneg = h->oldflagvalue/h->oldflagneg (the pre-mutation
> state stashed by apply_check_flag), else fname = fvalue = NULL, fneg = 0; push a frame via
> `[spec:foma:sem:apply.apply-stack-push-fn]`(marksource, fname, fvalue, fneg); set
> ptr = statemap[target], ipos += eatupi, opos += eatupo; recompute the index cursor with
> `[spec:foma:sem:apply.apply-set-iptr-fn]`; return 1.

> [spec:foma:def:apply.apply-force-clear-stack-fn]
> static void apply_force_clear_stack(struct apply_handle *h)

> [spec:foma:sem:apply.apply-force-clear-stack-fn]
> Ensures the search stack is empty and all visit marks left by an abandoned search are cleared.
> If the stack is non-empty: zero marks[state of current h->ptr]; then repeatedly pop
> (`[spec:foma:sem:apply.apply-stack-pop-fn]` restores ptr/ipos/opos/iptr/flag state/mark) and
> zero the mark of each restored state, until the stack is empty; then set h->iterator = 0 and
> h->iterate_old = 0 and clear the stack pointer. If the stack is already empty, do nothing
> (marks are assumed clean).

> [spec:foma:def:apply.apply-index-fn]
> void apply_index(struct apply_handle *h, int inout, int densitycutoff, int mem_limit, int flags_only)

> [spec:foma:sem:apply.apply-index-fn]
> Builds a per-state, per-symbol index over the arc array for lookup direction `inout`
> (APPLY_INDEX_INPUT = 1 indexes each arc's in symbol into h->index_in; anything else indexes the
> out symbol into h->index_out). Returns immediately if flags_only is set but the net has no
> flags. Pass 1 over the arc lines computes maxtrans, the largest per-state count of real arcs
> (target != -1). Pass 2 buckets states by their arc count into pre_index[count] (the calloc'd
> bucket head, initialized to state_no -1, is reused when free; further states are prepended as
> nodes). Both passes only close out a state when the next line's state_no differs, so the final
> state block before the -1 sentinel is never registered and therefore never indexed — latent
> bug, harmless because unindexed states fall back to linear/binary traversal.
> Memory accounting: a running counter cnt adds round_up_to_power_of_two(bytes) per allocation
> and is compared with mem_limit. If even the statecount-sized pointer array would exceed the
> limit, skip indexing entirely (the resulting index pointer is NULL). Otherwise calloc the
> per-state pointer array. If flags_only, ensure h->flagstates exists via
> `[spec:foma:sem:apply.apply-mark-flagstates-fn]`. Visit states densest first (bucket maxtrans
> down to 0): skip states with fewer than densitycutoff arcs unless flags_only and the state has
> flag arcs; if adding sigma_size cells would exceed mem_limit, stop allocating (states already
> given arrays keep them); otherwise malloc a sigma_size array of {fsmptr, next} cells, each
> fsmptr = -1, the EPSILON (0) slot's next = NULL, and every other slot's next = the base EPSILON
> slot, so chain traversal automatically falls through to epsilon arcs.
> Fill pass over all arc lines with target != -1 whose source state got an array: sym = the
> indexed side's symbol; a flag-diacritic symbol is indexed under EPSILON (it consumes no input);
> UNKNOWN is indexed under IDENTITY (identical match set). The first arc for a symbol goes into
> the slot's fsmptr; later arcs become calloc'd overflow nodes spliced immediately after the slot
> head, so traversal order is first arc, then the remaining arcs newest-first. Overflow nodes are
> counted in cnt but never limit-checked. Finally free the pre_index buckets and store the array
> in h->index_in or h->index_out.

> [spec:foma:def:apply.apply-init-fn]
> struct apply_handle *apply_init(struct fsm *net)

> [spec:foma:sem:apply.apply-init-fn]
> Creates and returns an apply handle over `net` (borrowed, never owned or freed). Seeds the C
> PRNG with srand((unsigned)time(NULL)) on every call, so random applies are time-seeded and two
> handles created within the same second replay identical sequences. callocs the handle, then
> sets: iterate_old = 0, iterator = 0, instring = NULL, flag_list = NULL, flag_lookup = NULL,
> obey_flags = 1, show_flags = 0, print_space = 0, print_pairs = 0, separator = strdup(":"),
> epsilon_symbol = strdup("0"), last_net = net, outstring = malloc(DEFAULT_OUTSTRING_SIZE = 4096)
> with outstring[0] = '\0' and outstringtop = 4096, gstates = net->states, gsigma = net->sigma,
> printcount = 1. Builds the state tables via `[spec:foma:sem:apply.apply-create-statemap-fn]`,
> allocates the search stack with DEFAULT_STACK_SIZE (128) frames (apply_stack_top = 128, stack
> pointer cleared), and builds the symbol tables via
> `[spec:foma:sem:apply.apply-create-sigarray-fn]`.

> [spec:foma:def:apply.apply-lower-words-fn]
> char *apply_lower_words(struct apply_handle *h)

> [spec:foma:sem:apply.apply-lower-words-fn]
> Enumerates lower-side (output) words: sets h->mode = DOWN + ENUMERATE + LOWER and returns
> `[spec:foma:sem:apply.apply-enumerate-fn]`. Each call yields the next path's lower string;
> NULL when exhausted.

> [spec:foma:def:apply.apply-mark-flagstates-fn]
> void apply_mark_flagstates(struct apply_handle *h)

> [spec:foma:sem:apply.apply-mark-flagstates-fn]
> (Re)builds h->flagstates, a bit array (one bit per state, BITNSLOTS(statecount) bytes,
> calloc'd) marking states that have at least one arc whose in or out symbol is a flag diacritic;
> such states are excluded from binary search. No-op if has_flags is false or flag_lookup is
> NULL. Frees any previous array first. For every arc line with target != -1, sets the source
> state_no's bit if flag_lookup[in].type or flag_lookup[out].type is nonzero.

> [spec:foma:def:apply.apply-mark-state-fn]
> void apply_mark_state(struct apply_handle *h)

> [spec:foma:sem:apply.apply-mark-state-fn]
> Records arrival at the current state (state of line h->ptr) for epsilon-loop detection. No-op
> in RANDOM mode. Encoding of marks[s]: 0 = unseen; ipos+1 = last entered having consumed ipos
> input bytes; -(ipos+1) = entered a second time at that same position. If marks[s] already
> equals h->ipos+1, set it to -(h->ipos+1); otherwise set it to h->ipos+1. Combined with the
> rejection test in `[spec:foma:sem:apply.apply-follow-next-arc-fn]` (reject when the target's
> mark equals -1 - ipos - eatupi), a state can be entered at most twice per input position on one
> DFS branch — once around an epsilon cycle — and a third entry is pruned.

> [spec:foma:def:apply.apply-match-length-fn]
> int apply_match_length(struct apply_handle *h, int symbol)

> [spec:foma:sem:apply.apply-match-length-fn]
> Pure lookahead (no side effects): returns how many input bytes `symbol` would consume at
> h->ipos, or -1 if it cannot match. EPSILON consumes 0. A flag-diacritic symbol (has_flags and
> flag_lookup[symbol].type nonzero) consumes 0 — consistency is not checked here. In ENUMERATE
> mode everything consumes 0. Otherwise: if ipos >= current_instring_length return -1 (input
> exhausted); if sigmatch_array[ipos].signumber == symbol return its consumes; if symbol is
> IDENTITY or UNKNOWN and the current token's signumber is IDENTITY (not in sigma) return its
> consumes; else return -1.

> [spec:foma:def:apply.apply-match-str-fn]
> int apply_match_str(struct apply_handle *h, int symbol, int position)

> [spec:foma:sem:apply.apply-match-str-fn]
> Authoritative, side-effecting match of `symbol` at input byte offset `position`; returns bytes
> consumed or -1 on failure. ENUMERATE mode: a flag symbol returns 0 when obey_flags is off,
> otherwise runs `[spec:foma:sem:apply.apply-check-flag-fn]` with the symbol's
> {type, name, value} from flag_lookup — mutating global flag state and setting
> h->oldflagvalue/h->oldflagneg — and returns 0 on SUCCEED, -1 on FAIL; any non-flag symbol
> returns 0. Non-ENUMERATE: EPSILON returns 0; a flag symbol is checked exactly as above (0 or
> -1); if position >= current_instring_length return -1; if
> sigmatch_array[position].signumber == symbol return its consumes; if symbol is IDENTITY or
> UNKNOWN and the token's signumber is IDENTITY return its consumes; else return -1.

> [spec:foma:def:apply.apply-net-fn]
> char *apply_net(struct apply_handle *h)

> [spec:foma:sem:apply.apply-net-fn]
> The backtracking DFS engine over the arc array; yields one result string per call and suspends
> its state in the handle. Resume protocol: if h->iterate_old == 1, jump directly to the resume
> point with everything intact (ptr/iptr/ipos/opos, marks, flag state, search stack); the resume
> point first applies the arrival mark that was deferred when the previous call returned a
> string, then continues descending. Fresh start (iterate_old == 0): iptr = NULL, ptr = 0 (line 0
> = first line of state 0, the start state), ipos = opos = 0; position the index cursor via
> `[spec:foma:sem:apply.apply-set-iptr-fn]`; clear the stack; clear the flag state if the net has
> flags; then enter the loop at the arrival point (so an empty input can match a final start
> state immediately).
> Loop (runs while the stack is non-empty, plus that initial entry). Arrival at a state: if it is
> final and either ipos == current_instring_length or mode has ENUMERATE, call
> `[spec:foma:sem:apply.apply-return-string-fn]`; if that yields non-NULL, return it (suspend;
> the current state is still unmarked — marking happens on resume). Then mark the state
> (`[spec:foma:sem:apply.apply-mark-state-fn]`) and descend via
> `[spec:foma:sem:apply.apply-follow-next-arc-fn]`: on success, handle the new state as an
> arrival. On failure, zero the current state's mark and backtrack: pop a frame
> (`[spec:foma:sem:apply.apply-stack-pop-fn]` restores the cursor onto the arc line previously
> followed, plus ipos/opos/iptr/flag/mark state); if that arc was its state's last candidate
> (`[spec:foma:sem:apply.apply-at-last-arc-fn]`), zero the state's mark and pop again; otherwise
> step past it (`[spec:foma:sem:apply.apply-skip-this-arc-fn]`) and try to follow a later
> sibling.
> Exhaustion (stack empty): in RANDOM mode, clear the stack, reset iterator and iterate_old to 0,
> and return h->outstring as-is — its NUL terminator is whatever the last return-string call
> wrote; if no final state was ever reached the buffer content is stale (literal behavior).
> Otherwise clear the stack and return NULL.

> [spec:foma:def:apply.apply-random-lower-fn]
> char *apply_random_lower(struct apply_handle *h)

> [spec:foma:sem:apply.apply-random-lower-fn]
> Random walk emitting the lower side: clears the global flag state via
> `[spec:foma:sem:apply.apply-clear-flags-fn]`, sets h->mode = DOWN + ENUMERATE + LOWER + RANDOM,
> and returns `[spec:foma:sem:apply.apply-enumerate-fn]`. Arc choice is randomized in
> `[spec:foma:sem:apply.apply-follow-next-arc-fn]` and stopping at final states is a coin flip
> in `[spec:foma:sem:apply.apply-return-string-fn]`.

> [spec:foma:def:apply.apply-random-upper-fn]
> char *apply_random_upper(struct apply_handle *h)

> [spec:foma:sem:apply.apply-random-upper-fn]
> Random walk emitting the upper side: clears flag state, sets
> h->mode = DOWN + ENUMERATE + UPPER + RANDOM, and returns
> `[spec:foma:sem:apply.apply-enumerate-fn]`.

> [spec:foma:def:apply.apply-random-words-fn]
> char *apply_random_words(struct apply_handle *h)

> [spec:foma:sem:apply.apply-random-words-fn]
> Random walk emitting both sides (pairs joined by the separator where they differ): clears flag
> state, sets h->mode = DOWN + ENUMERATE + LOWER + UPPER + RANDOM, and returns
> `[spec:foma:sem:apply.apply-enumerate-fn]`.

> [spec:foma:def:apply.apply-reset-enumerator-fn]
> void apply_reset_enumerator(struct apply_handle *h)

> [spec:foma:sem:apply.apply-reset-enumerator-fn]
> Resets enumeration so the next `[spec:foma:sem:apply.apply-enumerate-fn]` call starts a fresh
> search: zeroes marks[i] for all statecount states and sets h->iterator = 0 and
> h->iterate_old = 0. Does not touch the search stack (apply_enumerate's fresh path force-clears
> it).

> [spec:foma:def:apply.apply-return-string-fn]
> char *apply_return_string(struct apply_handle *h)

> [spec:foma:sem:apply.apply-return-string-fn]
> Finalizes the accumulated output by writing '\0' at h->outstring + h->opos (truncating residue
> from longer earlier results). Non-RANDOM mode: return h->outstring. RANDOM mode: with
> probability 1/2 (rand() % 2 == 0) end the walk — clear the search stack, reset h->iterator and
> h->iterate_old to 0, and return h->outstring; otherwise return NULL, telling
> `[spec:foma:sem:apply.apply-net-fn]` to continue walking past this final state.

> [spec:foma:def:apply.apply-set-epsilon-fn]
> void apply_set_epsilon(struct apply_handle *h, char *symbol)

> [spec:foma:sem:apply.apply-set-epsilon-fn]
> Sets the display string for EPSILON (default "0"): frees the old h->epsilon_symbol, strdups
> `symbol` into it, and points sigs[EPSILON] at the new string with its strlen as length. Requires
> the handle to be fully initialized (h->sigs allocated by
> `[spec:foma:sem:apply.apply-create-sigarray-fn]`).

> [spec:foma:def:apply.apply-set-iptr-fn]
> void apply_set_iptr(struct apply_handle *h)

> [spec:foma:sem:apply.apply-set-iptr-fn]
> Positions the index cursor for the state at line h->ptr and the current input token. Selects
> idx = h->index_in if mode has DOWN, else h->index_out; if that index is NULL, returns leaving
> h->iptr and h->state_has_index untouched. Otherwise resets iptr = NULL and
> state_has_index = 0; reads stateno = gstates[ptr].state_no and returns if negative; if
> idx[stateno] is NULL (state not indexed) returns with state_has_index = 0. Else sets
> state_has_index = 1 and reads seeksym = sigmatch_array[ipos].signumber — unconditionally, so
> at ipos == current_instring_length, or in ENUMERATE mode where no sigmatch table was built for
> this run, this reads stale or out-of-bounds data (latent bug). Takes the slot base+seeksym; if
> its fsmptr == -1, falls through to slot->next (the base EPSILON slot for non-epsilon symbols;
> returns if next is NULL, as for the EPSILON slot itself). Sets h->iptr to the chosen node, but
> back to NULL if that node's fsmptr is also -1 — meaning the state is indexed
> (state_has_index == 1) but has no candidate arcs for this token.

> [spec:foma:def:apply.apply-set-obey-flags-fn]
> void apply_set_obey_flags(struct apply_handle *h, int value)

> [spec:foma:sem:apply.apply-set-obey-flags-fn]
> Setter: h->obey_flags = value. Nonzero (default 1) makes application enforce flag-diacritic
> consistency via `[spec:foma:sem:apply.apply-check-flag-fn]`; zero makes flag symbols
> unconditionally traversable, input-epsilon arcs.

> [spec:foma:def:apply.apply-set-print-pairs-fn]
> void apply_set_print_pairs(struct apply_handle *h, int value)

> [spec:foma:sem:apply.apply-set-print-pairs-fn]
> Setter: h->print_pairs = value. When nonzero, non-enumerate application renders arcs whose two
> sides differ as "<upper" + separator + "lower>" instead of only the output side (see
> `[spec:foma:sem:apply.apply-append-fn]`).

> [spec:foma:def:apply.apply-set-print-space-fn]
> void apply_set_print_space(struct apply_handle *h, int value)

> [spec:foma:sem:apply.apply-set-print-space-fn]
> Sets h->print_space = value and unconditionally sets h->space_symbol = strdup(" ") — even when
> value is 0 — leaking any previously installed space symbol. When print_space is on,
> `[spec:foma:sem:apply.apply-append-fn]` appends the space symbol after every nonempty symbol
> rendering.

> [spec:foma:def:apply.apply-set-separator-fn]
> void apply_set_separator(struct apply_handle *h, char *symbol)

> [spec:foma:sem:apply.apply-set-separator-fn]
> Setter: h->separator = strdup(symbol), leaking the previous separator string. The separator
> (default ":") joins the two sides in pair and word rendering.

> [spec:foma:def:apply.apply-set-show-flags-fn]
> void apply_set_show_flags(struct apply_handle *h, int value)

> [spec:foma:sem:apply.apply-set-show-flags-fn]
> Setter: h->show_flags = value. Nonzero renders flag-diacritic symbols with their full display
> strings in output; zero (default) renders them as empty strings.

> [spec:foma:def:apply.apply-set-space-symbol-fn]
> void apply_set_space_symbol(struct apply_handle *h, char *space)

> [spec:foma:sem:apply.apply-set-space-symbol-fn]
> Sets h->space_symbol = strdup(space) (leaking any previous value) and turns h->print_space on.
> Note `[spec:foma:sem:apply.apply-append-fn+1]` advances the output position by the space symbol's
> full byte length, so a multi-byte space symbol is emitted intact (the C source corrupted it).

> [spec:foma:def:apply.apply-skip-this-arc-fn]
> void apply_skip_this_arc(struct apply_handle *h)

> [spec:foma:sem:apply.apply-skip-this-arc-fn]
> Advances the resumption cursor past the arc just popped so the search resumes with the next
> sibling: if an index cursor is active (h->iptr non-NULL), set ptr = iptr->fsmptr (the popped
> arc's line) and advance iptr = iptr->next (the next candidate consumed by the index branch of
> `[spec:foma:sem:apply.apply-follow-next-arc-fn]`); otherwise simply increment ptr to the next
> arc line.

> [spec:foma:def:apply.apply-stack-clear-fn]
> void apply_stack_clear (struct apply_handle *h)

> [spec:foma:sem:apply.apply-stack-clear-fn]
> Empties the search stack by setting h->apply_stack_ptr = 0. Capacity and old frame contents are
> retained.

> [spec:foma:def:apply.apply-stack-isempty-fn]
> int apply_stack_isempty (struct apply_handle *h)

> [spec:foma:sem:apply.apply-stack-isempty-fn]
> Returns 1 if h->apply_stack_ptr == 0, else 0.

> [spec:foma:def:apply.apply-stack-pop-fn]
> void apply_stack_pop (struct apply_handle *h)

> [spec:foma:sem:apply.apply-stack-pop-fn]
> Pops the top backtrack frame (decrements apply_stack_ptr; no underflow guard) and restores from
> it: h->iptr, h->ptr = the frame's offset (the arc line that was followed when the frame was
> pushed), h->ipos, h->opos, h->state_has_index, and the visit mark of that arc's state
> (marks[gstates[ptr].state_no] = frame's visitmark). If the net has flags and the frame's
> flagname is non-NULL, finds the flag_list node whose name strcmp-equals it and restores that
> node's value and neg from the frame; if no node matches it prints "***Nothing to pop" via
> perror and then dereferences the NULL list pointer anyway — latent crash (unreachable in
> practice: every feature name is pre-registered).

> [spec:foma:def:apply.apply-stack-push-fn]
> static void apply_stack_push (struct apply_handle *h, int vmark, char *sflagname, char *sflagvalue, int sflagneg)

> [spec:foma:sem:apply.apply-stack-push-fn]
> Pushes a backtrack frame. If full (apply_stack_ptr == apply_stack_top), realloc the stack to
> double capacity and double apply_stack_top (on realloc failure: perror("Apply stack full!!!")
> and exit(0)). The frame records: offset = h->curr_ptr (the arc line being followed — note, not
> h->ptr), ipos, opos, visitmark = the vmark argument (the source state's mark at arrival),
> iptr = h->iptr, and state_has_index; if the net has flags, also
> flagname/flagvalue/flagneg = the arguments (NULL/NULL/0 when the arc modified no flag) — for
> flag-free nets these three fields are left unwritten. Increments apply_stack_ptr.

> [spec:foma:def:apply.apply-up-fn]
> char *apply_up(struct apply_handle *h, char *word)

> [spec:foma:sem:apply.apply-up-fn]
> Applies `word` on the lower (output) side, yielding upper-side strings: sets h->mode = UP (8);
> h->indexed = 1 iff h->index_out exists (write-only bookkeeping); h->binsearch = 1 iff
> h->last_net->arcs_sorted_out == 1, else 0; delegates to
> `[spec:foma:sem:apply.apply-updown-fn]`. Dereferences last_net before the NULL guard in
> apply_updown, like `[spec:foma:sem:apply.apply-down-fn]`.

> [spec:foma:def:apply.apply-updown-fn]
> char *apply_updown(struct apply_handle *h, char *word)

> [spec:foma:sem:apply.apply-updown-fn]
> Shared iterator entry for apply up/down. Returns NULL if h->last_net is NULL or has no final
> states (finalcount == 0). If `word` is NULL: resume mode — set iterate_old = 1 and call
> `[spec:foma:sem:apply.apply-net-fn]` to yield the next result for the previously supplied word.
> Otherwise: set iterate_old = 0, store the word pointer into h->instring (borrowed — never
> copied or freed; it must stay valid while iterating), tokenize it via
> `[spec:foma:sem:apply.apply-create-sigmatch-fn]`, clear stale search state via
> `[spec:foma:sem:apply.apply-force-clear-stack-fn]`, and run apply_net. The returned string
> points into h->outstring and is valid only until the next call on this handle; NULL means no
> (further) result.

> [spec:foma:def:apply.apply-upper-words-fn]
> char *apply_upper_words(struct apply_handle *h)

> [spec:foma:sem:apply.apply-upper-words-fn]
> Enumerates upper-side (input) words: sets h->mode = DOWN + ENUMERATE + UPPER and returns
> `[spec:foma:sem:apply.apply-enumerate-fn]`. Each call yields the next path's upper string;
> NULL when exhausted.

> [spec:foma:def:apply.apply-words-fn]
> char *apply_words(struct apply_handle *h)

> [spec:foma:sem:apply.apply-words-fn]
> Enumerates whole words (both sides): sets h->mode = DOWN + ENUMERATE + LOWER + UPPER and
> returns `[spec:foma:sem:apply.apply-enumerate-fn]`. With both sides selected,
> `[spec:foma:sem:apply.apply-append-fn]` prints identical sides once and differing sides as
> upper + separator + lower. Each call yields the next path's string; NULL when exhausted.

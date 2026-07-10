# foma/sigma.c

> [spec:foma:def:sigma.sigma-add-fn]
> int sigma_add (char *symbol, struct sigma *sigma)

> [spec:foma:sem:sigma.sigma-add-fn+1]
> Adds `symbol` to the sigma alphabet (a `Vec` of entries `{int number; symbol}` in insertion order; the empty alphabet is an empty Vec) and returns the number assigned. First classify: if `symbol` string-equals "@_EPSILON_SYMBOL_@" the target number is EPSILON=0; "@_IDENTITY_SYMBOL_@" → IDENTITY=2; "@_UNKNOWN_SYMBOL_@" → UNKNOWN=1; otherwise non-special.
> Non-special path: append a new entry at the tail with number = tail->number+1, clamped up to 3 if tail->number+1 < 3; an empty alphabet starts at 3. Return that number. Note the number comes from the tail entry, not the alphabet maximum — callers must keep the entries sorted ascending by number for this to be unique.
> Special path: insert the entry keeping the alphabet sorted ascending by number — before the first entry whose number is >= the special number (so a duplicate code lands before any equal-numbered entry), or at the tail. Return the special number.
> No duplicate check in either path: adding an existing symbol creates a second entry. The input `symbol` is never consumed; the alphabet owns freshly allocated copies. Caller must re-sort sigma (see `[spec:foma:sem:sigma.sigma-sort-fn]`) before any sigma merge if non-special insertion broke ordering.

> [spec:foma:def:sigma.sigma-add-number-fn]
> int sigma_add_number(struct sigma *sigma, char *symbol, int number)

> [spec:foma:sem:sigma.sigma-add-number-fn+1]
> Appends `symbol` with an explicitly chosen `number` at the tail of the alphabet. Always returns 1. No sorting, no duplicate checking; symbol string is copied, input not consumed.

> [spec:foma:def:sigma.sigma-add-special-fn]
> int sigma_add_special (int symbol, struct sigma *sigma)

> [spec:foma:sem:sigma.sigma-add-special-fn+2]
> Inserts one special symbol by numeric code, keeping the alphabet sorted by number. Maps code→string: EPSILON=0→"@_EPSILON_SYMBOL_@", IDENTITY=2→"@_IDENTITY_SYMBOL_@", UNKNOWN=1→"@_UNKNOWN_SYMBOL_@". A non-reserved code yields a well-formed placeholder "@_SPECIAL_<code>_@". The C source left the string NULL for any other code, so a symbol-less node was inserted and later crashed when read back.
> Insert the `{number=code, symbol=str}` entry before the first entry whose number is >= code (or at the tail); return code.
> Same as the special path of `[spec:foma:sem:sigma.sigma-add-fn]` but keyed by number. No duplicate check: if the code is already present the new entry is inserted before the existing equal-numbered entry.

> [spec:foma:def:sigma.sigma-cleanup-fn]
> void sigma_cleanup (struct fsm *net, int force)

> [spec:foma:sem:sigma.sigma-cleanup-fn+1]
> Removes symbols never used on any arc from net->sigma and renumbers the remainder consecutively; rewrites arc labels to match. If force==0, returns immediately (no-op) when sigma contains IDENTITY or UNKNOWN (checked via `[spec:foma:sem:sigma.sigma-find-number-fn]`); force==1 always proceeds.
> Steps: maxsigma = sigma_max(net->sigma); if < 0 return. Allocate int array `attested[0..maxsigma]` zeroed. Scan net->states lines (array of fsm_state terminated by state_no==-1): for each line set attested[in]=1 if in>=0 and attested[out]=1 if out>=0. Then build the renumbering inside the same array: with j starting at 3, for i=3..maxsigma in order, if attested[i] nonzero set attested[i]=j and j++ (attested entries thus hold their new number, ≥3; unattested stay 0; indices 0–2 keep the 0/1 mark). Second pass over all lines: if in>2 replace in with attested[in]; likewise out.
> Then, in alphabet order, drop every entry with attested[number]==0 and, for the entries that survive, set number=attested[number] when number>=3 (numbers 0–2 are kept as-is); order is otherwise preserved. Note EPSILON/UNKNOWN/IDENTITY entries (numbers 0–2) are also removed if no arc uses them.

> [spec:foma:def:sigma.sigma-copy-fn]
> struct sigma *sigma_copy(struct sigma *sigma)

> [spec:foma:sem:sigma.sigma-copy-fn+1]
> Deep-copies a sigma alphabet: an empty alphabet copies to an empty alphabet; otherwise every entry is copied in order (number and symbol). Returns the new alphabet; the source is untouched.

> [spec:foma:def:sigma.sigma-create-fn]
> struct sigma *sigma_create()

> [spec:foma:sem:sigma.sigma-create-fn+1]
> Returns a fresh empty sigma alphabet (an empty Vec — there is no sentinel node).

> [spec:foma:def:sigma.sigma-find-fn]
> int sigma_find(char *symbol, struct sigma *sigma)

> [spec:foma:sem:sigma.sigma-find-fn]
> Returns the number of the first node whose symbol string-equals `symbol`, scanning until NULL or a node with number==-1. Returns -1 if sigma is NULL, empty (head number==-1), or the symbol is absent.

> [spec:foma:def:sigma.sigma-find-number-fn]
> int sigma_find_number(int number, struct sigma *sigma)

> [spec:foma:sem:sigma.sigma-find-number-fn]
> Returns `number` itself if some node (scanning until NULL or number==-1) has that number, else -1. NULL sigma or empty sentinel head → -1.

> [spec:foma:def:sigma.sigma-max-fn]
> int sigma_max(struct sigma *sigma)

> [spec:foma:sem:sigma.sigma-max-fn+1]
> Returns the maximum number over all alphabet entries, starting the accumulator at -1; an empty alphabet therefore yields -1.

> [spec:foma:def:sigma.sigma-remove-fn]
> struct sigma *sigma_remove(char *symbol, struct sigma *sigma)

> [spec:foma:sem:sigma.sigma-remove-fn+1]
> Removes, in place, the first alphabet entry whose symbol string-equals `symbol`; the remaining entries keep their order. An empty alphabet or a symbol that is absent is a no-op.

> [spec:foma:def:sigma.sigma-remove-num-fn]
> struct sigma *sigma_remove_num(int num, struct sigma *sigma)

> [spec:foma:sem:sigma.sigma-remove-num-fn+1]
> Identical to `[spec:foma:sem:sigma.sigma-remove-fn]` but matches on number == num instead of the symbol string: removes, in place, the first entry with that number; an empty alphabet or a number that is absent is a no-op.

> [spec:foma:def:sigma.sigma-size-fn]
> int sigma_size(struct sigma *sigma)

> [spec:foma:sem:sigma.sigma-size-fn+1]
> Returns the number of alphabet entries; an empty alphabet returns 0.

> [spec:foma:def:sigma.sigma-sort-fn]
> int sigma_sort(struct fsm *net)

> [spec:foma:sem:sigma.sigma-sort-fn+2]
> Sorts the non-special part of net->sigma alphabetically by symbol string (strcmp/byte order) and renumbers those symbols consecutively from 3, rewriting arc labels accordingly. Returns 1 always (also when sigma is empty: if sigma_max < 0, return 1 immediately with no work).
> Steps: let size = sigma_max(net->sigma). Collect every alphabet entry with number > IDENTITY (2) into a scratch array of {symbol,number} pairs (struct ssort), moving each symbol out; let max = count collected. Sort that array with `[spec:foma:sem:sigma.ssortcmp-fn]`. Build a replacearray of size+3 entries; the C left slots for numbers absent from sigma uninitialized (garbage), corrupting any arc labelled with a missing number. Seed the array with the identity map (replacearray[k]=k) so an absent label is left unchanged, then for sorted index i in 0..max-1 set replacearray[oldnumber]=i+3. Rewrite arcs: for each line of net->states (until state_no==-1), if in>IDENTITY set in=replacearray[in]; likewise out. Rewrite sigma: walk the entries again in order with counter i=0; for each entry with number>IDENTITY assign number=i+3 and symbol=sorted[i].symbol (moving each sorted symbol back — symbols are permuted among the existing entries), i++.
> Net effect: special entries (numbers 0–2) keep position/number; the k-th non-special entry in alphabet order receives the k-th alphabetically-smallest symbol and number k+3.

> [spec:foma:def:sigma.sigma-string-fn]
> char *sigma_string(int number, struct sigma *sigma)

> [spec:foma:sem:sigma.sigma-string-fn]
> Returns the symbol pointer (aliased, not copied — caller must not free) of the first node whose number equals `number`, scanning until NULL or number==-1. Returns NULL if sigma is NULL, empty, or the number is absent.

> [spec:foma:def:sigma.sigma-substitute-fn]
> int sigma_substitute(char *symbol, char *sub, struct sigma *sigma)

> [spec:foma:sem:sigma.sigma-substitute-fn]
> Renames a symbol in place: finds the first node whose symbol string-equals `symbol` (scan until NULL or number==-1), frees that node's old symbol, sets it to strdup(sub), and returns the node's number. Returns -1 if the head is the empty sentinel or the symbol is not found. No duplicate check: if `sub` already exists in sigma, the alphabet ends up with two nodes carrying the same string.

> [spec:foma:def:sigma.sigma-to-list-fn]
> struct fsm_sigma_list *sigma_to_list(struct sigma *sigma)

> [spec:foma:sem:sigma.sigma-to-list-fn]
> Builds a number-indexed lookup table: calloc's an array of sigma_max(sigma)+1 fsm_sigma_list entries (each holds a char *symbol), then for each real node (scan until NULL or number==-1) sets slot[node->number].symbol = node->symbol. Symbol pointers are aliased into the sigma, not copied; slots for absent numbers stay NULL. Caller owns/frees the array only. An empty sigma yields a zero-length calloc.

> [spec:foma:def:sigma.ssort]
> struct ssort {
>   char *symbol;
>   int number;
> }

> [spec:foma:def:sigma.ssortcmp-fn]
> int ssortcmp(const void *_a, const void *_b)

> [spec:foma:sem:sigma.ssortcmp-fn]
> qsort comparator over struct ssort: returns strcmp(a->symbol, b->symbol) — plain byte-wise ordering of the symbol strings; the number field is ignored.


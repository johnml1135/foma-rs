# foma/flags.c

> [spec:foma:def:flags.flag-build-fn]
> int flag_build(int ftype, char *fname, char *fvalue, int fftype, char *ffname, char *ffvalue)

> [spec:foma:sem:flags.flag-build-fn]
> Pairwise compatibility oracle used by `[spec:foma:sem:flags.flag-eliminate-fn]`: given the flag
> currently being eliminated (ftype/fname/fvalue) and some other flag occurring in the network
> (fftype/ffname/ffvalue), returns one of the file-local constants FAIL=1, SUCCEED=2, NONE=3.
> SUCCEED means "seeing this other flag (with nothing failing in between) licenses the current
> flag"; FAIL means it blocks it; NONE means irrelevant.
> Returns NONE immediately if fname and ffname differ. selfnull = (fvalue == NULL), i.e. the
> eliminated flag is valueless like @R.A@ or @D.A@. NULL values are then replaced by "" and
> valeq = strcmp(fvalue, ffvalue) (equal iff 0). The first matching row below wins; anything not
> listed (including any ftype of C, N, P, or E) yields NONE. Types: U=UNIFY, C=CLEAR, D=DISALLOW,
> N=NEGATIVE, P=POSITIVE, R=REQUIRE.
> ftype U: ff P equal value SUCCEED; ff C SUCCEED (any value); ff U unequal FAIL; ff P unequal
> FAIL; ff N equal FAIL. (U vs U with equal value is NONE.)
> ftype R, valueless: ff U SUCCEED; ff P SUCCEED; ff N SUCCEED; ff C FAIL.
> ftype R, with value: ff P equal SUCCEED; ff U equal SUCCEED; ff P unequal FAIL; ff U unequal
> FAIL; ff N FAIL (any value); ff C FAIL.
> ftype D, valueless: ff C SUCCEED; ff P FAIL; ff U FAIL; ff N FAIL.
> ftype D, with value: ff P unequal SUCCEED; ff C SUCCEED; ff N equal SUCCEED; ff P equal FAIL;
> ff U equal FAIL; ff N unequal FAIL.

> [spec:foma:def:flags.flag-check-fn]
> int flag_check(char *s)

> [spec:foma:sem:flags.flag-check-fn]
> Byte-level DFA deciding whether s is a syntactically well-formed flag-diacritic symbol; returns
> 1 on a complete match, 0 otherwise. With ND = any byte that is neither '.' nor NUL, the accepted
> language is: "@" [U|P|N|E] "." ND+ "." ND+ "@"  (U/P/N/E require both an attribute and a value),
> or "@" [R|D] "." ND+ ["." ND+] "@"  (R/D value optional), or "@" C "." ND+ "@"  (C never takes a
> value). Exactly one or two '.'-separated fields after the operator letter; the string must end
> (NUL) immediately after the closing '@'. Operates on raw bytes (arbitrary UTF-8 allowed in
> fields). Quirk to preserve: '@' only acts as the terminator in the states scanning the LAST
> field (the value field of U/P/N/E and the first-or-second field of R/D/C); in the mandatory
> first field of U/P/N/E, '@' is treated as an ordinary ND byte, so e.g. "@U.a@b.c@" is accepted.

> [spec:foma:def:flags.flag-create-symbol-fn]
> struct fsm *flag_create_symbol(int type, char *name, char *value)

> [spec:foma:sem:flags.flag-create-symbol-fn]
> Builds the canonical text of a flag symbol and returns a fresh single-symbol FSM (fsm_symbol)
> accepting exactly that string: "@" + flag_type_to_char(type) + "." + name + "." + value + "@",
> where the "." + value part is omitted when value is NULL or empty (NULL is first normalized to
> ""). Buffer is malloc'd to strlen(name)+strlen(value)+6 bytes and never freed (leak). E.g.
> (FLAG_UNIFY, "F", "V") yields the net for "@U.F.V@"; (FLAG_REQUIRE, "A", NULL) yields "@R.A@".

> [spec:foma:def:flags.flag-eliminate-fn]
> struct fsm *flag_eliminate(struct fsm *net, char *name)

> [spec:foma:sem:flags.flag-eliminate-fn+1]
> Eliminates one flag attribute (all flags whose attribute equals name) or ALL flags (name==NULL)
> from net by building per-flag filter automata, composing them on both sides, and erasing the flag
> arcs. Uses file-local constants FAIL=1, SUCCEED=2, NONE=3.
> 1. If net->pathcount == 0: when g_verbose, print "Skipping flag elimination since there are no
> paths in network.\n" to stderr (fflush) — return net unchanged.
> 2. flags = `[spec:foma:sem:flags.flag-extract-fn]`(net). If name != NULL and no extracted flag
> has that exact attribute name: when g_verbose print "Flag attribute '%s' does not occur in the
> network.\n" and return net unchanged (flags list leaked).
> 3. For each extracted flag f: if (name==NULL || f->name equals name) &&
> (f->type & (FLAG_UNIFY | FLAG_REQUIRE | FLAG_DISALLOW | FLAG_EQUAL)) — Wave 4 fix: the C wrote
> this with `|`, which is always nonzero, so the intended restriction to U/R/D/E types was a no-op
> and the body ran for every type; `&` restricts it as intended. The observable language is
> unchanged (see below). Build fail_flags and succeed_flags as minimized unions (starting from
> fsm_empty_set) of flag_create_symbol(ff) over ALL extracted flags ff, classified by
> `[spec:foma:sem:flags.flag-build-fn]`(f, ff): FAIL goes to fail_flags, SUCCEED to succeed_flags;
> set a marker if anything classified. self = flag_create_symbol(f). The fix changes nothing
> observable: flag_build classifies pairs only when f's type is U, R, or D, so for E (and, before
> the fix, P/N/C) types nothing is classified and no filter is added (those succeed/fail/self nets
> leak).
> 4. If the marker is set, build newfilter: for f->type == FLAG_REQUIRE,
> ~[ (?* fail_flags)^0,1 ~$[succeed_flags] self ?* ] — literally fsm_complement(concat(
> optionality(concat(universal, fail_flags)), concat(complement(contains(succeed_flags)),
> concat(self, universal)))); for all other types, ~$[ fail_flags ~$[succeed_flags] self ] —
> fsm_complement(contains(concat(fail_flags, concat(complement(contains(succeed_flags)), self)))).
> Accumulate filter as the intersection of all newfilters; reset the marker per flag.
> 5. If any filter was built: temporarily force the global g_flag_is_epsilon to 0, compute
> newnet = fsm_compose(copy(filter), fsm_compose(net, copy(filter))) (net consumed; filter itself
> is never destroyed — leak), restore g_flag_is_epsilon. Filters sit on BOTH sides because upper
> and lower flags are independent in a transducer; plain intersection would be wrong.
> 6. `[spec:foma:sem:flags.flag-purge-fn]`(newnet, name) — turns targeted flag arcs into epsilon
> and drops the symbols from sigma; fsm_minimize; sigma_cleanup(newnet, 0); sigma_sort;
> free(flags) frees only the list head (remaining nodes and name/value strings leak); return
> fsm_topsort(newnet).

> [spec:foma:def:flags.flag-extract-fn]
> struct flags *flag_extract (struct fsm *net)

> [spec:foma:sem:flags.flag-extract-fn]
> Walks net->sigma; for every symbol accepted by `[spec:foma:sem:flags.flag-check-fn]`, prepends a
> malloc'd struct flags node to the result list with type = flag_get_type(symbol),
> name = flag_get_name(symbol) (freshly allocated), value = flag_get_value(symbol) (freshly
> allocated, NULL for valueless flags). Returns the head, i.e. nodes appear in reverse sigma
> order; NULL when the net contains no flag symbols. One node per sigma entry — distinct symbols
> sharing an attribute each get their own node.

> [spec:foma:def:flags.flag-get-name-fn]
> char *flag_get_name(char *string)

> [spec:foma:sem:flags.flag-get-name-fn]
> Extracts the attribute field from a flag-symbol string: iterates over the string advancing
> utf8skip(s+i)+1 bytes per step (UTF-8 code points); start = index just after the FIRST '.'
> encountered; end = index of the next '.' or '@' found after start is set. If both were found
> (start > 0 and end > 0), returns xxstrndup(string+start, end-start) — a fresh allocation of the
> bytes in between; otherwise NULL. E.g. "@U.FEAT.VAL@" yields "FEAT", "@R.A@" yields "A".

> [spec:foma:def:flags.flag-get-type-fn]
> int flag_get_type(char *string)

> [spec:foma:sem:flags.flag-get-type-fn]
> Inspects the two bytes at string+1 (the leading '@' is assumed, never verified): "U." returns
> FLAG_UNIFY (1), "C." FLAG_CLEAR (2), "D." FLAG_DISALLOW (4), "N." FLAG_NEGATIVE (8), "P."
> FLAG_POSITIVE (16), "R." FLAG_REQUIRE (32), "E." FLAG_EQUAL (64); anything else returns 0.

> [spec:foma:def:flags.flag-get-value-fn]
> char *flag_get_value(char *string)

> [spec:foma:sem:flags.flag-get-value-fn]
> Extracts the value field, iterating UTF-8 code-point-wise like flag_get_name. first is set to
> the index after the FIRST '.'; each subsequent '.' (while first is set) updates start to the
> index after it — so start ends up after the LAST '.' in the string; the first '@' seen while
> start != 0 sets end and stops the scan. Returns xxstrndup(string+start, end-start) when both
> start and end were set, else NULL — valueless flags ("@R.A@", "@C.X@", "@D.F@") yield NULL.
> "@U.FEAT.VAL@" yields "VAL".

> [spec:foma:def:flags.flag-purge-fn]
> void flag_purge (struct fsm *net, char *name)

> [spec:foma:sem:flags.flag-purge-fn]
> Erases targeted flag symbols from net in place (arcs become epsilon). Allocates
> ftable[0..sigma_max(net->sigma)], zeroed. For every sigma entry with number != -1 that passes
> `[spec:foma:sem:flags.flag-check-fn]`: mark its number when name == NULL (purge every flag), or
> when the symbol text at byte offset +3 (skipping "@X.", i.e. a one-byte operator) starts with
> name (strncmp over strlen(name)), the remainder is strictly longer than name, and the byte
> immediately after the name is '.' or '@' — this matches both "@U.name.val@" and "@D.name@" but
> not attributes that merely share a prefix. Each marked number is removed from sigma via
> sigma_remove_num. Then every state-table line with in >= 0 and out >= 0 gets marked in/out
> replaced by EPSILON (0); lines with negative in/out (e.g. arcless final states) are untouched.
> Sets is_deterministic, is_minimized, is_epsilon_free to NO; frees ftable; returns void.

> [spec:foma:def:flags.flag-twosided-fn]
> struct fsm *flag_twosided(struct fsm *net)

> [spec:foma:sem:flags.flag-twosided-fn]
> Enforces two-sided flag diacritics: rewrites net so every arc touching a flag symbol carries the
> flag identically on both tapes, splitting mixed arcs in two via fresh intermediate states.
> 1. Mark flag symbols: isflag[0..sigma_max] = 1 for sigma numbers whose symbol passes
> `[spec:foma:sem:flags.flag-check-fn]`, else 0.
> 2. Pass 1 over the state table (skipping lines with target == -1), tracking maxstate = highest
> state_no seen: if in is a flag and out == EPSILON, set out = in (change=1); else if out is a flag
> and in == EPSILON, set in = out (change=1). After the repair, count newarcs = arcs where (in or
> out is a flag) and in != out (flag paired with a real symbol, or two distinct flags).
> 3. If newarcs == 0: when change, set is_deterministic/is_minimized/is_pruned to UNK and return
> fsm_topsort(fsm_minimize(net)); otherwise return net untouched.
> 4. Else realloc net->states to (lines + newarcs) entries — BUG (preserve or fix knowingly): the
> element size used is sizeof(struct fsm), not sizeof(struct fsm_state); the huge over-allocation
> also hides that lines+newarcs+1 slots are needed for the new sentinel. Then for each offending
> arc (scanning the original lines): let S be a fresh state number (starting at old maxstate+1,
> incrementing per split) — in-flag/out-nonflag: append arc S --EPSILON:out--> old target, retarget
> the original arc to S with label in:in; out-flag/in-nonflag: append S --out:out--> old target,
> original becomes in:EPSILON into S; both flags (distinct): append S --out:out--> old target,
> original becomes in:in into S. Appended arcs use add_fsm_arc with final_state=0, start_state=0.
> 5. Append a sentinel line (all fields -1), set is_deterministic/is_minimized to UNK, return
> fsm_topsort(fsm_minimize(net)).

> [spec:foma:def:flags.flag-type-to-char-fn]
> char *flag_type_to_char (int type)

> [spec:foma:sem:flags.flag-type-to-char-fn]
> Maps a FLAG_* type constant to its operator letter as a static string: FLAG_UNIFY "U",
> FLAG_CLEAR "C", FLAG_DISALLOW "D", FLAG_NEGATIVE "N", FLAG_POSITIVE "P", FLAG_REQUIRE "R",
> FLAG_EQUAL "E"; any other value returns NULL. Inverse of
> `[spec:foma:sem:flags.flag-get-type-fn]`.

> [spec:foma:def:flags.flags]
> struct flags {
>   int type;
>   char *name;
>   char *value;
>   struct flags *next;
> }


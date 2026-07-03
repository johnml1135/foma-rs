# foma/iface.c

> [spec:foma:def:iface.foma-net-print-fn]
> extern int foma_net_print(struct fsm *net, gzFile outfile)

> [spec:foma:sem:iface.foma-net-print-fn]
> Extern declaration only in iface.c; implemented in foma/io.c (see `[spec:foma:sem:io.foma-net-print-fn]`).
> Serializes `net` to the gzipped text stream `outfile` in foma save format: line "##foma-net 1.0##", line
> "##props##", one properties line `"%i %i %i %i %i %lld %i %i %i %i %i %i %s\n"` with arity, arccount,
> statecount, linecount, finalcount, pathcount (long long), is_deterministic, is_pruned, is_minimized,
> is_epsilon_free, is_loop_free, extras (= is_completed | arcs_sorted_in<<2 | arcs_sorted_out<<4), name;
> then "##sigma##" with one "number symbol" line per sigma entry (stopping at a number == -1 entry); then
> "##states##" with per-transition lines: on a new state_no, "state in out target final" if in != out else
> "state in target final"; on a repeated state_no, "in out target" if in != out else "in target"; then the
> sentinel line "-1 -1 -1 -1 -1"; then, if net->medlookup->confusion_matrix exists, "##cmatrix##" followed
> by maxsigma*maxsigma integers one per line (maxsigma = sigma_max+1); finally "##end##". Always returns 1.
> The net is not consumed or modified.

> [spec:foma:def:iface.g-v]
> struct g_v {
>   void *ptr;
>   char *name;
>   int type;
> }

> [spec:foma:def:iface.global-help]
> struct global_help {
>   char *name;
>   char *help;
>   char *longhelp;
> }

> [spec:foma:def:iface.iface-ambiguous-upper-fn]
> void iface_ambiguous_upper()

> [spec:foma:sem:iface.iface-ambiguous-upper-fn]
> Requires ≥1 net on the stack via iface_stack_check(1) (on failure that check prints "Not enough networks
> on stack. Operation requires at least 1.\n" and this function does nothing). Pops the top net (consumed)
> and pushes fsm_extract_ambiguous_domain(net) — the input-side words that have multiple transduction
> paths. No fsm_minimize/fsm_topsort wrapping.

> [spec:foma:def:iface.iface-apply-down-fn]
> void iface_apply_down(char *word)

> [spec:foma:sem:iface.iface-apply-down-fn]
> Requires ≥1 net (iface_stack_check(1); prints its error and returns otherwise). Gets the cached apply
> handle for the top net via stack_get_ah() (created lazily on the stack entry), configures it from
> globals via iface_apply_set_params. Calls apply_down(ah, word); if NULL prints "???\n" and returns.
> Otherwise prints the result plus "\n", then loops at most g_list_limit (default 100) additional times
> calling apply_down(ah, NULL), printing each result plus "\n", stopping early on NULL. So at most
> 1 + g_list_limit results are printed. Top net is not consumed; the apply handle stays cached.

> [spec:foma:def:iface.iface-apply-file-fn]
> int iface_apply_file(char *infilename, char *outfilename, int direction)

> [spec:foma:sem:iface.iface-apply-file-fn]
> Batch-applies every line of infilename through the top net. direction must be AP_D (1, apply down) or
> AP_U (2, apply up); anything else does perror("Invalid direction in iface_apply_file().\n") and returns 1.
> If the stack is empty, iface_stack_check(1) prints its message and this returns 0. Opens infilename for
> reading; on failure prints "<infilename>: " to stderr, perror("Error opening file"), returns 1. If
> outfilename is NULL output goes to stdout; else it is fopen'd "w" and "Writing output to file %s.\n" is
> printed to stdout (note: printed before the NULL check of fopen's result), and on open failure prints
> "<outfilename>: " to stderr, perror("Error opening output file."), returns 1. Gets the cached apply handle
> (stack_get_ah) and sets params from globals (iface_apply_set_params). For each input line (fgets, buffer
> 8192): strips one trailing '\n' if present; writes "\n<word>\n" to the output; applies down/up per
> direction; if the first result is NULL writes "???\n" and continues to the next line; else writes every
> result (no limit) each followed by "\n" until the apply enumerator returns NULL. Closes the output file
> only if outfilename was non-NULL; the input file is never fclose'd (latent leak). Returns 0.

> [spec:foma:def:iface.iface-apply-med-fn]
> void iface_apply_med(char *word)

> [spec:foma:sem:iface.iface-apply-med-fn]
> Minimum-edit-distance lookup of `word` against the top net. Requires ≥1 net (iface_stack_check(1), else
> returns). Gets the cached MED handle via stack_get_med_ah() (lazily apply_med_init'd with align symbol
> "-"). Sets heap max to 4194305 (4194304+1), med limit to g_med_limit (default 3), med cutoff to
> g_med_cutoff (default 15). Calls apply_med(amedh, word): if NULL prints "???\n" and returns. Otherwise,
> for the first and each subsequent match (apply_med(amedh, NULL) until NULL), prints three lines: the
> result string, apply_med_get_instring(amedh), and "Cost[f]: %i\n\n" with apply_med_get_cost(amedh)
> (i.e. cost line ends with a blank line). Net not consumed.

> [spec:foma:def:iface.iface-apply-random-fn]
> void iface_apply_random(char *(*applyer)(struct apply_handle *h), int limit)

> [spec:foma:sem:iface.iface-apply-random-fn]
> Shared driver for the random-word commands. If limit == -1 use g_list_random_limit (default 15).
> Requires ≥1 net (iface_stack_check(1), else nothing). Allocates (calloc) an array of `limit`
> {string,count} slots. Gets the cached apply handle (stack_get_ah), sets params from globals
> (iface_apply_set_params). Calls applyer(ah) exactly `limit` times; each non-NULL result is merged into
> the array by linear scan: the first slot whose string strcmp-equals the result gets count++, otherwise
> the first empty (NULL-string) slot receives strdup(result) with count = 1. Then prints each occupied
> slot in first-seen order as "[%i] %s\n" (count, string), freeing each string, frees the array, and
> calls apply_reset_enumerator(ah). Output is nondeterministic (random applyer); duplicates are
> aggregated with counts. Net not consumed.

> [spec:foma:def:iface.iface-apply-set-params-fn]
> void iface_apply_set_params(struct apply_handle *h)

> [spec:foma:sem:iface.iface-apply-set-params-fn]
> Copies the four apply-related globals onto apply handle h, in this order: apply_set_print_space(h,
> g_print_space) (default 0), apply_set_print_pairs(h, g_print_pairs) (default 0), apply_set_show_flags(h,
> g_show_flags) (default 0), apply_set_obey_flags(h, g_obey_flags) (default 1). No other effect.

> [spec:foma:def:iface.iface-apply-up-fn]
> void iface_apply_up(char *word)

> [spec:foma:sem:iface.iface-apply-up-fn]
> Identical to iface_apply_down except it calls apply_up: requires ≥1 net (iface_stack_check(1)); gets the
> cached apply handle (stack_get_ah), sets params from globals (iface_apply_set_params); calls
> apply_up(ah, word); on NULL prints "???\n" and returns; else prints the result plus "\n" and then up to
> g_list_limit (default 100) further apply_up(ah, NULL) results, one per line, stopping on NULL.
> Net not consumed.

> [spec:foma:def:iface.iface-apropos-fn]
> void iface_apropos(char *s)

> [spec:foma:sem:iface.iface-apropos-fn]
> Searches the built-in global_help table (NULL-name terminated) for entries where strstr(name, s) or
> strstr(help, s) matches (case-sensitive byte search on the command name and short-help strings). Two
> passes: first computes maxlen = max utf8strlen(name) over the matching entries only; second prints each
> matching entry as: name, then (maxlen - utf8strlen(name) + 1) space characters, then the short help and
> "\n". No matches → prints nothing.

> [spec:foma:def:iface.iface-close-fn]
> void iface_close()

> [spec:foma:sem:iface.iface-close-fn]
> "close sigma": requires ≥1 net (iface_stack_check(1)). Pops the top net (consumed) and pushes
> fsm_topsort(fsm_minimize(fsm_close_sigma(net, 0))) — removes unknown/IDENTITY symbols from the
> alphabet (mode argument 0), then minimizes and topologically sorts.

> [spec:foma:def:iface.iface-compact-fn]
> void iface_compact()

> [spec:foma:sem:iface.iface-compact-fn]
> "compact sigma": requires ≥1 net. Mutates the top net in place first: fsm_compact(top->fsm) (removes
> redundant sigma symbols) then sigma_sort(top->fsm); then pops that same net and pushes
> fsm_topsort(fsm_minimize(net)).

> [spec:foma:def:iface.iface-complete-fn]
> void iface_complete()

> [spec:foma:sem:iface.iface-complete-fn]
> "complete net": requires ≥1 net (iface_stack_check(1)). Pops the top net (consumed) and pushes
> fsm_complete(net). No minimize/topsort wrapping.

> [spec:foma:def:iface.iface-compose-fn]
> void iface_compose()

> [spec:foma:sem:iface.iface-compose-fn]
> "compose net": requires ≥2 nets (iface_stack_check(2), else nothing). Folds the ENTIRE stack down to
> one net: while stack_size() > 1, pop `one` (top), pop `two` (next), push
> fsm_topsort(fsm_minimize(fsm_compose(one, two))) — the net nearer the top is the first (upper)
> composition operand. Both operands consumed each iteration.

> [spec:foma:def:iface.iface-conc-fn]
> void iface_conc()

> [spec:foma:sem:iface.iface-conc-fn]
> "concatenate": requires ≥2 nets (iface_stack_check(2)). Folds the entire stack: while stack_size() > 1,
> prints the literal string "dd" to stdout (no newline — leftover debug printf; document-and-flag: this is
> a latent bug but is the shipped behavior), then pops `one` (top), pops `two` (next), pushes
> fsm_topsort(fsm_minimize(fsm_concat(one, two))) — the top net is the left/first operand of the
> concatenation. Both operands consumed each iteration.

> [spec:foma:def:iface.iface-crossproduct-fn]
> void iface_crossproduct()

> [spec:foma:sem:iface.iface-crossproduct-fn]
> "crossproduct net": requires ≥2 nets. Pops `one` (top), pops `two` (next), pushes
> fsm_topsort(fsm_minimize(fsm_cross_product(one, two))) — top net is the first (upper-side) operand.
> Single step only: unlike compose/concatenate/union it does NOT fold the whole stack. Operands consumed.

> [spec:foma:def:iface.iface-determinize-fn]
> void iface_determinize()

> [spec:foma:sem:iface.iface-determinize-fn]
> "determinize net": requires ≥1 net. Pops the top net (consumed) and pushes fsm_determinize(net).
> No minimize/topsort wrapping.

> [spec:foma:def:iface.iface-eliminate-flag-fn]
> void iface_eliminate_flag(char *name)

> [spec:foma:sem:iface.iface-eliminate-flag-fn]
> "eliminate flag <name>": requires ≥1 net. Pops the top net (consumed) and pushes
> flag_eliminate(net, name) — eliminates only the flag-diacritic feature called `name`.
> No minimize/topsort wrapping.

> [spec:foma:def:iface.iface-eliminate-flags-fn]
> void iface_eliminate_flags()

> [spec:foma:sem:iface.iface-eliminate-flags-fn]
> "eliminate flags": requires ≥1 net. Pops the top net (consumed) and pushes flag_eliminate(net, NULL) —
> NULL name means eliminate all flag diacritics. No minimize/topsort wrapping.

> [spec:foma:def:iface.iface-extract-ambiguous-fn]
> void iface_extract_ambiguous()

> [spec:foma:sem:iface.iface-extract-ambiguous-fn]
> "extract ambiguous": requires ≥1 net. Pops the top net (consumed) and pushes
> fsm_extract_ambiguous(net) — the transducer paths whose input words are ambiguous.
> No minimize/topsort wrapping.

> [spec:foma:def:iface.iface-extract-number-fn]
> int iface_extract_number(char *s)

> [spec:foma:sem:iface.iface-extract-number-fn]
> Utility: scans string s forward to the first ASCII digit ('0'–'9', compared as unsigned char) and
> returns atoi() of the string starting at that digit. If s contains no digit, the scan stops at the
> terminating NUL and atoi("") returns 0. Minus signs are skipped, so negative numbers are read as
> positive (e.g. "abc-5" → 5). s is not modified.

> [spec:foma:def:iface.iface-extract-unambiguous-fn]
> void iface_extract_unambiguous()

> [spec:foma:sem:iface.iface-extract-unambiguous-fn]
> "extract unambiguous": requires ≥1 net. Pops the top net (consumed) and pushes
> fsm_extract_unambiguous(net). No minimize/topsort wrapping.

> [spec:foma:def:iface.iface-factorize-fn]
> void iface_factorize()

> [spec:foma:sem:iface.iface-factorize-fn]
> "factorize" (bimachine factorization): requires ≥1 net. Pops the top net (consumed) and pushes
> fsm_bimachine(net). No minimize/topsort wrapping.

> [spec:foma:def:iface.iface-help-fn]
> void iface_help()

> [spec:foma:sem:iface.iface-help-fn]
> "help": prints the entire built-in global_help table (a fixed array of {name, help, longhelp} entries,
> NULL-name terminated, listing every command, variable, and regex operator). First pass computes
> maxlen = max utf8strlen(name) over all entries; second pass prints for each entry: the name, then
> (maxlen - utf8strlen(name) + 1) space characters (note: always at least one space; padding loop runs
> from maxlen-len down to 0 inclusive), then the short help text and "\n". Long help is not printed here.

> [spec:foma:def:iface.iface-help-search-fn]
> void iface_help_search(char *s)

> [spec:foma:sem:iface.iface-help-search-fn]
> "help <string>": for each global_help entry where strstr(name, s) or strstr(help, s) matches
> (case-sensitive, matches on name or short help only, not longhelp), prints "##\n" followed by
> printf("%-32.32s%s\n%s\n", name, help, longhelp) — the name left-justified and padded/truncated to
> exactly 32 bytes (byte-based %-32.32s, not UTF-8 aware), then the short help, newline, the long help,
> newline. No matches → prints nothing.

> [spec:foma:def:iface.iface-ignore-fn]
> void iface_ignore()

> [spec:foma:sem:iface.iface-ignore-fn]
> "ignore net": requires ≥2 nets. Pops `one` (top), pops `two` (next), pushes
> fsm_topsort(fsm_minimize(fsm_ignore(one, two, OP_IGNORE_ALL))) — top net is the base language, second
> net the ignored material (A/B with A = top). Single step, does not fold the stack. Operands consumed.

> [spec:foma:def:iface.iface-intersect-fn]
> void iface_intersect()

> [spec:foma:sem:iface.iface-intersect-fn]
> "intersect net": requires ≥2 nets. Folds the entire stack: while stack_size() > 1, pushes
> fsm_topsort(fsm_minimize(fsm_intersect(stack_pop(), stack_pop()))). The two pops occur as function
> arguments in one expression (C evaluation order unspecified; intersection is commutative so the
> resulting language is the same either way). Operands consumed.

> [spec:foma:def:iface.iface-invert-fn]
> void iface_invert()

> [spec:foma:sem:iface.iface-invert-fn]
> "invert net": requires ≥1 net. Pops the top net (consumed) and pushes fsm_invert(net) — swaps
> upper/lower sides. No minimize/topsort wrapping.

> [spec:foma:def:iface.iface-label-net-fn]
> void iface_label_net()

> [spec:foma:sem:iface.iface-label-net-fn]
> "label net": requires ≥1 net. Pops the top net (consumed) and pushes fsm_sigma_pairs_net(net) — a
> single-arc-per-path machine accepting exactly the attested symbol pairs (labels) of the net.
> No minimize/topsort wrapping.

> [spec:foma:def:iface.iface-letter-machine-fn]
> void iface_letter_machine()

> [spec:foma:sem:iface.iface-letter-machine-fn]
> "letter machine": requires ≥1 net. Pops the top net (consumed) and pushes
> fsm_topsort(fsm_minimize(fsm_letter_machine(net))) — converts multi-character symbols to sequences of
> single-letter transitions, then minimizes and topsorts.

> [spec:foma:def:iface.iface-load-defined-fn]
> void iface_load_defined(char *filename)

> [spec:foma:sem:iface.iface-load-defined-fn]
> "load defined <filename>": pure delegation to load_defined(g_defines, filename) (foma/io.c, see
> `[spec:foma:sem:io.load-defined-fn]`); its int return value is discarded. That helper prints "Loading
> definitions from %s.\n", reads the gzipped file fully into memory (on failure prints "File error.\n" to
> stderr and returns 0), then reads consecutive saved networks and add_defined()s each into g_defines
> under its stored name. The stack is untouched.

> [spec:foma:def:iface.iface-load-stack-fn]
> void iface_load_stack(char *filename)

> [spec:foma:sem:iface.iface-load-stack-fn]
> "load stack <filename>": calls fsm_read_binary_file_multiple_init(filename); if that returns NULL,
> prints "<filename>: " to stderr followed by perror("File error") and returns. Otherwise repeatedly calls
> fsm_read_binary_file_multiple(handle) and stack_add()s each returned net until NULL. Nets are pushed in
> file order, so the LAST net in the file ends up on top of the stack. No stack-size precondition.

> [spec:foma:def:iface.iface-lower-side-fn]
> void iface_lower_side()

> [spec:foma:sem:iface.iface-lower-side-fn]
> "lower-side net": requires ≥1 net. Pops the top net (consumed) and pushes
> fsm_topsort(fsm_minimize(fsm_lower(net))) — the lower projection, minimized and topsorted.

> [spec:foma:def:iface.iface-lower-words-fn]
> void iface_lower_words(int limit)

> [spec:foma:sem:iface.iface-lower-words-fn]
> "print lower-words": if limit == -1 substitute g_list_limit (default 100). Requires ≥1 net
> (iface_stack_check(1) is called twice — once before and once after the limit substitution; harmless).
> Gets the cached apply handle (stack_get_ah), sets params from globals (iface_apply_set_params), then
> calls apply_lower_words(ah) at most `limit` times, printing each result plus "\n" and stopping early on
> NULL. Finally calls apply_reset_enumerator(ah). Net not consumed.

> [spec:foma:def:iface.iface-minimize-fn]
> void iface_minimize()

> [spec:foma:sem:iface.iface-minimize-fn]
> "minimize net": requires ≥1 net. Saves the current g_minimal, sets g_minimal = 1, pops the top net
> (consumed) and pushes fsm_topsort(fsm_minimize(net)), then restores g_minimal to the saved value.
> The temporary override forces actual minimization even when the user variable `minimal` is OFF
> (fsm_minimize is a no-op when g_minimal == 0).

> [spec:foma:def:iface.iface-name-net-fn]
> void iface_name_net(char *name)

> [spec:foma:sem:iface.iface-name-net-fn]
> "name net <string>": requires ≥1 net. Copies `name` into the top net's fixed name field with
> strncpy(top->fsm->name, name, 40) — the field is char[40] (FSM_NAME_LEN), so if strlen(name) >= 40 the
> field is truncated WITHOUT NUL termination (latent bug; document literal behavior). Then calls
> iface_print_name(), printing the (new) name plus "\n". Does not pop; net stays on the stack.

> [spec:foma:def:iface.iface-negate-fn]
> void iface_negate()

> [spec:foma:sem:iface.iface-negate-fn]
> "negate net": requires ≥1 net. Pops the top net (consumed) and pushes
> fsm_topsort(fsm_minimize(fsm_complement(net))).

> [spec:foma:def:iface.iface-one-plus-fn]
> void iface_one_plus()

> [spec:foma:sem:iface.iface-one-plus-fn]
> "one-plus net" (Kleene plus): requires ≥1 net. Pops the top net (consumed) and pushes
> fsm_topsort(fsm_minimize(fsm_kleene_plus(net))).

> [spec:foma:def:iface.iface-pairs-call-fn]
> void iface_pairs_call(int limit, int random)

> [spec:foma:sem:iface.iface-pairs-call-fn]
> Shared driver for "print pairs"/"print random-pairs". If limit == -1 substitute g_list_limit (default
> 100). Requires ≥1 net. Gets the cached apply handle; sets ONLY show_flags and obey_flags from globals
> (not print_space/print_pairs), then temporarily reconfigures the handle's formatting symbols: space =
> "\x01", epsilon = "\x02", separator = "\x03". Loops at most `limit` times: result =
> apply_random_words(ah) if random == 1 else apply_words(ah); NULL breaks. Each result is split with
> iface_split_result into freshly allocated upper and lower strings, printed as "%s\t%s\n" (upper TAB
> lower), and both strings freed. Afterwards restores space " ", epsilon "0", separator ":" on the handle
> and calls apply_reset_enumerator(ah). Net not consumed.

> [spec:foma:def:iface.iface-pairs-file-fn]
> void iface_pairs_file(char *filename)

> [spec:foma:sem:iface.iface-pairs-file-fn]
> "print pairs > filename": requires ≥1 net. If the top net's pathcount == PATHCOUNT_CYCLIC (-1) prints
> "FSM is cyclic: can't write all pairs to file.\n" and returns. Prints "Writing to %s.\n". Opens filename
> "w"; on failure perror("Error opening file") and returns. Then identical handle setup to
> iface_pairs_call (show_flags/obey_flags from globals; space "\x01", epsilon "\x02", separator "\x03"),
> but loops WITHOUT limit: apply_words(ah) until NULL, splitting each result via iface_split_result and
> writing "%s\t%s\n" (upper TAB lower) to the file, freeing the split strings. Restores space " ",
> epsilon "0", separator ":", calls apply_reset_enumerator, and fcloses the file. Net not consumed.

> [spec:foma:def:iface.iface-pairs-fn]
> void iface_pairs(int limit)

> [spec:foma:sem:iface.iface-pairs-fn]
> "print pairs": pure delegation — calls iface_pairs_call(limit, 0) (enumerated, non-random pairs).
> See `[spec:foma:sem:iface.iface-pairs-call-fn]`.

> [spec:foma:def:iface.iface-pop-fn]
> void iface_pop()

> [spec:foma:sem:iface.iface-pop-fn]
> "pop stack": if stack_size() < 1 prints "Stack is empty.\n" (note: does NOT use iface_stack_check, so
> no "Not enough networks..." message); otherwise pops the top net and fsm_destroy()s it (memory freed,
> net gone).

> [spec:foma:def:iface.iface-print-bool-fn]
> void iface_print_bool(int value)

> [spec:foma:sem:iface.iface-print-bool-fn]
> Prints the integer test result as exactly "%i (1 = TRUE, 0 = FALSE)\n" — e.g. "1 (1 = TRUE, 0 =
> FALSE)". Used by all the "test ..." commands to report their boolean outcome.

> [spec:foma:def:iface.iface-print-cmatrix-att-fn]
> void iface_print_cmatrix_att(char *filename)

> [spec:foma:sem:iface.iface-print-cmatrix-att-fn]
> "export cmatrix": requires ≥1 net. If top->fsm->medlookup or top->fsm->medlookup->confusion_matrix is
> NULL, prints "No confusion matrix defined.\n". Otherwise: outfile = stdout when filename is NULL, else
> fopen(filename, "w") plus message "Writing confusion matrix to file '%s'.\n"; the fopen result is NOT
> checked for NULL (latent crash bug on open failure — document literal behavior). Calls
> cmatrix_print_att(top->fsm, outfile). The opened file is never fclose'd (latent leak/flush reliance).
> Net not consumed.

> [spec:foma:def:iface.iface-print-cmatrix-fn]
> void iface_print_cmatrix()

> [spec:foma:sem:iface.iface-print-cmatrix-fn]
> "print cmatrix": requires ≥1 net. If top->fsm->medlookup or its confusion_matrix is NULL, prints
> "No confusion matrix defined.\n"; otherwise calls cmatrix_print(top->fsm) (tabular dump to stdout).
> Net not consumed.

> [spec:foma:def:iface.iface-print-defined-fn]
> void iface_print_defined()

> [spec:foma:sem:iface.iface-print-defined-fn]
> "print defined": if g_defines == NULL prints "No defined symbols.\n" (and then still falls through to
> the loops, which are no-ops over NULL lists). For every node in the g_defines linked list whose name is
> non-NULL: prints "%s\t" (name, TAB) followed by print_stats(net) (the one-line size summary; see
> `[spec:foma:sem:iface.print-stats-fn]`). Then for every node in g_defines_f (defined regex functions)
> with non-NULL name: prints "%s@%i)\t" — name, '@', numargs, then a literal unmatched ')' and TAB
> (literal quirk in the format string) — followed by the function's regex source and "\n".

> [spec:foma:def:iface.iface-print-dot-fn]
> void iface_print_dot(char *filename)

> [spec:foma:sem:iface.iface-print-dot-fn]
> "print dot (> filename)": requires ≥1 net. If filename != NULL first prints "Writing dot file to
> %s.\n". Delegates to the static helper print_dot(top->fsm, filename) (see
> `[spec:foma:sem:iface.print-dot-fn]`; NULL filename → stdout). Net not consumed.

> [spec:foma:def:iface.iface-print-name-fn]
> void iface_print_name()

> [spec:foma:sem:iface.iface-print-name-fn]
> "print name": requires ≥1 net (iface_stack_check(1)). Prints the top net's name field plus "\n".
> Net not consumed.

> [spec:foma:def:iface.iface-print-net-fn]
> void iface_print_net(char *netname, char *filename)

> [spec:foma:sem:iface.iface-print-net-fn]
> "print net (name) (> filename)": if netname != NULL, looks it up with find_defined(g_defines, netname);
> if not found and g_verbose (default 1) prints "No defined network %s.\n" to stderr and fflushes stderr,
> then returns (silently when g_verbose is 0); if found, calls the static helper print_net(net, filename)
> (see `[spec:foma:sem:iface.print-net-fn]`). If netname == NULL, requires ≥1 net on the stack and calls
> print_net(top->fsm, filename). Nothing is consumed.

> [spec:foma:def:iface.iface-print-shortest-string-fn]
> void iface_print_shortest_string()

> [spec:foma:sem:iface.iface-print-shortest-string-fn]
> "print shortest-string": requires ≥1 net; works on copies, top net not consumed. Computes the sub-
> language of shortest strings via the regex identity L - ?+ [[L .o. [?:"@TMP@"]*].l .o. ["@TMP@":?]*].l,
> built literally as: fsm_minimize(fsm_minus(fsm_copy(L), fsm_concat(fsm_kleene_plus(fsm_identity()),
> fsm_lower(fsm_compose(fsm_lower(fsm_compose(fsm_copy(L), fsm_kleene_star(fsm_cross_product(
> fsm_identity(), fsm_symbol("@TMP@"))))), fsm_kleene_star(fsm_cross_product(fsm_symbol("@TMP@"),
> fsm_identity()))))))). If top arity == 1: L = a copy of the top net; apply_init the Result, take the
> first apply_words() string; if non-NULL print it plus "\n" (nothing printed when the language is
> empty); apply_clear the handle and fsm_destroy(Result). (The initial fsm_copy of the top net is leaked
> in this branch.) If arity == 2: compute the same Result independently for the upper projection
> (fsm_upper) and lower projection (fsm_lower) of a copy; print "Upper: %s\n" and then "Lower: %s\n",
> substituting "" for a NULL first word; each handle cleared and each Result destroyed.

> [spec:foma:def:iface.iface-print-shortest-string-size-fn]
> void iface_print_shortest_string_size()

> [spec:foma:sem:iface.iface-print-shortest-string-size-fn]
> "print shortest-string-size": requires ≥1 net; works on a copy, top net not consumed. Computes the
> unary-image automaton [L .o. [?:a]*].l, literally fsm_minimize(fsm_lower(fsm_compose(L,
> fsm_kleene_star(fsm_cross_product(fsm_identity(), fsm_symbol("a")))))), and reports statecount - 1 of
> the minimized result. If top arity == 1 prints "Shortest acyclic path length: %i\n"; if arity == 2 does
> this separately for the upper and lower projections and prints "Shortest acyclic upper path length:
> %i\n" then "Shortest acyclic lower path length: %i\n". Caveat (document literal behavior): for a
> minimal unary DFA, statecount-1 equals the shortest length only when the language is acyclic and
> length-uniform in the right way; for languages with several string lengths the minimal chain's
> statecount-1 is the LONGEST length — latent bug. The Result nets are never fsm_destroy'd (leak).

> [spec:foma:def:iface.iface-print-sigma-fn]
> void iface_print_sigma()

> [spec:foma:sem:iface.iface-print-sigma-fn]
> "print sigma": requires ≥1 net. Calls the static helper print_sigma(top->fsm->sigma, stdout) (see
> `[spec:foma:sem:iface.print-sigma-fn]`). Net not consumed.

> [spec:foma:def:iface.iface-print-stats-fn]
> void iface_print_stats()

> [spec:foma:sem:iface.iface-print-stats-fn]
> "print size": requires ≥1 net. Calls the static helper print_stats(top->fsm) (one-line size/statistics
> summary; see `[spec:foma:sem:iface.print-stats-fn]`). Net not consumed.

> [spec:foma:def:iface.iface-prune-fn]
> void iface_prune()

> [spec:foma:sem:iface.iface-prune-fn]
> "prune net": requires ≥1 net. Pops the top net (consumed) and pushes
> fsm_topsort(fsm_coaccessible(net)) — removes non-coaccessible states; topsorted but NOT minimized.

> [spec:foma:def:iface.iface-quit-fn]
> void iface_quit()

> [spec:foma:sem:iface.iface-quit-fn]
> "quit": calls remove_defined(g_defines, NULL) — NULL name means destroy every defined net
> (fsm_destroy) and free every stored name; then pops and fsm_destroy()s every remaining stack net until
> stack_isempty(); finally exit(0). Never returns; prints nothing.

> [spec:foma:def:iface.iface-random-lower-fn]
> void iface_random_lower(int limit)

> [spec:foma:sem:iface.iface-random-lower-fn]
> "print random-lower": pure delegate — calls iface_apply_random(&apply_random_lower, limit). All
> behavior (limit == -1 → g_list_random_limit default 15, aggregation, "[%i] %s\n" output) per
> `[spec:foma:sem:iface.iface-apply-random-fn]`, with apply_random_lower as the applyer (random walk
> emitting the lower-side string).

> [spec:foma:def:iface.iface-random-pairs-fn]
> void iface_random_pairs(int limit)

> [spec:foma:sem:iface.iface-random-pairs-fn]
> "print random-pairs": pure delegate — calls iface_pairs_call(limit, 1) (see
> `[spec:foma:sem:iface.iface-pairs-call-fn]`); random == 1 makes the driver draw results with
> apply_random_words and print each as "upper\tlower\n" after splitting. limit == -1 becomes
> g_list_limit (default 100) inside iface_pairs_call — note: the list limit, NOT g_list_random_limit,
> unlike the other random commands.

> [spec:foma:def:iface.iface-random-upper-fn]
> void iface_random_upper(int limit)

> [spec:foma:sem:iface.iface-random-upper-fn]
> "print random-upper": pure delegate — calls iface_apply_random(&apply_random_upper, limit); see
> `[spec:foma:sem:iface.iface-apply-random-fn]`. applyer is apply_random_upper (random walk emitting the
> upper-side string).

> [spec:foma:def:iface.iface-random-words-fn]
> void iface_random_words(int limit)

> [spec:foma:sem:iface.iface-random-words-fn]
> "print random-words": pure delegate — calls iface_apply_random(&apply_random_words, limit); see
> `[spec:foma:sem:iface.iface-apply-random-fn]`. applyer is apply_random_words (random walk emitting the
> whole word, pairs formatted per apply settings).

> [spec:foma:def:iface.iface-read-att-fn]
> int iface_read_att(char *filename)

> [spec:foma:sem:iface.iface-read-att-fn]
> "read att <filename>": prints "Reading AT&T file: %s\n" (filename) to stdout FIRST, then calls
> read_att(filename) (io.c; parses tab-separated AT&T transition lines, honoring g_att_epsilon,
> default "@0@"). If it returns NULL, prints "<filename>: " to stderr, perror("Error opening file"),
> returns 1. Otherwise pushes the net onto the stack as-is (no minimize/topsort) and returns 0.

> [spec:foma:def:iface.iface-read-prolog-fn]
> int iface_read_prolog(char *filename)

> [spec:foma:sem:iface.iface-read-prolog-fn]
> "read prolog <filename>": prints "Reading prolog: %s\n" (filename) to stdout first, then calls
> fsm_read_prolog(filename) (io.c). If NULL, prints "<filename>: " to stderr, perror("Error opening
> file"), returns 1. Otherwise pushes the net as-is (no minimize/topsort) and returns 0.

> [spec:foma:def:iface.iface-read-spaced-text-fn]
> int iface_read_spaced_text(char *filename)

> [spec:foma:sem:iface.iface-read-spaced-text-fn]
> "read spaced-text <filename>": calls fsm_read_spaced_text_file(filename) (io.c; each line is a
> space-separated symbol sequence, alternating lines form upper/lower of a pair). If NULL, prints
> "<filename>: " to stderr, perror("File error"), returns 1. Otherwise pushes
> fsm_topsort(fsm_minimize(net)) and returns 0. No success message is printed by this function
> (stack_add prints stats when g_verbose).

> [spec:foma:def:iface.iface-read-text-fn]
> int iface_read_text(char *filename)

> [spec:foma:sem:iface.iface-read-text-fn]
> "read text <filename>": calls fsm_read_text_file(filename) (io.c; one word per line, compiled into
> an automaton). If NULL, prints "<filename>: " to stderr, perror("File error"), returns 1. Otherwise
> pushes fsm_topsort(fsm_minimize(net)) and returns 0. No success message printed here.

> [spec:foma:def:iface.iface-reverse-fn]
> void iface_reverse()

> [spec:foma:sem:iface.iface-reverse-fn]
> "reverse net": requires ≥1 net (iface_stack_check(1)). Pops the top net (consumed) and pushes
> fsm_topsort(fsm_determinize(fsm_reverse(net))) — note: determinized and topsorted but NOT minimized.

> [spec:foma:def:iface.iface-rotate-fn]
> void iface_rotate()

> [spec:foma:sem:iface.iface-rotate-fn]
> "rotate stack": requires ≥1 net (iface_stack_check(1)), then calls stack_rotate(), which despite the
> name only SWAPS the fsm pointers of the top and bottom stack entries (no-op when size is 1). Prints
> nothing on success. Latent bug: stack_rotate swaps only the ->fsm fields; any cached apply/med
> handles (ah/amedh) stay on their entries and now reference the swapped-away nets.

> [spec:foma:def:iface.iface-save-defined-fn]
> void iface_save_defined(char *filename)

> [spec:foma:sem:iface.iface-save-defined-fn]
> "save defined <filename>": pure delegate — calls save_defined(g_defines, filename) (io.c), return
> value ignored. That helper: if g_defines is NULL prints "No defined networks.\n" to stderr; on
> gzopen(filename,"wb") failure prints "Error opening file %s for writing.\n"; else prints "Writing
> definitions to file %s.\n" and, for each defined entry (skipping net-less entries with "Skipping
> definition without network.\n"), copies the define name into net->name (strncpy, 40 chars) and
> appends the net via foma_net_print to the single gzipped file; then gzclose. Nets not consumed.

> [spec:foma:def:iface.iface-save-stack-fn]
> void iface_save_stack(char *filename)

> [spec:foma:sem:iface.iface-save-stack-fn]
> "save stack <filename>": requires ≥1 net (iface_stack_check(1)). gzopen(filename, "wb"); on failure
> prints "Error opening file %s for writing.\n" and returns. Else prints "Writing to file %s.\n" and
> walks the stack from stack_find_bottom() following ->next while ->next != NULL (i.e. every real
> entry, bottom→top, excluding the sentinel), writing each fsm with foma_net_print (see
> `[spec:foma:sem:iface.foma-net-print-fn]`) into the one gzipped file; then gzclose. Nets not consumed;
> stack unchanged. Bottom→top order means a later "load stack" (which pushes in file order) restores
> the original stack order.

> [spec:foma:def:iface.iface-sequentialize-fn]
> void iface_sequentialize()

> [spec:foma:sem:iface.iface-sequentialize-fn]
> "sequentialize": requires ≥1 net (iface_stack_check(1)). Pops the top net (consumed) and pushes
> fsm_sequentialize(net). No minimize/topsort wrapping.

> [spec:foma:def:iface.iface-set-variable-fn]
> void iface_set_variable(char *name, char *value)

> [spec:foma:sem:iface.iface-set-variable-fn]
> "set <name> <value>": scans the global_vars table in order; a variable matches when
> strncmp(name, var.name, 8) == 0 — only the FIRST 8 characters are compared (latent bug: any
> 8-char-prefix-equal name matches; first table match wins). Table order: flag-is-epsilon, minimal,
> name-nets, obey-flags, print-pairs, print-sigma, print-space, quit-on-fail, recursive-define,
> quote-special, show-flags, sort-arcs, verbose, hopcroft-min, compose-tristate (all BOOL),
> med-limit, med-cutoff (INT), lexc-align (BOOL), att-epsilon (STRING). For BOOL: value "ON" or "1"
> → 1, "OFF" or "0" → 0, else print "Invalid value '%s' for variable '%s'\n" and return; on success
> store and print "variable %s = %s\n" (ON/OFF, re-read from the variable). For STRING: store
> strdup(value) (old string leaked) and print "variable %s = %s\n". For INT: strtol base 10; if errno
> set, no digits consumed (endptr == value), or result < 0, print "invalid value %s for variable %s\n"
> (lowercase 'i') and return; else print "variable %s = %i\n" then store. If no table entry matches,
> prints "*There is no global variable '%s'.\n".

> [spec:foma:def:iface.iface-show-variable-fn]
> void iface_show_variable(char *name)

> [spec:foma:sem:iface.iface-show-variable-fn]
> "show variable <name>": scans global_vars with the same 8-character-prefix strncmp match as
> iface_set_variable; on the first match prints "%s = %s\n" with the full variable name and
> ON/OFF computed as *(int*)ptr == 1 ? "ON" : "OFF" REGARDLESS of the variable's type — latent bug:
> for FVAR_INT it prints ON only when the value is exactly 1, and for FVAR_STRING (att-epsilon) it
> reinterprets the char* pointer bytes as an int (effectively garbage, practically "OFF"). If nothing
> matches prints "*There is no global variable '%s'.\n".

> [spec:foma:def:iface.iface-show-variables-fn]
> void iface_show_variables()

> [spec:foma:sem:iface.iface-show-variables-fn]
> "show variables": iterates the whole global_vars table in declaration order (see
> `[spec:foma:sem:iface.iface-set-variable-fn]` for the order and types) printing one line per
> variable: FVAR_BOOL as "%-17.17s: %s\n" with "ON" iff the int value == 1 else "OFF"; FVAR_INT as
> "%-17.17s: %i\n"; FVAR_STRING as "%-17.17s: %s\n". Name field is left-justified, padded/truncated
> to exactly 17 characters.

> [spec:foma:def:iface.iface-shuffle-fn]
> void iface_shuffle()

> [spec:foma:sem:iface.iface-shuffle-fn]
> "shuffle net": requires ≥2 nets (iface_stack_check(2)). Folds the entire stack: while stack_size() >
> 1, pushes fsm_minimize(fsm_shuffle(stack_pop(), stack_pop())) — both pops occur as arguments in one
> expression (C evaluation order unspecified; shuffle is commutative so the result language is the
> same). Minimized but NOT topsorted. Operands consumed.

> [spec:foma:def:iface.iface-sigma-net-fn]
> void iface_sigma_net()

> [spec:foma:sem:iface.iface-sigma-net-fn]
> "sigma net": requires ≥1 net (iface_stack_check(1)). Pops the top net (consumed) and pushes
> fsm_sigma_net(net) — a one-state-per-symbol machine accepting exactly the single symbols of the
> net's alphabet. No minimize/topsort wrapping.

> [spec:foma:def:iface.iface-sort-fn]
> void iface_sort()

> [spec:foma:sem:iface.iface-sort-fn]
> "sort net": requires ≥1 net (iface_stack_check(1)). First sigma_sort(top->fsm) in place (sorts the
> alphabet and renumbers arc labels accordingly), then pops that same net and pushes
> fsm_topsort(net) — topological state renumbering. No minimization.

> [spec:foma:def:iface.iface-sort-input-fn]
> void iface_sort_input()

> [spec:foma:sem:iface.iface-sort-input-fn]
> "sort in": requires ≥1 net (iface_stack_check(1)). Calls fsm_sort_arcs(top->fsm, 1) — sorts each
> state's arcs by input-symbol number, in place, setting the net's arcs_sorted_in flag. The net stays
> on the stack (not popped/re-pushed); nothing printed on success.

> [spec:foma:def:iface.iface-sort-output-fn]
> void iface_sort_output()

> [spec:foma:sem:iface.iface-sort-output-fn]
> "sort out": identical to iface_sort_input but calls fsm_sort_arcs(top->fsm, 2) — sorts each state's
> arcs by output-symbol number in place, setting arcs_sorted_out. Net stays on the stack.

> [spec:foma:def:iface.iface-split-result-fn]
> void iface_split_result(char *result, char **upper, char **lower)

> [spec:foma:sem:iface.iface-split-result-fn]
> Helper for the pairs commands: splits an apply result encoded with space='\001', epsilon='\002',
> separator='\003' into freshly allocated upper and lower strings. Allocates *upper and *lower with
> calloc(strlen(result), 1) each — latent bug: no +1 for the NUL terminator, so if nothing is
> filtered out (result contains no \001/\002/\003 bytes) the terminator writes one byte past the
> allocation. Then: iface_split_string(result, *upper) extracts the upper side; xstrrev(result)
> (in-place byte reversal); iface_split_string(result, *lower) — on the reversed string the same
> upper-side filter extracts the lower symbols (in each reversed "u\003l" pair the lower symbol now
> precedes the separator); xstrrev(*lower) restores its order; xstrrev(result) restores the caller's
> buffer. Byte-wise reversal corrupts multi-byte UTF-8 symbols. Caller frees *upper/*lower.

> [spec:foma:def:iface.iface-split-string-fn]
> void iface_split_string(char *result, char *string)

> [spec:foma:sem:iface.iface-split-string-fn]
> Two-state filter that appends to `string` (must be a pre-zeroed buffer) the upper side of `result`,
> a pairs-encoded apply output using bytes '\001' (space), '\002' (epsilon), '\003' (separator). It
> simulates "SEPARATOR \SPACE+ @-> 0 .o. SPACE|SEPARATOR|EPSILON -> 0". State ZERO (initial): NUL →
> stop; '\001' or '\002' → skip, stay ZERO; '\003' → skip, go to state ONE; any other byte → append
> that one byte (strncat of 1) to `string`, stay ZERO. State ONE: NUL → stop; '\001' → skip, go to
> ZERO; anything else → skip, stay ONE. Net effect: for each space-separated "u\003l" pair only u is
> kept; bare symbols are kept whole; epsilons and separators vanish.

> [spec:foma:def:iface.iface-stack-check-fn]
> int iface_stack_check (int size)

> [spec:foma:sem:iface.iface-stack-check-fn]
> Guard used by nearly every command: if stack_size() < size, prints "Not enough networks on stack.
> Operation requires at least %i.\n" (size) to stdout and returns 0; otherwise returns 1. No other
> side effects.

> [spec:foma:def:iface.iface-substitute-defined-fn]
> void iface_substitute_defined (char *original, char *substitute)

> [spec:foma:sem:iface.iface-substitute-defined-fn]
> "substitute defined <substitute> for <original>": requires ≥1 net. Dequotes both arguments in place
> (dequote_string: strips surrounding double quotes and decodes \uXXXX escapes, only if quoted). If
> find_defined(g_defines, substitute) is NULL prints "No defined network '%s'.\n" (substitute) and
> stops. Else if fsm_symbol_occurs(top->fsm, original, M_UPPER + M_LOWER) == 0 (symbol appears on
> neither side) prints "Symbol '%s' does not occur.\n" (original) and stops. Otherwise computes
> newnet = fsm_substitute_label(top->fsm, original, subnet) (splices a COPY of the defined net over
> every arc labeled `original`; top net itself not consumed by this call), pops the old top (popped
> net is NOT fsm_destroy'd — latent leak), prints "Substituted network '%s' for '%s'.\n" (substitute,
> original), and pushes fsm_topsort(fsm_minimize(newnet)).

> [spec:foma:def:iface.iface-substitute-symbol-fn]
> void iface_substitute_symbol (char *original, char *substitute)

> [spec:foma:sem:iface.iface-substitute-symbol-fn]
> "substitute symbol <substitute> for <original>": requires ≥1 net. Dequotes both arguments in place
> (dequote_string). Pops the top net (consumed) and pushes
> fsm_topsort(fsm_minimize(fsm_substitute_symbol(net, original, substitute))) — every arc occurrence
> of `original` is relabeled `substitute` ("0" meaning epsilon) and `original` is removed from the
> alphabet. Then prints "Substituted '%s' for '%s'.\n" (substitute, original). No existence check:
> if `original` is not in the alphabet the operation is a no-op apart from the message.

> [spec:foma:def:iface.iface-test-equivalent-fn]
> void iface_test_equivalent()

> [spec:foma:sem:iface.iface-test-equivalent-fn]
> "test equivalent": requires ≥2 nets (iface_stack_check(2)). Makes fsm_copy of the top net (`one`)
> and of the second-from-top net (`two`) — originals stay on the stack untouched. Runs fsm_count on
> both copies, then prints iface_print_bool(fsm_equivalent(one, two)) i.e. "%i (1 = TRUE, 0 =
> FALSE)\n". fsm_equivalent does a parallel path-equivalence traversal (reliable for minimized
> recognizers; equivalence is undecidable for transducers in general). Latent leak: the two copies
> are never fsm_destroy'd.

> [spec:foma:def:iface.iface-test-functional-fn]
> void iface_test_functional()

> [spec:foma:sem:iface.iface-test-functional-fn]
> "test functional": requires ≥1 net. Prints iface_print_bool(fsm_isfunctional(top->fsm)) — "%i (1 =
> TRUE, 0 = FALSE)\n"; fsm_isfunctional works on internal copies (tests identity of net.i ∘ net), so
> the top net is neither consumed nor modified.

> [spec:foma:def:iface.iface-test-identity-fn]
> void iface_test_identity()

> [spec:foma:sem:iface.iface-test-identity-fn]
> "test identity": requires ≥1 net. Prints iface_print_bool(fsm_isidentity(top->fsm)) — "%i (1 =
> TRUE, 0 = FALSE)\n"; true iff the transducer maps every accepted string only to itself
> (fsm_isidentity analyzes a minimized copy; top net not consumed).

> [spec:foma:def:iface.iface-test-lower-universal-fn]
> void iface_test_lower_universal()

> [spec:foma:sem:iface.iface-test-lower-universal-fn]
> "test lower-universal": requires ≥1 net. Computes tmp = fsm_complement(fsm_lower(fsm_copy(top->fsm)))
> (works on a copy; top preserved), prints iface_print_bool(fsm_isempty(tmp)) — "%i (1 = TRUE, 0 =
> FALSE)\n", true iff the lower projection is Σ* — then fsm_destroy(tmp).

> [spec:foma:def:iface.iface-test-nonnull-fn]
> void iface_test_nonnull()

> [spec:foma:sem:iface.iface-test-nonnull-fn]
> "test non-null": requires ≥1 net. Prints iface_print_bool(!fsm_isempty(top->fsm)) — "%i (1 = TRUE,
> 0 = FALSE)\n", 1 iff the language is non-empty. fsm_isempty minimizes a copy internally; top net
> not consumed.

> [spec:foma:def:iface.iface-test-null-fn]
> void iface_test_null()

> [spec:foma:sem:iface.iface-test-null-fn]
> "test null": requires ≥1 net. Prints iface_print_bool(fsm_isempty(top->fsm)) — "%i (1 = TRUE, 0 =
> FALSE)\n", 1 iff the language is the empty language ∅. Top net not consumed.

> [spec:foma:def:iface.iface-test-sequential-fn]
> void iface_test_sequential()

> [spec:foma:sem:iface.iface-test-sequential-fn]
> "test sequential": requires ≥1 net. Prints iface_print_bool(fsm_issequential(top->fsm)) — "%i (1 =
> TRUE, 0 = FALSE)\n". Note fsm_issequential itself additionally prints "fails at state %i\n" to
> stdout (before the bool line) when the machine is not sequential. Top net not consumed.

> [spec:foma:def:iface.iface-test-unambiguous-fn]
> void iface_test_unambiguous()

> [spec:foma:sem:iface.iface-test-unambiguous-fn]
> "test unambiguous": requires ≥1 net. Prints iface_print_bool(fsm_isunambiguous(top->fsm)) — "%i (1 =
> TRUE, 0 = FALSE)\n", 1 iff no input string has more than one transduction path (computed on internal
> copies via lower-side determinization; top net not consumed).

> [spec:foma:def:iface.iface-test-upper-universal-fn]
> void iface_test_upper_universal()

> [spec:foma:sem:iface.iface-test-upper-universal-fn]
> "test upper-universal": identical to iface_test_lower_universal but with fsm_upper: tmp =
> fsm_complement(fsm_upper(fsm_copy(top->fsm))); prints iface_print_bool(fsm_isempty(tmp)) — 1 iff
> the upper projection is Σ*; fsm_destroy(tmp). Top preserved.

> [spec:foma:def:iface.iface-turn-fn]
> void iface_turn()

> [spec:foma:sem:iface.iface-turn-fn]
> "turn stack": requires ≥1 net (iface_stack_check(1)) then calls stack_rotate() — byte-for-byte the
> same behavior as iface_rotate. Latent bug: despite the help text "turns stack upside down", it does
> NOT call stack_turn(); it only swaps the top and bottom entries' fsm pointers (see
> `[spec:foma:sem:iface.iface-rotate-fn]`).

> [spec:foma:def:iface.iface-twosided-flags-fn]
> void iface_twosided_flags()

> [spec:foma:sem:iface.iface-twosided-flags-fn]
> "twosided flag-diacritics": requires ≥1 net. Pops the top net (consumed) and pushes
> flag_twosided(net) — rewrites flag-diacritic arcs so flags always appear as identity pairs on both
> sides. No minimize/topsort wrapping.

> [spec:foma:def:iface.iface-union-fn]
> void iface_union()

> [spec:foma:sem:iface.iface-union-fn]
> "union net": requires ≥2 nets (iface_stack_check(2)). Folds the entire stack: while stack_size() >
> 1, pushes fsm_minimize(fsm_union(stack_pop(), stack_pop())) — pops occur as arguments in one
> expression (C evaluation order unspecified; union is commutative). Minimized but NOT topsorted.
> Operands consumed.

> [spec:foma:def:iface.iface-upper-side-fn]
> void iface_upper_side()

> [spec:foma:sem:iface.iface-upper-side-fn]
> "upper-side net": requires ≥1 net. Pops the top net (consumed) and pushes
> fsm_topsort(fsm_minimize(fsm_upper(net))) — the upper (input-side) projection, minimized and
> topsorted.

> [spec:foma:def:iface.iface-upper-words-fn]
> void iface_upper_words(int limit)

> [spec:foma:sem:iface.iface-upper-words-fn]
> "print upper-words [limit]": if limit == -1 use g_list_limit (default 100). Requires ≥1 net
> (iface_stack_check(1)). Gets the cached apply handle (stack_get_ah), sets params from globals
> (iface_apply_set_params), then calls apply_upper_words(ah) up to `limit` times, printing each
> non-NULL result plus "\n" and stopping early on NULL; finally apply_reset_enumerator(ah).
> Net not consumed.

> [spec:foma:def:iface.iface-view-fn]
> void iface_view()

> [spec:foma:sem:iface.iface-view-fn]
> "view net": requires ≥1 net (iface_stack_check(1)); calls view_net(top->fsm) (see
> `[spec:foma:sem:iface.view-net-fn]`) which renders the net to a temp dot file, converts it to PNG
> with graphviz `dot`, and opens it in the system viewer. Net not consumed.

> [spec:foma:def:iface.iface-warranty-fn]
> void iface_warranty()

> [spec:foma:sem:iface.iface-warranty-fn]
> "help warranty": prints the fixed global warranty[] string verbatim to stdout:
> "\nLicensed under the Apache License, Version 2.0 (the \"License\")\nyou may not use this file
> except in compliance with the License.\nYou may obtain a copy of the License at\n\n
> http://www.apache.org/licenses/LICENSE-2.0\n\n" (the URL line is indented with four spaces; the
> whole text starts with a blank line and ends with a blank line). No other effects.

> [spec:foma:def:iface.iface-words-file-fn]
> void iface_words_file(char *filename, int type)

> [spec:foma:sem:iface.iface-words-file-fn]
> "print words/upper-words/lower-words > filename": type selects the enumerator — 1 → 
> apply_upper_words, 2 → apply_lower_words, otherwise the current value of a STATIC function-pointer
> variable initialized to apply_words. Latent bug: because the pointer is static and type 0 never
> resets it, after any call with type 1 or 2 a later type-0 call reuses the previous
> upper/lower enumerator. Requires ≥1 net. If top->fsm->pathcount == PATHCOUNT_CYCLIC (-1) prints
> "FSM is cyclic: can't write all words to file.\n" and returns. Prints "Writing to %s.\n" (before
> opening), fopen(filename, "w"); on failure perror("Error opening file") and return. Gets the cached
> apply handle, sets params from globals, then loops with NO limit writing every enumerated word plus
> "\n" to the file until NULL; apply_reset_enumerator(ah); fclose. Net not consumed.

> [spec:foma:def:iface.iface-words-fn]
> void iface_words(int limit)

> [spec:foma:sem:iface.iface-words-fn]
> "print words [limit]": if limit == -1 use g_list_limit (default 100). Requires ≥1 net. Gets the
> cached apply handle (stack_get_ah), sets params from globals (iface_apply_set_params), calls
> apply_words(ah) up to `limit` times printing each non-NULL result plus "\n", stopping early on
> NULL; then apply_reset_enumerator(ah). Net not consumed.

> [spec:foma:def:iface.iface-write-att-fn]
> int iface_write_att(char *filename)

> [spec:foma:sem:iface.iface-write-att-fn]
> "write att [> filename]": if the stack is empty (iface_stack_check(1) fails) returns 1. Uses the top
> net without consuming it. filename == NULL → output to stdout; else prints "Writing AT&T file: %s\n"
> then fopen(filename, "w") — on failure prints "<filename>: " to stderr, perror("File error
> opening."), returns 1. Calls net_print_att(net, outfile) (tab-separated AT&T lines; epsilon written
> as g_att_epsilon, default "@0@"). Closes the file only when filename was non-NULL. Returns 0.

> [spec:foma:def:iface.iface-write-prolog-fn]
> void iface_write_prolog(char *filename)

> [spec:foma:sem:iface.iface-write-prolog-fn]
> "write prolog [> filename]": requires ≥1 net (iface_stack_check(1)); delegates to
> foma_write_prolog(top->fsm, filename) (io.c, `[spec:foma:sem:io.foma-write-prolog-fn]`): filename ==
> NULL → stdout; else fopen "w" (on failure prints "Error writing to file '%s'. Using stdout.\n" and
> falls back to stdout) and prints "Writing prolog to file '%s'.\n"; emits "network(<name>).\n",
> symbol/arc/final clauses. Net not consumed; return value ignored.

> [spec:foma:def:iface.iface-zero-plus-fn]
> void iface_zero_plus()

> [spec:foma:sem:iface.iface-zero-plus-fn]
> "zero-plus net" (Kleene star): requires ≥1 net. Pops the top net (consumed) and pushes
> fsm_topsort(fsm_minimize(fsm_kleene_star(net))).

> [spec:foma:def:iface.print-dot-fn]
> static int print_dot(struct fsm *net, char *filename)

> [spec:foma:sem:iface.print-dot-fn]
> Writes `net` in Graphviz dot format to filename (fopen "w", no error check — NULL filename →
> stdout). Steps: fsm_count(net); build a finals[] table indexed by state_no. Emit "digraph A {\n"
> "rankdir = LR;\n"; then for each state 0..statecount-1 one line "node
> [shape=doublecircle,style=filled] %i\n" if final else "node [shape=circle,style=filled] %i\n".
> Then arcs: allocates a printed[] flag per transition line (calloc(linecount, sizeof(printed)) —
> sizeof of the POINTER, i.e. over-allocation bug, harmless). For each unprinted transition with
> target != -1, emits one edge "%i -> %i [label=\"" and merges into its label ALL same-source
> same-target transitions (marking them printed): each label item is the symbol via
> sigptr(net->sigma, in) escaped for '"' (escape_string) when in == out and out != UNKNOWN, else
> "<in:out>" with both sides escaped; after each item, if accumulated label length exceeds 12 emit
> literal "\\n" (dot newline) and reset the counter, else a space. Edge ends "\"];\n" (so the label
> always has a trailing space or \n before the quote). Finally "}\n", free tables, fclose only when
> filename non-NULL, return 1. Strings returned by escape_string/sigptr may be freshly allocated and
> are leaked.

> [spec:foma:def:iface.print-mem-size-fn]
> void print_mem_size(struct fsm *net)

> [spec:foma:sem:iface.print-mem-size-fn]
> Prints an approximate memory footprint of `net` to stdout with no newline, then fflush(stdout).
> Size s (unsigned int) = Σ over sigma entries with number != -1 of (strlen(symbol) + 1 +
> sizeof(struct sigma)) + sizeof(struct fsm) + sizeof(struct fsm_state) * net->linecount. Format:
> s < 1024 → "%i bytes. "; 1024 ≤ s < 1048576 → "%.1f kB. " (s/1024.0); < 1073741824 → "%.1f MB. ";
> else "%.1f GB. ". Note trailing space and period, no newline. Net unchanged.

> [spec:foma:def:iface.print-net-fn]
> static int print_net(struct fsm *net, char *filename)

> [spec:foma:sem:iface.print-net-fn]
> Human-readable dump of `net`. Output target: filename == NULL → stdout; else fopen "w" (on failure
> prints "Error writing to file %s. Using stdout.\n" and falls back to stdout) and then
> unconditionally prints "Writing network to file %s.\n" to stdout (even after the fallback).
> Steps: fsm_count(net); build finals[] indexed by state_no; while scanning, if any transition has
> in != out set net->arity = 2 (mutates the net; never resets to 1). Then: print_sigma(net->sigma,
> out); "Net: %s\n" (name); "Flags: " followed by any of "deterministic " "pruned " "minimized "
> "epsilon_free " "loop_free " "arcs_sorted_in " "arcs_sorted_out " (in that order, each only if the
> corresponding property is set; deterministic/pruned/minimized/epsilon_free require == YES) then
> "\n"; "Arity: %i\n". Then per transition line (states in array order): on the first line of a new
> state print "S" if start state, "f" if final, then "s%i:\t(no arcs).\n" and skip if in == -1, else
> "s%i:\t". Each arc: if in == out — IDENTITY → "@ -> ", UNKNOWN → "?:? -> ", else "<sym> -> " via
> sigptr; if in != out → "<%s:%s> -> " (sigptr each side). Then "f" if the target is final, "s%i"
> (target), and ", " if the next array entry has the same state_no else ".\n". fclose only when
> filename non-NULL; free finals; return 0.

> [spec:foma:def:iface.print-sigma-fn]
> static int print_sigma(struct sigma *sigma, FILE *out)

> [spec:foma:sem:iface.print-sigma-fn]
> Writes "Sigma:" then walks the sigma list in order: entries with number > 2 print " <symbol>" and
> increment a size counter; number == IDENTITY (2) prints " @"; number == UNKNOWN (1) prints " ?";
> EPSILON (0) prints nothing. Then "\n" and "Size: %i.\n" where the size counts ONLY the regular
> (number > 2) symbols, excluding @ and ?. Returns 1.

> [spec:foma:def:iface.print-stats-fn]
> int print_stats(struct fsm *net)

> [spec:foma:sem:iface.print-stats-fn]
> One-line size summary printed to stdout (used by stack_add when g_verbose, and by print defined).
> Calls print_mem_size(net) first (no newline), then "1 state, " or "%i states, ", then "1 arc, " or
> "%i arcs, ", then pathcount: 1 → "1 path"; -1 (PATHCOUNT_CYCLIC) → "Cyclic"; -2
> (PATHCOUNT_OVERFLOW) → "more than %lld paths" with LLONG_MAX; -3 (PATHCOUNT_UNKNOWN) → "unknown
> number of paths"; otherwise "%lld paths"; finally ".\n". Reads the cached statecount/arccount/
> pathcount fields (does NOT call fsm_count). Returns 0.

> [spec:foma:def:iface.sigptr-fn]
> static char *sigptr(struct sigma *sigma, int number)

> [spec:foma:sem:iface.sigptr-fn]
> Maps a symbol number to a display string for print_net/print_dot. Special numbers first: EPSILON
> (0) → "0", UNKNOWN (1) → "?", IDENTITY (2) → "@". Otherwise linear-scans the sigma list for a
> matching number and returns: "\"0\"" if the symbol text is "0", "\"?\"" if it is "?", "\\n" (two
> characters, backslash-n) for a literal newline symbol, "\\r" for carriage return, else the sigma's
> own symbol pointer (not copied). If the number is absent from sigma, returns a freshly malloc'd
> (40-byte, leaked) string "NONE(%i)" with the number.

> [spec:foma:def:iface.view-net-fn]
> static int view_net(struct fsm *net)

> [spec:foma:sem:iface.view-net-fn]
> Displays `net` graphically: generates a temp name with tempnam(NULL, "foma"), appends ".dot",
> strdups it, and writes the net there via print_dot. Makes a second tempnam-based name for the PNG,
> then runs via system(): on macOS (__APPLE__) "dot -Tpng <dotfile> > <png>.png " then
> "/usr/bin/open <png>.png 2>/dev/null &"; elsewhere "dot -Tpng <dotfile> > <png> " then
> "/usr/bin/xdg-open <png> 2>/dev/null &". If either system() call returns -1 prints "Error writing
> tempfile.\n" or "Error opening viewer.\n" respectively (command exit status otherwise ignored).
> Frees the two name strings (temp files are never deleted), returns 1. Requires graphviz `dot` on
> PATH; commands are built with sprintf into a 255-byte buffer (no bounds checking).


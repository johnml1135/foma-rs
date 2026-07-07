# foma/foma.c, foma/foma.h

> [spec:foma:def:foma.add-history-fn]
> extern int add_history (const char *)

> [spec:foma:sem:foma.add-history-fn]
> Not defined anywhere in the foma tree: this is an extern prototype (in foma.c) for GNU
> readline's history function, resolved at link time from libreadline. It appends a copy of the
> given NUL-terminated line to readline's interactive history list (enabling arrow-up recall);
> its int return value is ignored by foma.
> Sole call site is rl_gets(): invoked only when the global use_readline == 1 and the line just
> read is non-NULL and non-empty (first byte != '\0'). A port needs an equivalent line-history
> facility, not a reimplementation of this symbol.

> [spec:foma:def:foma.add-quantifier-fn]
> void add_quantifier (char *string)

> [spec:foma:sem:foma.add-quantifier-fn]
> Implemented in foma/structures.c. Appends a node to the global singly-linked quantifier list
> headed by the file-static pointer `quantifiers` (node type: struct defined_quantifiers
> { char *name; struct defined_quantifiers *next; }).
> If the head is NULL, mallocs a node and makes it the head; otherwise walks to the last node
> (the one with next == NULL) and appends a newly malloc'd node after it.
> The new node gets name = strdup(string) (the list owns the copy; the caller keeps ownership
> of its argument) and next = NULL. No duplicate check is made: adding the same name twice
> yields two nodes. Called from the regex lexer when a logical quantifier variable is scanned.

> [spec:foma:def:foma.clear-quantifiers-fn]
> void clear_quantifiers()

> [spec:foma:sem:foma.clear-quantifiers-fn]
> Implemented in foma/structures.c. Resets the global quantifier list by assigning NULL to the
> file-static head pointer `quantifiers`. It does NOT free the nodes or their strdup'd name
> strings, so every previously added quantifier leaks (latent leak — document, do not depend
> on the memory surviving). Called at the start of each regular-expression parse so each
> expression gets a fresh quantifier scope.

> [spec:foma:def:foma.count-quantifiers-fn]
> int count_quantifiers()

> [spec:foma:sem:foma.count-quantifiers-fn]
> Implemented in foma/structures.c. Walks the global `quantifiers` singly-linked list from the
> head, incrementing a counter for each node, and returns the total node count. Returns 0 when
> the list is empty (head == NULL). No state is modified.

> [spec:foma:def:foma.find-quantifier-fn]
> char *find_quantifier(char *string)

> [spec:foma:sem:foma.find-quantifier-fn]
> Implemented in foma/structures.c. Linear scan of the global `quantifiers` list from the head;
> returns the stored `name` pointer of the first node whose name is strcmp-equal to `string`,
> or NULL if no node matches. The returned pointer aliases list-owned memory: callers must not
> free or mutate it; it serves as a found/not-found test and a stable reference to the name.

> [spec:foma:def:foma.fsm-options]
> struct _fsm_options {
>   _Bool skip_word_boundary_marker;
> }

> [spec:foma:def:foma.iface-ambiguous-upper-fn]
> void iface_ambiguous_upper(void)

> [spec:foma:sem:foma.iface-ambiguous-upper-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack via iface_stack_check(1);
> on failure that check prints "Not enough networks on stack. Operation requires at least 1."
> and the command is a no-op.
> Otherwise pops the top FSM (ownership transferred, the popped net is consumed) and pushes
> fsm_extract_ambiguous_domain(net): the automaton of input-side words that have more than one
> transduction path. stack_add prints size stats when g_verbose is set.

> [spec:foma:def:foma.iface-apply-down-fn]
> void iface_apply_down(char *word)

> [spec:foma:sem:foma.iface-apply-down-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack (iface_stack_check(1) prints
> its "Not enough networks..." diagnostic and the function returns otherwise).
> Fetches the top stack entry's cached apply handle via stack_get_ah() (lazily apply_init'd
> once per entry and reused thereafter), then syncs print-space/print-pairs/show-flags/
> obey-flags from the globals via iface_apply_set_params.
> Calls apply_down(ah, word) to start enumerating outputs of `word` on the lower side. If the
> first result is NULL, prints "???\n" and returns. Otherwise prints the result plus newline,
> then loops at most g_list_limit (global, default 100) continuation calls apply_down(ah, NULL),
> printing each result until one returns NULL. Total output is capped at g_list_limit + 1
> lines. Result strings are owned by the apply handle and are not freed here.

> [spec:foma:def:foma.iface-apply-file-fn]
> int iface_apply_file(char *infilename, char *outfilename, int direction)

> [spec:foma:sem:foma.iface-apply-file-fn]
> Implemented in foma/iface.c. Batch-applies every line of infilename through the top network.
> direction must be AP_D (1, apply down) or AP_U (2, apply up); any other value perror()s
> "Invalid direction in iface_apply_file().\n" and returns 1.
> If the stack is empty, iface_stack_check(1) prints its diagnostic and the function returns 0
> (note: the success code). fopen(infilename, "r") failure prints "<infilename>: " to stderr,
> perror("Error opening file"), returns 1. outfilename == NULL selects stdout; otherwise
> fopen(outfilename, "w"), with "Writing output to file %s.\n" printed to stdout BEFORE the
> NULL check; open failure prints "<outfilename>: " to stderr, perror(...), returns 1.
> Gets the cached apply handle (stack_get_ah) and syncs params (iface_apply_set_params). For
> each input line read by fgets into an 8192-byte buffer (LINE_LIMIT): strips one trailing
> '\n' if present (latent bug: a line whose first byte is NUL gives strlen() == 0 and writes
> inword[-1], out of bounds); writes "\n<word>\n" to the output; applies the word in the given
> direction. A NULL first result writes "???\n" and continues with the next line; otherwise
> the result line is written, then apply_*(ah, NULL) continuations are written in an unbounded
> loop until NULL (no g_list_limit cap here).
> Closes the output file only when outfilename was given; the input FILE* is never fclosed
> (latent leak). Returns 0 on completion.

> [spec:foma:def:foma.iface-apply-med-fn]
> void iface_apply_med(char *word)

> [spec:foma:sem:foma.iface-apply-med-fn]
> Implemented in foma/iface.c. Requires >= 1 network (stack-check diagnostic + return
> otherwise). Gets the top entry's cached minimum-edit-distance handle via stack_get_med_ah()
> (lazily apply_med_init'd once per entry, with align symbol "-").
> Reconfigures the handle on every call: heap max 4194305 (written as 4194304+1), match limit
> g_med_limit (default 3), cost cutoff g_med_cutoff (default 15). Does NOT sync the
> show-flags/obey-flags/print apply params.
> First match: apply_med(amedh, word). NULL means no match: prints "???\n" and returns.
> Otherwise prints a three-line block per match: the matched/aligned output string, then
> apply_med_get_instring(amedh), then "Cost[f]: %i\n\n" with apply_med_get_cost(amedh).
> Continues with apply_med(amedh, NULL) until NULL, printing the same block per match; the
> match count is bounded internally by the med-limit/med-cutoff settings, not by this loop.

> [spec:foma:def:foma.iface-apply-random-fn]
> void iface_apply_random(char *(*applyer)(struct apply_handle *h), int limit)

> [spec:foma:sem:foma.iface-apply-random-fn]
> Implemented in foma/iface.c. Shared driver for the `random upper/lower/words` commands;
> `applyer` is one of apply_random_upper/apply_random_lower/apply_random_words. A limit of -1
> is replaced by g_list_random_limit (default 15).
> Requires >= 1 network on the stack. Allocates a zeroed (calloc) table of `limit` slots of
> { char *string; int count; }. Gets the cached apply handle via stack_get_ah() and syncs
> params via iface_apply_set_params.
> Calls applyer(ah) exactly `limit` times. Each non-NULL result is deduplicated by a linear
> scan of the table: if a slot's string strcmp-equals the result its count is incremented,
> else the first empty (NULL-string) slot receives strdup(result) with count = 1. NULL results
> are skipped but still consume an iteration, so fewer than `limit` distinct strings can
> appear.
> Finally prints each occupied slot in table order as "[<count>] <string>\n", frees each
> strdup'd string and then the table, and calls apply_reset_enumerator(ah).

> [spec:foma:def:foma.iface-apply-set-params-fn]
> void iface_apply_set_params(struct apply_handle *h)

> [spec:foma:sem:foma.iface-apply-set-params-fn]
> Implemented in foma/iface.c. Copies four interface globals into the given apply handle, in
> this order: apply_set_print_space(h, g_print_space), apply_set_print_pairs(h, g_print_pairs),
> apply_set_show_flags(h, g_show_flags), apply_set_obey_flags(h, g_obey_flags).
> No return value; no other state read or written. Called before each interactive apply/word
> enumeration so the handle reflects the current `set` variables.

> [spec:foma:def:foma.iface-apply-up-fn]
> void iface_apply_up(char *word)

> [spec:foma:sem:foma.iface-apply-up-fn]
> Implemented in foma/iface.c. Exact mirror of iface_apply_down but analyzes on the upper
> side: requires >= 1 network (stack-check diagnostic otherwise); gets the cached apply handle
> via stack_get_ah(); syncs params via iface_apply_set_params; calls apply_up(ah, word).
> NULL first result prints "???\n" and returns; otherwise prints the result plus newline, then
> up to g_list_limit (default 100) continuations apply_up(ah, NULL), printing each until NULL.
> At most g_list_limit + 1 lines are printed; result strings remain owned by the handle.

> [spec:foma:def:foma.iface-apropos-fn]
> void iface_apropos(char *s)

> [spec:foma:sem:foma.iface-apropos-fn]
> Implemented in foma/iface.c. Searches the static global_help table (entries of
> { name, help, longhelp }, terminated by a NULL name) for entries whose name OR short help
> contains `s` as a case-sensitive byte substring (strstr on both fields).
> Two passes: the first computes maxlen = maximum utf8strlen(name) over matching entries only
> (utf8strlen counts UTF-8 code points, not bytes). The second prints each matching entry as:
> the name, then (maxlen - utf8strlen(name) + 1) spaces (the padding loop runs while the
> counter >= 0), then the short help and a newline. Prints nothing when no entry matches;
> longhelp is never shown here.

> [spec:foma:def:foma.iface-close-fn]
> void iface_close(void)

> [spec:foma:sem:foma.iface-close-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack (iface_stack_check(1)
> diagnostic and no-op otherwise). Pops the top FSM and pushes
> fsm_topsort(fsm_minimize(fsm_close_sigma(net, 0))): "close sigma" with mode argument 0
> removes the unknown/identity symbols from the network's alphabet, and the result is
> minimized and topologically sorted before being pushed. The popped net is consumed.

> [spec:foma:def:foma.iface-compact-fn]
> void iface_compact(void)

> [spec:foma:sem:foma.iface-compact-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Operates in place on the
> top FSM first: fsm_compact(top) removes redundant symbols from its sigma, then
> sigma_sort(top) re-sorts the alphabet. Then pops that same network and pushes
> fsm_topsort(fsm_minimize(net)). Net stack effect: the top network is replaced by its
> compacted, minimized, topsorted equivalent.

> [spec:foma:def:foma.iface-complete-fn]
> void iface_complete(void)

> [spec:foma:sem:foma.iface-complete-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Pops the top FSM and pushes
> fsm_complete(net), which completes the automaton (total transition function, adding a sink
> state as needed). No fsm_minimize or fsm_topsort wrapper is applied.

> [spec:foma:def:foma.iface-compose-fn]
> void iface_compose(void)

> [spec:foma:sem:foma.iface-compose-fn]
> Implemented in foma/iface.c. Requires >= 2 networks (iface_stack_check(2) prints "Not enough
> networks on stack. Operation requires at least 2." and no-ops otherwise).
> Folds the ENTIRE stack: while stack_size() > 1, pops one = former top, then two = the next
> network, and pushes fsm_topsort(fsm_minimize(fsm_compose(one, two))) — the popped top is the
> first (upper) operand of the composition. Repeats until a single composed, minimized,
> topsorted network remains; all intermediate nets are consumed.

> [spec:foma:def:foma.iface-conc-fn]
> void iface_conc(void)

> [spec:foma:sem:foma.iface-conc-fn+1]
> Implemented in foma/iface.c. Requires >= 2 networks (stack-check diagnostic otherwise).
> Folds the entire stack: while stack_size() > 1, pops one = former top and two = next, and pushes
> fsm_topsort(fsm_minimize(fsm_concat(one, two))) with the popped top as the LEFT/first concatenand.
> Ends with one concatenated network on the stack. The C emitted a stray debug string "dd"
> (no newline) to stdout once per iteration — a leftover latent bug, now deleted.

> [spec:foma:def:foma.iface-crossproduct-fn]
> void iface_crossproduct(void)

> [spec:foma:sem:foma.iface-crossproduct-fn]
> Implemented in foma/iface.c. Requires >= 2 networks. Performs exactly ONE step (no fold,
> unlike compose/concatenate/intersect): one = pop (former top), two = pop (next), push
> fsm_topsort(fsm_minimize(fsm_cross_product(one, two))) with the popped top as the first
> operand. Any networks below the top two are left untouched.

> [spec:foma:def:foma.iface-determinize-fn]
> void iface_determinize(void)

> [spec:foma:sem:foma.iface-determinize-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Pops the top FSM and pushes
> fsm_determinize(net). No minimization or topological sort is applied.

> [spec:foma:def:foma.iface-eliminate-flag-fn]
> void iface_eliminate_flag(char *name)

> [spec:foma:sem:foma.iface-eliminate-flag-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Pops the top FSM and pushes
> flag_eliminate(net, name): compiles out only the flag-diacritic feature whose name equals
> `name`, enforcing its constraints structurally in the automaton and removing those flag
> symbols. Passing a non-NULL name distinguishes this from iface_eliminate_flags.

> [spec:foma:def:foma.iface-eliminate-flags-fn]
> void iface_eliminate_flags(void)

> [spec:foma:sem:foma.iface-eliminate-flags-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Pops the top FSM and pushes
> flag_eliminate(net, NULL); the NULL feature name means eliminate ALL flag diacritics,
> compiling their constraints into the automaton structure.

> [spec:foma:def:foma.iface-extract-ambiguous-fn]
> void iface_extract_ambiguous(void)

> [spec:foma:sem:foma.iface-extract-ambiguous-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Pops the top FSM and pushes
> fsm_extract_ambiguous(net): the transducer restricted to those paths whose input word has
> more than one transduction (the ambiguous paths). The popped net is consumed.

> [spec:foma:def:foma.iface-extract-number-fn]
> int iface_extract_number(char *s)

> [spec:foma:sem:foma.iface-extract-number-fn+1]
> Implemented in foma/iface.c. Scans s byte-by-byte from index 0 until it reaches either the
> NUL terminator or a byte in '0'..'9' (each byte compared as unsigned char), then returns
> atoi() of the suffix starting at that position — or at an immediately preceding '-' (see fix).
> Returns 0 when the string contains no digit (atoi of the empty suffix); conversion stops at the
> first non-digit after the digits per atoi; overflow behavior is atoi's (undefined). A '-'
> immediately before the first digit is included, so "abc-5" yields -5. The C scan skipped a
> leading '-' (a non-digit), so "abc-5" yielded 5.

> [spec:foma:def:foma.iface-extract-unambiguous-fn]
> void iface_extract_unambiguous(void)

> [spec:foma:sem:foma.iface-extract-unambiguous-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Pops the top FSM and pushes
> fsm_extract_unambiguous(net): the transducer restricted to paths whose input word has
> exactly one transduction (the unambiguous paths). The popped net is consumed.

> [spec:foma:def:foma.iface-factorize-fn]
> void iface_factorize(void)

> [spec:foma:sem:foma.iface-factorize-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Pops the top FSM and pushes
> fsm_bimachine(net): the bimachine factorization of the transducer. No minimize/topsort
> wrapper.

> [spec:foma:def:foma.iface-help-fn]
> void iface_help(void)

> [spec:foma:sem:foma.iface-help-fn]
> Implemented in foma/iface.c. Prints the ENTIRE static global_help table ({ name, help,
> longhelp } entries, NULL-name terminated; roughly 180 entries covering commands, variables,
> and regex operators).
> First pass computes maxlen = maximum utf8strlen(name) across all entries (UTF-8 code points,
> not bytes). Second pass prints each entry in table order: the name, then
> (maxlen - utf8strlen(name) + 1) spaces (padding loop counts from the difference down to and
> including 0), then the short help text and a newline. The longhelp field is not printed.

> [spec:foma:def:foma.iface-help-search-fn]
> void iface_help_search(char *s)

> [spec:foma:sem:foma.iface-help-search-fn]
> Implemented in foma/iface.c. For each global_help entry whose name or short help contains
> substring `s` (strstr, case-sensitive, either field), prints "##\n" followed by
> printf("%-32.32s%s\n%s\n", name, help, longhelp): the name is left-justified and truncated
> to exactly 32 BYTES (byte-based, may split a multibyte UTF-8 sequence), immediately followed
> by the short help, a newline, the long help, and a newline. Prints nothing if no entry
> matches.

> [spec:foma:def:foma.iface-ignore-fn]
> void iface_ignore(void)

> [spec:foma:sem:foma.iface-ignore-fn]
> Implemented in foma/iface.c. Requires >= 2 networks. Single step (no fold): one = pop
> (former top), two = pop (next), push
> fsm_topsort(fsm_minimize(fsm_ignore(one, two, OP_IGNORE_ALL))) — the ignore operation with
> the popped top as the base language (first operand) and the next network as the interspersed
> language, mode OP_IGNORE_ALL. Networks below the top two are untouched.

> [spec:foma:def:foma.iface-intersect-fn]
> void iface_intersect(void)

> [spec:foma:sem:foma.iface-intersect-fn]
> Implemented in foma/iface.c. Requires >= 2 networks. Folds the entire stack: while
> stack_size() > 1, pushes fsm_topsort(fsm_minimize(fsm_intersect(stack_pop(), stack_pop()))).
> Both pops appear as arguments of one call, so their evaluation order is unspecified C
> behavior; intersection is commutative so the result is unaffected, but a port must not
> assume which network becomes which operand. Ends with a single intersected network.

> [spec:foma:def:foma.iface-invert-fn]
> void iface_invert(void)

> [spec:foma:sem:foma.iface-invert-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Pops the top FSM and pushes
> fsm_invert(net), which swaps the input and output labels of every arc (transducer
> inversion). No minimize/topsort wrapper.

> [spec:foma:def:foma.iface-label-net-fn]
> void iface_label_net(void)

> [spec:foma:sem:foma.iface-label-net-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Pops the top FSM and pushes
> fsm_sigma_pairs_net(net): a network accepting exactly the symbol pairs (arc labels) actually
> attested on the arcs of the input, each as a one-transition path. The popped net is
> consumed. Companion of `sigma net`, which uses single symbols rather than pairs.

> [spec:foma:def:foma.iface-letter-machine-fn]
> void iface_letter_machine(void)

> [spec:foma:sem:foma.iface-letter-machine-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Pops the top FSM and pushes
> fsm_topsort(fsm_minimize(fsm_letter_machine(net))): converts the network to a letter
> machine (multi-character symbols split into sequences of single UTF-8 letters), then
> minimizes and topologically sorts before pushing.

> [spec:foma:def:foma.iface-load-defined-fn]
> void iface_load_defined(char *filename)

> [spec:foma:sem:foma.iface-load-defined-fn]
> Implemented in foma/iface.c as a thin delegate: calls load_defined(g_defines, filename)
> (io.c) and discards its 0/1 return. That routine prints "Loading definitions from %s.\n",
> gz-reads the whole file into memory (on failure prints "File error.\n" to stderr and returns
> 0 having loaded nothing), then repeatedly reads (network, name) pairs with io_net_read and
> calls add_defined(g_defines, net, name) for each until EOF. add_defined replaces (destroying
> the old net of) any existing definition with the same name, and silently rejects names
> longer than FSM_NAME_LEN (40). The FSM stack is not touched, and no error status reaches
> the caller.

> [spec:foma:def:foma.iface-load-stack-fn]
> void iface_load_stack(char *filename)

> [spec:foma:sem:foma.iface-load-stack-fn]
> Implemented in foma/iface.c. Opens the saved-stack file via
> fsm_read_binary_file_multiple_init(filename); if that returns NULL, prints "<filename>: "
> to stderr followed by perror("File error") and returns without changing the stack.
> Otherwise loops fsm_read_binary_file_multiple(handle), stack_add()ing every network in file
> order — so the LAST network in the file ends up on top of the stack. The loop stops at the
> first NULL (EOF), at which point the reader has already freed the handle. Each stack_add
> prints size stats when g_verbose is set. Networks are owned by the stack after pushing.

> [spec:foma:def:foma.iface-lower-side-fn]
> void iface_lower_side(void)

> [spec:foma:sem:foma.iface-lower-side-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Pops the top FSM and pushes
> fsm_topsort(fsm_minimize(fsm_lower(net))): the lower-side (output) projection of the
> transducer, minimized and topologically sorted.

> [spec:foma:def:foma.iface-lower-words-fn]
> void iface_lower_words(int limit)

> [spec:foma:sem:foma.iface-lower-words-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack (the check is performed
> twice — once up front with early return, once redundantly guarding the body). A limit of -1
> is replaced by g_list_limit (default 100).
> Gets the cached apply handle via stack_get_ah(), syncs params via iface_apply_set_params,
> then calls apply_lower_words(ah) at most `limit` times, printing each returned word plus a
> newline and stopping early on NULL (enumeration exhausted). A limit <= 0 prints nothing.
> Finally calls apply_reset_enumerator(ah) so a subsequent enumeration restarts from the
> first word. Result strings remain owned by the apply handle.

> [spec:foma:def:foma.iface-minimize-fn]
> void iface_minimize(void)

> [spec:foma:sem:foma.iface-minimize-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Saves the current value of
> the global g_minimal, forces g_minimal = 1 so that fsm_minimize really minimizes even when
> the user has `set minimal OFF`, pushes fsm_topsort(fsm_minimize(stack_pop())), then restores
> the saved g_minimal value. Minimization algorithm choice (Hopcroft vs. Brzozowski) still
> follows g_minimize_hopcroft inside fsm_minimize.

> [spec:foma:def:foma.iface-name-net-fn]
> void iface_name_net(char *name)

> [spec:foma:sem:foma.iface-name-net-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Copies `name` into the top
> FSM's fixed-size name field with strncpy(dest, name, 40) (FSM_NAME_LEN = 40): shorter names
> are NUL-padded to 40 bytes; names of 40 or more bytes fill the field WITHOUT a terminating
> NUL (latent bug — later prints of the name can read past the field). Then calls
> iface_print_name(), which prints the top network's name plus a newline. The stack itself is
> unchanged; the FSM is modified in place.

> [spec:foma:def:foma.iface-negate-fn]
> void iface_negate(void)

> [spec:foma:sem:foma.iface-negate-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Pops the top FSM and pushes
> fsm_topsort(fsm_minimize(fsm_complement(net))): the complement (Sigma* minus L), minimized
> and topologically sorted.

> [spec:foma:def:foma.iface-one-plus-fn]
> void iface_one_plus(void)

> [spec:foma:sem:foma.iface-one-plus-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Pops the top FSM and pushes
> fsm_topsort(fsm_minimize(fsm_kleene_plus(net))): the Kleene plus of the network, minimized
> and topologically sorted.

> [spec:foma:def:foma.iface-pairs-file-fn]
> void iface_pairs_file(char *filename)

> [spec:foma:sem:foma.iface-pairs-file-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. If the top FSM's pathcount
> equals PATHCOUNT_CYCLIC (-1), prints "FSM is cyclic: can't write all pairs to file.\n" and
> returns. Prints "Writing to %s.\n", then fopen(filename, "w"); on failure perror("Error
> opening file") and return.
> Gets the cached apply handle; sets show-flags and obey-flags from the globals (print-space
> and print-pairs are NOT synced), and temporarily reprograms the handle's word formatting:
> space symbol "\001", epsilon "\002", input:output separator "\003".
> Loops apply_words(ah) until NULL with NO count limit (every path is written). Each result is
> split by iface_split_result into freshly calloc'd upper and lower strings: upper is the
> result with \001/\002 bytes deleted and every span from a \003 up to the next \001 dropped;
> lower is the same filter applied to the byte-reversed result and reversed back. Writes
> "<upper>\t<lower>\n" to the file, then frees both strings. (The split buffers are
> strlen(result) bytes each — a result needing no filtering at all overflows by one byte;
> latent bug inherited from iface_split_result.)
> Finally restores space " ", epsilon "0", separator ":", calls apply_reset_enumerator(ah),
> and fcloses the file.

> [spec:foma:def:foma.iface-pairs-fn]
> void iface_pairs(int limit)

> [spec:foma:sem:foma.iface-pairs-fn]
> Implemented in foma/iface.c as iface_pairs_call(limit, 0). That driver: a limit of -1 is
> replaced by g_list_limit (default 100); requires >= 1 network on the stack; configures the
> cached apply handle with show-flags/obey-flags from the globals and the temporary markers
> space "\001", epsilon "\002", separator "\003".
> Loops at most `limit` times calling apply_words(ah) (random = 0 selects sequential
> enumeration; iface_random_pairs passes 1 for apply_random_words), stopping early on NULL.
> Each result is split via iface_split_result into upper/lower halves and printed to stdout as
> "<upper>\t<lower>\n"; both halves are then freed.
> Afterwards restores the handle's markers (space " ", epsilon "0", separator ":") and calls
> apply_reset_enumerator(ah). Unlike iface_pairs_file there is no cyclicity guard: a cyclic
> FSM is simply truncated at `limit` pairs.

> [spec:foma:def:foma.iface-pop-fn]
> void iface_pop(void)

> [spec:foma:sem:foma.iface-pop-fn]
> Implemented in foma/iface.c. If stack_size() < 1, prints "Stack is empty.\n" (its own check
> and message, not the iface_stack_check one). Otherwise pops the top network and immediately
> destroys it with fsm_destroy(), freeing all its memory. Nothing is printed on success.

> [spec:foma:def:foma.iface-print-cmatrix-att-fn]
> void iface_print_cmatrix_att(char *filename)

> [spec:foma:sem:foma.iface-print-cmatrix-att-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. If the top FSM has no
> medlookup handle or its medlookup->confusion_matrix is NULL, prints "No confusion matrix
> defined.\n" and stops.
> Otherwise: filename == NULL selects stdout; else fopen(filename, "w") followed by printing
> "Writing confusion matrix to file '%s'.\n". The fopen result is NOT checked for NULL
> (latent bug: an unwritable path passes a NULL FILE* to the printer and crashes).
> Calls cmatrix_print_att(topfsm, outfile), emitting the confusion matrix as AT&T-format
> transducer lines. The opened file is never fclosed (latent leak; output may stay unflushed
> until process exit).

> [spec:foma:def:foma.iface-print-cmatrix-fn]
> void iface_print_cmatrix(void)

> [spec:foma:sem:foma.iface-print-cmatrix-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. If the top FSM's medlookup
> handle or its medlookup->confusion_matrix is NULL, prints "No confusion matrix defined.\n"
> and stops. Otherwise calls cmatrix_print(topfsm), rendering the matrix on stdout as a table
> (maxsigma = sigma_max(sigma)+1, cm indexed cm[i*maxsigma+j]):
> header row = lsymbol+2 spaces (lsymbol = longest symbol name with sigma number >= 3), then
> "0 ", then each symbol string for consecutive numbers 3,4,... until sigma_string returns
> NULL, each followed by a space.
> Then one row per row-index i from 0 to maxsigma-1, skipping i = 1 and 2 (UNKNOWN/IDENTITY):
> the j == 0 column prints the row label right-justified in lsymbol+1 chars ("0" for i == 0)
> plus the cm[i][0] count right-justified in width 2 ("*" instead for i == 0), then columns
> 1 and 2 are skipped; each remaining column j prints "*" on the diagonal (i == j), else the
> count zero-padded via printf("%.*d", strlen(symbol_j)+1, count). Row 0 holds insertion
> costs, column 0 deletion costs, cell (i,j) substitution cost of i by j. The net is neither
> modified nor popped.

> [spec:foma:def:foma.iface-print-defined-fn]
> void iface_print_defined(void)

> [spec:foma:sem:foma.iface-print-defined-fn+1]
> Implemented in foma/iface.c. If the global define list g_defines is NULL, prints "No defined
> symbols.\n" (both loops below are then no-ops). Walks g_defines in list order: every entry
> with a non-NULL name prints "<name>\t" followed by print_stats(entry->net) — the same
> "<memsize> N states, N arcs, N paths.\n" line as `print stats` (see
> foma.iface-print-stats-fn). Then walks the defined-functions list g_defines_f: every entry
> with a non-NULL name prints "<fname>@<numargs>\t" (format "%s@%i\t") followed by the stored
> regex string and "\n". Read-only; nothing is popped or modified. The C format was
> "%s@%i)\t" with a stray unmatched closing paren before the TAB — the ')' is now dropped.

> [spec:foma:def:foma.iface-print-dot-fn]
> void iface_print_dot(char *filename)

> [spec:foma:sem:foma.iface-print-dot-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. If filename != NULL first
> prints "Writing dot file to %s.\n". Renders the top net (not popped) as GraphViz dot via
> static print_dot: fsm_count(net) refreshes counts; a finals[statecount] array is filled from
> the state table. Output FILE* is fopen(filename,"w") or stdout when filename is NULL; the
> fopen result is unchecked (latent bug: unwritable path -> NULL FILE* crash).
> Emits "digraph A {\nrankdir = LR;\n", then per state 0..statecount-1 one line
> "node [shape=doublecircle,style=filled] %i\n" for finals, shape=circle otherwise.
> Arcs are merged per (source,target) pair: scanning state-table lines in order, each line
> with target != -1 not yet marked printed opens an edge "%i -> %i [label=\""; every later
> line of the same source state sharing that target is folded into the label and marked. A
> label item is the bare symbol when in == out and out != UNKNOWN, else "<in:out>"; names come
> from sigptr (EPSILON=0 -> "0", UNKNOWN=1 -> "?", IDENTITY=2 -> "@"; sigma symbols "0"/"?"
> print quoted as "\"0\""/"\"?\""; "\n"/"\r" print as "\\n"/"\\r"; a number missing from sigma
> yields a leaked 40-byte malloc'd "NONE(%i)"), each escaped for '"' via escape_string. Items
> are space-separated; once the accumulated item length exceeds 12 a literal "\n" escape is
> emitted and the counter resets. Each edge ends "\"];\n"; output ends "}\n". The printed[]
> bookkeeping array is calloc'd with sizeof(short *) per element instead of sizeof(short)
> (harmless over-allocation). File is fclosed only when filename != NULL.

> [spec:foma:def:foma.iface-print-name-fn]
> void iface_print_name(void)

> [spec:foma:sem:foma.iface-print-name-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Prints the top network's
> name field (a 40-char buffer, FSM_NAME_LEN) followed by "\n" to stdout. The net is not
> popped or modified.

> [spec:foma:def:foma.iface-print-net-fn]
> void iface_print_net(char *netname, char *filename)

> [spec:foma:sem:foma.iface-print-net-fn]
> Implemented in foma/iface.c. If netname != NULL, looks it up in g_defines (find_defined,
> exact strcmp); when absent prints "No defined network %s.\n" to stderr only if g_verbose
> (silent otherwise) and returns; when found pretty-prints that defined net (which stays owned
> by the define list). If netname == NULL, requires >= 1 network on the stack and prints the
> top net (not popped).
> Pretty-printing (static print_net): out = fopen(filename,"w"), falling back to stdout when
> filename == NULL or fopen fails (failure prints "Error writing to file %s. Using stdout.\n");
> a non-NULL filename also prints "Writing network to file %s.\n" to stdout. Calls fsm_count,
> builds finals[statecount] from the state table, and while scanning sets net->arity = 2 if
> any line has in != out — a persistent side effect; arity is never lowered back to 1.
> Output: the sigma block via print_sigma (same format as foma.iface-print-sigma-fn);
> "Net: %s\n" (name); "Flags: " plus any of "deterministic ", "pruned ", "minimized ",
> "epsilon_free ", "loop_free ", "arcs_sorted_in ", "arcs_sorted_out " that are set, then
> "\n"; "Arity: %i\n". Then per state, prefix "S" if start state and "f" if final: an arcless
> state (in == -1) prints "s%i:\t(no arcs).\n"; otherwise "s%i:\t" followed by its arcs
> separated by ", " and terminated ".\n". Each arc prints "LABEL -> " then "f" if the target
> state is final then "s%i" (target). LABEL is "@" for IDENTITY:IDENTITY, "?:?" for
> UNKNOWN:UNKNOWN, the sigptr symbol when in == out, else "<in:out>" via sigptr (mappings and
> NONE(%i) leak as described in foma.iface-print-dot-fn). Frees finals, fcloses real files,
> returns 0.

> [spec:foma:def:foma.iface-print-shortest-string-fn]
> void iface_print_shortest_string()

> [spec:foma:sem:foma.iface-print-shortest-string-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack; works on a copy, the top
> net is untouched. For an automaton (arity == 1) computes the regex
> L - [?+ [[L .o. [?:"@TMP@"]*].l .o. ["@TMP@":?]*].l], i.e. fsm_minimize(fsm_minus(L,
> fsm_concat(fsm_kleene_plus(fsm_identity()), <all strings whose length equals some string
> length of L>))) — subtracting every string strictly longer than some member leaves exactly
> the length-minimal strings. Then apply_init on the result and one apply_words() call: a
> non-NULL word prints "%s\n"; an empty result prints nothing at all. Only the first shortest
> string is printed even if several exist. The handle is apply_clear'd and the result
> destroyed; the initial fsm_copy leaks in this branch.
> For a transducer the same computation runs separately on the fsm_upper and fsm_lower
> projections, printing "Upper: %s\n" then "Lower: %s\n", substituting "" when a projection
> yields no word (so those lines always appear).

> [spec:foma:def:foma.iface-print-shortest-string-size-fn]
> void iface_print_shortest_string_size()

> [spec:foma:sem:foma.iface-print-shortest-string-size-fn+1]
> Implemented in foma/iface.c. Requires >= 1 network on the stack; works on a copy of the top
> net. For an automaton (arity == 1) builds Result = fsm_minimize([L .o. [?:"a"]*].l) — the
> unary "length language" { a^n | L contains a string of length n }, using the literal symbol
> "a" — and prints "Shortest acyclic path length: %i\n" with the SHORTEST accepted length,
> computed as a breadth-first arc distance from the start state to the nearest final of Result
> (0 for the empty language). For a transducer, does the same independently for the upper and
> lower projections and prints "Shortest acyclic upper path length: %i\n" then
> "Shortest acyclic lower path length: %i\n". The C source printed Result->statecount - 1, but
> the minimal unary DFA of an acyclic net is a chain of (max length)+1 states, so statecount-1 is
> the LONGEST string length whenever the language has strings of several lengths; the BFS returns
> the true shortest. All intermediate Result nets leak (never fsm_destroy'd); the stack is
> unchanged.

> [spec:foma:def:foma.iface-print-sigma-fn]
> void iface_print_sigma(void)

> [spec:foma:sem:foma.iface-print-sigma-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Prints the top net's sigma
> to stdout via static print_sigma: "Sigma:" then, walking the sigma list in stored order,
> " %s" for every symbol with number > 2, " @" for the IDENTITY (2) entry, " ?" for the
> UNKNOWN (1) entry, nothing for EPSILON (0); then "\n" and "Size: %i.\n" where the count
> includes only the number > 2 symbols (@ and ? are excluded). Non-destructive.

> [spec:foma:def:foma.iface-print-stats-fn]
> void iface_print_stats(void)

> [spec:foma:sem:foma.iface-print-stats-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack; calls print_stats on the
> top net (not popped; the stored statecount/arccount/pathcount/linecount are trusted, no
> fsm_count). First print_mem_size: s (unsigned int) = sum over sigma entries (stopping at
> NULL or an entry with number == -1) of strlen(symbol)+1+sizeof(struct sigma), plus
> sizeof(struct fsm) + sizeof(struct fsm_state)*linecount; printed as "%i bytes. " when
> s < 1024, "%.1f kB. " when < 2^20, "%.1f MB. " when < 2^30, else "%.1f GB. " (divisors
> 1024/2^20/2^30), then fflush(stdout). Then "%i states, " and "%i arcs, " with singular
> forms "1 state, "/"1 arc, "; then pathcount: 1 -> "1 path", -1 (PATHCOUNT_CYCLIC) ->
> "Cyclic", -2 -> "more than %lld paths" with LLONG_MAX, -3 -> "unknown number of paths",
> anything else "%lld paths"; terminated ".\n".

> [spec:foma:def:foma.iface-prune-fn]
> void iface_prune(void)

> [spec:foma:sem:foma.iface-prune-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Pops the top net and pushes
> fsm_topsort(fsm_coaccessible(net)): removes states from which no final state is reachable
> (plus their arcs) and renumbers the remaining states topologically. No determinization or
> minimization is performed.

> [spec:foma:def:foma.iface-quit-fn]
> void iface_quit(void)

> [spec:foma:sem:foma.iface-quit-fn]
> Implemented in foma/iface.c. Calls remove_defined(g_defines, NULL): with a NULL name that
> helper destroys every defined network (fsm_destroy) and frees every defined name, though the
> list nodes themselves are not freed (moot, the process is exiting). Then, while the stack is
> non-empty, pops each entry and fsm_destroy()s it (stack_pop also clears any cached apply/med
> handles). Finally calls exit(0); never returns.

> [spec:foma:def:foma.iface-random-lower-fn]
> void iface_random_lower(int limit)

> [spec:foma:sem:foma.iface-random-lower-fn]
> Implemented in foma/iface.c as iface_apply_random(&apply_random_lower, limit): the shared
> driver (see foma.iface-apply-random-fn) draws up to `limit` random paths (limit == -1 is
> replaced by g_list_random_limit, default 15) from the top network, keeping the LOWER
> (output) side of each, and prints the deduplicated strings with occurrence counts as
> "[<count>] <string>\n". Requires >= 1 network on the stack; the net is not modified.

> [spec:foma:def:foma.iface-random-pairs-fn]
> void iface_random_pairs(int limit)

> [spec:foma:sem:foma.iface-random-pairs-fn+1]
> Implemented in foma/iface.c. Resolves limit == -1 to g_list_random_limit (default 15), then calls
> iface_pairs_call(limit, 1): the same driver as iface_pairs (see foma.iface-pairs-fn) — temporary
> markers space "\001"/epsilon "\002"/separator "\003" installed on the cached apply handle —
> except random = 1 makes each of the up-to-`limit` iterations call apply_random_words(ah)
> (one random path per call) instead of sequential enumeration. Each result is split into
> upper/lower and printed as "<upper>\t<lower>\n"; duplicates are possible and are not
> deduplicated or counted (unlike iface_apply_random). Markers are then restored and the
> enumerator reset. The C passed limit straight through, so limit == -1 became
> g_list_limit (default 100) inside iface_pairs_call — it now uses g_list_random_limit like the
> other random commands.

> [spec:foma:def:foma.iface-random-upper-fn]
> void iface_random_upper(int limit)

> [spec:foma:sem:foma.iface-random-upper-fn]
> Implemented in foma/iface.c as iface_apply_random(&apply_random_upper, limit): identical to
> iface_random_lower (see foma.iface-random-lower-fn and the driver rule
> foma.iface-apply-random-fn) except each random path contributes its UPPER (input) side
> string. Output is the deduplicated "[<count>] <string>\n" table; limit == -1 means
> g_list_random_limit (default 15).

> [spec:foma:def:foma.iface-random-words-fn]
> void iface_random_words(int limit)

> [spec:foma:sem:foma.iface-random-words-fn]
> Implemented in foma/iface.c as iface_apply_random(&apply_random_words, limit): identical to
> iface_random_lower/upper except each sample is a whole random word/path formatted by the
> apply module (both sides, honoring the handle's print-pairs/print-space/flag settings synced
> from the globals). Deduplicated output "[<count>] <string>\n"; limit == -1 means
> g_list_random_limit (default 15).

> [spec:foma:def:foma.iface-read-att-fn]
> int iface_read_att(char *filename)

> [spec:foma:sem:foma.iface-read-att-fn]
> Implemented in foma/iface.c. Unconditionally prints "Reading AT&T file: %s\n" first, then
> parses the tab-separated AT&T transition file with read_att(filename) (io.c). If that
> returns NULL, prints "<filename>: " to stderr followed by perror("Error opening file") and
> returns 1. Otherwise pushes the resulting net onto the stack as-is (no minimize/topsort;
> stack_add itself runs fsm_count and names unnamed nets) and returns 0.

> [spec:foma:def:foma.iface-read-prolog-fn]
> int iface_read_prolog(char *filename)

> [spec:foma:sem:foma.iface-read-prolog-fn]
> Implemented in foma/iface.c. Unconditionally prints "Reading prolog: %s\n", then parses the
> file with fsm_read_prolog(filename). NULL result: prints "<filename>: " to stderr followed
> by perror("Error opening file") and returns 1. Otherwise pushes the net unchanged onto the
> stack and returns 0.

> [spec:foma:def:foma.iface-read-spaced-text-fn]
> int iface_read_spaced_text(char *filename)

> [spec:foma:sem:foma.iface-read-spaced-text-fn]
> Implemented in foma/iface.c. Calls fsm_read_spaced_text_file(filename), which builds the
> union of the file's lines with space-separated multi-character symbols (alternating
> upper/lower lines forming pairs where present). NULL result: prints "<filename>: " to
> stderr, perror("File error"), returns 1. Otherwise pushes fsm_topsort(fsm_minimize(net))
> and returns 0. No banner is printed on success.

> [spec:foma:def:foma.iface-read-text-fn]
> int iface_read_text(char *filename)

> [spec:foma:sem:foma.iface-read-text-fn]
> Implemented in foma/iface.c. Calls fsm_read_text_file(filename), building the union of the
> file's lines as words (each UTF-8 character a symbol, one word per line). NULL result:
> prints "<filename>: " to stderr, perror("File error"), returns 1. Otherwise pushes
> fsm_topsort(fsm_minimize(net)) and returns 0. No banner is printed on success.

> [spec:foma:def:foma.iface-reverse-fn]
> void iface_reverse(void)

> [spec:foma:sem:foma.iface-reverse-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Pops the top net and pushes
> fsm_topsort(fsm_determinize(fsm_reverse(net))) — the reversal, determinized and
> topologically renumbered but NOT minimized.

> [spec:foma:def:foma.iface-rotate-fn]
> void iface_rotate(void)

> [spec:foma:sem:foma.iface-rotate-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack, then calls stack_rotate().
> Despite its name, stack_rotate only SWAPS the fsm pointers of the top and bottom stack
> entries (a 3-deep stack [bottom,mid,top] becomes [top,mid,bottom]); with a single entry it
> is a no-op returning 1, and on an empty stack it would print "Stack is empty.\n" (unreached
> here because of the guard). Only the fsm fields are exchanged: any cached apply handles
> (ah/amedh) stay with their entries, so a handle created before the rotate can afterwards
> reference the wrong net (latent bug).

> [spec:foma:def:foma.iface-save-defined-fn]
> void iface_save_defined(char *filename)

> [spec:foma:sem:foma.iface-save-defined-fn]
> Implemented in foma/iface.c as save_defined(g_defines, filename) (io.c); its int result is
> discarded. If g_defines is NULL prints "No defined networks.\n" to stderr and does nothing.
> Otherwise gzopen(filename, "wb"); failure prints "Error opening file %s for writing.\n" and
> aborts. Success prints "Writing definitions to file %s.\n" and iterates the define list in
> order: entries whose net is NULL print "Skipping definition without network.\n"; for each
> real entry the define's name is strncpy'd into the net's 40-byte (FSM_NAME_LEN) name field
> (no forced NUL terminator when the name is >= 40 chars — latent bug), then the net is
> written with foma_net_print into the single gzipped foma binary stream, definitions
> concatenated back to back. gzclose finishes. The nets remain owned by g_defines.

> [spec:foma:def:foma.iface-save-stack-fn]
> void iface_save_stack(char *filename)

> [spec:foma:sem:foma.iface-save-stack-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. gzopen(filename, "wb");
> NULL -> "Error opening file %s for writing.\n" and return. Otherwise prints "Writing to
> file %s.\n" and walks from the bottom entry (stack_find_bottom) along ->next while
> entry->next != NULL — i.e. every real entry, excluding the trailing sentinel — writing each
> entry's fsm with foma_net_print into one gzipped stream. Nets are therefore written in
> bottom-to-top order, so a subsequent `load stack` (which stack_adds nets in file order)
> reproduces the original stack order. gzclose finishes; the stack is unchanged.

> [spec:foma:def:foma.iface-sequentialize-fn]
> void iface_sequentialize(void)

> [spec:foma:sem:foma.iface-sequentialize-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Pops the top net and pushes
> fsm_sequentialize(net) — the input-sequentialized (p-sequential) form of the transducer. No
> additional minimization or topsort is applied.

> [spec:foma:def:foma.iface-set-variable-fn]
> void iface_set_variable(char *name, char *value)

> [spec:foma:sem:foma.iface-set-variable-fn+1]
> Implemented in foma/iface.c. Matches variables by full name (the C source compared only the
> first 8 characters via strncmp, so an 8-char-prefix-equal name matched). Scans the static
> global_vars table in declaration order —
> flag-is-epsilon, minimal, name-nets, obey-flags, print-pairs, print-sigma, print-space,
> quit-on-fail, recursive-define, quote-special, show-flags, sort-arcs, verbose, hopcroft-min,
> compose-tristate (all FVAR_BOOL), med-limit, med-cutoff (FVAR_INT), lexc-align (FVAR_BOOL),
> att-epsilon (FVAR_STRING) — each pointing at the corresponding g_* global. Matching uses
> strncmp(name, entry_name, 8): only the first 8 characters are compared, so any input sharing
> an 8-char prefix with an entry matches it (e.g. "recursive-anything" hits recursive-define);
> the first table match wins.
> FVAR_BOOL: value "ON" or "1" -> 1, "OFF" or "0" -> 0, anything else prints "Invalid value
> '%s' for variable '%s'\n" and returns without storing; on success stores and prints
> "variable %s = ON\n" or "= OFF". FVAR_STRING: stores strdup(value) — the previous string is
> leaked — and prints "variable %s = %s\n". FVAR_INT: strtol base 10 with errno cleared;
> errno set, no digits consumed (endptr == value), or a negative result prints "invalid value
> %s for variable %s\n" (lowercase, no quotes); otherwise prints "variable %s = %i\n" and
> stores. If no entry matches, prints "*There is no global variable '%s'.\n".

> [spec:foma:def:foma.iface-show-variable-fn]
> void iface_show_variable(char *name)

> [spec:foma:sem:foma.iface-show-variable-fn+2]
> Implemented in foma/iface.c. Finds the variable using the same full-name comparison against
> global_vars as iface_set_variable (the C source compared only the first 8 characters) and prints "%s = %s\n" with
> the value formatted by the variable's declared type: FVAR_BOOL as "ON"/"OFF" (value == 1 ? ON :
> OFF), FVAR_INT as the integer value, FVAR_STRING as the string. If nothing matches, prints
> "*There is no global variable '%s'.\n". The C printed ON/OFF from *(int *)ptr == 1
> regardless of type — med-limit/med-cutoff showed ON only at value 1, and att-epsilon reinterpreted
> the leading bytes of its char* pointer as an int. Now formatted per declared type.

> [spec:foma:def:foma.iface-show-variables-fn]
> void iface_show_variables(void)

> [spec:foma:sem:foma.iface-show-variables-fn]
> Implemented in foma/iface.c. Iterates the entire global_vars table in declaration order (see
> foma.iface-set-variable-fn for the 19 entries), printing one line per variable with the name
> left-justified and truncated to 17 characters ("%-17.17s: "). FVAR_BOOL prints "ON" when the
> int is exactly 1, else "OFF"; FVAR_INT prints the value with %i; FVAR_STRING prints the
> string with %s. Read-only.

> [spec:foma:def:foma.iface-shuffle-fn]
> void iface_shuffle(void)

> [spec:foma:sem:foma.iface-shuffle-fn]
> Implemented in foma/iface.c. Requires >= 2 networks on the stack. While more than one net
> remains, pops two and pushes fsm_minimize(fsm_shuffle(top, second)) — the shuffle
> (asynchronous interleaving) product — folding the entire stack down to one net. Note: only
> minimized, no fsm_topsort (unlike most other binary stack operations).

> [spec:foma:def:foma.iface-sigma-net-fn]
> void iface_sigma_net()

> [spec:foma:sem:foma.iface-sigma-net-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Pops the top net and pushes
> fsm_sigma_net(net), which rebuilds the net in place as a two-state machine (state 0 initial,
> state 1 final) accepting exactly the single-symbol strings of its own alphabet: one arc
> 0 -> 1 for every sigma entry with number >= 3 plus IDENTITY (2); UNKNOWN (1) and EPSILON (0)
> contribute no arc. pathcount is set to the arc count, is_minimized/is_loop_free set to YES,
> and unused sigma symbols cleaned up. If the sigma is empty the input is destroyed and the
> empty language (fsm_empty_set) is pushed instead.

> [spec:foma:def:foma.iface-sort-fn]
> void iface_sort(void)

> [spec:foma:sem:foma.iface-sort-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. First calls
> sigma_sort(topfsm) in place, renumbering the alphabet so the symbols with number >= 3 are in
> canonical sorted order and rewriting all arc labels accordingly; then pops the net and
> pushes fsm_topsort(net), renumbering states topologically. The accepted language/relation
> is unchanged — only internal symbol and state numbering are normalized.

> [spec:foma:def:foma.iface-sort-input-fn]
> void iface_sort_input(void)

> [spec:foma:sem:foma.iface-sort-input-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Calls
> fsm_sort_arcs(topfsm, 1) in place: for each state whose arc block has more than one line,
> qsorts the lines by input symbol number. Afterwards the flags are set: for arity-1 nets
> both arcs_sorted_in and arcs_sorted_out become 1; otherwise arcs_sorted_in = 1 and
> arcs_sorted_out = 0. The net is neither popped nor re-pushed; nothing is printed.

> [spec:foma:def:foma.iface-sort-output-fn]
> void iface_sort_output(void)

> [spec:foma:sem:foma.iface-sort-output-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Calls
> fsm_sort_arcs(topfsm, 2) in place: per-state qsort of arc lines by output symbol number.
> Flag effect mirrors foma.iface-sort-input-fn: arity-1 nets set both sorted flags; otherwise
> arcs_sorted_out = 1 and arcs_sorted_in = 0. In-place; nothing popped or printed.

> [spec:foma:def:foma.iface-stack-check-fn]
> int iface_stack_check(int size)

> [spec:foma:sem:foma.iface-stack-check-fn]
> Implemented in foma/iface.c. Guard used by nearly every iface_* command: if stack_size() <
> size, prints "Not enough networks on stack. Operation requires at least %i.\n" and returns
> 0; otherwise returns 1. No other side effects.

> [spec:foma:def:foma.iface-substitute-defined-fn]
> void iface_substitute_defined (char *original, char *substitute)

> [spec:foma:sem:foma.iface-substitute-defined-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. dequote_string()s both
> arguments in place (if a string begins and ends with '"' the quotes are stripped and quoted
> escape sequences decoded). Looks up `substitute` in g_defines with find_defined; if absent
> prints "No defined network '%s'.\n" and stops. Otherwise if fsm_symbol_occurs(top->fsm,
> original, M_UPPER+M_LOWER) == 0 (symbol appears on neither tape) prints "Symbol '%s' does
> not occur.\n" and stops. Else builds newnet = fsm_substitute_label(top->fsm, original,
> subnet): every arc labeled original:original is replaced by an epsilon-bracketed spliced
> copy of the defined net, and every one-sided occurrence by the defined net cross-producted
> with the other tape's symbol; as a side effect fsm_merge_sigma mutates the defined network's
> sigma. Then stack_pop()s the old top and discards the pointer without fsm_destroy — the
> input net leaks, since fsm_substitute_label does not free it (latent bug) — prints
> "Substituted network '%s' for '%s'.\n" (substitute first), and pushes
> fsm_topsort(fsm_minimize(newnet)). The defined net remains owned by g_defines.

> [spec:foma:def:foma.iface-substitute-symbol-fn]
> void iface_substitute_symbol (char *original, char *substitute)

> [spec:foma:sem:foma.iface-substitute-symbol-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. dequote_string()s both
> arguments in place, then pushes fsm_topsort(fsm_minimize(fsm_substitute_symbol(stack_pop(),
> original, substitute))) and finally prints "Substituted '%s' for '%s'.\n" (substitute
> first). fsm_substitute_symbol mutates the popped net: it is a no-op (net returned unchanged)
> when original == substitute or original is not in sigma; substitute "0" denotes EPSILON (0),
> otherwise substitute is looked up in sigma and sigma_add'ed if missing; every arc whose
> in/out equals original's number is renumbered to the substitute's; original is removed from
> sigma, sigma_sort renumbers the alphabet, all is_* flags reset, unused symbols cleaned up,
> and the result is fsm_determinize(net) (minimization then happens in the wrapper).

> [spec:foma:def:foma.iface-test-equivalent-fn]
> void iface_test_equivalent(void)

> [spec:foma:sem:foma.iface-test-equivalent-fn]
> Implemented in foma/iface.c. Requires >= 2 networks on the stack. Takes fsm_copy()s of the
> top and second-from-top nets (originals untouched), fsm_count()s both copies, and prints
> iface_print_bool(fsm_equivalent(copy_of_top, copy_of_second)) — format "%i (1 = TRUE, 0 =
> FALSE)\n". fsm_equivalent merges the copies' sigmas, then runs a parallel depth-first
> traversal from state pair (0,0) requiring equal finality and, in both directions, an arc
> with identical in/out numbers for every arc — i.e. it tests structural path equivalence
> (correct when both nets are in minimized canonical form) and destroys both copies before
> returning.

> [spec:foma:def:foma.iface-test-functional-fn]
> void iface_test_functional(void)

> [spec:foma:sem:foma.iface-test-functional-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Prints
> iface_print_bool(fsm_isfunctional(top->fsm)) — "%i (1 = TRUE, 0 = FALSE)\n".
> fsm_isfunctional computes fsm_isidentity(fsm_minimize(fsm_compose(fsm_invert(fsm_copy(net)),
> fsm_copy(net)))): TRUE iff the transducer maps every input to at most one output. The top
> net is neither popped nor modified; the temporary is destroyed.

> [spec:foma:def:foma.iface-test-identity-fn]
> void iface_test_identity(void)

> [spec:foma:sem:foma.iface-test-identity-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Prints
> iface_print_bool(fsm_isidentity(top->fsm)): TRUE iff the net encodes only identity
> relations, determined by a DFS that tracks a per-state upper/lower "discrepancy" (debt)
> string and fails on any arc mismatching the pending discrepancy or on revisiting a state
> with a different discrepancy than recorded. Top net unchanged.

> [spec:foma:def:foma.iface-test-lower-universal-fn]
> void iface_test_lower_universal(void)

> [spec:foma:sem:foma.iface-test-lower-universal-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Computes tmp =
> fsm_complement(fsm_lower(fsm_copy(top->fsm))), prints iface_print_bool(fsm_isempty(tmp)) —
> TRUE iff the lower-side (output) projection equals the universal language ?* — then
> fsm_destroy(tmp). Top net unchanged.

> [spec:foma:def:foma.iface-test-nonnull-fn]
> void iface_test_nonnull(void)

> [spec:foma:sem:foma.iface-test-nonnull-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Prints
> iface_print_bool(!fsm_isempty(top->fsm)): TRUE iff the language/relation is non-empty.
> fsm_isempty minimizes a copy and reports empty iff the result is a single non-final state
> with no arcs; the copy is destroyed and the top net is unchanged.

> [spec:foma:def:foma.iface-test-null-fn]
> void iface_test_null(void)

> [spec:foma:sem:foma.iface-test-null-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Prints
> iface_print_bool(fsm_isempty(top->fsm)) — TRUE iff the net accepts nothing (fsm_isempty
> minimizes a copy; empty iff a single non-final arcless state remains). Top net unchanged.

> [spec:foma:def:foma.iface-test-sequential-fn]
> void iface_test_sequential(void)

> [spec:foma:sem:foma.iface-test-sequential-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Prints
> iface_print_bool(fsm_issequential(top->fsm)): scans the state-array lines in order (arcs of
> a state are contiguous) and per state fails if two arcs share the same input-symbol number,
> or if an EPSILON-input arc coexists with any other arc of the state (whether seen before or
> after it) — i.e. tests input-side (p-)sequentiality. Lines with in < 0 (arcless states) are
> skipped. On failure fsm_issequential additionally prints "fails at state %i\n" before the
> boolean line. Top net unchanged.

> [spec:foma:def:foma.iface-test-unambiguous-fn]
> void iface_test_unambiguous(void)

> [spec:foma:sem:foma.iface-test-unambiguous-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Prints
> iface_print_bool(fsm_isunambiguous(top->fsm)): builds loweruniq = fsm_lowerdet(fsm_copy(net))
> and tests fsm_isidentity(fsm_minimize(fsm_compose(fsm_invert(fsm_copy(loweruniq)),
> fsm_copy(loweruniq)))) — TRUE iff no input string is accepted along two distinct successful
> paths. Both temporaries are destroyed; the top net is unchanged.

> [spec:foma:def:foma.iface-test-upper-universal-fn]
> void iface_test_upper_universal(void)

> [spec:foma:sem:foma.iface-test-upper-universal-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Computes tmp =
> fsm_complement(fsm_upper(fsm_copy(top->fsm))), prints iface_print_bool(fsm_isempty(tmp)) —
> TRUE iff the upper-side (input) projection equals the universal language ?* — then
> fsm_destroy(tmp). Top net unchanged.

> [spec:foma:def:foma.iface-turn-fn]
> void iface_turn(void)

> [spec:foma:sem:foma.iface-turn-fn+1]
> Implemented in foma/iface.c. Requires >= 1 network on the stack, then calls stack_turn(),
> reversing the whole stack, as the "turn stack" command name suggests. The C source called
> stack_rotate() instead — identical to iface_rotate — swapping only the fsm pointers of the top
> and bottom stack entries rather than reversing the stack.

> [spec:foma:def:foma.iface-twosided-flags-fn]
> void iface_twosided_flags(void)

> [spec:foma:sem:foma.iface-twosided-flags-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Pops the top net and pushes
> flag_twosided(net), which enforces two-sided flag diacritics: symbols passing flag_check
> (names of the "@...@" flag-diacritic form) are marked; any arc pairing a flag with EPSILON
> on the other tape is rewritten in place to flag:flag; any arc pairing a flag with a
> different symbol is split through a fresh intermediate state so the flag rides both tapes of
> one arc and the residual symbol pair the other (order depends on which side held the flag;
> the state array is realloc'ed to hold the new arcs). If any arc changed or was split, the
> determinism/minimality flags are reset to UNK and the result is fsm_topsort(fsm_minimize(net));
> if nothing changed the net is re-pushed unmodified.

> [spec:foma:def:foma.iface-union-fn]
> void iface_union(void)

> [spec:foma:sem:foma.iface-union-fn]
> Implemented in foma/iface.c. Requires >= 2 networks on the stack. While more than one net
> remains, pops two and pushes fsm_minimize(fsm_union(top, second)), folding the entire stack
> into a single net accepting the union. Only minimized — no fsm_topsort (like shuffle, unlike
> most other binary stack operations).

> [spec:foma:def:foma.iface-upper-side-fn]
> void iface_upper_side(void)

> [spec:foma:sem:foma.iface-upper-side-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Pops the top net and pushes
> fsm_topsort(fsm_minimize(fsm_upper(net))) — the upper-side (input) projection as a
> single-tape acceptor.

> [spec:foma:def:foma.iface-upper-words-fn]
> void iface_upper_words(int limit)

> [spec:foma:sem:foma.iface-upper-words-fn]
> Implemented in foma/iface.c. limit == -1 is replaced by the global g_list_limit (default
> 100). Requires >= 1 network on the stack. Fetches the top entry's cached apply handle via
> stack_get_ah() (created with apply_init(top->fsm) on first use), copies the global print
> settings onto it (apply_set_print_space/print_pairs/show_flags/obey_flags from
> g_print_space, g_print_pairs, g_show_flags, g_obey_flags), then calls apply_upper_words(ah)
> at most limit times, printing each returned word followed by "\n" and stopping early on
> NULL; finally apply_reset_enumerator(ah). limit <= 0 prints nothing. The net is not popped;
> the handle remains cached on the stack entry.

> [spec:foma:def:foma.iface-view-fn]
> void iface_view(void)

> [spec:foma:sem:foma.iface-view-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack; calls the file-static
> view_net(top->fsm) (see foma.view-net-fn): renders the net to a temporary Graphviz DOT
> file, converts it to PNG with the external `dot` program and opens it with the system image
> viewer. Stack and net are unchanged.

> [spec:foma:def:foma.iface-warranty-fn]
> void iface_warranty(void)

> [spec:foma:sem:foma.iface-warranty-fn]
> Implemented in foma/iface.c. Prints the global warranty[] string verbatim to stdout: a
> leading newline, then "Licensed under the Apache License, Version 2.0 (the \"License\")",
> "you may not use this file except in compliance with the License.", "You may obtain a copy
> of the License at", a blank line, the URL http://www.apache.org/licenses/LICENSE-2.0
> indented four spaces, and two trailing newlines. No other effects.

> [spec:foma:def:foma.iface-words-file-fn]
> void iface_words_file(char *filename, int type)

> [spec:foma:sem:foma.iface-words-file-fn+1]
> Implemented in foma/iface.c. type selects the enumerator fresh on every call: 0 = apply_words
> (whole words / pairs), 1 = apply_upper_words, 2 = apply_lower_words. The C held the
> function pointer in a STATIC local initialized once to apply_words and only overwritten for type 1
> or 2, so a later type-0 call silently reused whatever a previous call installed — it is now a
> per-call local. Requires >= 1
> network on the stack. If top->fsm->pathcount == PATHCOUNT_CYCLIC (-1) prints "FSM is cyclic:
> can't write all words to file.\n" and returns. Prints "Writing to %s.\n" then
> fopen(filename, "w"); on NULL perror("Error opening file") and return. Fetches the cached
> apply handle (stack_get_ah), applies the global print params (iface_apply_set_params), then
> loops without limit writing each enumerated word plus "\n" to the file until the enumerator
> returns NULL; then apply_reset_enumerator(ah) and fclose. The net is not popped.

> [spec:foma:def:foma.iface-words-fn]
> void iface_words(int limit)

> [spec:foma:sem:foma.iface-words-fn]
> Implemented in foma/iface.c. Same shape as foma.iface-upper-words-fn but enumerates with
> apply_words (whole words/pairs of the relation rather than the upper projection): limit ==
> -1 becomes g_list_limit (default 100); requires >= 1 network; uses the top entry's cached
> apply handle with the global print params applied (iface_apply_set_params); prints at most
> limit results, one per line, stopping early on NULL; apply_reset_enumerator afterwards. The
> net is not popped.

> [spec:foma:def:foma.iface-write-att-fn]
> int iface_write_att(char *filename)

> [spec:foma:sem:foma.iface-write-att-fn]
> Implemented in foma/iface.c. If the stack is empty (iface_stack_check(1) fails and prints
> its usual message) returns 1. Otherwise uses the top net (not popped). filename == NULL
> writes to stdout; otherwise prints "Writing AT&T file: %s\n" and fopen(filename, "w") — on
> failure prints "<filename>: " to stderr, perror("File error opening."), and returns 1.
> Writes the net in AT&T tab-separated format via net_print_att, fclose()s only when a file
> was opened, and returns 0.

> [spec:foma:def:foma.iface-write-prolog-fn]
> void iface_write_prolog(char *filename)

> [spec:foma:sem:foma.iface-write-prolog-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack; calls
> foma_write_prolog(top->fsm, filename), which emits the net as Prolog clauses to the named
> file (stdout when filename is NULL); its int result is discarded. The net is not popped.

> [spec:foma:def:foma.iface-zero-plus-fn]
> void iface_zero_plus(void)

> [spec:foma:sem:foma.iface-zero-plus-fn]
> Implemented in foma/iface.c. Requires >= 1 network on the stack. Pops the top net and pushes
> fsm_topsort(fsm_minimize(fsm_kleene_star(net))) — the Kleene star (zero or more
> self-concatenations; always accepts the empty string).

> [spec:foma:def:foma.main-fn]
> int main(int argc, char *argv[])

> [spec:foma:sem:foma.main-fn]
> Implemented in foma/foma.c. Startup: stack_init(); srand(time(NULL)); g_defines =
> defined_networks_init(); g_defines_f = defined_functions_init(). Then a getopt loop with
> option string "e:f:hl:pqrsv", options acted on in command-line order: -e ARG runs
> my_interfaceparse(ARG) immediately; -f FILE reads the whole file into memory (file_to_mem),
> and if non-NULL sets the global input_is_file = 1 and my_interfaceparse()s the contents,
> then exit(0) (it also exits 0 when the file could not be read); -h calls print_help() and
> exit(0); -l FILE is like -f but frees the buffer and continues instead of exiting; -p sets
> pipe_mode = 1; -q sets g_verbose = 0; -r sets use_readline = 0; -s exit(0) immediately; -v
> prints "argv[0] MAJOR.MINOR.BUILD STATUS\n" (e.g. "foma 0.10.0alpha") and exit(0); any
> unknown option prints the usage string to stderr and exit(EXIT_FAILURE). After options:
> unless pipe_mode or !g_verbose, prints the multi-line version/copyright disclaimer banner.
> Readline is configured with rl_basic_word_break_characters = " >" and
> rl_attempted_completion_function = my_completion. The REPL then loops forever: the prompt is
> "foma[N]: " (N = stack_size()) when promptmode == PROMPT_MAIN, or "apply down> " /
> "apply up> " / "apply med> " when promptmode == PROMPT_A per apply_direction
> (AP_D=1/AP_U=2/AP_M=3), emptied to "" in pipe or quiet mode; fflush(stdout); read a line via
> rl_gets(prompt). EOF (NULL) at the main prompt prints "\n" and exit(0); EOF at an apply
> prompt prints "\n", resets promptmode to PROMPT_MAIN and continues. Otherwise clears
> input_is_file to 0 and hands the line to my_interfaceparse(). Never returns normally.

> [spec:foma:def:foma.my-completion-fn]
> static char **my_completion(const char *text, int start, int end)

> [spec:foma:sem:foma.my-completion-fn]
> Implemented in foma/foma.c (file-static). readline attempted-completion hook: stores `start`
> (the index in rl_line_buffer where the word being completed begins) into the file-static
> global smatch — consumed by my_generator — then returns rl_completion_matches(text,
> my_generator). Returns NULL when nothing matches, letting readline fall back to its default
> (filename) completion.

> [spec:foma:def:foma.my-generator-fn]
> char *my_generator(const char *text, int state)

> [spec:foma:sem:foma.my-generator-fn]
> Implemented in foma/foma.c. readline match generator, called repeatedly with state == 0 on
> the first call. Ignores the passed text and matches against the ENTIRE line (text is
> reassigned to rl_line_buffer) so multi-word commands complete correctly. On state == 0 it
> resets its static cursors (list_index, list_index2, nummatches = 0) and caches len =
> strlen(line). It then resumes scanning the cmd[] table (the full command names, "ambiguous
> upper" ... "zero-plus net", NULL-terminated) and returns, for each entry of which the line
> is a prefix (strncmp(name, line, len) == 0), strdup(name + smatch) — the candidate with its
> first smatch characters dropped, because readline substitutes only the word starting at
> column smatch (see foma.my-completion-fn). When cmd[] is exhausted and rl_point > 0 it scans
> the abbrvcmd[] abbreviation table the same way. Returns NULL when both lists are exhausted,
> ending enumeration. nummatches is incremented for cmd[] hits but never read. readline frees
> the returned strings.

> [spec:foma:def:foma.my-yyparse-fn]
> extern int my_yyparse(char *my_string)

> [spec:foma:sem:foma.my-yyparse-fn]
> foma/foma.c declares `extern int my_yyparse(char *my_string)` but never calls it, and this
> 1-argument declaration does not match the real definition in foma/regex.l: int
> my_yyparse(char *my_string, int lineno, struct defined_networks *, struct
> defined_functions *) — the regex-grammar entry point. That implementation packs the two
> defined-tables into a struct defs, creates a reentrant flex scanner with yylex_init_extra,
> points it at my_string via yy_scan_string and sets the scanner's line number to lineno. If
> the global g_parse_depth > 0 (re-entrant parse, e.g. a recursive define): when depth >=
> MAX_PARSE_DEPTH (100) it prints "Exceeded parser stack depth.  Self-recursive call?\n" to
> stderr and returns 1 (leaking the scanner and buffer); otherwise the parser globals rewrite,
> rule_direction, contexts, rules and rewrite_rules are saved into
> parservarstack[g_parse_depth]. It increments g_parse_depth, runs yyp = yyparse(scanner,
> defined_nets, defined_funcs), decrements, and restores the saved globals when the depth is
> still > 0. Finally deletes the scan buffer, destroys the scanner and returns yyparse's
> status: 0 = success, with the parsed net left in the global current_parse.

> [spec:foma:def:foma.print-help-fn]
> void print_help()

> [spec:foma:sem:foma.print-help-fn]
> Implemented in foma/foma.c. Prints to stdout the usage string ("Usage: foma [-e \"command\"]
> [-f run-once-script] [-l startupscript] [-p] [-q] [-s] [-v]\n"), then "Options:\n" followed
> by one tab-aligned line per option: -e "command" (execute a command on startup, repeatable),
> -f scriptfile (read commands from scriptfile on startup, and quit), -l scriptfile (read
> commands from scriptfile on startup), -p (pipe-mode), -q (quiet mode, more quiet than
> pipe-mode), -r (don't use readline library for input), -s (stop execution and exit), -v
> (print version number). Pure output; no other effects.

> [spec:foma:def:foma.print-stats-fn]
> int print_stats(struct fsm *net)

> [spec:foma:sem:foma.print-stats-fn]
> Implemented in foma/iface.c. First print_mem_size(net): estimates memory as the sum over
> sigma entries of strlen(symbol)+1+sizeof(struct sigma), plus sizeof(struct fsm) +
> sizeof(struct fsm_state) * net->linecount, printed as "%i bytes. " under 1024, else
> "%.1f kB. ", "%.1f MB. " or "%.1f GB. " (1024-based thresholds), flushed to stdout. Then
> prints "%i states, " ("1 state, " for exactly 1), "%i arcs, " ("1 arc, "), and the path
> count: 1 -> "1 path"; PATHCOUNT_CYCLIC (-1) -> "Cyclic"; PATHCOUNT_OVERFLOW (-2) ->
> "more than <LLONG_MAX> paths"; PATHCOUNT_UNKNOWN (-3) -> "unknown number of paths";
> otherwise "%lld paths"; then ".\n". Reads the cached statecount/arccount/pathcount fields —
> no recount — and always returns 0.

> [spec:foma:def:foma.purge-quantifier-fn]
> void purge_quantifier (char *string)

> [spec:foma:sem:foma.purge-quantifier-fn+1]
> Implemented in foma/structures.c. Walks the global singly-linked `quantifiers` list and unlinks
> EVERY node whose name strcmp-equals string; removed nodes and their names are dropped (the C
> leaked them). The C trailing pointer advanced onto a node it had just unlinked, so
> with two ADJACENT same-name nodes the second was spliced out of the dead node rather than the
> live list and survived; this removes all matching nodes, adjacent or not. No output; safe on an
> empty list.

> [spec:foma:def:foma.rl-gets-fn]
> char *rl_gets(char *prompt)

> [spec:foma:sem:foma.rl-gets-fn]
> Implemented in foma/foma.c. Line reader for the REPL; returns a pointer the caller must NOT
> free, or NULL on EOF. When the file-static use_readline is 1 (default; cleared by the -r
> option): frees the previously returned line (kept in static line_read), reads with
> readline(prompt) (which displays the prompt itself), and if the new line is non-NULL and
> non-empty adds it to the readline history via add_history. When use_readline is 0: prints
> the prompt with printf, fgets up to 510 characters into the static 512-byte buffer
> no_readline_line (returns NULL at EOF), and strip_newline() replaces the first '\n' with
> NUL. Returns line_read.

> [spec:foma:def:foma.stack-add-fn]
> int stack_add(struct fsm *fsm)

> [spec:foma:sem:foma.stack-add-fn]
> Implemented in foma/stack.c. Pushes fsm as the new top of the global main_stack list, taking
> ownership of it. First fsm_count(fsm) refreshes statecount/linecount/arccount/finalcount
> (crashes if fsm is NULL). If the net's name is "" it is assigned a random one:
> sprintf(fsm->name, "%X", rand()). Then walks from main_stack to the trailing sentinel entry
> (number == -1) counting the real entries into i; the sentinel itself becomes the new top
> (fsm stored, ah = amedh = NULL, number = i = former stack size, previous linked to the old
> top) and a fresh sentinel {number -1, fsm NULL, next NULL, previous = new entry} is malloc'ed
> and appended. If g_verbose, print_stats(fsm) echoes the size line. Returns the assigned
> entry number (0-based depth from the bottom).

> [spec:foma:def:foma.stack-clear-fn]
> int stack_clear()

> [spec:foma:sem:foma.stack-clear-fn]
> Implemented in foma/stack.c. Destroys every real entry from the bottom up: for each entry
> whose next != NULL it apply_clear()s / apply_med_clear()s any cached handles, advances
> main_stack to the next entry, fsm_destroy()s the entry's fsm (NULL-safe) and free()s the
> entry. Finally frees the trailing sentinel and re-creates an empty stack via stack_init(),
> returning its result (always 1).

> [spec:foma:def:foma.stack-entry]
> struct stack_entry {
>   int number;
>   struct apply_handle *ah;
>   struct apply_med_handle *amedh;
>   struct fsm *fsm;
>   struct stack_entry *next;
>   struct stack_entry *previous;
> }

> [spec:foma:def:foma.stack-find-bottom-fn]
> struct stack_entry *stack_find_bottom()

> [spec:foma:sem:foma.stack-find-bottom-fn]
> Implemented in foma/stack.c. Returns NULL when the stack is empty (main_stack->number == -1,
> i.e. main_stack is just the sentinel); otherwise returns main_stack itself — the BOTTOM
> entry, from which ->next chains toward the top and ends at the sentinel.

> [spec:foma:def:foma.stack-find-second-fn]
> struct stack_entry *stack_find_second()

> [spec:foma:sem:foma.stack-find-second-fn]
> Implemented in foma/stack.c. Walks from main_stack until the next entry is the sentinel
> (next->number == -1), landing on the top entry, and returns its ->previous: the
> second-from-top entry, or NULL when the stack holds exactly one entry (the bottom's previous
> is NULL). The empty-stack guard is commented out, so on an empty stack main_stack->next is
> NULL and the walk dereferences it — undefined behavior/crash (latent bug); callers must
> ensure stack_size() >= 1 first.

> [spec:foma:def:foma.stack-find-top-fn]
> struct stack_entry *stack_find_top()

> [spec:foma:sem:foma.stack-find-top-fn]
> Implemented in foma/stack.c. Returns NULL when the stack is empty (main_stack->number ==
> -1); otherwise walks ->next from main_stack until the following entry is the sentinel and
> returns that entry — the top of the stack (most recently pushed net; note the top is the
> entry FARTHEST from main_stack, which is the bottom).

> [spec:foma:def:foma.stack-get-ah-fn]
> struct apply_handle *stack_get_ah()

> [spec:foma:sem:foma.stack-get-ah-fn]
> Implemented in foma/stack.c. Returns NULL if the stack is empty (stack_find_top() == NULL).
> Otherwise returns the top entry's cached apply handle, creating it on first use with
> apply_init(top->fsm) and storing it in the entry's ah field. The handle stays cached until
> the entry is popped or the stack cleared, whereupon apply_clear frees it; callers never free
> it themselves.

> [spec:foma:def:foma.stack-get-med-ah-fn]
> struct apply_med_handle *stack_get_med_ah()

> [spec:foma:sem:foma.stack-get-med-ah-fn]
> Implemented in foma/stack.c. Returns NULL if the stack is empty. Otherwise returns the top
> entry's cached minimum-edit-distance apply handle, creating it on first use with
> apply_med_init(top->fsm) followed by apply_med_set_align_symbol(handle, "-"), stored in the
> entry's amedh field. Freed by apply_med_clear when the entry is popped or the stack cleared;
> callers never free it.

> [spec:foma:def:foma.stack-init-fn]
> int stack_init()

> [spec:foma:sem:foma.stack-init-fn]
> Implemented in foma/stack.c. mallocs a single sentinel stack_entry {number = -1, fsm = NULL,
> next = NULL, previous = NULL} and assigns it to the global main_stack, representing the
> empty stack (ah/amedh are left uninitialized in the sentinel and never read there). Returns
> 1. Does not free any existing stack — calling it over a live stack leaks the old list (only
> stack_clear calls it safely).

> [spec:foma:def:foma.stack-isempty-fn]
> int stack_isempty()

> [spec:foma:sem:foma.stack-isempty-fn]
> Implemented in foma/stack.c. Returns 1 iff main_stack->next == NULL (only the sentinel
> exists), else 0.

> [spec:foma:def:foma.stack-pop-fn]
> struct fsm *stack_pop()

> [spec:foma:sem:foma.stack-pop-fn]
> Implemented in foma/stack.c. Removes and returns the top net; ownership transfers to the
> caller. Size-1 fast path: grabs main_stack->fsm, NULLs the field, calls stack_clear() —
> which apply_clear()s/apply_med_clear()s the entry's cached handles, frees both entries and
> reinitializes an empty stack; the saved fsm survives because the field was NULLed first and
> fsm_destroy(NULL) is a no-op — and returns the fsm. General path (size > 1): walks to the
> top entry (the one whose next is the sentinel), unlinks it from the doubly-linked list,
> apply_clear()s/apply_med_clear()s its cached handles (so handles previously obtained via
> stack_get_ah/stack_get_med_ah become dangling), frees the entry and returns its fsm. Called
> on an empty stack it dereferences main_stack->next == NULL and crashes (latent bug); callers
> guard with stack_size()/iface_stack_check.

> [spec:foma:def:foma.stack-print-fn]
> int stack_print()

> [spec:foma:sem:foma.stack-print-fn]
> Implemented in foma/stack.c. Stub: does nothing and returns 1 unconditionally.

> [spec:foma:def:foma.stack-rotate-fn]
> int stack_rotate()

> [spec:foma:sem:foma.stack-rotate-fn+1]
> Implemented in foma/stack.c. On an empty stack prints "Stack is empty.\n" and returns -1;
> with exactly one entry returns 1 doing nothing. Otherwise swaps the fsm pointers of the bottom
> entry (main_stack) and the top entry — together with their cached ah/amedh apply/med handles —
> and returns 1; intermediate entries keep their nets and both affected entries keep their numbers.
> The C source swapped only the fsm pointers, so cached handles afterwards belonged to the other
> entry's original net (latent bug). Despite the name it
> performs a top/bottom swap, not a rotation.

> [spec:foma:def:foma.stack-size-fn]
> int stack_size()

> [spec:foma:sem:foma.stack-size-fn]
> Implemented in foma/stack.c. Walks ->next from main_stack counting entries whose next is
> non-NULL — i.e. the number of real entries, excluding the trailing sentinel — and returns
> that count (0 for the empty stack). O(n) on every call.

> [spec:foma:def:foma.stack-turn-fn]
> int stack_turn()

> [spec:foma:sem:foma.stack-turn-fn+1]
> Implemented in foma/stack.c. Reverses the entire stack in place. Empty stack: prints
> "Stack is empty.\n" and returns 0. Exactly one entry: returns 1, no change. Otherwise it
> reverses the order of the real entries so the former top becomes the new bottom
> (main_stack) and the former bottom becomes the new top, relinking every ->next/->previous
> pointer and leaving the sentinel at the tail; entries keep their own fsm/ah/amedh/number
> (numbers are not renumbered). Returns 1. The C code's final previous-pointer
> fixup loop `for (stack_ptr = main_stack; stack_ptr->number != -1;) {
> (stack_ptr->next)->previous = stack_ptr; }` never advanced stack_ptr, so with >= 2 entries
> the function looped forever (dead code: the "turn stack" command goes through iface_turn,
> which calls stack_rotate). The evident intent — a genuine terminating reversal — is
> implemented.

> [spec:foma:def:foma.union-quantifiers-fn]
> struct fsm *union_quantifiers()

> [spec:foma:sem:foma.union-quantifiers-fn+1]
> Implemented in foma/structures.c. Builds and returns a fresh fsm (caller owns it) from the
> global `quantifiers` list: fsm_create("") with flags set deterministic/pruned/minimized/
> epsilon-free = YES and loop-free/completed = NO via fsm_update_flags. Each quantifier name
> is sigma_add'ed to the net's sigma (numbers assigned consecutively from the first free
> number >= 3; the first assigned number is remembered as symlo, and the count as syms). The
> state array is malloc'ed with syms+1 lines: for each i in 0..syms-1 one line for state 0 —
> marked both initial and final — with in = out = symlo+i and target 0, i.e. a one-state
> machine where every quantifier symbol labels a SELF-LOOP (so it actually accepts any
> sequence of quantifier symbols including the empty string, despite the comment claiming a
> plain union of single symbols); then the -1 sentinel line. Sets arccount = syms and
> statecount = finalcount = 1. Linecount = syms+1, INCLUDING the sentinel line per
> fsm_count's convention (was: syms, excluding it); every caller recounts via fsm_count before
> reading linecount, so no downstream value changed. With no quantifiers defined the state array
> holds only the sentinel line (a machine with no states, linecount 1) while statecount/finalcount
> still claim 1.

> [spec:foma:def:foma.view-net-fn]
> int view_net(struct fsm *net)

> [spec:foma:sem:foma.view-net-fn]
> foma/foma.c declares a non-static `int view_net(struct fsm *net)` but never calls it; the
> only definition is file-STATIC in foma/iface.c, so the external symbol never exists and any
> other translation unit calling this prototype would fail to link. The iface.c implementation
> (reached via iface_view): takes tempnam(NULL, "foma"), strncpy()s at most 250 characters of
> it into a 255-byte buffer, appends ".dot" and strdups the result as the DOT filename;
> print_dot(net, dotname) writes a Graphviz digraph (rankdir=LR; doublecircle nodes for final
> states, circle otherwise; one edge per source/target state pair whose label collects all its
> arc labels, "sym" for two-sided arcs and "<in:out>" otherwise, wrapped roughly every 12
> characters). Then pngname = strdup(tempnam(NULL, "foma")) and two system() calls: on macOS
> "dot -Tpng <dot> > <png>.png " then "/usr/bin/open <png>.png 2>/dev/null &"; elsewhere
> "dot -Tpng <dot> > <png> " then "/usr/bin/xdg-open <png> 2>/dev/null &". A -1 return from
> system() prints "Error writing tempfile.\n" or "Error opening viewer.\n" respectively.
> Frees pngname and dotname (the temp files themselves are never deleted) and returns 1.
> Requires the Graphviz `dot` binary on PATH.

> [spec:foma:def:foma.xprintf-fn]
> void xprintf(char *string)

> [spec:foma:sem:foma.xprintf-fn]
> Implemented in foma/foma.c as `{ return ; printf("%s",string); }`: the function returns
> immediately and the printf is unreachable dead code — effectively a no-op that discards its
> argument (a disabled debug/output hook).


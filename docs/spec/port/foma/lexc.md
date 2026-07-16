# foma/lexc.h

> [spec:foma:def:lexc.lexc-add-mc-fn]
> void lexc_add_mc(char *symbol)

> [spec:foma:sem:lexc.lexc-add-mc-fn]
> Prototype exported to the lexc lexer (foma/lexc.l), which calls it once per whitespace-delimited token in a `Multichar_Symbols` section. De-escapes '%'-escapes, then registers the symbol in the longest-first multichar list, in lexsigma, in the sigma hashtable, and in the two-byte-prefix filter. Implemented in foma/lexcread.c; full behavior: `[spec:foma:sem:lexcread.lexc-add-mc-fn]`.

> [spec:foma:def:lexc.lexc-add-word-fn]
> void lexc_add_word()

> [spec:foma:sem:lexc.lexc-add-word-fn]
> Prototype exported to the lexc lexer, called when an entry's terminating `Target ;` (optionally followed by a quoted info string, which the lexer consumes and discards) has been matched, after `lexc_set_current_lexicon(target, 1)`. Commits the pending entry as a path from the current source lexicon state to the target state — word entries extend the shared-prefix trie, `< regex >` entries splice the parsed network. Implemented in foma/lexcread.c; full behavior: `[spec:foma:sem:lexcread.lexc-add-word-fn]`. The lexer follows every call with `[spec:foma:sem:lexc.lexc-clear-current-word-fn]` and increments its entry counter (printing `%i...` to stdout every 10000 entries).

> [spec:foma:def:lexc.lexc-clear-current-word-fn]
> void lexc_clear_current_word()

> [spec:foma:sem:lexc.lexc-clear-current-word-fn]
> Prototype exported to the lexc lexer, called after every committed entry. Resets the current word to a single EPSILON:EPSILON pair and current_entry to WORD_ENTRY, so an entry with no word part yields an epsilon transition. Implemented in foma/lexcread.c; full behavior: `[spec:foma:sem:lexcread.lexc-clear-current-word-fn]`.

> [spec:foma:def:lexc.lexc-find-lex-state-fn]
> struct states *lexc_find_lex_state(char *name)

> [spec:foma:sem:lexc.lexc-find-lex-state-fn]
> Not ported to Rust: dead API — declared in lexc.h but never called anywhere in the C tree. The C behaviour was:
> Looks up a lexicon by name in the internal lexicon list and returns its state, or NULL. Declared in lexc.h but never called anywhere in the C tree (dead API). Implemented in foma/lexcread.c; full behavior: `[spec:foma:sem:lexcread.lexc-find-lex-state-fn]`.

> [spec:foma:def:lexc.lexc-find-mc-fn]
> int lexc_find_mc(char *symbol)

> [spec:foma:sem:lexc.lexc-find-mc-fn]
> Returns 1 if `symbol` is already a registered multichar symbol, else 0 (linear strcmp scan). Declared in lexc.h; its only caller is `[spec:foma:sem:lexcread.lexc-add-mc-fn]` itself. Implemented in foma/lexcread.c; full behavior: `[spec:foma:sem:lexcread.lexc-find-mc-fn]`.

> [spec:foma:def:lexc.lexc-init-fn]
> void lexc_init()

> [spec:foma:sem:lexc.lexc-init-fn]
> Prototype exported to the lexc lexer driver fsm_lexc_parse_string (`[spec:foma:sem:fomalib.fsm-lexc-parse-string-fn]`), which calls it once before scanning. Resets all file-static lexc compiler state: fresh sigma, empty multichar/lexicon/state lists, cleared current word, 3079-bucket sigma hashtable, 65536-entry multichar prefix filter. Implemented in foma/lexcread.c; full behavior: `[spec:foma:sem:lexcread.lexc-init-fn]`.

> [spec:foma:def:lexc.lexc-set-current-lexicon-fn]
> void lexc_set_current_lexicon(char *name, int which)

> [spec:foma:sem:lexc.lexc-set-current-lexicon-fn]
> Prototype exported to the lexc lexer, which calls it with which=SOURCE_LEXICON (0) for each trimmed `LEXICON Name` / `Lexicon Name` header (the lexer also prints `Name...` plus the previous lexicon's entry count to stdout) and which=TARGET_LEXICON (1) for each entry's trimmed continuation class just before `[spec:foma:sem:lexc.lexc-add-word-fn]`. Selects — creating on first mention — the lexicon named `name` as current source (clexicon, marking has_outgoing) or target (ctarget). Implemented in foma/lexcread.c; full behavior: `[spec:foma:sem:lexcread.lexc-set-current-lexicon-fn]`.

> [spec:foma:def:lexc.lexc-set-current-word-fn]
> void lexc_set_current_word(char *name)

> [spec:foma:sem:lexc.lexc-set-current-word-fn]
> Prototype exported to the lexc lexer, called on the word token of a lexicon entry (a run of non-reserved/%-escaped characters). Splits at an unescaped ':' into input:output sides, resolves '%'-escapes and '0'-as-epsilon markers, tokenizes both sides against multichar symbols and sigma, and pads/aligns them to equal length into the current-word buffers. Implemented in foma/lexcread.c; full behavior: `[spec:foma:sem:lexcread.lexc-set-current-word-fn]`.

> [spec:foma:def:lexc.lexc-set-network-fn]
> void lexc_set_network(struct fsm *net)

> [spec:foma:sem:lexc.lexc-set-network-fn]
> Prototype exported to the lexc lexer: after collecting a `< ... >` regex entry (the lexer rewrites the closing '>' to ';' and parses the text with my_yyparse against the current defined networks), it passes the parsed net here. Stores it as the pending regex network and flips the entry mode to REGEX_ENTRY so the following `[spec:foma:sem:lexc.lexc-add-word-fn]` splices it in. Implemented in foma/lexcread.c; full behavior: `[spec:foma:sem:lexcread.lexc-set-network-fn]`.

> [spec:foma:def:lexc.lexc-to-fsm-fn]
> struct fsm *lexc_to_fsm(void)

> [spec:foma:sem:lexc.lexc-to-fsm-fn]
> Prototype exported to the lexc lexer driver fsm_lexc_parse_string, which calls it (unconditionally, even after a syntax error) once scanning finishes. Merges suffixes, numbers states, emits warnings ("no Root lexicon", "defined but not used", "used but never defined", "# is never reached"), converts the graph to a struct fsm sharing the accumulated sigma, frees the intermediate structures, and returns fsm_topsort(fsm_minimize(fsm_determinize(net))) — or fsm_empty_set() when "#" is unreachable. Implemented in foma/lexcread.c; full behavior: `[spec:foma:sem:lexcread.lexc-to-fsm-fn]`.

> [spec:foma:def:lexc.lexc-trim-fn]
> void lexc_trim(char *s)

> [spec:foma:sem:lexc.lexc-trim-fn]
> Implemented in foma/lexc.l (not lexcread.c); also used by the interface lexer. In-place trim of matched lexer text (`LEXICON Name` tails, `name =` definition heads, `Target ;` continuations). Two phases: (1) starting at the last byte (index strlen(s)-1) and moving backward, overwrite every trailing ';', '=', ' ' or '\t' with '\0', stopping at the first byte that is none of these; (2) skip leading ' ', '\t' and '\n' bytes, then shift the remaining bytes — including the final copied NUL terminator — to the front of the buffer.
> Phase 1 is bounded at index 0 in the port. The C loop had no lower bound, so an empty or all-trimmable string underran the buffer (reading and NUL-writing bytes before its start until a non-trimmable byte happened to appear — UB); the lexer's patterns always supply at least one non-trimmable character so this was not hit in practice, and safe Rust simply stops the backward scan at index 0.


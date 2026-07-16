# foma/fomalibconf.h

> [spec:foma:def:fomalibconf.add-fsm-arc-fn]
> int add_fsm_arc(struct fsm_state *fsm, int offset, int state_no, int in, int out, int target, int final_state, int start_state)

> [spec:foma:sem:fomalibconf.add-fsm-arc-fn]
> Writes one transition line into a pre-allocated `fsm_state` array and returns the next free index. Sets the six fields of `fsm[offset]` from the arguments in order — `state_no`, `in`, `out`, `target`, `final_state`, `start_state` — then returns `offset+1`.
> No bounds check and no allocation: the caller must guarantee room at `offset`. The `int` arguments `in`/`out` are truncated into `short int` struct fields and `final_state`/`start_state` into `char` fields.
> Used both for real arcs and for dummy/sentinel lines (`in`/`out`/`target` = -1; the array-terminating sentinel additionally has `state_no` = -1). Implementation: foma/constructions.c.

> [spec:foma:def:fomalibconf.apply-handle]
> struct apply_handle {
>   int ptr;
>   int curr_ptr;
>   int ipos;
>   int opos;
>   int mode;
>   int printcount;
>   int *numlines;
>   int *statemap;
>   int *marks;
>   struct sigma_trie { int signum; struct sigma_trie *next; } *sigma_trie;
>   struct sigmatch_array { int signumber ; int consumes ; } *sigmatch_array;
>   struct sigma_trie_arrays { struct sigma_trie *arr; struct sigma_trie_arrays *next; } *sigma_trie_arrays;
>   int binsearch;
>   int indexed;
>   int state_has_index;
>   int sigma_size;
>   int sigmatch_array_size;
>   int current_instring_length;
>   int has_flags;
>   int obey_flags;
>   int show_flags;
>   int print_space;
>   char *space_symbol;
>   char *separator;
>   char *epsilon_symbol;
>   int print_pairs;
>   int apply_stack_ptr;
>   int apply_stack_top;
>   int oldflagneg;
>   int outstringtop;
>   int iterate_old;
>   int iterator;
>   uint8_t *flagstates;
>   char *outstring;
>   char *instring;
>   struct sigs { char *symbol; int length; } *sigs;
>   char *oldflagvalue;
>   struct fsm *last_net;
>   struct fsm_state *gstates;
>   struct sigma *gsigma;
>   struct apply_state_index { int fsmptr; struct apply_state_index *next; } **index_in, **index_out, *iptr;
>   struct flag_list { char *name; char *value; short neg; struct flag_list *next; } *flag_list;
>   struct flag_lookup { int type; char *name; char *value; } *flag_lookup;
>   struct searchstack { int offset; struct apply_state_index *iptr; int state_has_index; int opos; int ipos; int visitmark; char *flagname; char *flagvalue; int...;
> }

> [spec:foma:def:fomalibconf.apply-handle.apply-state-index]
> struct apply_state_index {
>   int fsmptr;
>   struct apply_state_index *next;
> }

> [spec:foma:def:fomalibconf.apply-handle.flag-list]
> struct flag_list {
>   char *name;
>   char *value;
>   short neg;
>   struct flag_list *next;
> }

> [spec:foma:def:fomalibconf.apply-handle.flag-lookup]
> struct flag_lookup {
>   int type;
>   char *name;
>   char *value;
> }

> [spec:foma:def:fomalibconf.apply-handle.searchstack]
> struct searchstack {
>   int offset;
>   struct apply_state_index *iptr;
>   int state_has_index;
>   int opos;
>   int ipos;
>   int visitmark;
>   char *flagname;
>   char *flagvalue;
>   int flagneg;
> }

> [spec:foma:def:fomalibconf.apply-handle.sigma-trie]
> struct sigma_trie {
>   int signum;
>   struct sigma_trie *next;
> }

> [spec:foma:def:fomalibconf.apply-handle.sigma-trie-arrays]
> struct sigma_trie_arrays {
>   struct sigma_trie *arr;
>   struct sigma_trie_arrays *next;
> }

> [spec:foma:def:fomalibconf.apply-handle.sigmatch-array]
> struct sigmatch_array {
>   int signumber;
>   int consumes;
> }

> [spec:foma:def:fomalibconf.apply-handle.sigs]
> struct sigs {
>   char *symbol;
>   int length;
> }

> [spec:foma:def:fomalibconf.apply-med-handle]
> struct apply_med_handle {
>   struct astarnode { short int wordpos; int fsmstate; short int f; short int g; short int h; int in; int out; int parent; } *agenda;
>   int bytes_per_letter_array;
>   uint8_t *letterbits;
>   uint8_t *nletterbits;
>   int astarcount;
>   int heapcount;
>   int heap_size;
>   int agenda_size;
>   int maxdepth;
>   int maxsigma;
>   int wordlen;
>   int utf8len;
>   int cost;
>   int nummatches;
>   int curr_state;
>   int curr_g;
>   int curr_pos;
>   int lines;
>   int curr_agenda_offset;
>   int curr_node_has_match;
>   int med_limit;
>   int med_cutoff;
>   int med_max_heap_size;
>   int nodes_expanded;
>   int *cm;
>   char *word;
>   char *instring;
>   int instring_length;
>   char *outstring;
>   int outstring_length;
>   char *align_symbol;
>   int *heap;
>   int *intword;
>   struct sh_handle *sigmahash;
>   struct state_array *state_array;
>   struct fsm *net;
>   struct fsm_state *curr_ptr;
>   _Bool hascm;
> }

> [spec:foma:def:fomalibconf.apply-med-handle.astarnode]
> struct astarnode {
>   short int wordpos;
>   int fsmstate;
>   short int f;
>   short int g;
>   short int h;
>   int in;
>   int out;
>   int parent;
> }

> [spec:foma:def:fomalibconf.decode-quoted-fn]
> void decode_quoted(char *s)

> [spec:foma:sem:fomalibconf.decode-quoted-fn+1]
> In-place decoder of `\uXXXX` escapes in string `s`. Let `len = strlen(s)`. Scan with read index `i` and write index `j`, both from 0, while `i < len`:
> - If `s[i]` is backslash (0x5C), `len-i > 5`, `s[i+1]` is lowercase 'u' (0x75), and the following 4 bytes are hex digits (per `ishexstr`): parse those 4 hex digits as a 16-bit code point (first two digits = high byte) and UTF-8-encode it via `utf8code16tostr`; copy the resulting 1–3 UTF-8 bytes to `s[j..]` (advancing `j` per byte); advance `i` by 6.
> - Otherwise copy one whole UTF-8 character verbatim: `utf8skip(s+i)+1` bytes, advancing `i` and `j` together. A malformed lead byte gives `utf8skip == -1`, so this width would be 0 and the C looped forever; the port forces at least one byte to be copied (lossy), so decoding terminates.
> After the loop the string is truncated at `j`. A 6-byte escape always shrinks to at most 3 bytes, so `j <= i` throughout and in-place compaction is safe. Only lowercase `u` introduces an escape; the hex digits may be upper- or lowercase. Implementation: crates/foma/src/utf8.rs.

> [spec:foma:def:fomalibconf.dequote-string-fn]
> void dequote_string(char *s)

> [spec:foma:sem:fomalibconf.dequote-string-fn]
> In-place: if `s` both begins and ends with a double-quote byte (0x22), strip those two quotes by shifting the interior `len-2` bytes one position left and writing a NUL terminator, then call `decode_quoted(s)` to expand any `\uXXXX` escapes in the result. Otherwise leave `s` untouched.
> `len = strlen(s)`; the end-quote test reads `s[len-1]`, so an empty string reads `s[-1]` — out-of-bounds read (latent bug). A one-byte string `"` passes both tests (same byte inspected twice) and becomes the empty string. Implementation: foma/utf8.c.

> [spec:foma:def:fomalibconf.escape-string-fn]
> char *escape_string(char *string, char chr)

> [spec:foma:sem:fomalibconf.escape-string-fn]
> Returns a version of `string` with every occurrence of byte `chr` preceded by a backslash. First pass counts the occurrences `j`. If `j == 0`, returns `string` itself (no copy — caller cannot assume it owns a fresh buffer). Otherwise callocs `strlen(string)+j` bytes and copies byte-by-byte, emitting `'\\'` immediately before each `chr`; returns the new buffer (caller frees).
> Latent bug: the escaped text is exactly `strlen(string)+j` bytes and completely fills the buffer, leaving no room for a NUL terminator — the returned string is unterminated (the calloc zero-fill is fully overwritten). A correct clean-room port should allocate one extra byte, or reproduce the bug knowingly. Byte-oriented, not UTF-8-aware. Implementation: foma/utf8.c.

> [spec:foma:def:fomalibconf.find-arccount-fn]
> int find_arccount(struct fsm_state *fsm)

> [spec:foma:sem:fomalibconf.find-arccount-fn]
> Despite the name, returns the number of lines (not arcs) in an `fsm_state` line array: scans from index 0 until the line whose `state_no == -1` (the terminating sentinel) and returns that index. The sentinel is not counted; dummy lines with `target == -1` (arcless states) are counted. Source carries a `TODO: separate linecount and arccount` comment acknowledging the misnomer. Implementation: foma/structures.c.

> [spec:foma:def:fomalibconf.flag-check-fn]
> int flag_check(char *sm)

> [spec:foma:sem:fomalibconf.flag-check-fn]
> Returns 1 iff `sm` is a well-formed flag-diacritic symbol, else 0. Hand-coded byte-level DFA (not UTF-8-aware), anchored to the whole string: the byte after the closing `'@'` must be NUL. Accepted forms (ND = any byte that is neither `'.'` nor NUL):
> - `@X.A.B@` with X in {U,P,N,E}: A is 1+ ND bytes (`'@'` is permitted inside A; the first `'.'` after A ends it); B is 1+ bytes none of which is `'.'`, `'@'`, or NUL (the first `'@'` ends B and must be the last byte).
> - `@X.A@` or `@X.A.B@` with X in {R,D}: A is 1+ bytes not `'.'`, `'@'`, or NUL (an `'@'` closes the one-argument form; a `'.'` starts B); B as above.
> - `@C.A@` only (one-argument): A is 1+ bytes not `'.'`, `'@'`, or NUL.
> Any other shape — wrong operator letter, empty A or B, missing delimiters, or trailing bytes after the closing `'@'` — returns 0. Implementation: foma/flags.c (parameter is named `s` there).

> [spec:foma:def:fomalibconf.flag-get-name-fn]
> char *flag_get_name(char *string)

> [spec:foma:sem:fomalibconf.flag-get-name-fn]
> Extracts the attribute name from a flag diacritic like `@U.name.value@`: returns a copy of the substring strictly between the first `'.'` and the next `'.'` or `'@'` after it; returns NULL if that delimiter pair is not found. Caller owns the result.
> Algorithm: iterate byte positions over `strlen(string)` bytes, advancing by whole UTF-8 characters (`i += utf8skip(string+i)+1`). The first `'.'` sets `start = i+1` and continues; the first later `'.'` or `'@'` (with `start` already set) sets `end = i` and breaks. Returns the copy only when both `start > 0` and `end > 0`. An empty name (`@U..v@`) therefore yields a malloc'd empty string, not NULL. Implementation: foma/flags.c.

> [spec:foma:def:fomalibconf.flag-get-type-fn]
> int flag_get_type(char *string)

> [spec:foma:sem:fomalibconf.flag-get-type-fn]
> Returns the action type of a flag-diacritic string by comparing only bytes 1 and 2 (the leading `'@'` at byte 0 is never verified): `"U."` → FLAG_UNIFY (1), `"C."` → FLAG_CLEAR (2), `"D."` → FLAG_DISALLOW (4), `"N."` → FLAG_NEGATIVE (8), `"P."` → FLAG_POSITIVE (16), `"R."` → FLAG_REQUIRE (32), `"E."` → FLAG_EQUAL (64); anything else returns 0. Tests run in that order via `strncmp(string+1, "X.", 2)`; assumes the string is at least 1 byte long (reads from `string+1`). Implementation: foma/flags.c.

> [spec:foma:def:fomalibconf.flag-get-value-fn]
> char *flag_get_value(char *string)

> [spec:foma:sem:fomalibconf.flag-get-value-fn]
> Extracts the value part of a flag diacritic like `@U.name.value@`: returns a copy of the substring between the last `'.'` preceding the closing `'@'` and that `'@'`; returns NULL when there is no value (one-argument flags like `@D.name@`, or malformed input). Caller owns the result.
> Algorithm: iterate byte positions over `strlen(string)` bytes, advancing by whole UTF-8 characters. The first `'.'` sets `first = i+1`; every subsequent `'.'` sets `start = i+1` (so `start` tracks the dot immediately before the value); an `'@'` encountered while `start != 0` sets `end = i` and breaks — the leading `'@'` at i=0 is ignored because `start` is still 0 there. Returns the copy only if `start > 0 && end > 0`. Implementation: foma/flags.c.

> [spec:foma:def:fomalibconf.fsm-construct-handle]
> struct fsm_construct_handle {
>   struct fsm_state_list *fsm_state_list;
>   int fsm_state_list_size;
>   struct fsm_sigma_list *fsm_sigma_list;
>   int fsm_sigma_list_size;
>   struct fsm_sigma_hash *fsm_sigma_hash;
>   int fsm_sigma_hash_size;
>   int maxstate;
>   int maxsigma;
>   int numfinals;
>   int hasinitial;
>   char *name;
> }

> [spec:foma:def:fomalibconf.fsm-count-fn]
> FEXPORT void fsm_count(struct fsm *net)

> [spec:foma:sem:fomalibconf.fsm-count-fn]
> Recomputes and stores a net's counters in one pass over `net->states`, stopping at (excluding) the `state_no == -1` sentinel. Per line: track `maxstate` = maximum `state_no` seen (initialized to 0); increment `linecount`; increment `arccount` if `target != -1`; and whenever `state_no` differs from the previous line's (previous initialized to -1), if that first line of the block has nonzero `final_state`, increment `finalcount`.
> Finality is thus sampled once per contiguous block of lines sharing a state number — lines are assumed grouped by state; a final state split across non-adjacent blocks would be counted twice.
> After the loop, increment `linecount` once more (to include the sentinel), then write `net->statecount = maxstate+1`, `net->linecount`, `net->arccount`, `net->finalcount`. `statecount` assumes states are densely numbered from 0. Implementation: foma/constructions.c.

> [spec:foma:def:fomalibconf.fsm-read-binary-handle]
> typedef void *fsm_read_binary_handle

> [spec:foma:def:fomalibconf.fsm-sigma-hash]
> struct fsm_sigma_hash {
>   char *symbol;
>   short int sym;
>   struct fsm_sigma_hash *next;
> }

> [spec:foma:def:fomalibconf.fsm-sigma-list]
> struct fsm_sigma_list {
>   char *symbol;
> }

> [spec:foma:def:fomalibconf.fsm-sort-lines-fn]
> void fsm_sort_lines(struct fsm *net)

> [spec:foma:sem:fomalibconf.fsm-sort-lines-fn]
> Sorts the transition lines of `net->states` in place into ascending `state_no` order: calls `qsort` over the first `find_arccount(fsm)` lines (all lines preceding the `state_no == -1` sentinel; the sentinel stays in its final slot) with comparator `sort_cmp` (difference of the two lines' `state_no`). `qsort` is not stable, so the relative order of lines within one state is unspecified. Does not touch `net->arcs_sorted_in`/`arcs_sorted_out` or any other net field. Implementation: foma/constructions.c.

> [spec:foma:def:fomalibconf.fsm-state-add-arc-fn]
> void fsm_state_add_arc(int state_no, int in, int out, int target, int final_state, int start_state)

> [spec:foma:sem:fomalibconf.fsm-state-add-arc-fn]
> Appends one line to the machine being built by the dynarray module (module-static state initialized by `fsm_state_init`). Steps in order:
> 1. If `in != out`, set the module-global `arity` to 2.
> 2. If `in == EPSILON (0) && out == EPSILON`: when `state_no == target`, return immediately (epsilon self-loops are silently dropped); otherwise clear the module flags `is_deterministic` and `is_epsilon_free`.
> 3. If `in != -1 && out != -1` (a real arc): look up the dedup entry `slookup[ssize*in + out]`. If its `mainloop` stamp equals the current global `mainloop` generation (same in:out label already added for this state): if its recorded `target` equals this arc's target, return (exact duplicate skipped); otherwise clear `is_deterministic` and continue. Then increment the global `arccount` and overwrite the entry with the current `mainloop` stamp and this `target`. Dummy/sentinel lines (a -1 field) bypass this step entirely and are never counted as arcs. Note: because the entry's target is overwritten, a third same-label arc repeating the FIRST target is added again as a duplicate line.
> 4. Set `current_trans = 1` (the currently open state now has a line). If `current_fsm_linecount >= current_fsm_size`, double `current_fsm_size` and realloc the line array; on realloc failure, perror "Fatal error: out of memory" and exit(1).
> 5. Write the six arguments into the next slot and increment `current_fsm_linecount`. `in`/`out` truncate to `short int` fields, `final_state`/`start_state` to `char` fields. Implementation: foma/dynarray.c.

> [spec:foma:def:fomalibconf.fsm-state-close-fn]
> void fsm_state_close(struct fsm *net)

> [spec:foma:sem:fomalibconf.fsm-state-close-fn]
> Finalizes a dynarray construction and hands the result to `net`. Steps: append the terminating sentinel line via `fsm_state_add_arc(-1,-1,-1,-1,-1,-1)` (all six fields -1, including final/start); realloc the line array down to exactly `current_fsm_linecount` lines; copy the module-global counters into the net: `arity`, `arccount`, `statecount`, `linecount` (includes the sentinel), `finalcount = num_finals`, `pathcount = PATHCOUNT_UNKNOWN (-3)`.
> If more than one initial state was declared (`num_initials > 1`), clear the deterministic flag. Then set `net->is_deterministic` and `net->is_epsilon_free` from the tracked module flags; `is_pruned`, `is_minimized`, `is_loop_free`, `is_completed` all to UNK (2); `arcs_sorted_in = arcs_sorted_out = 0`. Finally store the array in `net->states` (ownership transfers to the net) and free the `slookup` dedup table. Implementation: foma/dynarray.c.

> [spec:foma:def:fomalibconf.fsm-state-copy-fn]
> struct fsm_state *fsm_state_copy(struct fsm_state *fsm_state, int linecount)

> [spec:foma:sem:fomalibconf.fsm-state-copy-fn]
> Returns a malloc'd byte-for-byte copy of the first `linecount` lines of `fsm_state`: allocates `linecount * sizeof(struct fsm_state)` bytes and memcpys them. The caller owns and frees the copy. `linecount` must include the sentinel line for the copy to be a valid line array (callers pass `net->linecount`, which does). No validation; malloc result unchecked. Implementation: foma/structures.c.

> [spec:foma:def:fomalibconf.fsm-state-end-state-fn]
> void fsm_state_end_state()

> [spec:foma:sem:fomalibconf.fsm-state-end-state-fn]
> Closes the state opened by `fsm_state_set_current_state` in the dynarray module. If no line was emitted for the state (`current_trans == 0`), emit a dummy arcless line via `fsm_state_add_arc(current_state_no, -1, -1, -1, current_final, current_start)` so the state still appears in the line array. Then increment the module-global `statecount`, and increment the `mainloop` generation counter — this lazily invalidates every `slookup` duplicate-arc entry for the next state without clearing the table. Implementation: foma/dynarray.c.

> [spec:foma:def:fomalibconf.fsm-state-init-fn]
> struct fsm_state *fsm_state_init(int sigma_size)

> [spec:foma:sem:fomalibconf.fsm-state-init-fn]
> Begins dynamic construction of an `fsm_state` line array by (re)initializing the dynarray module's static state — only one construction may be in flight at a time (not reentrant, not thread-safe). Mallocs the line array for INITIAL_SIZE = 16384 lines, storing it in the module head pointer; sets line count to 0; sets `ssize = sigma_size+1` and callocs the duplicate-arc table `slookup` with `ssize*ssize` entries of `{int target; unsigned int mainloop}` — so `sigma_size` must be at least the largest symbol number that will appear on any arc.
> Initializes the module counters/flags: `mainloop = 1`, `is_deterministic = 1`, `is_epsilon_free = 1`, `arccount = 0`, `num_finals = 0`, `num_initials = 0`, `statecount = 0`, `arity = 1`, `current_trans = 1`. Returns the array pointer (same as the module global; ownership stays with the module until `fsm_state_close`). malloc/calloc results are not checked. Implementation: foma/dynarray.c.

> [spec:foma:def:fomalibconf.fsm-state-list]
> struct fsm_state_list {
>   _Bool used;
>   _Bool is_final;
>   _Bool is_initial;
>   short int num_trans;
>   int state_number;
>   struct fsm_trans_list *fsm_trans_list;
> }

> [spec:foma:def:fomalibconf.fsm-state-set-current-state-fn]
> void fsm_state_set_current_state(int state_no, int final_state, int start_state)

> [spec:foma:sem:fomalibconf.fsm-state-set-current-state-fn]
> Opens a new state in the dynarray construction: stores `state_no`, `final_state`, `start_state` in the module's current-state globals and clears `current_trans` (no lines emitted yet for this state). Increments the module counter `num_finals` iff `final_state == 1` exactly, and `num_initials` iff `start_state == 1` exactly — other nonzero values are recorded on lines but not counted. Must be paired with a later `fsm_state_end_state`. Implementation: foma/dynarray.c.

> [spec:foma:def:fomalibconf.fsm-trans-list]
> struct fsm_trans_list {
>   short int in;
>   short int out;
>   int target;
>   struct fsm_trans_list *next;
> }

> [spec:foma:def:fomalibconf.fsm-update-flags-fn]
> void fsm_update_flags(struct fsm *net, int det, int pru, int min, int eps, int loop, int completed)

> [spec:foma:sem:fomalibconf.fsm-update-flags-fn]
> Bulk-sets a net's property flags: assigns `net->is_deterministic = det`, `is_pruned = pru`, `is_minimized = min`, `is_epsilon_free = eps`, `is_loop_free = loop`, `is_completed = completed` verbatim (callers pass YES=1, NO=0, or UNK=2), and unconditionally resets `net->arcs_sorted_in` and `net->arcs_sorted_out` to NO (0). No other side effects. Implementation: foma/constructions.c.

> [spec:foma:def:fomalibconf.int-stack-clear-fn]
> void int_stack_clear()

> [spec:foma:sem:fomalibconf.int-stack-clear-fn]
> Empties the module-global integer stack (a static array of MAX_STACK = 2097152 ints in foma/int_stack.c with a static `top` index, initially -1) by resetting `top` to -1. Nothing is freed or zeroed; old contents remain in the array but become unreachable.

> [spec:foma:def:fomalibconf.int-stack-find-fn]
> int int_stack_find (int entry)

> [spec:foma:sem:fomalibconf.int-stack-find-fn]
> Membership test on the global integer stack: returns 0 immediately if the stack is empty; otherwise linearly scans slots 0 through `top` inclusive and returns 1 on the first element equal to `entry`, or 0 if none matches. Does not modify the stack. Implementation: foma/int_stack.c.

> [spec:foma:def:fomalibconf.int-stack-isempty-fn]
> int int_stack_isempty()

> [spec:foma:sem:fomalibconf.int-stack-isempty-fn]
> Returns nonzero (true) iff the global integer stack is empty, i.e. its static `top` index equals -1; otherwise 0. Implementation: foma/int_stack.c.

> [spec:foma:def:fomalibconf.int-stack-isfull-fn]
> int int_stack_isfull()

> [spec:foma:sem:fomalibconf.int-stack-isfull-fn+1]
> The integer stack grows unbounded (a `Vec`), so it is never full — always returns 0. The C `MAX_STACK` cap (2097152 fixed slots) and the `top == MAX_STACK - 1` boundary are gone. Implementation: crates/foma/src/int_stack.rs.

> [spec:foma:def:fomalibconf.int-stack-pop-fn]
> int int_stack_pop()

> [spec:foma:sem:fomalibconf.int-stack-pop-fn]
> Returns the element at the global integer stack's `top` index and post-decrements `top`. No underflow check: popping an empty stack reads array index -1 (undefined behavior) and leaves `top` at -2; callers must guard with `int_stack_isempty` themselves. Implementation: foma/int_stack.c.

> [spec:foma:def:fomalibconf.int-stack-push-fn]
> void int_stack_push(int c)

> [spec:foma:sem:fomalibconf.int-stack-push-fn+1]
> Pushes `c` onto the global integer stack, growing its backing `Vec` on demand — infallible and unbounded. The C `MAX_STACK` cap and the "Stack full!\n" + exit(1) overflow path are gone. Implementation: crates/foma/src/int_stack.rs.

> [spec:foma:def:fomalibconf.int-stack-size-fn]
> int int_stack_size()

> [spec:foma:sem:fomalibconf.int-stack-size-fn]
> Returns the number of elements on the global integer stack: `top + 1` (0 when empty). Implementation: foma/int_stack.c.

> [spec:foma:def:fomalibconf.int-stack-status-fn]
> int int_stack_status()

> [spec:foma:sem:fomalibconf.int-stack-status-fn+1]
> Dead prototype: declared in fomalibconf.h but never defined anywhere in the C tree (a link error if called). Wave 4 gives it an honest signature `Result<i32, FomaError>` that always returns `Err(FomaError::Unimplemented(..))` (was: panic). Implementation: crates/foma/src/int_stack.rs.

> [spec:foma:def:fomalibconf.ishexstr-fn]
> int ishexstr(char *str)

> [spec:foma:sem:fomalibconf.ishexstr-fn]
> Returns 1 iff the first 4 bytes of `str` are all ASCII hex digits, else 0; used to validate the XXXX of a `\uXXXX` escape. Each byte is tested against three exclusive ranges: 0x30–0x39 ('0'–'9'), 0x41–0x46 ('A'–'F'), 0x61–0x66 ('a'–'f'). The first non-matching byte returns 0 immediately, so a NUL within the first 4 bytes returns 0 without reading past the terminator. With default-signed `char`, bytes >= 0x80 compare negative and fail every range (returns 0). Implementation: foma/utf8.c.

> [spec:foma:def:fomalibconf.map-firstlines-fn]
> struct state_array *map_firstlines(struct fsm *net)

> [spec:foma:sem:fomalibconf.map-firstlines-fn]
> Builds a state-number → first-transition-line index for `net`: mallocs an array of `net->statecount + 1` `struct state_array` entries, then scans `net->states` until the `state_no == -1` sentinel; whenever a line's `state_no` differs from the previous line's (previous initialized to -1), stores a pointer to that line in entry `[state_no].transitions`. Returns the array; caller owns and frees it.
> Caveats: the array is malloc'd, not zeroed, so entries for state numbers that never appear as a source hold garbage pointers; lines are assumed grouped by state — if a state's lines occur in multiple non-adjacent runs, the last run's first line overwrites the earlier pointer; state numbers must be <= `net->statecount` or the writes go out of bounds. Implementation: foma/structures.c.

> [spec:foma:def:fomalibconf.next-power-of-two-fn]
> int next_power_of_two(int v)

> [spec:foma:sem:fomalibconf.next-power-of-two-fn]
> Returns 1 left-shifted by the number of significant bits of `v`: loop `i` from 0, right-shifting `v` by 1 each iteration, until `v <= 0`; return `1 << i`. For `v > 0` this is the smallest power of two strictly greater than `v` — exact powers of two are doubled (8 → 16). For `v == 0` or negative `v` the loop never runs and the result is 1. If `v >= 2^30` the result `1 << 31` overflows signed int (UB); callers only use it for buffer growth well below that. Implementation: foma/mem.c.

> [spec:foma:def:fomalibconf.ptr-stack-clear-fn]
> void ptr_stack_clear()

> [spec:foma:sem:fomalibconf.ptr-stack-clear-fn]
> Empties the module-global pointer stack (a static array of MAX_PTR_STACK = 2097152 `void *` slots in foma/int_stack.c with a static `ptr_stack_top` index, initially -1) by resetting `ptr_stack_top` to -1. The stored pointers are not freed — any owned memory still on the stack leaks unless the caller freed it first.

> [spec:foma:def:fomalibconf.ptr-stack-isempty-fn]
> int ptr_stack_isempty()

> [spec:foma:sem:fomalibconf.ptr-stack-isempty-fn]
> Returns nonzero (true) iff the global pointer stack is empty, i.e. its static `ptr_stack_top` index equals -1; otherwise 0. Implementation: foma/int_stack.c.

> [spec:foma:def:fomalibconf.ptr-stack-isfull-fn]
> int ptr_stack_isfull()

> [spec:foma:sem:fomalibconf.ptr-stack-isfull-fn+1]
> The pointer stack grows unbounded (a `Vec`), so it is never full — always returns 0. The C `MAX_PTR_STACK` cap (2097152 fixed slots) and the `ptr_stack_top == MAX_PTR_STACK - 1` boundary are gone. Implementation: crates/foma/src/int_stack.rs.

> [spec:foma:def:fomalibconf.ptr-stack-pop-fn]
> void *ptr_stack_pop()

> [spec:foma:sem:fomalibconf.ptr-stack-pop-fn]
> Returns the `void *` at the global pointer stack's `ptr_stack_top` index and post-decrements `ptr_stack_top`. No underflow check: popping an empty stack reads array index -1 (undefined behavior) and leaves `ptr_stack_top` at -2; callers must guard with `ptr_stack_isempty` themselves. Ownership of the returned pointer transfers to the caller (the stack never frees stored pointers). Implementation: foma/int_stack.c.

> [spec:foma:def:fomalibconf.ptr-stack-push-fn]
> void ptr_stack_push(void *ptr)

> [spec:foma:sem:fomalibconf.ptr-stack-push-fn+1]
> Pushes `ptr` (an index/handle token) onto the global pointer stack, growing its backing `Vec` on demand — infallible and unbounded. The C `MAX_PTR_STACK` cap and the "Pointer stack full!\n" + exit(1) overflow path are gone. The stack stores the token only; no ownership is taken. Implementation: crates/foma/src/int_stack.rs.

> [spec:foma:def:fomalibconf.round-up-to-power-of-two-fn]
> unsigned int round_up_to_power_of_two(unsigned int v)

> [spec:foma:sem:fomalibconf.round-up-to-power-of-two-fn]
> Rounds `v` up to the nearest power of two using the classic 32-bit bit-smear: decrement `v`, then OR `v` with itself right-shifted by 1, 2, 4, 8, 16 (in that order), then increment and return. Exact powers of two return themselves (unlike `next_power_of_two`, which doubles them). Edge cases from unsigned wraparound: input 0 gives 0 (0-1 = UINT_MAX, smear keeps UINT_MAX, +1 wraps to 0); any input above 2^31 also gives 0. Pure function, no state. Implementation: foma/mem.c.

> [spec:foma:def:fomalibconf.sigma-add-fn]
> FEXPORT int sigma_add (char *symbol, struct sigma *sigma)

> [spec:foma:sem:fomalibconf.sigma-add-fn+1]
> Adds `symbol` (copied) to the sigma alphabet (a `Vec` of entries `{ int number; symbol }` in insertion order; an empty alphabet is an empty Vec) and returns the number it was assigned. First classify: the literal strings "@_EPSILON_SYMBOL_@", "@_UNKNOWN_SYMBOL_@", "@_IDENTITY_SYMBOL_@" map to reserved codes EPSILON=0, UNKNOWN=1, IDENTITY=2 respectively; anything else is non-special.
> Non-special: append a new entry at the tail with number = last->number + 1, forced up to 3 if last->number + 1 < 3 (an empty alphabet starts at 3); return that number. Note this assumes the alphabet's last entry carries the maximum number (entries kept sorted); no duplicate check is done, and per the source comment the caller is responsible for sorting sigma before merge_sigma is called.
> Special (code `assert` in 0..2): insert the `{number: assert, symbol}` entry keeping the alphabet sorted by number — before the first entry whose number is >= assert (so a duplicate code lands before any equal-numbered entry), or at the tail. Return assert. No duplicate check here either. Implementation: crates/foma/src/sigma.rs.

> [spec:foma:def:fomalibconf.sigma-add-number-fn]
> FEXPORT int sigma_add_number(struct sigma *sigma, char *symbol, int number)

> [spec:foma:sem:fomalibconf.sigma-add-number-fn+1]
> Appends `symbol` with an explicit caller-chosen `number` at the tail of the sigma alphabet, always returning 1. No sortedness maintenance, no duplicate check, no validation that `number` is unused — the caller controls numbering entirely. Implementation: crates/foma/src/sigma.rs.

> [spec:foma:def:fomalibconf.sigma-add-special-fn]
> FEXPORT int sigma_add_special (int symbol, struct sigma *sigma)

> [spec:foma:sem:fomalibconf.sigma-add-special-fn+2]
> Inserts the reserved symbol with code `symbol` into the sigma alphabet in sorted (ascending number) position and returns `symbol`. The stored string is the canonical name: EPSILON=0 → "@_EPSILON_SYMBOL_@", IDENTITY=2 → "@_IDENTITY_SYMBOL_@", UNKNOWN=1 → "@_UNKNOWN_SYMBOL_@". C left the string NULL for any other code (later strcmp/free on that node dereferenced NULL); a non-reserved code now yields a well-formed placeholder "@_SPECIAL_<code>_@" so the entry is never symbol-less.
> Insertion is identical to the special branch of `sigma_add`: place the `{number: symbol code, str}` entry before the first entry whose number is >= symbol (or at the tail). No duplicate check: adding an already-present code inserts a duplicate entry before the existing one. Implementation: crates/foma/src/sigma.rs.

> [spec:foma:def:fomalibconf.sigma-cleanup-fn]
> void sigma_cleanup (struct fsm *net, int force)

> [spec:foma:sem:fomalibconf.sigma-cleanup-fn+1]
> Removes from `net->sigma` every symbol that never occurs on any transition, and renumbers the survivors densely. Steps:
> 1. If `force == 0` and the sigma contains IDENTITY (2) or UNKNOWN (1) (checked via `sigma_find_number`), return immediately without changes — unknown-matching symbols make "unused" undecidable. If `force == 1`, always proceed.
> 2. Compute `maxsigma = sigma_max(net->sigma)`; if negative (NULL or empty sigma), return.
> 3. malloc an int array `attested[0..maxsigma]`, zero it. Scan `net->states` lines until the `state_no == -1` sentinel; for each line set attested[in] = 1 and attested[out] = 1 when the respective field is >= 0 (negative in/out, e.g. the -1s on dummy lines, are skipped).
> 4. Build the renumbering in place: with `j` starting at 3, for `i` from 3 to maxsigma, if attested[i] is set, overwrite attested[i] = j and increment j. Reserved codes 0–2 keep their numbers (their attested slots stay 0/1 flags).
> 5. Rewrite the machine: rescan all transition lines; any `in` > 2 becomes attested[in], any `out` > 2 becomes attested[out].
> 6. In alphabet order, drop every entry with attested[number] == 0 (unused — including reserved codes 0–2 that appear in sigma but on no arc); for each surviving entry set number = attested[number] when >= 3 (reserved numbers unchanged). Order is otherwise preserved.
> 7. Returns void; mutates both `net->states` and `net->sigma` in place. Implementation: crates/foma/src/sigma.rs.

> [spec:foma:def:fomalibconf.sigma-create-fn]
> sigma *sigma_create ()

> [spec:foma:sem:fomalibconf.sigma-create-fn+1]
> Returns a fresh empty sigma alphabet (an empty Vec — there is no sentinel node). Implementation: crates/foma/src/sigma.rs.

> [spec:foma:def:fomalibconf.sigma-find-fn]
> int sigma_find (char *symbol, struct sigma *sigma)

> [spec:foma:sem:fomalibconf.sigma-find-fn]
> Looks up `symbol` by string in the sigma list: returns -1 immediately if `sigma` is NULL or its head is the empty sentinel (number == -1); otherwise walks the list while the node is non-NULL and its number != -1, returning the `number` of the first node whose `symbol` compares equal via strcmp; returns -1 if no match. A node with a NULL symbol (possible via `sigma_add_special` with a non-reserved code) would crash strcmp. Implementation: foma/sigma.c.

> [spec:foma:def:fomalibconf.sigma-find-number-fn]
> int sigma_find_number (int number, struct sigma *sigma)

> [spec:foma:sem:fomalibconf.sigma-find-number-fn]
> Membership test by symbol number: returns -1 if `sigma` is NULL or its head is the empty sentinel (number == -1); otherwise walks the list while node != NULL and node->number != -1 and returns `number` itself on the first node with node->number == number; -1 if absent. So the return value is either the queried number (present) or -1 (absent); note querying for -1 always returns -1. Implementation: foma/sigma.c.

> [spec:foma:def:fomalibconf.sigma-max-fn]
> FEXPORT int sigma_max(struct sigma *sigma)

> [spec:foma:sem:fomalibconf.sigma-max-fn+1]
> Returns the maximum `number` over all entries in the sigma alphabet, starting the accumulator at -1; an empty alphabet therefore yields -1. Implementation: crates/foma/src/sigma.rs.

> [spec:foma:def:fomalibconf.sigma-remove-fn]
> sigma *sigma_remove(char *symbol, struct sigma *sigma)

> [spec:foma:sem:fomalibconf.sigma-remove-fn+1]
> Removes, in place, the first entry whose `symbol` strcmp-equals the argument from the sigma alphabet; the remaining entries keep their order. An empty alphabet or a symbol that is absent is a no-op. Implementation: crates/foma/src/sigma.rs.

> [spec:foma:def:fomalibconf.sigma-remove-num-fn]
> sigma *sigma_remove_num(int num, struct sigma *sigma)

> [spec:foma:sem:fomalibconf.sigma-remove-num-fn+1]
> Identical to `sigma_remove` except the first matching entry is selected by number == num instead of by strcmp on the symbol string: removes it in place; an empty alphabet or a number that is absent is a no-op. Implementation: crates/foma/src/sigma.rs.

> [spec:foma:def:fomalibconf.sigma-size-fn]
> int sigma_size(struct sigma *sigma)

> [spec:foma:sem:fomalibconf.sigma-size-fn+1]
> Returns the number of entries in the sigma alphabet; an empty alphabet returns 0. Implementation: crates/foma/src/sigma.rs.

> [spec:foma:def:fomalibconf.sigma-sort-fn]
> int sigma_sort (struct fsm *net)

> [spec:foma:sem:fomalibconf.sigma-sort-fn+2]
> Sorts the non-reserved symbols of `net->sigma` by symbol string (strcmp order via qsort), renumbers them consecutively from 3, and rewrites all transition in/out numbers to match. Always returns 1 (also returns 1 early, doing nothing, when `sigma_max(net->sigma)` < 0). Steps:
> 1. `size = sigma_max(net->sigma)`; allocate a temp array of `size` {symbol, number} pairs.
> 2. Collect every alphabet entry with number > IDENTITY (2) into the array (moving each symbol out); let `max` = count; sort the array by strcmp on symbol.
> 3. Build an int `replacearray` of size+3 entries. The C left it uninitialized (garbage; the Wave-2 port zeroed it), so an arc numbered with a symbol absent from sigma got corrupted. Seed it with the identity map (replacearray[k]=k), then set replacearray[pair.number] = sorted-index + 3 for each collected pair — old number → new number; absent numbers keep their own value.
> 4. Walk `net->states` lines to the state_no == -1 sentinel, replacing any `in` > 2 with replacearray[in] and any `out` > 2 with replacearray[out]. An arc carrying a number absent from sigma is now left unchanged rather than corrupted.
> 5. Walk the entries again in alphabet order with counter i: each entry with number > 2 gets number = i+3 and symbol = sorted array entry i's symbol (moved back), i++. Since reserved entries 0–2 sort before the rest, the alphabet ends up sorted by number with symbols in strcmp order. Implementation: crates/foma/src/sigma.rs.

> [spec:foma:def:fomalibconf.sigma-string-fn]
> FEXPORT char *sigma_string(int number, struct sigma *sigma)

> [spec:foma:sem:fomalibconf.sigma-string-fn]
> Reverse lookup: returns the symbol string for symbol number `number`, or NULL. Returns NULL if `sigma` is NULL or its head is the empty sentinel (number == -1); otherwise walks while node != NULL and node->number != -1 and returns the first matching node's `symbol` pointer — an alias into the sigma's own storage, not a copy; the caller must not free it and it dangles if the sigma entry is later removed/substituted. NULL if the number is absent. Implementation: foma/sigma.c.

> [spec:foma:def:fomalibconf.sigma-substitute-fn]
> int sigma_substitute(char *orig, char *sub, struct sigma *sigma)

> [spec:foma:sem:fomalibconf.sigma-substitute-fn]
> Renames a symbol in place: finds the first node whose symbol strcmp-equals `orig` (walking while node != NULL and node->number != -1; returns -1 immediately if the head is the empty sentinel), frees that node's old symbol string, replaces it with strdup(sub), and returns the node's unchanged `number`. Returns -1 if `orig` is not found. Per the source comment there is no duplicate check: if `sub` already exists in sigma this silently creates two entries with the same string and different numbers. Implementation: foma/sigma.c.

> [spec:foma:def:fomalibconf.sigma-to-list-fn]
> struct fsm_sigma_list *sigma_to_list(struct sigma *sigma)

> [spec:foma:sem:fomalibconf.sigma-to-list-fn]
> Converts the sigma linked list into a number-indexed array for O(1) lookup: calloc's `sigma_max(sigma)+1` entries of `struct fsm_sigma_list { char *symbol; }`, then walks sigma (stopping at NULL or the number == -1 sentinel) setting entry[node->number].symbol = node->symbol. Symbol pointers are aliased, not copied: the caller owns and frees the array itself but not the strings, and the array is invalidated by any mutation of the source sigma. Slots for numbers with no sigma entry remain NULL (calloc-zeroed). For a NULL or empty sigma, sigma_max is -1 and this is calloc(0, ...) — a zero-size allocation whose result must not be dereferenced. Implementation: foma/sigma.c.

> [spec:foma:def:fomalibconf.sort-cmp-fn]
> int sort_cmp(const void *a, const void *b)

> [spec:foma:sem:fomalibconf.sort-cmp-fn+1]
> qsort comparator for arrays of `struct fsm_state` (transition lines): casts both arguments to `const struct fsm_state *` and orders by a->state_no - b->state_no, i.e. ascending by source state number. Lines within the same state compare equal, so their relative order after qsort is unspecified. Returns an Ordering (Less/Equal/Greater); the C `int` sign of the subtraction carries the same information. Implementation: foma/constructions.c.

> [spec:foma:def:fomalibconf.state-array]
> struct state_array {
>   struct fsm_state *transitions;
> }

> [spec:foma:def:fomalibconf.streqrep-fn]
> char *streqrep(char *s, char *oldstring, char *newstring)

> [spec:foma:sem:fomalibconf.streqrep-fn+1]
> Replaces every non-overlapping occurrence of `oldstring` in `s` with the same-length prefix of `newstring`, in place, and returns `s`. Algorithm: let len = strlen(oldstring); a single left-to-right scan advances past each replacement (by len) so it always terminates and is O(|s|). Intended for equal-length replacement only (per the source comment). C rescanned from the start of `s` after every replacement, so if the written bytes still contained `oldstring` (newstring == oldstring, or overlapping self-reproduction, or an empty oldstring that matches everywhere) the loop never terminated; an empty `oldstring` and a `newstring` shorter than `oldstring` (which C's memcpy read past) are now no-ops rather than a hang/overread. Implementation: foma/utf8.c.

> [spec:foma:def:fomalibconf.utf8code16tostr-fn]
> unsigned char *utf8code16tostr(char *str)

> [spec:foma:sem:fomalibconf.utf8code16tostr-fn]
> Converts the 4 hex digits at `str` (the XXXX of a `\uXXXX` escape, already validated by `ishexstr`) into a freshly malloc'd, NUL-terminated UTF-8 byte string, which the caller owns. Steps: codepoint = (hexstrtoint(str) << 8) + hexstrtoint(str+2), where the static helper hexstrtoint converts 2 hex chars — per char: value > 0x60 → char - 0x57 (lowercase a–f), else > 0x40 → char - 0x37 (uppercase A–F), else char - 0x30 (digit); first char shifted left 4, no validation. Then encode via helper int2utf8str into a malloc'd 5-byte buffer: codepoint < 0x80 → 1 byte as-is; < 0x800 → 2 bytes (0xC0|cp>>6, 0x80|cp&0x3F); < 0x10000 → 3 bytes (0xE0|cp>>12, 0x80|(cp>>6)&0x3F, 0x80|cp&0x3F); each followed by a 0 terminator. The >= 0x10000 branch returns NULL but is unreachable here (max 0xFFFF). Surrogate codepoints D800–DFFF are encoded blindly, yielding invalid UTF-8. Note: encoding U+0000 (" ") yields a buffer whose first byte is the terminator, i.e. an empty string. Implementation: foma/utf8.c.

> [spec:foma:def:fomalibconf.utf8iscombining-fn]
> int utf8iscombining(unsigned char *s)

> [spec:foma:sem:fomalibconf.utf8iscombining-fn]
> Tests whether the UTF-8 sequence starting at `s` encodes a Unicode 7.0 combining character; returns the byte length of its UTF-8 representation (2 or 3), or 0 if not combining. Checks raw bytes, no decoding:
> 1. If s[0] or s[1] is '\0', return 0.
> 2. If s[0] is not one of 0xCC, 0xCD, 0xE1, 0xE2, 0xEF, return 0 (fast reject).
> 3. Two-byte ranges (U+0300–036F Combining Diacritical Marks): s[0]==0xCC with s[1] in 0x80–0xBF → 2; s[0]==0xCD with s[1] in 0x80–0xAF → 2.
> 4. If s[2] is '\0', return 0.
> 5. Three-byte ranges: 0xE1 0xAA with s[2] in 0xB0–0xBE (U+1AB0–1ABE, Extended) → 3; 0xE1 0xB7 with s[2] in 0x80–0xBF (U+1DC0–1DFF, Supplement) → 3; 0xE2 0x83 with s[2] in 0x90–0xB0 (U+20D0–20F0, for Symbols) → 3; 0xEF 0xB8 with s[2] in 0xA0–0xAD (U+FE20–FE2D, Half Marks) → 3.
> 6. Otherwise 0. Parameter is `unsigned char *`, so the 0x80+ comparisons work directly. Implementation: foma/utf8.c.

> [spec:foma:def:fomalibconf.utf8skip-fn]
> int utf8skip(char *str)

> [spec:foma:sem:fomalibconf.utf8skip-fn]
> Returns the number of continuation bytes that follow the UTF-8 lead byte `*str` (so the full character is utf8skip+1 bytes): cast the byte to unsigned char; < 0x80 (ASCII, including NUL) → 0; top bits 110xxxxx ((b & 0xE0) == 0xC0) → 1; 1110xxxx ((b & 0xF0) == 0xE0) → 2; 11110xxx ((b & 0xF8) == 0xF0) → 3; anything else (a stray continuation byte 0x80–0xBF, or 0xF8–0xFF) → -1. Only the lead byte is examined; continuation bytes are never validated, and the string may be shorter than the announced length. Implementation: foma/utf8.c.

> [spec:foma:def:fomalibconf.utf8strlen-fn]
> int utf8strlen(char *str)

> [spec:foma:sem:fomalibconf.utf8strlen-fn+1]
> Counts UTF-8 characters in NUL-terminated `str`: let len = strlen(str); with byte index i = 0 and count j = 0, loop while str[i] != '\0' and i < len, each iteration advancing i by utf8skip(str+i) + 1 and incrementing j; return j. A truncated final multibyte character makes i jump past len and still counts as one character. If a stray continuation byte appears in lead position utf8skip returns -1, so i would advance by 0 (the C hung); the port forces the step to at least 1, counting the malformed byte as one character (lossy) so counting terminates. Implementation: crates/foma/src/utf8.rs.

> [spec:foma:def:fomalibconf.xprintf-fn]
> void xprintf(char *string)

> [spec:foma:sem:fomalibconf.xprintf-fn]
> Not ported to Rust: a disabled no-op output hook with no callers. The C behaviour was:
> No-op. The only definition is in foma/foma.c (the interactive CLI's main file, not the library sources): the body is `return ; printf("%s",string);` — the unconditional `return` comes first, so the printf is unreachable dead code and calling `xprintf` does nothing and returns immediately. Presumably a debugging switch left disabled. Because it is defined in the CLI, not in libfoma proper, a library-only consumer that calls it gets a link error. A port should treat it as a no-op hook. Implementation: foma/foma.c.



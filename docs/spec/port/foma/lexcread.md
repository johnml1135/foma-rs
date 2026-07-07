# foma/lexcread.c

> [spec:foma:def:lexcread.lexc-add-mc-fn]
> void lexc_add_mc(char *symbol)

> [spec:foma:sem:lexcread.lexc-add-mc-fn]
> Registers one Multichar_Symbols entry. First de-escapes `symbol` in place via `[spec:foma:sem:lexcread.lexc-deescape-string-fn]` with escape '%' and mode 0 (resolves '%'-escapes; note mode 0 silently deletes unescaped '0' bytes). If `[spec:foma:sem:lexcread.lexc-find-mc-fn]` says the symbol is already registered, does nothing else. Otherwise mallocs a `multichar_symbols` node with symbol=strdup(symbol) and inserts it into the file-static `mc` list, which is kept sorted in strictly decreasing utf8strlen order: walk with a prev pointer while existing symbols are longer, insert before the first node whose length is <= the new length (head is replaced when the list is empty or insertion lands at the front). Longest-first order makes the tokenizer's first linear-scan hit the longest match.
> Then s = sigma_add(symbol, lexsigma) (`[spec:foma:sem:sigma.sigma-add-fn]`), the symbol->s mapping is added to the sigma hashtable via `[spec:foma:sem:lexcread.lexc-add-sigma-hash-fn]`, the 65536-entry `mchash` boolean filter is set at index (unsigned byte0)*256 + (unsigned byte1) of the symbol (for a 1-byte symbol byte1 is the NUL terminator; such symbols are still found via the ordinary per-character sigma lookup), and s is stored in the node's `short int sigma_number`.

> [spec:foma:def:lexcread.lexc-add-network-fn]
> void lexc_add_network()

> [spec:foma:sem:lexcread.lexc-add-network-fn]
> Splices the stored regex network `current_regex_network` (set by `[spec:foma:sem:lexcread.lexc-set-network-fn]` for a `< regex >` entry) between clexicon->state (source lexicon state) and ctarget->state (continuation state). Steps:
> 1. Sigma import: allocate sigreplace = calloc(sigma_max(net->sigma)+1, sizeof(int)). For each net sigma node with number != -1: if `[spec:foma:sem:lexcread.lexc-find-sigma-hash-fn]` misses, sigma_add the symbol to lexsigma and register it via `[spec:foma:sem:lexcread.lexc-add-sigma-hash-fn]` (the special names @_EPSILON/UNKNOWN/IDENTITY_SYMBOL_@ thereby map back to 0/1/2 through sigma_add's special-casing); either way set sigreplace[old_number] = new_number. A local first_new_sigma records the first freshly assigned number but is never read (dead code).
> 2. Arc renumbering: walk net->states lines until the state_no==-1 terminator, remapping every in/out != -1 through sigreplace in place (the caller's network is mutated); track maxstate = highest state_no seen; set unknown_symbols=1 if any remapped arc has in==IDENTITY (2), in==UNKNOWN (1), or out==UNKNOWN (out==IDENTITY alone is not checked — harmless since identity is always two-sided).
> 3. If unknown_symbols: build a 0-terminated int array `unk` (calloc sigma_max(lexsigma)+2 ints) listing every lexsigma symbol with number > 2 whose string is absent from net->sigma (sigma_find == -1) — the concrete symbols the net's ?/@ must also match.
> 4. Allocate maxstate+1 fresh `struct states` (trans=NULL, lexstate=NULL, number=-1, hashval=(unsigned)-1, mergeable=0, distance=0, merge_with=self); each is prepended to the global statelist directly (start=0, final=0) without going through `[spec:foma:sem:lexcread.lexc-add-state-fn]`, so lexc_statecount is not bumped (harmless; it is recomputed in `[spec:foma:sem:lexcread.lexc-number-states-fn]`).
> 5. Prepend an EPSILON:EPSILON transition from the source lexicon state to the new state corresponding to net state 0 (assumed initial).
> 6. For each arc line with target != -1: prepend a transition {in, out, target=newstate[target]} onto newstate[state_no]->trans; if unknown_symbols and the arc has in==IDENTITY or out==IDENTITY, additionally prepend one sym:sym transition per entry of `unk` to the same target. Every line (including target==-1 ones) records finals[state_no] = final_state.
> 7. For every net state with finals[i]==1, prepend an EPSILON:EPSILON transition to the target lexicon state.
> 8. If unknown_symbols: free unk and set the global net_has_unknown=1, so all later sigma additions patch identity arcs via `[spec:foma:sem:lexcread.lexc-update-unknowns-fn]`. Free slist and finals. Leaks: sigreplace is never freed, and the source network itself (current_regex_network) is never freed.

> [spec:foma:def:lexcread.lexc-add-sigma-hash-fn]
> void lexc_add_sigma_hash(char *symbol, int number)

> [spec:foma:sem:lexcread.lexc-add-sigma-hash-fn]
> Inserts the mapping symbol->number into the file-static sigma hashtable: SIGMA_HASH_TABLESIZE=3079 bucket-head cells of `struct lexc_hashtable` (calloc'd array) with separate chaining via `next`. Bucket index = `[spec:foma:sem:lexcread.lexc-symbol-hash-fn]`(symbol). Before inserting, if the global net_has_unknown flag is 1 (some spliced `< regex >` contained unknown/identity), first calls `[spec:foma:sem:lexcread.lexc-update-unknowns-fn]`(number) to patch existing identity arcs with the new symbol.
> If the bucket head's symbol is NULL, fill it in place: symbol=strdup(symbol), sigma_number=number (next left untouched). Otherwise walk to the chain tail and append a malloc'd node {symbol=strdup(symbol), sigma_number=number, next=NULL}. No duplicate detection: re-adding an existing symbol creates a shadowed second entry.

> [spec:foma:def:lexcread.lexc-add-state-fn]
> void lexc_add_state(struct states *s)

> [spec:foma:sem:lexcread.lexc-add-state-fn]
> Registers state `s` in the global state registry: mallocs a `struct statelist` cell {state=s, start=0, final=0}, prepends it to the file-static `statelist` (LIFO: the most recently created state sits at the head), sets s->number = -1 (unnumbered), and increments lexc_statecount. Does not initialize any other field of `s`; callers must set trans, lexstate, mergeable, hashval, distance and merge_with themselves.

> [spec:foma:def:lexcread.lexc-add-word-fn]
> void lexc_add_word()

> [spec:foma:sem:lexcread.lexc-add-word-fn]
> Commits the pending entry as a path from the current source lexicon state to the current target lexicon state (both previously set by `[spec:foma:sem:lexcread.lexc-set-current-lexicon-fn]`). If current_entry == REGEX_ENTRY (2), delegates to `[spec:foma:sem:lexcread.lexc-add-network-fn]` and returns. Otherwise (WORD_ENTRY, 1): sourcestate = clexicon->state, deststate = ctarget->state, len = token count of cwordin (positions until -1); global maxlen = max(maxlen, len).
> Prefix sharing (trie construction): iterate i over the -1-terminated pairs cwordin[i]:cwordout[i] with a `follow` flag initially 1. While follow==1, scan sourcestate's transition list (prepend order, so most recently added first) for a transition with in==cwordin[i], out==cwordout[i] and target->lexstate==NULL; when this is the last pair (cwordin[i+1]==-1) a candidate is additionally rejected unless its target is deststate — which contradicts target->lexstate==NULL, so following never succeeds on the final pair and a fresh (possibly duplicate) transition to deststate is always created (duplicates are removed by determinization later). On a successful follow: sourcestate = trans->target, set that state's mergeable = 0 (it now carries multiple continuations, so suffix merging must skip it), and proceed to the next pair.
> At the first non-match, follow is cleared for the rest of the word; each remaining pair mallocs a transition prepended to sourcestate->trans with in=cwordin[i], out=cwordout[i]. Its target is deststate for the last pair; otherwise a fresh malloc'd state registered via `[spec:foma:sem:lexcread.lexc-add-state-fn]` with trans=NULL, lexstate=NULL, mergeable=1, hashval = `[spec:foma:sem:lexcread.lexc-suffix-hash-fn]`(i+1) (hash of the remaining pairs), distance = len-i-1, merge_with = itself. sourcestate then advances to the transition's target.
> An empty entry (cleared word = single EPSILON:EPSILON pair, see `[spec:foma:sem:lexcread.lexc-clear-current-word-fn]`) yields one epsilon transition from source to target.

> [spec:foma:def:lexcread.lexc-cleanup-fn]
> void lexc_cleanup()

> [spec:foma:sem:lexcread.lexc-cleanup-fn]
> Frees all file-static compile structures (called from `[spec:foma:sem:lexcread.lexc-to-fsm-fn]` after the fsm arc lines have been emitted): the 65536-byte mchash filter; for each of the 3079 hashtable buckets, every non-NULL symbol string in the chain plus the chained (malloc'd) nodes — the head cells belong to the calloc'd array and are not individually freed — then the bucket array itself; the mc list (symbol strings and nodes); the lexstates list (names and nodes; their states are freed through statelist); every statelist state's transition cells, then each state, then the statelist cells.
> lexsigma is not freed here — ownership was transferred to the output net. The static pointers are left dangling; `[spec:foma:sem:lexcread.lexc-init-fn]` must run before any further lexc use.

> [spec:foma:def:lexcread.lexc-clear-current-word-fn]
> void lexc_clear_current_word()

> [spec:foma:sem:lexcread.lexc-clear-current-word-fn]
> Resets the current-entry token buffers to the empty word: cwordin[0]=cwordout[0]=0 (EPSILON) and cwordin[1]=cwordout[1]=-1 (the arrays are -1-terminated), so an entry consisting only of a continuation class compiles to a single EPSILON:EPSILON transition. Also sets current_entry = WORD_ENTRY (1), cancelling any pending regex entry.

> [spec:foma:def:lexcread.lexc-deescape-string-fn]
> void lexc_deescape_string(char *name, char escape, int mode)

> [spec:foma:sem:lexcread.lexc-deescape-string-fn]
> In-place unescaping of `name` with read cursor i and write cursor j. Each iteration first copies name[i] to name[j], then: if name[i]==escape, overwrite name[j] with name[i+1] (the escaped character kept literally, whatever it is — including escape itself or '0'), j++, and skip the escaped character; else if mode==1 and the character is '0', write byte 0xff at name[j] (the internal marker for an alignment EPSILON) and j++; else if the character is neither escape nor '0', j++ (plain copy). Finally writes name[j]='\0'.
> Literal consequences: with mode==0 an unescaped '0' matches no branch, so j does not advance and the '0' is silently deleted (used for multichar symbols, where 0 means epsilon); a trailing escape at end of string copies the terminating NUL as the "escaped" character, truncating the result there.

> [spec:foma:def:lexcread.lexc-eq-paths-fn]
> int lexc_eq_paths(struct states *one, struct states *two)

> [spec:foma:sem:lexcread.lexc-eq-paths-fn]
> Tests whether two suffix chains are identical. Loop while BOTH states have lexstate==NULL: compare only the head transition of each (one->trans->in vs two->trans->in, and likewise out); on any mismatch return 0; otherwise advance both to their head transition's target. When the loop exits (at least one state is a lexicon state), return 1 iff one->lexstate == two->lexstate (pointer equality — both chains end in the same lexicon state; if only one side has arrived, the other's NULL lexstate compares unequal, giving 0).
> Assumes each non-lexicon state on the chains has a transition and that the head transition is the entire path — true for mergeable trie-suffix states, which have exactly one outgoing transition (states that gained a second continuation were demoted to mergeable=0 in `[spec:foma:sem:lexcread.lexc-add-word-fn]`). Dereferences trans without a NULL check. The caller only compares states of equal `distance`, so both sides reach their lexstate simultaneously.

> [spec:foma:def:lexcread.lexc-find-delim-fn]
> char *lexc_find_delim(char *name, char delimiter, char escape)

> [spec:foma:sem:lexcread.lexc-find-delim-fn]
> Returns a pointer to the first unescaped occurrence of `delimiter` in NUL-terminated `name`, or NULL if none. Byte scan: when the current byte equals `escape` and the following byte is not NUL, skip both (an escape protects any following byte, including another escape or the delimiter); a lone escape as the final byte does not skip. Used to split entry words at ':' with '%' escaping.

> [spec:foma:def:lexcread.lexc-find-lex-state-fn]
> struct states *lexc_find_lex_state(char *name)

> [spec:foma:sem:lexcread.lexc-find-lex-state-fn]
> Linear search of the file-static `lexstates` list for a lexicon whose name strcmp-equals `name`; returns that lexicon's state pointer, or NULL if no such lexicon has been created. Exported via lexc.h but has no callers in the C tree (dead API).

> [spec:foma:def:lexcread.lexc-find-mc-fn]
> int lexc_find_mc(char *symbol)

> [spec:foma:sem:lexcread.lexc-find-mc-fn]
> Linear scan of the file-static `mc` multichar-symbol list; returns 1 if some node's symbol strcmp-equals `symbol`, else 0. Used by `[spec:foma:sem:lexcread.lexc-add-mc-fn]` to suppress duplicate registration.

> [spec:foma:def:lexcread.lexc-find-sigma-hash-fn]
> int lexc_find_sigma_hash(char *symbol)

> [spec:foma:sem:lexcread.lexc-find-sigma-hash-fn]
> Looks up `symbol` in the sigma hashtable. Bucket = `[spec:foma:sem:lexcread.lexc-symbol-hash-fn]`(symbol). If the bucket head's symbol is NULL, return -1 immediately (chained nodes behind an empty head would be unreachable, but the insert routine always fills the head first, so none exist). Otherwise walk the chain starting at the head and return the sigma_number of the first strcmp match; -1 if the chain is exhausted.

> [spec:foma:def:lexcread.lexc-hashtable]
> struct lexc_hashtable {
>   char *symbol;
>   struct lexc_hashtable *next;
>   int sigma_number;
> }

> [spec:foma:def:lexcread.lexc-init-fn]
> void lexc_init()

> [spec:foma:sem:lexcread.lexc-init-fn]
> Resets all file-static compiler state for a fresh lexc compile (called once per fsm_lexc_parse_string, before lexing): lexsigma = sigma_create() (fresh empty sigma); mc, lexstates, clexicon, ctarget, statelist = NULL; lexc_statecount = 0; net_has_unknown = 0; clears the current word via `[spec:foma:sem:lexcread.lexc-clear-current-word-fn]`; hashtable = calloc of SIGMA_HASH_TABLESIZE=3079 `struct lexc_hashtable` bucket heads, each then explicitly set to {symbol=NULL, sigma_number=-1, next=NULL}; maxlen = 0; mchash = calloc(256*256, sizeof(_Bool)) — the all-false first-two-bytes multichar filter.
> Does not free structures from a previous run (that is `[spec:foma:sem:lexcread.lexc-cleanup-fn]`'s job, invoked from `[spec:foma:sem:lexcread.lexc-to-fsm-fn]`); calling lexc_init twice without an intervening cleanup leaks the old tables. current_regex_network is not reset.

> [spec:foma:def:lexcread.lexc-medpad-fn]
> void lexc_medpad()

> [spec:foma:sem:lexcread.lexc-medpad-fn]
> Minimum-edit-distance alignment of cwordin/cwordout (used instead of `[spec:foma:sem:lexcread.lexc-pad-fn]` when the g_lexc_align global is set). If both arrays are empty (first element -1), set both to a single EPSILON, -1-terminate, and return.
> Otherwise first compact both arrays in place, deleting every EPSILON (0) token (these came from explicit `0` characters in the source; the aligner re-derives epsilon positions itself). Compute s1len (input tokens) and s2len (output tokens).
> Dynamic program over calloc'd (s1len+2) x (s2len+2) int matrices `matrix` (cost) and `dirmatrix` (backpointer): matrix[x][0]=x with direction LEV_LEFT (1) for x in 1..s1len; matrix[0][y]=y with direction LEV_DOWN (0); interior cell (x,y): diag = matrix[x-1][y-1] + (cwordin[x-1]==cwordout[y-1] ? 0 : 100), down = matrix[x][y-1]+1, left = matrix[x-1][y]+1; pick diag when diag<=left && diag<=down (LEV_DIAG=2), else left when left<=diag && left<=down (LEV_LEFT), else down (LEV_DOWN). Substitution cost 100 effectively forbids pairing unequal symbols; insertion/deletion cost 1.
> Backtrace from (s1len, s2len) while x>0 || y>0, emitting pairs (reversed) into the file-static scratch arrays medcwordin/medcwordout (2000 ints each): DIAG emits (cwordin[x-1], cwordout[y-1]) and decrements both; DOWN emits (EPSILON, cwordout[y-1]) and decrements y; LEFT emits (cwordin[x-1], EPSILON) and decrements x. Copy the scratch pairs back into cwordin/cwordout in reverse (correct) order, -1-terminate both, and free the matrices.

> [spec:foma:def:lexcread.lexc-merge-states-fn]
> void lexc_merge_states()

> [spec:foma:sem:lexcread.lexc-merge-states-fn]
> Suffix merging: coalesces identical word tails (single-transition chains ending in the same lexicon state), shrinking the raw trie before FSM conversion. Builds two indexes over all states with mergeable==1: `lenlist`, an array of maxlen+1 inline bucket heads keyed by state->distance (remaining suffix length), and `hashstates`, a chained hash table of `tablesize` inline bucket heads keyed by suffix hash, where tablesize is the first entry of the static prime table {61,127,251,509,1021,...,2147483647} (26 roughly-doubling primes) that is >= numstates/4 (numstates = count of mergeable states). Each mergeable state's hashval (raw value from `[spec:foma:sem:lexcread.lexc-suffix-hash-fn]`) is reduced modulo tablesize in place, then the state is inserted into both indexes: an empty head cell is filled directly, otherwise a calloc'd cell is spliced in right after the head.
> Merge pass: for distance i from maxlen down to 1, for each state in lenlist[i] (stop at an empty head cell; skip entries whose mergeable != 1, i.e. already merged away): let `state` be the survivor; scan its hashstates bucket for any other state with mergeable==1, equal distance, and `[spec:foma:sem:lexcread.lexc-eq-paths-fn]` true; each such loser gets merge_with = state, and every state on the loser's chain (itself, then trans->target repeatedly until a state with lexstate != NULL) is marked mergeable=2 (deleted). Longest-first order deletes whole tails at once; interior states of deleted chains are skipped later because their mergeable is 2.
> Rewrite pass over every statelist cell (deleted ones included): redirect each transition's target through target->merge_with. For deleted states (mergeable==2) the transition cells themselves are freed (each one iteration deferred via a prev pointer, plus a final free after the loop); for the surviving states, each transition targeting a lexicon state sets that lexstate's targeted=1 (consumed by the "defined but not used" warning in `[spec:foma:sem:lexcread.lexc-number-states-fn]`).
> Removal pass: walk statelist unlinking and freeing cells (and their states) whose mergeable==2. Latent quirk (kept, benign): when the deleted cell is the list head the code sets `statelist = s` (the removed cell) instead of s->next, so one deleted cell survives as the list head. In the arena model this is not a memory hazard — the stray deleted state has no incoming arcs (they were redirected through merge_with), so `[spec:foma:sem:lexcread.lexc-to-fsm-fn]` emits it as an unreachable component that determinization/minimization prune; output is unaffected.
> Cleanup: free the chained (non-head) cells of both indexes, then free(hashstates) and free(lenlist). The lenlist chain-freeing loop runs i = 0..maxlen-1 over an array of maxlen+1 buckets, so chained cells in bucket [maxlen] leak (off-by-one); the hashstates loop covers all tablesize buckets.

> [spec:foma:def:lexcread.lexc-number-states-fn]
> void lexc_number_states()

> [spec:foma:sem:lexcread.lexc-number-states-fn+1]
> Assigns integer numbers to all surviving states, marks start/final flags on their statelist cells, sets the globals hasfinal and lexc_statecount, and prints warnings. Runs after `[spec:foma:sem:lexcread.lexc-merge-states-fn]`.
> 1. Root: first set smax = the total number of statelist cells (a full pass). Then scan statelist from the head; the first state whose lexstate name is "Root" gets number 0, cell start=1, n=1, and that scan stops. (smax is the true state count. The C incremented smax inside the Root scan and stopped at Root, so smax was only Root's 1-based position, not the state count — see the fix note below.) If there is no Root lexicon, continue a fresh scan to the LAST cell (= the first state ever created = the first lexicon mentioned in the file), give it number 0 and start=1 (n=1), and if g_verbose print to stderr (fflushed): `*Warning: no Root lexicon, using '%s' as Root.\n` with that lexicon's name.
> 2. Finals: the state of lexicon "#", if any, gets number smax-1, cell final=1, and global hasfinal=1. Every other lexicon state whose has_outgoing==0 (used as a continuation target but never defined by a LEXICON header) also gets final=1 — dead ends are made final; the "used but never defined" warning below covers them.
> 3. Every state still numbered -1 receives consecutive numbers n, n+1, ... in statelist order (reverse creation order). Then lexc_statecount = n+1.
> 4. Warnings, per lexstates node, stderr + fflush, only when g_verbose: if targeted==0 and its state's number != 0: `*Warning: lexicon '%s' defined but not used\n`; if has_outgoing==0 and the name is not "#": `***Warning: lexicon '%s' used but never defined\n`.
> "#" gets the highest number (smax-1 = statecount-1) because smax is now the total state count. The C computed smax as Root's list position (= 1 + number of states created after Root's state), which equalled the total count only when Root's state was the very first created (Root the first lexicon name encountered); otherwise "#" collided with a number assigned sequentially in step 3, corrupting the number-indexed array built by `[spec:foma:sem:lexcread.lexc-to-fsm-fn]`.

> [spec:foma:def:lexcread.lexc-pad-fn]
> void lexc_pad()

> [spec:foma:sem:lexcread.lexc-pad-fn]
> Tail-pads the shorter of cwordin/cwordout with EPSILON so both token arrays have equal length (default alignment when g_lexc_align is off). If both are empty (first element -1): set both to a single EPSILON with -1 terminator and return.
> Otherwise scan index i upward with a pad state initially 0. At each position: if pad==1 and cwordout[i]==-1, write cwordin[i]=-1 and stop; if pad==2 and cwordin[i]==-1, write cwordout[i]=-1 and stop; if cwordin[i]==-1 while cwordout[i]!=-1, enter pad=1 (pad the input side); if the reverse, pad=2 (pad the output side); while pad==1 write cwordin[i]=EPSILON, while pad==2 write cwordout[i]=EPSILON (the first EPSILON overwrites the shorter side's former -1 terminator); if pad is still 0 and cwordin[i]==-1 (equal lengths), stop with no change.

> [spec:foma:def:lexcread.lexc-set-current-lexicon-fn]
> void lexc_set_current_lexicon(char *name, int which)

> [spec:foma:sem:lexcread.lexc-set-current-lexicon-fn]
> Sets the current source lexicon (which==0, from a `LEXICON Name` header) or target lexicon (which==1, from an entry's continuation class), creating the lexicon on first mention. Search the file-static lexstates list by strcmp on name: if found, set clexicon=it plus has_outgoing=1 when which==0, or ctarget=it when which==1, and return.
> If not found: prepend a malloc'd lexstates node {name=strdup(name), targeted=0, has_outgoing=0}; malloc its state, register it via `[spec:foma:sem:lexcread.lexc-add-state-fn]`, then set the state's lexstate=node, trans=NULL, mergeable=0, merge_with=itself (hashval and distance are left uninitialized — never read because mergeable==0); node->state = state. Finally clexicon=node with has_outgoing=1 (which==0) or ctarget=node (which==1).
> Consequences: a duplicate `LEXICON Foo` block reuses the same state, so entries simply accumulate; "#" is an ordinary name here whose state is created on first use as a target, receiving its end-of-word treatment only in `[spec:foma:sem:lexcread.lexc-number-states-fn]`; an undefined continuation class is a lexicon whose has_outgoing stays 0.

> [spec:foma:def:lexcread.lexc-set-current-word-fn]
> void lexc_set_current_word(char *name)

> [spec:foma:sem:lexcread.lexc-set-current-word-fn]
> Parses one entry's word part `name` (mutated in place) into the token buffers cwordin/cwordout. Set carity=1. Locate an unescaped ':' via `[spec:foma:sem:lexcread.lexc-find-delim-fn]`(name, ':', '%'); if found, split in place (the ':' byte becomes NUL): the prefix is the upper/input string, the suffix the lower/output string; de-escape the lower string with `[spec:foma:sem:lexcread.lexc-deescape-string-fn]`('%', mode 1) and set carity=2. De-escape the upper string the same way (mode 1: '%'-escapes resolved; unescaped '0' becomes the 0xff epsilon marker).
> Tokenize the upper string into cwordin via `[spec:foma:sem:lexcread.lexc-string-to-tokens-fn]`. If carity==2: tokenize the lower string into cwordout, then align — `[spec:foma:sem:lexcread.lexc-medpad-fn]` when the g_lexc_align global is set, else `[spec:foma:sem:lexcread.lexc-pad-fn]`. If carity==1: copy cwordin into cwordout up to and including the -1 terminator (identity mapping). Finally set current_entry = WORD_ENTRY.

> [spec:foma:def:lexcread.lexc-set-network-fn]
> void lexc_set_network(struct fsm *net)

> [spec:foma:sem:lexcread.lexc-set-network-fn]
> Stores `net` in the file-static current_regex_network and sets current_entry = REGEX_ENTRY (2), so the next `[spec:foma:sem:lexcread.lexc-add-word-fn]` call splices this network between the source and target lexicon states (via `[spec:foma:sem:lexcread.lexc-add-network-fn]`) instead of adding a word path. Called by the lexc lexer after successfully parsing a `< regex >` entry. The network is later mutated in place (arc labels renumbered) but never freed by the lexc machinery.

> [spec:foma:def:lexcread.lexc-string-to-tokens-fn]
> void lexc_string_to_tokens(char *string, int *intarr)

> [spec:foma:sem:lexcread.lexc-string-to-tokens-fn+1]
> Tokenizes UTF-8 `string` into a -1-terminated array of sigma numbers written to intarr, extending lexsigma with unseen symbols. len = strlen(string); scan byte offset i from 0 while i < len:
> - Byte 0xff (the alignment-epsilon marker produced by `[spec:foma:sem:lexcread.lexc-deescape-string-fn]` mode 1 for unescaped '0') emits EPSILON (0) and advances 1 byte.
> - Multichar attempt: if at least two bytes remain (i < len-1) and the mchash filter bit at (unsigned byte i)*256 + (unsigned byte i+1) is set, linearly scan the mc list (sorted decreasing by UTF-8 length, so the longest symbol wins) for the first whose symbol is a byte prefix of string+i (strncmp over the symbol's strlen); on a hit, emit its sigma_number and advance by the symbol's byte length.
> - Otherwise consume one UTF-8 character: skip = utf8skip(string+i) continuation bytes (`[spec:foma:sem:utf8.utf8skip-fn]`); copy skip+1 bytes into a 5-byte local buffer via `[spec:foma:sem:lexcread.mystrncpy-fn]`; look it up with `[spec:foma:sem:lexcread.lexc-find-sigma-hash-fn]`; on a miss, sigma_add it to lexsigma and register the number via `[spec:foma:sem:lexcread.lexc-add-sigma-hash-fn]`. Emit the number and advance skip+1 bytes.
> Intarr is a growable Vec (cwordin/cwordout/medcwordin/medcwordout are Vecs, grown on write). The C wrote into fixed 1000/2000-int arrays, so an entry side of 1000+ tokens overflowed and corrupted adjacent memory (latent bug). Malformed UTF-8 (utf8skip == -1) made skip+1 == 0 so the copy yielded an empty string and i never advanced — an infinite loop that then overflowed the buffer (latent bug); the port clamps skip to 0 when utf8skip returns -1, consuming the offending byte as a single-byte symbol so tokenization makes progress and terminates.

> [spec:foma:def:lexcread.lexc-suffix-hash-fn]
> static unsigned int lexc_suffix_hash(int offset)

> [spec:foma:sem:lexcread.lexc-suffix-hash-fn]
> Hashes the remaining suffix of the current word starting at token index `offset`: h = 0; for each position p from offset until cwordin[p] == -1, h = (h << 4) + (unsigned)(cwordin[p] | (cwordout[p] << 8)); then if g = h & 0xf0000000 is nonzero, h ^= g >> 24 and h ^= g (PJW-style overflow fold). Both sides of each pair contribute (the output symbol shifted into bits 8+), so suffixes differing only on the output side hash differently. Returns the raw 32-bit value with no table modulus — the table size is chosen later by `[spec:foma:sem:lexcread.lexc-merge-states-fn]`, which reduces stored hashvals in place.

> [spec:foma:def:lexcread.lexc-symbol-hash-fn]
> static unsigned int lexc_symbol_hash(char *s)

> [spec:foma:sem:lexcread.lexc-symbol-hash-fn]
> djb2 string hash reduced to a sigma-hashtable bucket index: hash = 5381; for each byte c of NUL-terminated s, hash = hash*33 + c (computed as (hash<<5)+hash+c, unsigned 32-bit wraparound); returns hash % SIGMA_HASH_TABLESIZE (3079).

> [spec:foma:def:lexcread.lexc-to-fsm-fn]
> struct fsm *lexc_to_fsm()

> [spec:foma:sem:lexcread.lexc-to-fsm-fn]
> Final compilation driver: converts the accumulated lexc graph to a struct fsm and returns it determinized, minimized and topologically sorted. If g_verbose, prints "Building lexicon...\n" to stderr (fflushed; likewise for every message below). Calls `[spec:foma:sem:lexcread.lexc-merge-states-fn]`; net = fsm_create(""); frees net->sigma and substitutes lexsigma (ownership transfer); calls `[spec:foma:sem:lexcread.lexc-number-states-fn]`.
> If the global hasfinal is 0 ("#" never appears): warn (g_verbose only) `Warning: # is never reached!!!\n` on stderr and return fsm_empty_set(). On this path the freshly created net, the whole state graph and the hash tables leak — lexc_cleanup is not called.
> Otherwise: sa = malloc(lexc_statecount * sizeof(struct statelist)) indexed by state number, filled from each surviving statelist cell with {state, start, final} (with the "#" numbering collision fixed in `[spec:foma:sem:lexcread.lexc-number-states-fn]` every state number is distinct, so each entry is written exactly once); linecount = number of states + total transitions; fsm = malloc((linecount+1) fsm_state lines). For each state number j in ascending order: a state with no transitions emits one line (state_no=number, in=out=target=-1, final, start); otherwise one line per transition {number, in, out, target->number, final, start} in list (reverse-insertion) order. Append the all -1 terminator line via add_fsm_arc.
> net->states = fsm; net->statecount = lexc_statecount; fsm_update_flags(net, all six flags UNK). If EPSILON (0) is absent from lexsigma (sigma_find_number == -1), sigma_add_special(EPSILON, lexsigma) — epsilon arcs were created structurally without registering the symbol. Then `free(s)` where s is the exhausted list cursor (NULL) — a no-op, so the sa array leaks (latent bug: free(sa) was intended). Call `[spec:foma:sem:lexcread.lexc-cleanup-fn]`, sigma_cleanup(net, 0), sigma_sort(net).
> Finish: verbose "Determinizing...\n"; net = fsm_determinize(net); verbose "Minimizing...\n"; net = fsm_topsort(fsm_minimize(net)); verbose "Done!\n"; return net.

> [spec:foma:def:lexcread.lexc-update-unknowns-fn]
> void lexc_update_unknowns(int sigma_number)

> [spec:foma:sem:lexcread.lexc-update-unknowns-fn]
> Invoked from `[spec:foma:sem:lexcread.lexc-add-sigma-hash-fn]` for every symbol added to sigma after some spliced `< regex >` contained unknown/identity symbols (global net_has_unknown == 1). For each live state in statelist (states with mergeable == 2 are skipped) and each of its transitions with in == IDENTITY (2) or out == IDENTITY: malloc a new transition with in = out = sigma_number and the same target, inserted immediately after the identity transition — keeping @-arcs equivalent to "any symbol, including symbols introduced later". The ongoing iteration visits the inserted arc next but does not recurse (its labels are the new symbol, not IDENTITY).
> One-sided unknown arcs (?:x, x:?) are not patched — flagged TODO in the source, a latent gap. Cost is O(all transitions) per newly added symbol.

> [spec:foma:def:lexcread.lexstates]
> struct lexstates {
>   char *name;
>   struct states *state;
>   struct lexstates *next;
>   unsigned char targeted;
>   unsigned char has_outgoing;
> }

> [spec:foma:def:lexcread.multichar-symbols]
> struct multichar_symbols {
>   char *symbol;
>   short int sigma_number;
>   struct multichar_symbols *next;
> }

> [spec:foma:def:lexcread.mystrncpy-fn]
> char *mystrncpy(char *dest, char *src, int len)

> [spec:foma:sem:lexcread.mystrncpy-fn]
> Copies up to `len` bytes from src to dest; if a NUL byte is copied it returns immediately (dest terminated there). Otherwise writes dest[len] = '\0' — i.e. writes len+1 bytes in total. Unlike strncpy it always NUL-terminates and never pads; returns dest.

> [spec:foma:def:lexcread.statelist]
> struct statelist {
>   struct states *state;
>   struct statelist *next;
>   char start;
>   char final;
> }

> [spec:foma:def:lexcread.states]
> struct states {
>   struct trans { short int in; short int out; struct states *target; struct trans *next; } *trans;
>   struct lexstates *lexstate;
>   int number;
>   unsigned int hashval;
>   unsigned char mergeable;
>   unsigned short int distance;
>   struct states *merge_with;
> }

> [spec:foma:def:lexcread.states.trans]
> struct trans {
>   short int in;
>   short int out;
>   struct states *target;
>   struct trans *next;
> }


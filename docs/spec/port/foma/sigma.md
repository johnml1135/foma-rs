# foma/sigma.c

> [spec:foma:def:sigma.sigma-add-fn]
> int sigma_add (char *symbol, struct sigma *sigma)

> [spec:foma:sem:sigma.sigma-add-fn]
> Adds `symbol` to the sigma linked list (nodes `{int number; char *symbol; struct sigma *next}`; the empty sigma is a single sentinel node with number=-1, symbol=NULL) and returns the number assigned. First classify: if `symbol` string-equals "@_EPSILON_SYMBOL_@" the target number is EPSILON=0; "@_IDENTITY_SYMBOL_@" → IDENTITY=2; "@_UNKNOWN_SYMBOL_@" → UNKNOWN=1; otherwise non-special.
> Non-special path: if the head is the empty sentinel (number==-1), overwrite it in place: number=3, next=NULL, symbol=strdup(symbol); return 3. Otherwise walk to the tail node (next==NULL), append a fresh malloc'd node whose number is tail->number+1, clamped up to 3 if tail->number+1 < 3; set its next=NULL and symbol=strdup(symbol); return that number. Note the number comes from the tail node, not the list maximum — callers must keep the list sorted ascending by number for this to be unique.
> Special path: if head is the empty sentinel, set head->number to the special number, head->next=NULL, head->symbol=strdup(symbol); return it. Otherwise scan from the head while node!=NULL, node->number < special-number, and node->number != -1, tracking the previous node; then malloc a splice node. If a previous node exists, link previous->next=splice, splice={number=special, symbol=fresh copy of `symbol`, next=current scan node (possibly NULL)}. If insertion is at the head (previous==NULL), instead copy the head's {symbol,number,next} into the splice node, then overwrite the head with {number=special, symbol=fresh copy, next=splice} — the head pointer never changes. Return the special number.
> No duplicate check in either path: adding an existing symbol creates a second entry. The input `symbol` is never consumed; sigma owns freshly allocated copies. Caller must re-sort sigma (see `[spec:foma:sem:sigma.sigma-sort-fn]`) before any sigma merge if non-special insertion broke ordering.

> [spec:foma:def:sigma.sigma-add-number-fn]
> int sigma_add_number(struct sigma *sigma, char *symbol, int number)

> [spec:foma:sem:sigma.sigma-add-number-fn]
> Adds `symbol` with an explicitly chosen `number`. If the head is the empty sentinel (number==-1), fill it in place: symbol=strdup(symbol), number=number, next=NULL. Otherwise walk to the tail and append a malloc'd node {strdup(symbol), number, next=NULL}. Always returns 1. No sorting, no duplicate checking; symbol string is copied, input not consumed.

> [spec:foma:def:sigma.sigma-add-special-fn]
> int sigma_add_special (int symbol, struct sigma *sigma)

> [spec:foma:sem:sigma.sigma-add-special-fn+1]
> Inserts one of the three special symbols by numeric code, keeping the list sorted by number. Maps code→string: EPSILON=0→"@_EPSILON_SYMBOL_@", IDENTITY=2→"@_IDENTITY_SYMBOL_@", UNKNOWN=1→"@_UNKNOWN_SYMBOL_@". A non-reserved code yields a well-formed placeholder "@_SPECIAL_<code>_@". The C source left the string NULL for any other code, so a symbol-less node was inserted and later crashed when read back.
> If head is the empty sentinel (number==-1): set head->number=code, head->next=NULL, head->symbol=str; return code. Otherwise scan while node!=NULL && node->number < code && node->number != -1, tracking previous; malloc a splice node. If previous exists: previous->next=splice, splice={number=code, symbol=str, next=scan node}. If inserting at head: copy head's fields into splice, overwrite head with {number=code, symbol=str, next=splice}. Return code.
> Same as the special path of `[spec:foma:sem:sigma.sigma-add-fn]` but keyed by number. No duplicate check: if the code is already present the new node is inserted before the existing equal-numbered node.

> [spec:foma:def:sigma.sigma-cleanup-fn]
> void sigma_cleanup (struct fsm *net, int force)

> [spec:foma:sem:sigma.sigma-cleanup-fn]
> Removes symbols never used on any arc from net->sigma and renumbers the remainder consecutively; rewrites arc labels to match. If force==0, returns immediately (no-op) when sigma contains IDENTITY or UNKNOWN (checked via `[spec:foma:sem:sigma.sigma-find-number-fn]`); force==1 always proceeds.
> Steps: maxsigma = sigma_max(net->sigma); if < 0 return. Allocate int array `attested[0..maxsigma]` zeroed. Scan net->states lines (array of fsm_state terminated by state_no==-1): for each line set attested[in]=1 if in>=0 and attested[out]=1 if out>=0. Then build the renumbering inside the same array: with j starting at 3, for i=3..maxsigma in order, if attested[i] nonzero set attested[i]=j and j++ (attested entries thus hold their new number, ≥3; unattested stay 0; indices 0–2 keep the 0/1 mark). Second pass over all lines: if in>2 replace in with attested[in]; likewise out.
> Then walk the sigma list (stop at NULL or a node with number==-1): if attested[node->number]==0, free the node's symbol and the node and unlink it (updating net->sigma when removing the head); otherwise, if node->number>=3, set node->number=attested[node->number] (numbers 0–2 are kept as-is). Free the attested array. Note EPSILON/UNKNOWN/IDENTITY entries (numbers 0–2) are also removed if no arc uses them.

> [spec:foma:def:sigma.sigma-copy-fn]
> struct sigma *sigma_copy(struct sigma *sigma)

> [spec:foma:sem:sigma.sigma-copy-fn]
> Deep-copies a sigma list. NULL input → NULL. Otherwise allocates a new head node and copies every node in order (including a trailing empty sentinel if present): number copied, symbol strdup'd (NULL symbol stays NULL), last node's next=NULL. Returns the new head; the source list is untouched.

> [spec:foma:def:sigma.sigma-create-fn]
> struct sigma *sigma_create()

> [spec:foma:sem:sigma.sigma-create-fn]
> Allocates and returns a single-node empty sigma sentinel: number=-1, symbol=NULL, next=NULL.

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

> [spec:foma:sem:sigma.sigma-max-fn]
> Returns the maximum node->number over the whole list (sentinel's -1 included in the max), starting the accumulator at -1; NULL sigma → -1. Hence an empty sigma (single sentinel) also yields -1.

> [spec:foma:def:sigma.sigma-remove-fn]
> struct sigma *sigma_remove(char *symbol, struct sigma *sigma)

> [spec:foma:sem:sigma.sigma-remove-fn]
> Removes the first node whose symbol string-equals `symbol` and returns the (possibly new) list head. Scans until NULL or a node with number==-1 (an empty-sentinel head is never matched or removed). On match: free the node's symbol and the node; if it was the head, the returned head is its next, otherwise relink previous->next=node->next; stop after the first match. If not found, list is unchanged and the original head returned.

> [spec:foma:def:sigma.sigma-remove-num-fn]
> struct sigma *sigma_remove_num(int num, struct sigma *sigma)

> [spec:foma:sem:sigma.sigma-remove-num-fn]
> Identical to `[spec:foma:sem:sigma.sigma-remove-fn]` but matches on node->number == num instead of the symbol string: removes and frees the first matching node (symbol and node), relinks or advances the head, stops scanning at NULL or number==-1, returns the resulting head.

> [spec:foma:def:sigma.sigma-size-fn]
> int sigma_size(struct sigma *sigma)

> [spec:foma:sem:sigma.sigma-size-fn]
> Returns the raw node count of the list (NULL → 0). The empty sentinel counts as a node, so an empty sigma returns 1.

> [spec:foma:def:sigma.sigma-sort-fn]
> int sigma_sort(struct fsm *net)

> [spec:foma:sem:sigma.sigma-sort-fn+1]
> Sorts the non-special part of net->sigma alphabetically by symbol string (strcmp/byte order) and renumbers those symbols consecutively from 3, rewriting arc labels accordingly. Returns 1 always (also when sigma is empty: if sigma_max < 0, return 1 immediately with no work).
> Steps: let size = sigma_max(net->sigma). Allocate an array of `size` {symbol,number} pairs (struct ssort). Walk the sigma list collecting every node with number > IDENTITY (2): store its symbol pointer and number; let max = count collected. qsort that array with `[spec:foma:sem:sigma.ssortcmp-fn]`. Build a replacearray of size+3 entries; the C left slots for numbers absent from sigma uninitialized (garbage), corrupting any arc labelled with a missing number. Seed the array with the identity map (replacearray[k]=k) so an absent label is left unchanged, then for sorted index i in 0..max-1 set replacearray[oldnumber]=i+3. Rewrite arcs: for each line of net->states (until state_no==-1), if in>IDENTITY set in=replacearray[in]; likewise out. Rewrite sigma: walk the list again in order with counter i=0; for each node with number>IDENTITY assign node->number=i+3 and node->symbol=sorted[i].symbol (pointer move, no copy/free — symbols are permuted among existing nodes), i++.
> Net effect: special nodes (numbers 0–2) keep position/number; the k-th non-special node in list order receives the k-th alphabetically-smallest symbol and number k+3.

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


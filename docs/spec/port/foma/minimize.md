# foma/minimize.c

> [spec:foma:def:minimize.agenda]
> struct agenda {
>   struct p *p;
>   struct agenda *next;
>   _Bool index;
> }

> [spec:foma:def:minimize.agenda-add-fn]
> static void agenda_add(struct p *pptr, int start)

> [spec:foma:sem:minimize.agenda-add-fn]
> Pushes block pptr onto the Hopcroft splitter agenda (LIFO). Take the next struct agenda
> from the preallocated pool by bumping the file-static Agenda_next cursor (pool of
> 2*num_states entries calloc'd in init_PE; no bounds check — capacity suffices because
> each of the at most num_states-2 splits performs at most two adds, plus the two initial
> entries). Set ag->next = the current Agenda list head (NULL if Agenda is NULL),
> ag->p = pptr, ag->index = start; then Agenda = ag and pptr->agenda = ag (back-pointer
> used to test "is this block scheduled" and to reach its entry). Because the main loop's
> cursor variable is Agenda itself, a block added mid-iteration is processed next.
> `start` semantics: 0 = process the block's symbol sweep from the lowest symbol with all
> member inverse-list tails reset; 1 = resume from the members' current tail positions
> (continuation of an interrupted sweep).

> [spec:foma:def:minimize.e]
> struct e {
>   struct p *group;
>   struct e *left;
>   struct e *right;
>   int inv_count;
> }

> [spec:foma:def:minimize.fsm-minimize-brz-fn]
> static struct fsm *fsm_minimize_brz(struct fsm *net)

> [spec:foma:sem:minimize.fsm-minimize-brz-fn]
> Brzozowski minimization in one line: return
> fsm_determinize(fsm_reverse(fsm_determinize(fsm_reverse(net)))). Each stage consumes
> its argument. Reverse the machine, determinize the reversal (yielding the minimal-DFA
> core of the reverse language up to reachability), reverse again, determinize again; the
> result is a minimal deterministic machine for the original relation. Relies on
> `[spec:foma:sem:determinize.fsm-determinize-fn]` handling epsilon arcs introduced by
> reversal and returning already-deterministic inputs untouched. Does not itself set any
> flags — the caller fsm_minimize marks the result.

> [spec:foma:def:minimize.fsm-minimize-fn]
> struct fsm *fsm_minimize(struct fsm *net)

> [spec:foma:sem:minimize.fsm-minimize-fn]
> Public entry point. If net == NULL return NULL. Preconditions are established by
> chained consuming calls: if net->is_deterministic != YES, net = fsm_determinize(net);
> if net->is_pruned != YES, net = fsm_coaccessible(net) (trim dead states). Then, only if
> net->is_minimized != YES and the global g_minimal == 1: dispatch on the global
> g_minimize_hopcroft — nonzero selects fsm_minimize_hop, zero selects fsm_minimize_brz —
> and afterwards call fsm_update_flags(net, YES, YES, YES, YES, UNK, UNK), marking the
> result deterministic, pruned, minimized, and epsilon-free while resetting loop-free and
> completed to unknown. Return net. With g_minimal == 0 or an already-minimized input,
> only the determinize/trim normalization happens.

> [spec:foma:def:minimize.fsm-minimize-hop-fn]
> static struct fsm *fsm_minimize_hop(struct fsm *net)

> [spec:foma:sem:minimize.fsm-minimize-hop-fn]
> Hopcroft partition-refinement minimization; mutates net in place (callers rely on
> this), except for the empty-language case. Steps: (1) fsm_count(net); if
> net->finalcount == 0, fsm_destroy(net) and return fsm_empty_set() — a fresh canonical
> empty machine. (2) num_states = statecount; P = NULL; sigma_to_pairs(net) builds the
> composite alphabet plus the finals bitmap/num_finals and sets net->arity; init_PE()
> builds the initial partition {F, Q-F}, the E state array, the block and agenda pools,
> and seeds the agenda; if total_states == num_states already (machine has <= 2 states,
> every block a singleton) jump to cleanup and return net unchanged (inverse index is
> never built). (3) generate_inverse(net) builds per-state inverse transition lists
> sorted by composite symbol plus inverse-degree counters. (4) Clear ->index on the one
> or two initial agenda entries so their sweeps start at symbol 0. (5) Agenda loop with
> cursor Agenda starting at Agenda_head: take current_w = Agenda->p and current_i =
> Agenda->index, set that block's ->agenda = NULL, advance Agenda. Copy current_w's
> member state numbers (walking first_e via ->right) into temp_group (count thissize);
> if current_i == 0 reset every member's inverse-list tail to 0; minsym = the smallest
> inout at any member's current tail. (6) Ascending symbol sweep (minsym/next_minsym
> pattern): for each member, consume inverse entries with inout == minsym, collecting the
> distinct *source* states into temp_move[j++] guarded by memo_table[source] == mainloop
> marking; advance tails; the next unconsumed entry of each member lowers next_minsym.
> If j == 0 continue to the next symbol; else mainloop++ and call refine_states(j); if it
> returns 1 (current_w itself was split), abandon this block's sweep — the two halves
> were re-enqueued with appropriate resume indices. (7) After each block, if
> total_states == num_states break out early (partition fully discrete: input already
> minimal). (8) rebuild_machine(net) collapses each block to one state in place; free
> trans_array_minimize/trans_list_minimize, then (cleanup shared with the bail path) free
> the agenda pool, memo_table, temp_move, temp_group, finals, E, the block pool (Phead),
> and both sigma arrays; return net.

> [spec:foma:def:minimize.generate-inverse-fn]
> static void generate_inverse(struct fsm *net)

> [spec:foma:sem:minimize.generate-inverse-fn]
> Builds the inverse-arc index used for splitter moves. Allocate trans_array_minimize =
> calloc(statecount struct trans_array) and trans_list_minimize = calloc(arccount struct
> trans_list). Pass 1 over all lines with target != -1: increment E[target].inv_count
> (per-state inverse degree), E[target].group->inv_count (per-block inverse mass), and
> trans_array_minimize[target].size. Pass 2: hand each state a contiguous slice of the
> entry pool, in state order, by setting its ->transitions pointer at a running offset
> accumulated from the sizes. Pass 3 over the same lines: append
> {inout = symbol_pair_to_single_symbol(in, out), source = state_no} into the target
> state's slice at position ->tail, incrementing tail — tails are left equal to size here
> and are reset to 0 by the main loop before first use (the initial agenda entries carry
> index 0). Pass 4: qsort each state's slice ascending by inout via trans_sort_cmp. After
> this, "inverse move of block B on symbol a" is computed by scanning each member's
> sorted slice at its tail cursor.

> [spec:foma:def:minimize.init-pe-fn]
> static void init_PE()

> [spec:foma:sem:minimize.init-pe-fn]
> Creates the initial partition (nonfinals, finals), the agenda, and the per-state E
> records. Steps: mainloop = 1; memo_table, temp_move, temp_group = calloc(num_states
> ints each). Block pool: Phead = P = Pnext = calloc(num_states+1 struct p); draw nonFP
> then FP from the pool (Pnext++ each); nonFP->count = num_states - num_finals;
> FP->count = num_finals; zero both blocks' t_count/inv_count/inv_t_count and set
> current_split = NULL (chain pointers set below). Agenda pool: Agenda_top = Agenda_next
> = calloc(num_states*2 struct agenda); Agenda_head = NULL. Reset P = NULL and
> total_states = 0. If num_finals > 0: take an agenda entry for FP (FP->agenda = it,
> it->p = FP, it->next = NULL), make P = FP the block-chain head with P->next = NULL,
> Agenda_head = the entry, total_states++. If num_states - num_finals > 0: take an entry
> for nonFP likewise and total_states++; if finals were enqueued, append the entry after
> Agenda_head and chain nonFP after FP (FP->next = nonFP, nonFP->next = NULL); otherwise
> nonFP becomes both P and Agenda_head. Empty blocks are neither chained nor scheduled.
> Then E = calloc(num_states struct e); for i = 0..num_states-1 in ascending order,
> assign E[i].group to FP or nonFP per finals[i] and weave the block's doubly linked
> member list (left/right pointers) in state-number order, maintaining the block's
> first_e/last_e; E[i].inv_count = 0. Finally NULL-terminate both lists' right pointers.
> total_states ends at the number of nonempty initial blocks (1 or 2); agenda order is
> finals first, then nonfinals.

> [spec:foma:def:minimize.p]
> struct p {
>   struct e *first_e;
>   struct e *last_e;
>   struct p *current_split;
>   struct p *next;
>   struct agenda *agenda;
>   int count;
>   int t_count;
>   int inv_count;
>   int inv_t_count;
> }

> [spec:foma:def:minimize.rebuild-machine-fn]
> static struct fsm *rebuild_machine(struct fsm *net)

> [spec:foma:sem:minimize.rebuild-machine-fn]
> Collapses each partition block to a single state, rewriting net->states in place. If
> net->statecount == total_states, return net unchanged (nothing merged). Force state 0
> to be the representative of its block by assigning E[0].group->first_e = &E[0] — a
> pointer overwrite only; the member list is not re-woven, it merely transfers
> "representative" status (tested as group->first_e == this) from the previous
> representative to state 0 so the start state keeps number 0. Zero ->count on every
> block along the P chain (count is recycled as a "block already numbered" flag and
> t_count as the block's new state number). Pass 1 over the line table: for each line
> whose state is its block's representative, increment new_linecount; if the line has
> start_state == 1, set the block's number t_count = 0 (and count = 1); else on the
> block's first such line assign t_count = group_num++ (group_num starts at 1). Since
> lines are sorted by state number and state 0's lines come first, the start block gets 0
> and the remaining blocks get 1..total_states-1 in order of their representative's first
> appearance. Pass 2: for each representative line, write to the next output offset j
> via add_fsm_arc(fsm, j, source = own block number, in, out, target = -1 if the line's
> target is -1 else the target state's block number, final_state = finals[state_no],
> start_state = the line's flag), incrementing j; count arccount for real targets only.
> In-place compaction is safe because the output offset never exceeds the read offset.
> Append the sentinel line (all fields -1) at offset j; realloc the array to
> new_linecount+1 lines; set net->states, net->linecount = j+1, net->arccount, and
> net->statecount = total_states; return net.

> [spec:foma:def:minimize.refine-states-fn]
> static INLINE int refine_states(int invstates)

> [spec:foma:sem:minimize.refine-states-fn]
> The splitting step. Input: temp_move[0..invstates-1] holds the distinct source states S
> that reach the current splitter block current_w on the current symbol. Returns 1 iff
> current_w itself was split (caller then abandons its sweep), else 0. Pass 1 (touch):
> for each s in S, with tP = E[s].group: tP->t_count++ and
> tP->inv_t_count += E[s].inv_count (asserting t_count <= count). Pass 2 (split), for
> each s in S with thise = &E[s], tP = thise->group: (a) if tP->t_count == tP->count
> (every member touched — no split), reset t_count and inv_t_count to 0 and continue;
> later members of the same block then fail the t_count > 0 test and are no-ops. (b) Else
> if tP->count > 1 and tP->t_count > 0: if tP->current_split == NULL, create the new
> block: total_states++; if total_states == num_states return 1 immediately (partition
> now discrete — counters are left dirty but the caller stops refining); newP = Pnext++
> (drawn from the block pool), tP->current_split = newP; initialize newP: first_e =
> last_e = thise, count = 0, inv_count = tP->inv_t_count, inv_t_count = t_count = 0,
> current_split = NULL, agenda = NULL. Agenda policy at creation: if tP->agenda != NULL
> (tP is scheduled): if tP->inv_count < tP->inv_t_count then agenda_add(newP, 1) and set
> tP->agenda->index = 0, else agenda_add(newP, 0). Else if tP == current_w (splitting the
> block we are splitting with): agenda_add the "smaller" (by the same comparison: tP if
> inv_count < inv_t_count else newP) with index 0, agenda_add the other with index 1 (it
> resumes the interrupted sweep), and set selfsplit = 1. Else: agenda_add the "smaller"
> (same comparison) with index 0. Latent bug to preserve: inv_t_count sums the inv_count
> of a subset of tP's members, so tP->inv_count < tP->inv_t_count is always false and the
> comparisons constant-fold — newP (the touched half) is always the one enqueued with
> index 0, and in the current_w case tP always gets index 1. This is still a correct
> refinement (unconditionally enqueueing the touched half is valid), but the intended
> Hopcroft smaller-half selection never executes as written. Finally link newP into the
> block chain: newP->next = P->next; P->next = newP. (c) For every touched member
> (whether or not it just created newP, with newP = tP->current_split): move thise into
> newP: thise->group = newP; newP->count++; unlink thise from tP's doubly linked list
> (updating tP->last_e/first_e when thise was an end, then splicing left/right
> neighbors); append at newP's tail unless it is the seed element already there
> (if newP->last_e != thise: newP->last_e->right = thise, thise->left = newP->last_e,
> newP->last_e = thise); set thise->right = NULL, and thise->left = NULL when it is
> newP->first_e. (d) When newP->count reaches tP->t_count (all touched members moved),
> finalize: tP->count -= newP->count; tP->inv_count -= tP->inv_t_count;
> tP->current_split = NULL; tP->t_count = 0; tP->inv_t_count = 0. Return selfsplit.

> [spec:foma:def:minimize.sigma-to-pairs-fn]
> static void sigma_to_pairs(struct fsm *net)

> [spec:foma:sem:minimize.sigma-to-pairs-fn]
> Same composite-alphabet construction as `[spec:foma:sem:determinize.sigma-to-pairs-fn]`,
> plus finals bookkeeping. Steps: epsilon_symbol = -1; maxsigma = sigma_max(net->sigma)
> + 1; allocate single_sigma_array (2*maxsigma*maxsigma ints, oversized) and
> double_sigma_array (maxsigma*maxsigma ints) filled with -1. Allocate finals =
> calloc(num_states _Bool) and num_finals = 0. Scan every line: if final_state == 1 and
> finals[state_no] not yet set, set it and increment num_finals (the bitmap guard
> prevents double-counting states spanning multiple lines); if in != out or either equals
> UNKNOWN (1), set net->arity = 2 (checked before the sentinel skip, but -1 sentinel
> values cannot trigger it); skip lines with in == -1 or out == -1; on first occurrence
> of an (in,out) pair assign the next composite number x in first-appearance order:
> double_sigma_array[maxsigma*in + out] = x, single_sigma_array[2x] = in,
> single_sigma_array[2x+1] = out; record epsilon_symbol = x if the pair is
> (EPSILON, EPSILON) (i.e. (0,0) — not expected here since input is deterministic and
> epsilon-free). num_symbols = the count of distinct pairs.

> [spec:foma:def:minimize.state-list]
> struct state_list {
>   int state;
>   struct state_list *next;
> }

> [spec:foma:def:minimize.statesym]
> struct statesym {
>   int target;
>   unsigned short int symbol;
>   struct state_list *states;
>   struct statesym *next;
> }

> [spec:foma:def:minimize.symbol-pair-to-single-symbol-fn]
> static INLINE int symbol_pair_to_single_symbol(int in, int out)

> [spec:foma:sem:minimize.symbol-pair-to-single-symbol-fn]
> Forward map into the composite alphabet: return double_sigma_array[maxsigma*in + out],
> the dense composite symbol assigned by sigma_to_pairs, or -1 for unregistered pairs
> (never passed in practice). The corresponding back-mapping function is compiled out in
> this file (commented prototype) — minimization never needs to recover (in,out) because
> rebuild_machine copies original line labels verbatim.

> [spec:foma:def:minimize.trans-array]
> struct trans_array {
>   struct trans_list *transitions;
>   unsigned int size;
>   unsigned int tail;
> }

> [spec:foma:def:minimize.trans-list]
> struct trans_list {
>   int inout;
>   int source;
> }

> [spec:foma:def:minimize.trans-sort-cmp-fn]
> static int trans_sort_cmp(const void *a, const void *b)

> [spec:foma:sem:minimize.trans-sort-cmp-fn]
> qsort comparator over struct trans_list: returns a->inout - b->inout, sorting inverse
> transition entries ascending by composite symbol; equal-symbol entries keep an
> unspecified relative order. Overflow-safe in practice because composite symbols are
> dense small nonnegative ints.


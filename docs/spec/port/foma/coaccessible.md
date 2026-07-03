# foma/coaccessible.c

> [spec:foma:def:coaccessible.fsm-coaccessible-fn]
> struct fsm *fsm_coaccessible(struct fsm *net)

> [spec:foma:sem:coaccessible.fsm-coaccessible-fn]
> Prunes states from which no final state is reachable, in place; returns `net` with is_pruned=YES. Operates on the fsm_state line array (terminated by state_no==-1).
> Build an inverse-transition table: calloc an array of statecount invtable heads ({int state; struct invtable *next}); initialize every head's state to -1 (empty). For each line with target t != -1 and state_no s != t (self-loops skipped — they cannot affect coaccessibility): if head[t].state==-1 set it to s, else malloc a new invtable node with state=s and insert it right after head[t] (head's next chain). Also allocate int arrays coacc[statecount] and added[statecount], zeroed, and mapping[statecount] (uninitialized).
> Mark phase: for each line whose final_state is set and whose state is not yet marked, push state_no on the global int stack, set coacc=1, markcount++. Then while the stack is non-empty: pop a state, walk its inverse list (stop at NULL or a head with state==-1), and for each unmarked predecessor set coacc=1, push it, markcount++. If at the end of any pop markcount >= statecount, everything is coaccessible: set terminate=1, clear the stack, and skip the rewrite phase entirely (counts unchanged).
> Rewrite phase (terminate==0): build mapping: mapping[0]=0 unconditionally ("state 0 always exists" — latent quirk: if state 0 is NOT coaccessible, surviving states are numbered from 1 and the result has no state 0); for i=1..statecount-1, coaccessible states get consecutive numbers j=1,2,... in increasing old-number order.
> Compact the line array in place with read index i and write index j (j never overtakes i): (a) when the state number changes between line i-1 and line i, and line i-1's state was final but had no line kept (added[prev]==0), emit a synthetic arcless line (mapping[prev_state], in=-1, out=-1, target=-1, final=1, start=prev line's start_state) via add_fsm_arc, j++, new_linecount++, added[prev]=1 — this preserves final states all of whose outgoing arcs were pruned; (b) keep line i iff coacc[state_no] and (target==-1 or coacc[target]): write it at slot j with state_no and target remapped through mapping (target -1 stays -1), copying in/out/final_state/start_state; j++, new_linecount++, added[state]=1, and new_arccount++ if target != -1.
> After the scan (i now indexes the terminator): if i>1 and the last real line's state was final and not yet added, emit the same synthetic final line for it. If new_linecount is still 0, emit a dummy line (0,-1,-1,-1,-1,-1). Write the terminator line (-1 six times) at slot j.
> If markcount==0 (no final states — empty language): free the line array, set net->states=fsm_empty() (the canonical empty machine), destroy net->sigma and replace it with a fresh sigma_create(). Then set net->linecount=new_linecount, net->arccount=new_arccount, net->statecount=markcount.
> Cleanup (both paths): for each state free the malloc'd chain nodes of its inverse list (the array-resident heads are not individually freed), free the head array, coacc, added, mapping. Set net->is_pruned=YES and return net.

> [spec:foma:def:coaccessible.invtable]
> struct invtable {
>   int state;
>   struct invtable *next;
> }


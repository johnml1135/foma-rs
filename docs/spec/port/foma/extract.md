# foma/extract.c

> [spec:foma:def:extract.fsm-lower-fn]
> struct fsm *fsm_lower(struct fsm *net)

> [spec:foma:sem:extract.fsm-lower-fn]
> Projects a transducer onto its lower (output) side, in place; returns `net`. Rebuilds the state array via the fsm_state_* construction API, seeded with fsm_state_init(sigma_max(net->sigma)).
> Iterate the old line array in order (lines grouped by state, terminated by state_no==-1), tracking prevstate (init -1): when the state number changes, call fsm_state_end_state() for the previous state (skipped before the first state) and fsm_state_set_current_state(state_no, final_state, start_state) for the new one. For every line with target != -1, add an arc whose BOTH labels are the line's `out` symbol — except that out==UNKNOWN (1) is mapped to IDENTITY (2), since a lone unknown on one tape becomes an identity pair in an acceptor: fsm_state_add_arc(state_no, out', out', target, final_state, start_state). After the loop, fsm_state_end_state() once more, free the old net->states array, and fsm_state_close(net) to install the newly built array and counts.
> Finally fsm_update_flags(net, deterministic=NO, pruned=NO, minimized=NO, epsilon_free=UNK, loop_free=UNK, completed=UNK) and sigma_cleanup(net, 0) (see `[spec:foma:sem:sigma.sigma-cleanup-fn]`; force=0, so unused symbols are only purged when neither UNKNOWN nor IDENTITY remains in sigma). Epsilon labels (0) on the lower side become epsilon:epsilon arcs; the result is an acceptor (identical in/out on every arc) but not determinized or minimized here.

> [spec:foma:def:extract.fsm-upper-fn]
> struct fsm *fsm_upper(struct fsm *net)

> [spec:foma:sem:extract.fsm-upper-fn]
> Projects a transducer onto its upper (input) side, in place; returns `net`. Identical algorithm to `[spec:foma:sem:extract.fsm-lower-fn]` except each kept arc uses the line's `in` symbol on both sides (with in==UNKNOWN (1) likewise replaced by IDENTITY (2)): fsm_state_add_arc(state_no, in', in', target, final_state, start_state). Same state-grouped traversal, same rebuild via fsm_state_init/set_current_state/end_state/close, same fsm_update_flags(net,NO,NO,NO,UNK,UNK,UNK) and sigma_cleanup(net,0) at the end.


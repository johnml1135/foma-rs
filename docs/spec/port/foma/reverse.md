# foma/reverse.c

> [spec:foma:def:reverse.fsm-reverse-fn]
> struct fsm *fsm_reverse(struct fsm *net)

> [spec:foma:sem:reverse.fsm-reverse-fn]
> Returns a new FSM accepting the reversal of net's language/relation; the input `net` is consumed (fsm_destroy'd). Built via the read/construct handle APIs:
> Open a read handle on `net` and a construct handle named after net->name; copy net->sigma into the new machine (fsm_construct_copy_sigma).
> All original state numbers are shifted up by 1 in the result; a brand-new state 0 is added as the sole initial state.
> For every arc (source, in, out, target) of the input, add the reversed arc by symbol numbers: from state target+1 to state source+1 with the same in/out numbers (label sides are NOT swapped — a transducer's reversal keeps upper/upper, lower/lower).
> For every final state f of the input, add an arc from state 0 to f+1 labeled EPSILON:EPSILON (0:0).
> For every initial state i of the input, mark state i+1 final in the result. Mark state 0 initial.
> Close the read handle, finalize construction (which builds the new struct fsm), then set is_deterministic=0 and is_epsilon_free=0 on the result (the epsilon arcs from state 0 make both unknown/false), destroy the input net, and return the new net.
> Consequence: the result generally has statecount = old statecount + 1, is nondeterministic, and contains one epsilon arc per original final state; sigma is a copy of the original alphabet.


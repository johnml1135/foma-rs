//! foma/minimize.c — literal (bug-for-bug) Wave-2 port per
//! docs/port/rust-conventions.md. Sem rules: docs/spec/port/foma/minimize.md
//! (per-file ids) plus the fomalib.h prototype ids.
//!
//! Hopcroft partition-refinement minimization plus the Brzozowski fallback.
//! The C's pointer pools (struct p / struct e / struct agenda and the
//! inverse-arc trans_array/trans_list index) become index-based pools
//! (Vec + usize indices) with the identical link discipline; NULL ↔ None.
//! A C `struct p *` argument into the block pool decomposes to
//! (pool borrow, index) — see agenda_add. File-static state →
//! thread_local! per the conventions (non-reentrancy is part of the
//! contract, exactly as in C).

use std::cell::{Cell, RefCell};

use crate::coaccessible::fsm_coaccessible;
use crate::constructions::{add_fsm_arc, fsm_count, fsm_update_flags};
use crate::determinize::fsm_determinize;
use crate::mem::{G_MINIMAL, G_MINIMIZE_HOPCROFT};
use crate::reverse::fsm_reverse;
use crate::sigma::sigma_max;
use crate::structures::{fsm_destroy, fsm_empty_set};
use crate::types::{Fsm, EPSILON, UNK, UNKNOWN, YES};

// [spec:foma:def:minimize.statesym]
/* Declared in the C but never used (dead declaration) — kept literally. */
#[derive(Debug, Clone)]
pub struct Statesym {
    pub target: i32,
    pub symbol: u16,
    pub states: Option<Box<StateList>>,
    pub next: Option<Box<Statesym>>,
}

// [spec:foma:def:minimize.state-list]
/* Declared in the C but never used (dead declaration) — kept literally. */
#[derive(Debug, Clone)]
pub struct StateList {
    pub state: i32,
    pub next: Option<Box<StateList>>,
}

// [spec:foma:def:minimize.p]
/* Block of the partition; lives in the PHEAD pool. The C's struct e * /
struct p * / struct agenda * fields are indices into the E / PHEAD /
AGENDA_TOP pools (None ↔ NULL). calloc zero-fill ↔ Default. */
#[derive(Debug, Clone, Default)]
pub struct P {
    pub first_e: Option<usize>,
    pub last_e: Option<usize>,
    pub current_split: Option<usize>,
    pub next: Option<usize>,
    pub agenda: Option<usize>,
    pub count: i32,
    pub t_count: i32,
    pub inv_count: i32,
    pub inv_t_count: i32,
}

// [spec:foma:def:minimize.e]
/* Per-state record; E array index == state number ("temp_E - E" in the C
is the index itself). group is a PHEAD-pool index (calloc NULL ↔ 0 here;
always assigned by init_PE before use); left/right are E indices. */
#[derive(Debug, Clone, Default)]
pub struct E {
    pub group: usize,
    pub left: Option<usize>,
    pub right: Option<usize>,
    pub inv_count: i32,
}

// [spec:foma:def:minimize.agenda]
/* Agenda entry; lives in the AGENDA_TOP pool. p is a PHEAD-pool index
(calloc NULL ↔ 0 here; always assigned before read); next is an
AGENDA_TOP index. */
#[derive(Debug, Clone, Default)]
pub struct Agenda {
    pub p: usize,
    pub next: Option<usize>,
    pub index: bool,
}

// [spec:foma:def:minimize.trans-list]
#[derive(Debug, Clone, Default)]
pub struct TransList {
    pub inout: i32,
    pub source: i32,
}

// [spec:foma:def:minimize.trans-array]
/* transitions is the C's struct trans_list * interior pointer into the
trans_list_minimize pool — here the base offset of this state's slice. */
#[derive(Debug, Clone, Default)]
pub struct TransArray {
    pub transitions: usize,
    pub size: u32,
    pub tail: u32,
}

thread_local! {
    // C: static int *single_sigma_array, *double_sigma_array, *memo_table,
    //    *temp_move, *temp_group, maxsigma, epsilon_symbol, num_states,
    //    num_symbols, num_finals, mainloop, total_states;
    static SINGLE_SIGMA_ARRAY: RefCell<Vec<i32>> = const { RefCell::new(Vec::new()) };
    static DOUBLE_SIGMA_ARRAY: RefCell<Vec<i32>> = const { RefCell::new(Vec::new()) };
    static MEMO_TABLE: RefCell<Vec<i32>> = const { RefCell::new(Vec::new()) };
    static TEMP_MOVE: RefCell<Vec<i32>> = const { RefCell::new(Vec::new()) };
    static TEMP_GROUP: RefCell<Vec<i32>> = const { RefCell::new(Vec::new()) };
    static MAXSIGMA: Cell<i32> = const { Cell::new(0) };
    static EPSILON_SYMBOL: Cell<i32> = const { Cell::new(0) };
    static NUM_STATES: Cell<i32> = const { Cell::new(0) };
    static NUM_SYMBOLS: Cell<i32> = const { Cell::new(0) };
    static NUM_FINALS: Cell<i32> = const { Cell::new(0) };
    static MAINLOOP: Cell<i32> = const { Cell::new(0) };
    static TOTAL_STATES: Cell<i32> = const { Cell::new(0) };
    // C: static _Bool *finals;
    static FINALS: RefCell<Vec<bool>> = const { RefCell::new(Vec::new()) };
    // C: struct trans_list { ... } *trans_list_minimize; (non-static global,
    // but referenced nowhere else in the tree)
    static TRANS_LIST_MINIMIZE: RefCell<Vec<TransList>> = const { RefCell::new(Vec::new()) };
    // C: struct trans_array { ... } *trans_array_minimize; (ditto)
    static TRANS_ARRAY_MINIMIZE: RefCell<Vec<TransArray>> = const { RefCell::new(Vec::new()) };
    // C: static struct p *P, *Phead, *Pnext, *current_w;
    // PHEAD owns the block pool (the C's saved base pointer); P is the
    // block-chain head index, PNEXT the bump-allocation cursor, CURRENT_W
    // the block currently used as splitter (always assigned before read).
    static P: Cell<Option<usize>> = const { Cell::new(None) };
    static PHEAD: RefCell<Vec<P>> = const { RefCell::new(Vec::new()) };
    static PNEXT: Cell<usize> = const { Cell::new(0) };
    static CURRENT_W: Cell<usize> = const { Cell::new(0) };
    // C: static struct e *E;
    static E: RefCell<Vec<E>> = const { RefCell::new(Vec::new()) };
    // C: static struct agenda *Agenda_head, *Agenda_top, *Agenda_next, *Agenda;
    // AGENDA_TOP owns the agenda pool (the C's base pointer kept for free);
    // the other three are indices into it.
    static AGENDA_HEAD: Cell<Option<usize>> = const { Cell::new(None) };
    static AGENDA_TOP: RefCell<Vec<Agenda>> = const { RefCell::new(Vec::new()) };
    static AGENDA_NEXT: Cell<usize> = const { Cell::new(0) };
    static AGENDA: Cell<Option<usize>> = const { Cell::new(None) };
}

/* C forward decl kept as a comment:
   static void single_symbol_to_symbol_pair(int symbol, int *symbol_in, int *symbol_out);
   (compiled out in minimize.c — the back-mapping is never needed here) */

// [spec:foma:def:minimize.fsm-minimize-fn]
// [spec:foma:sem:minimize.fsm-minimize-fn]
// [spec:foma:def:fomalib.fsm-minimize-fn]
// [spec:foma:sem:fomalib.fsm-minimize-fn]
pub fn fsm_minimize(net: Box<Fsm>) -> Box<Fsm> {
    /* extern int g_minimal; extern int g_minimize_hopcroft; → mem.rs */

    /* C: if (net == NULL) return NULL — a Box argument cannot be NULL;
    NULL-able callers keep the check at the call site */
    let mut net = net;
    /* The network needs to be deterministic and trim before we minimize */
    if net.is_deterministic != YES {
        net = fsm_determinize(net);
    }
    if net.is_pruned != YES {
        net = fsm_coaccessible(net);
    }
    if net.is_minimized != YES && G_MINIMAL.get() == 1 {
        if G_MINIMIZE_HOPCROFT.get() != 0 {
            net = fsm_minimize_hop(net);
        } else {
            net = fsm_minimize_brz(net);
        }
        fsm_update_flags(&mut net, YES, YES, YES, YES, UNK, UNK);
    }
    net
}

// [spec:foma:def:minimize.fsm-minimize-brz-fn]
// [spec:foma:sem:minimize.fsm-minimize-brz-fn]
pub(crate) fn fsm_minimize_brz(net: Box<Fsm>) -> Box<Fsm> {
    fsm_determinize(fsm_reverse(fsm_determinize(fsm_reverse(net))))
}

// [spec:foma:def:minimize.fsm-minimize-hop-fn]
// [spec:foma:sem:minimize.fsm-minimize-hop-fn]
#[allow(non_snake_case)]
pub(crate) fn fsm_minimize_hop(net: Box<Fsm>) -> Box<Fsm> {
    let mut net = net;

    fsm_count(&mut net);
    if net.finalcount == 0 {
        fsm_destroy(net);
        return fsm_empty_set();
    }

    NUM_STATES.set(net.statecount);

    P.set(None);

    /*
       1. generate the inverse lookup table
       2. generate P and E (partitions, states linked list)
       3. Init Agenda = {Q, Q-F}
       4. Split until Agenda is empty
    */

    sigma_to_pairs(&mut net);

    init_PE();

    'bail: {
        if TOTAL_STATES.get() == NUM_STATES.get() {
            break 'bail; /* goto bail */
        }

        generate_inverse(&net);

        AGENDA_TOP.with_borrow_mut(|ap| {
            /* C: Agenda_head->index = 0; — unconditional deref (the head
            exists here because num_finals > 0) */
            let head = AGENDA_HEAD.get().unwrap();
            ap[head].index = false;
            if let Some(next) = ap[head].next {
                ap[next].index = false;
            }
        });

        /* for (Agenda = Agenda_head; Agenda != NULL; ) */
        AGENDA.set(AGENDA_HEAD.get());
        while let Some(agptr) = AGENDA.get() {
            /* Remove current_w from agenda */
            let (current_w, current_i, agenda_next_entry) = AGENDA_TOP.with_borrow(|ap| {
                (ap[agptr].p, ap[agptr].index as i32, ap[agptr].next)
            });
            CURRENT_W.set(current_w);
            PHEAD.with_borrow_mut(|pp| pp[current_w].agenda = None);
            AGENDA.set(agenda_next_entry);

            /* Store current group state number in tmp_group */
            /* And figure out minsym */
            /* If index is 0 we start splitting from the first symbol */
            /* Otherwise we split from where we left off last time */

            let mut thissize: i32 = 0;
            let mut minsym: i32 = i32::MAX; /* INT_MAX */
            E.with_borrow(|e| {
                PHEAD.with_borrow(|pp| {
                    TRANS_ARRAY_MINIMIZE.with_borrow_mut(|ta| {
                        TRANS_LIST_MINIMIZE.with_borrow(|tl| {
                            TEMP_GROUP.with_borrow_mut(|tg| {
                                let mut temp_E = pp[current_w].first_e;
                                while let Some(te) = temp_E {
                                    let stateno = te; /* temp_E - E */
                                    tg[thissize as usize] = stateno as i32;
                                    thissize += 1;
                                    let tptr = &mut ta[stateno];
                                    /* Clear tails if symloop should start from 0 */
                                    if current_i == 0 {
                                        tptr.tail = 0;
                                    }
                                    let tail = tptr.tail;
                                    let transitions = tptr.transitions + tail as usize;
                                    if tail < tptr.size && tl[transitions].inout < minsym {
                                        minsym = tl[transitions].inout;
                                    }
                                    temp_E = e[te].right;
                                }
                            })
                        })
                    })
                })
            });

            /* for (next_minsym = INT_MAX; minsym != INT_MAX;
                    minsym = next_minsym, next_minsym = INT_MAX) */
            let mut next_minsym: i32 = i32::MAX;
            'symloop: while minsym != i32::MAX {
                'cont: {
                    /* Add states to temp_move */
                    let mut j: i32 = 0;
                    TRANS_ARRAY_MINIMIZE.with_borrow_mut(|ta| {
                        TRANS_LIST_MINIMIZE.with_borrow(|tl| {
                            TEMP_GROUP.with_borrow(|tg| {
                                MEMO_TABLE.with_borrow_mut(|memo| {
                                    TEMP_MOVE.with_borrow_mut(|tm| {
                                        let mut i: i32 = 0;
                                        while i < thissize {
                                            let tptr = &mut ta[tg[i as usize] as usize];
                                            let mut tail = tptr.tail;
                                            let mut transitions =
                                                tptr.transitions + tail as usize;
                                            while tail < tptr.size
                                                && tl[transitions].inout == minsym
                                            {
                                                let source = tl[transitions].source;
                                                if memo[source as usize] != MAINLOOP.get() {
                                                    memo[source as usize] = MAINLOOP.get();
                                                    tm[j as usize] = source;
                                                    j += 1;
                                                }
                                                tail += 1;
                                                transitions += 1;
                                            }
                                            tptr.tail = tail;
                                            if tail < tptr.size
                                                && tl[transitions].inout < next_minsym
                                            {
                                                next_minsym = tl[transitions].inout;
                                            }
                                            i += 1;
                                        }
                                    })
                                })
                            })
                        })
                    });
                    if j == 0 {
                        break 'cont; /* continue */
                    }
                    MAINLOOP.set(MAINLOOP.get() + 1);
                    if refine_states(j) == 1 {
                        break 'symloop; /* break loop if we split current_w */
                    }
                }
                minsym = next_minsym;
                next_minsym = i32::MAX;
            }
            if TOTAL_STATES.get() == NUM_STATES.get() {
                break;
            }
        }

        net = rebuild_machine(net);

        /* free(trans_array_minimize); free(trans_list_minimize); */
        TRANS_ARRAY_MINIMIZE.with_borrow_mut(|v| *v = Vec::new());
        TRANS_LIST_MINIMIZE.with_borrow_mut(|v| *v = Vec::new());
    }

    /* bail: */

    /* free(Agenda_top); */
    AGENDA_TOP.with_borrow_mut(|v| *v = Vec::new());

    /* free(memo_table); free(temp_move); free(temp_group); */
    MEMO_TABLE.with_borrow_mut(|v| *v = Vec::new());
    TEMP_MOVE.with_borrow_mut(|v| *v = Vec::new());
    TEMP_GROUP.with_borrow_mut(|v| *v = Vec::new());

    /* free(finals); free(E); free(Phead);
       free(single_sigma_array); free(double_sigma_array); */
    FINALS.with_borrow_mut(|v| *v = Vec::new());
    E.with_borrow_mut(|v| *v = Vec::new());
    PHEAD.with_borrow_mut(|v| *v = Vec::new());
    SINGLE_SIGMA_ARRAY.with_borrow_mut(|v| *v = Vec::new());
    DOUBLE_SIGMA_ARRAY.with_borrow_mut(|v| *v = Vec::new());

    net
}

// [spec:foma:def:minimize.rebuild-machine-fn]
// [spec:foma:sem:minimize.rebuild-machine-fn]
pub(crate) fn rebuild_machine(net: Box<Fsm>) -> Box<Fsm> {
    let mut net = net;
    let mut new_linecount: i32 = 0;
    let mut arccount: i32 = 0;

    if net.statecount == TOTAL_STATES.get() {
        return net;
    }
    /* fsm = net->states — the line table is rewritten in place below */

    /* We need to make sure state 0 is first in its group */
    /* to get the proper numbering of states */

    E.with_borrow(|e| {
        PHEAD.with_borrow_mut(|pp| {
            /* if (E->group->first_e != E) E->group->first_e = E; — pointer
            overwrite only; the member list is not re-woven */
            if pp[e[0].group].first_e != Some(0) {
                pp[e[0].group].first_e = Some(0);
            }
        })
    });

    /* Recycling t_count for group numbering use here */

    let mut group_num: i32 = 1;
    PHEAD.with_borrow_mut(|pp| {
        let mut myp = P.get();
        while let Some(m) = myp {
            pp[m].count = 0;
            myp = pp[m].next;
        }
    });

    E.with_borrow(|e| {
        PHEAD.with_borrow_mut(|pp| {
            let fsm = &net.states;
            let mut i: usize = 0;
            while fsm[i].state_no != -1 {
                let thise = fsm[i].state_no as usize; /* thise = E+state_no */
                let g = e[thise].group;
                if pp[g].first_e == Some(thise) {
                    new_linecount += 1;
                    if fsm[i].start_state == 1 {
                        pp[g].t_count = 0;
                        pp[g].count = 1;
                    } else if pp[g].count == 0 {
                        pp[g].t_count = group_num;
                        group_num += 1;
                        pp[g].count = 1;
                    }
                }
                i += 1;
            }
        })
    });

    let mut j: i32 = 0;
    E.with_borrow(|e| {
        PHEAD.with_borrow(|pp| {
            FINALS.with_borrow(|finals| {
                let mut i: usize = 0;
                while net.states[i].state_no != -1 {
                    let thise = net.states[i].state_no as usize;
                    let g = e[thise].group;
                    if pp[g].first_e == Some(thise) {
                        let source = pp[g].t_count;
                        let target = if net.states[i].target == -1 {
                            -1
                        } else {
                            pp[e[net.states[i].target as usize].group].t_count
                        };
                        let r#in = net.states[i].r#in as i32;
                        let out = net.states[i].out as i32;
                        let start_state = net.states[i].start_state as i32;
                        add_fsm_arc(
                            &mut net.states,
                            j,
                            source,
                            r#in,
                            out,
                            target,
                            finals[thise] as i32,
                            start_state,
                        );
                        /* C reads (fsm+i)->target again AFTER the write to
                        (fsm+j); when j == i the rewritten target's -1-ness
                        matches the original's */
                        arccount = if net.states[i].target == -1 {
                            arccount
                        } else {
                            arccount + 1
                        };
                        j += 1;
                    }
                    i += 1;
                }
            })
        })
    });

    add_fsm_arc(&mut net.states, j, -1, -1, -1, -1, -1, -1);
    /* fsm = realloc(fsm, sizeof(struct fsm_state)*(new_linecount+1));
       net->states = fsm; */
    net.states.truncate((new_linecount + 1) as usize);
    net.linecount = j + 1;
    net.arccount = arccount;
    net.statecount = TOTAL_STATES.get();
    net
}

// [spec:foma:def:minimize.refine-states-fn]
// [spec:foma:sem:minimize.refine-states-fn]
#[allow(non_snake_case)]
pub(crate) fn refine_states(invstates: i32) -> i32 {
    /*
       1. add inverse(P,a) to table of inverses, disallowing duplicates
       2. first pass on S, touch each state once, increasing P->t_count
       3. for each P where counter != count, split and add to agenda
    */
    E.with_borrow_mut(|e| {
        PHEAD.with_borrow_mut(|pp| {
            TEMP_MOVE.with_borrow(|tm| {
                /* Inverse to table of inverses */
                let mut selfsplit: i32 = 0;

                /* touch and increase P->counter */
                for i in 0..invstates as usize {
                    let s = tm[i] as usize;
                    let g = e[s].group;
                    pp[g].t_count += 1;
                    pp[g].inv_t_count += e[s].inv_count;
                    assert!(pp[g].t_count <= pp[g].count);
                }

                /* Split (this is the tricky part) */

                for i in 0..invstates as usize {
                    let thise = tm[i] as usize; /* thise = E+*(temp_move+i) */
                    let tP = e[thise].group;

                    /* Do we need to split?
                       if we've touched as many states as there are in the partition
                       we don't split */

                    if pp[tP].t_count == pp[tP].count {
                        pp[tP].t_count = 0;
                        pp[tP].inv_t_count = 0;
                        continue;
                    }

                    if (pp[tP].t_count != pp[tP].count)
                        && (pp[tP].count > 1)
                        && (pp[tP].t_count > 0)
                    {
                        /* Check if we already split this */
                        let mut newP = pp[tP].current_split;
                        if newP.is_none() {
                            /* printf("tP [%i] newP [%i]\n",tP->inv_count,tP->inv_t_count); */
                            /* Create new group newP */
                            TOTAL_STATES.set(TOTAL_STATES.get() + 1);
                            if TOTAL_STATES.get() == NUM_STATES.get() {
                                return 1; /* Abort now, machine is already minimal */
                            }
                            /* tP->current_split = Pnext++; */
                            let np = PNEXT.get();
                            PNEXT.set(np + 1);
                            pp[tP].current_split = Some(np);
                            newP = pp[tP].current_split;
                            pp[np].first_e = Some(thise);
                            pp[np].last_e = Some(thise);
                            pp[np].count = 0;
                            pp[np].inv_count = pp[tP].inv_t_count;
                            pp[np].inv_t_count = 0;
                            pp[np].t_count = 0;
                            pp[np].current_split = None;
                            pp[np].agenda = None;

                            /* Add to agenda */

                            /* If the current block (tP) is on the agenda, we add both back */
                            /* to the agenda */
                            /* In practice we need only add newP since tP stays where it is */
                            /* However, we mark the larger one as not starting the symloop */
                            /* from zero */
                            if pp[tP].agenda.is_some() {
                                /* Is tP smaller */
                                /* (latent quirk kept: inv_t_count sums a subset of
                                tP's members' inv_counts, so inv_count < inv_t_count
                                is always false and the else branch always runs) */
                                if pp[tP].inv_count < pp[tP].inv_t_count {
                                    agenda_add(pp, np, 1);
                                    let ag = pp[tP].agenda.unwrap();
                                    AGENDA_TOP.with_borrow_mut(|ap| ap[ag].index = false);
                                } else {
                                    agenda_add(pp, np, 0);
                                }
                                /* In the event that we're splitting the partition we're currently */
                                /* splitting with, we can simply add both new partitions to the agenda */
                                /* and break out of the entire sym loop after we're */
                                /* done with the current sym and move on with the agenda */
                                /* We process the larger one for all symbols */
                                /* and the smaller one for only the ones remaining in this symloop */
                            } else if tP == CURRENT_W.get() {
                                let smaller = if pp[tP].inv_count < pp[tP].inv_t_count { tP } else { np };
                                let larger = if pp[tP].inv_count >= pp[tP].inv_t_count { tP } else { np };
                                agenda_add(pp, smaller, 0);
                                agenda_add(pp, larger, 1);
                                selfsplit = 1;
                            } else {
                                /* If the block is not on the agenda, we add */
                                /* the smaller of tP, newP and start the symloop from 0 */
                                let smaller = if pp[tP].inv_count < pp[tP].inv_t_count { tP } else { np };
                                agenda_add(pp, smaller, 0);
                            }
                            /* Add to middle of P-chain */
                            /* newP->next = P->next; P->next = newP; — C derefs
                            the chain head P unconditionally */
                            let p = P.get().unwrap();
                            pp[np].next = pp[p].next;
                            pp[p].next = Some(np);
                        }

                        let newP = newP.unwrap();
                        e[thise].group = newP;
                        pp[newP].count += 1;

                        /* need to make tP->last_e point to the last untouched e */
                        if pp[tP].last_e == Some(thise) {
                            pp[tP].last_e = e[thise].left;
                        }
                        if pp[tP].first_e == Some(thise) {
                            pp[tP].first_e = e[thise].right;
                        }

                        /* Adjust links */
                        if let Some(left) = e[thise].left {
                            e[left].right = e[thise].right;
                        }
                        if let Some(right) = e[thise].right {
                            e[right].left = e[thise].left;
                        }

                        if pp[newP].last_e != Some(thise) {
                            let last = pp[newP].last_e.unwrap();
                            e[last].right = Some(thise);
                            e[thise].left = Some(last);
                            pp[newP].last_e = Some(thise);
                        }

                        e[thise].right = None;
                        if pp[newP].first_e == Some(thise) {
                            e[thise].left = None;
                        }

                        /* Are we done for this block? Adjust counters */
                        if pp[newP].count == pp[tP].t_count {
                            pp[tP].count = pp[tP].count - pp[newP].count;
                            pp[tP].inv_count = pp[tP].inv_count - pp[tP].inv_t_count;
                            pp[tP].current_split = None;
                            pp[tP].t_count = 0;
                            pp[tP].inv_t_count = 0;
                        }
                    }
                }
                /* We return 1 if we just split the partition we were working with */
                selfsplit
            })
        })
    })
}

// [spec:foma:def:minimize.agenda-add-fn]
// [spec:foma:sem:minimize.agenda-add-fn]
/* C: static void agenda_add(struct p *pptr, int start) — the struct p *
argument decomposes to (block-pool borrow, index) per the conventions */
pub(crate) fn agenda_add(p_pool: &mut [P], pptr: usize, start: i32) {
    /* Use FILO strategy here */

    AGENDA_TOP.with_borrow_mut(|ap| {
        //ag = malloc(sizeof(struct agenda));
        let ag = AGENDA_NEXT.get(); /* ag = Agenda_next++ (no bounds check in C) */
        AGENDA_NEXT.set(ag + 1);
        if AGENDA.get().is_some() {
            ap[ag].next = AGENDA.get();
        } else {
            ap[ag].next = None;
        }
        ap[ag].p = pptr;
        ap[ag].index = start != 0; /* int → _Bool */
        AGENDA.set(Some(ag));
        p_pool[pptr].agenda = Some(ag);
    });
}

// [spec:foma:def:minimize.init-pe-fn]
// [spec:foma:sem:minimize.init-pe-fn]
#[allow(non_snake_case)]
pub(crate) fn init_PE() {
    /* Create two members of P
       (nonfinals,finals)
       and put both of them on the agenda
    */

    let num_states = NUM_STATES.get();
    let num_finals = NUM_FINALS.get();

    MAINLOOP.set(1);
    MEMO_TABLE.with_borrow_mut(|v| *v = vec![0; num_states as usize]);
    TEMP_MOVE.with_borrow_mut(|v| *v = vec![0; num_states as usize]);
    TEMP_GROUP.with_borrow_mut(|v| *v = vec![0; num_states as usize]);
    /* Phead = P = Pnext = calloc(num_states+1, sizeof(struct p)); */
    PHEAD.with_borrow_mut(|v| *v = vec![P::default(); (num_states + 1) as usize]);
    P.set(Some(0));
    PNEXT.set(0);
    /* nonFP = Pnext++; FP = Pnext++; */
    let nonFP = PNEXT.get();
    PNEXT.set(nonFP + 1);
    let FP = PNEXT.get();
    PNEXT.set(FP + 1);
    PHEAD.with_borrow_mut(|pp| {
        pp[nonFP].next = Some(FP);
        pp[nonFP].count = num_states - num_finals;
        pp[FP].next = None;
        pp[FP].count = num_finals;
        pp[FP].t_count = 0;
        pp[nonFP].t_count = 0;
        pp[FP].current_split = None;
        pp[nonFP].current_split = None;
        /* FP->inv_count = nonFP->inv_count = FP->inv_t_count = nonFP->inv_t_count = 0; */
        pp[FP].inv_count = 0;
        pp[nonFP].inv_count = 0;
        pp[FP].inv_t_count = 0;
        pp[nonFP].inv_t_count = 0;
    });

    /* How many groups can we put on the agenda? */
    AGENDA_TOP.with_borrow_mut(|v| *v = vec![Agenda::default(); (num_states * 2) as usize]);
    AGENDA_NEXT.set(0);
    AGENDA_HEAD.set(None);

    P.set(None);
    TOTAL_STATES.set(0);

    if num_finals > 0 {
        let ag = AGENDA_NEXT.get();
        AGENDA_NEXT.set(ag + 1);
        PHEAD.with_borrow_mut(|pp| pp[FP].agenda = Some(ag));
        P.set(Some(FP));
        PHEAD.with_borrow_mut(|pp| pp[FP].next = None); /* P->next = NULL */
        AGENDA_TOP.with_borrow_mut(|ap| ap[ag].p = FP);
        AGENDA_HEAD.set(Some(ag));
        AGENDA_TOP.with_borrow_mut(|ap| ap[ag].next = None);
        TOTAL_STATES.set(TOTAL_STATES.get() + 1);
    }
    if num_states - num_finals > 0 {
        let ag = AGENDA_NEXT.get();
        AGENDA_NEXT.set(ag + 1);
        PHEAD.with_borrow_mut(|pp| pp[nonFP].agenda = Some(ag));
        AGENDA_TOP.with_borrow_mut(|ap| {
            ap[ag].p = nonFP;
            ap[ag].next = None;
        });
        TOTAL_STATES.set(TOTAL_STATES.get() + 1);
        if AGENDA_HEAD.get().is_some() {
            AGENDA_TOP.with_borrow_mut(|ap| ap[AGENDA_HEAD.get().unwrap()].next = Some(ag));
            PHEAD.with_borrow_mut(|pp| {
                let p = P.get().unwrap();
                pp[p].next = Some(nonFP);
                /* P->next->next = NULL; */
                let pn = pp[p].next.unwrap();
                pp[pn].next = None;
            });
        } else {
            P.set(Some(nonFP));
            PHEAD.with_borrow_mut(|pp| pp[nonFP].next = None);
            AGENDA_HEAD.set(Some(ag));
        }
    }

    /* Initialize doubly linked list E */
    E.with_borrow_mut(|v| *v = vec![E::default(); num_states as usize]);

    let mut last_f: Option<usize> = None;
    let mut last_nonf: Option<usize> = None;

    E.with_borrow_mut(|e| {
        PHEAD.with_borrow_mut(|pp| {
            FINALS.with_borrow(|finals| {
                for i in 0..num_states as usize {
                    if finals[i] {
                        e[i].group = FP;
                        e[i].left = last_f;
                        if i > 0 && last_f.is_some() {
                            e[last_f.unwrap()].right = Some(i);
                        }
                        if last_f.is_none() {
                            pp[FP].first_e = Some(i);
                        }
                        last_f = Some(i);
                        pp[FP].last_e = Some(i);
                    } else {
                        e[i].group = nonFP;
                        e[i].left = last_nonf;
                        if i > 0 && last_nonf.is_some() {
                            e[last_nonf.unwrap()].right = Some(i);
                        }
                        if last_nonf.is_none() {
                            pp[nonFP].first_e = Some(i);
                        }
                        last_nonf = Some(i);
                        pp[nonFP].last_e = Some(i);
                    }
                    e[i].inv_count = 0;
                }

                if let Some(lf) = last_f {
                    e[lf].right = None;
                }
                if let Some(lnf) = last_nonf {
                    e[lnf].right = None;
                }
            })
        })
    });
}

// [spec:foma:def:minimize.trans-sort-cmp-fn]
// [spec:foma:sem:minimize.trans-sort-cmp-fn]
/* C: qsort comparator over const void * — typed slice elements here */
pub(crate) fn trans_sort_cmp(a: &TransList, b: &TransList) -> i32 {
    a.inout - b.inout
}

// [spec:foma:def:minimize.generate-inverse-fn]
// [spec:foma:sem:minimize.generate-inverse-fn]
pub(crate) fn generate_inverse(net: &Fsm) {
    let fsm = &net.states;
    TRANS_ARRAY_MINIMIZE
        .with_borrow_mut(|v| *v = vec![TransArray::default(); net.statecount as usize]);
    TRANS_LIST_MINIMIZE
        .with_borrow_mut(|v| *v = vec![TransList::default(); net.arccount as usize]);

    /* Figure out the number of transitions each one has */
    E.with_borrow_mut(|e| {
        PHEAD.with_borrow_mut(|pp| {
            TRANS_ARRAY_MINIMIZE.with_borrow_mut(|ta| {
                let mut i: usize = 0;
                while fsm[i].state_no != -1 {
                    if fsm[i].target == -1 {
                        i += 1;
                        continue;
                    }
                    let target = fsm[i].target as usize;
                    e[target].inv_count += 1;
                    pp[e[target].group].inv_count += 1;
                    ta[target].size += 1;
                    i += 1;
                }
            })
        })
    });

    let mut offsetcount: i32 = 0;
    TRANS_ARRAY_MINIMIZE.with_borrow_mut(|ta| {
        for i in 0..net.statecount as usize {
            /* (trans_array_minimize+i)->transitions = trans_list_minimize + offsetcount; */
            ta[i].transitions = offsetcount as usize;
            offsetcount += ta[i].size as i32;
        }
    });

    TRANS_ARRAY_MINIMIZE.with_borrow_mut(|ta| {
        TRANS_LIST_MINIMIZE.with_borrow_mut(|tl| {
            let mut i: usize = 0;
            while fsm[i].state_no != -1 {
                if fsm[i].target == -1 {
                    i += 1;
                    continue;
                }
                let symbol = symbol_pair_to_single_symbol(fsm[i].r#in as i32, fsm[i].out as i32);
                let source = fsm[i].state_no;
                let target = fsm[i].target as usize;
                let tptr = &mut ta[target];
                tl[tptr.transitions + tptr.tail as usize].inout = symbol;
                tl[tptr.transitions + tptr.tail as usize].source = source;
                tptr.tail += 1;
                i += 1;
            }
        })
    });

    /* Sort arcs */
    TRANS_ARRAY_MINIMIZE.with_borrow(|ta| {
        TRANS_LIST_MINIMIZE.with_borrow_mut(|tl| {
            for i in 0..net.statecount as usize {
                let listptr = ta[i].transitions;
                let size = ta[i].size as i32;
                if size > 1 {
                    /* qsort(listptr, size, sizeof(struct trans_list), trans_sort_cmp) —
                    unstable sort; equal keys keep an unspecified relative order */
                    tl[listptr..listptr + size as usize]
                        .sort_unstable_by(|a, b| trans_sort_cmp(a, b).cmp(&0));
                }
            }
        })
    });
}

// [spec:foma:def:minimize.sigma-to-pairs-fn]
// [spec:foma:sem:minimize.sigma-to-pairs-fn]
pub(crate) fn sigma_to_pairs(net: &mut Fsm) {
    let mut next_x: i32 = 0;

    EPSILON_SYMBOL.set(-1);
    MAXSIGMA.set(sigma_max(net.sigma.as_deref()));

    MAXSIGMA.set(MAXSIGMA.get() + 1);
    let maxsigma = MAXSIGMA.get();

    /* single_sigma_array = malloc(2*maxsigma*maxsigma*sizeof(int));
       double_sigma_array = malloc(maxsigma*maxsigma*sizeof(int));
       — malloc'd (uninitialized) in C; zero-filled here (double is
       overwritten with -1 below, single only ever read where written) */
    SINGLE_SIGMA_ARRAY.with_borrow_mut(|v| *v = vec![0; (2 * maxsigma * maxsigma) as usize]);
    DOUBLE_SIGMA_ARRAY.with_borrow_mut(|v| *v = vec![0; (maxsigma * maxsigma) as usize]);

    DOUBLE_SIGMA_ARRAY.with_borrow_mut(|d| {
        for i in 0..maxsigma {
            for j in 0..maxsigma {
                d[(maxsigma * i + j) as usize] = -1;
            }
        }
    });

    /* f(x) -> y,z sigma pair */
    /* f(y,z) -> x simple entry */
    /* if exists f(n) <-> EPSILON, EPSILON, save n */
    /* symbol(x) x>=1 */

    /* Forward mapping: */
    /* *(double_sigma_array+maxsigma*in+out) */

    /* Backmapping: */
    /* *(single_sigma_array+(symbol*2) = in(symbol) */
    /* *(single_sigma_array+(symbol*2+1) = out(symbol) */

    /* Table for checking whether a state is final */

    FINALS.with_borrow_mut(|v| *v = vec![false; NUM_STATES.get() as usize]);
    let mut x: i32 = 0;
    NUM_FINALS.set(0);
    net.arity = 1;
    SINGLE_SIGMA_ARRAY.with_borrow_mut(|s| {
        DOUBLE_SIGMA_ARRAY.with_borrow_mut(|d| {
            FINALS.with_borrow_mut(|finals| {
                let mut i: usize = 0;
                while net.states[i].state_no != -1 {
                    /* C: finals[state_no] != 1 on a _Bool */
                    if net.states[i].final_state == 1
                        && finals[net.states[i].state_no as usize] != true
                    {
                        NUM_FINALS.set(NUM_FINALS.get() + 1);
                        finals[net.states[i].state_no as usize] = true;
                    }
                    let y = net.states[i].r#in as i32;
                    let z = net.states[i].out as i32;
                    if y != z || y == UNKNOWN || z == UNKNOWN {
                        net.arity = 2;
                    }
                    if (y == -1) || (z == -1) {
                        i += 1;
                        continue;
                    }
                    if d[(maxsigma * y + z) as usize] == -1 {
                        d[(maxsigma * y + z) as usize] = x;
                        s[next_x as usize] = y;
                        next_x += 1;
                        s[next_x as usize] = z;
                        next_x += 1;
                        if y == EPSILON && z == EPSILON {
                            EPSILON_SYMBOL.set(x);
                        }
                        x += 1;
                    }
                    i += 1;
                }
            })
        })
    });
    NUM_SYMBOLS.set(x);
}

// [spec:foma:def:minimize.symbol-pair-to-single-symbol-fn]
// [spec:foma:sem:minimize.symbol-pair-to-single-symbol-fn]
pub(crate) fn symbol_pair_to_single_symbol(r#in: i32, out: i32) -> i32 {
    DOUBLE_SIGMA_ARRAY.with_borrow(|d| d[(MAXSIGMA.get() * r#in + out) as usize])
}

//! Wave-4 split of constructions.c (see mod.rs). Cross-module and
//! external names come via `use super::*` (re-exported by mod.rs).
use super::*;

/* C: #define STACK_3_PUSH(a,b,c) / STACK_2_PUSH(a,b) — expanded inline at
each use site below (int_stack_push calls in the same order) */

// [spec:foma:def:constructions.mergesigma]
#[derive(Debug, Clone)]
pub struct Mergesigma {
    /* C: char *symbol aliases the source sigma node's string (no copy);
    owned clone here — observably equivalent (copy_mergesigma deep-copies
    again and the C list is freed without freeing the symbols) */
    pub symbol: Option<String>,
    /// 1 = in net 1, 2 = in net 2, 3 = in both
    pub presence: u8,
    pub number: i32,
    pub next: Option<Box<Mergesigma>>,
}

// [spec:foma:def:constructions.add-to-mergesigma-fn]
// [spec:foma:sem:constructions.add-to-mergesigma-fn]
pub fn add_to_mergesigma<'a>(
    msigma: &'a mut Mergesigma,
    sigma: &Sigma,
    presence: i16,
) -> &'a mut Mergesigma {
    let mut number = 0;

    let msigma = if msigma.number == -1 {
        number = 2;
        msigma
    } else {
        msigma.next = Some(Box::new(Mergesigma {
            symbol: None,
            presence: 0,
            number: 0,
            next: None,
        }));
        number = msigma.number;
        let msigma = msigma.next.as_deref_mut().unwrap();
        msigma.next = None;
        msigma
    };

    if sigma.number < 3 {
        msigma.number = sigma.number;
    } else {
        if number < 3 {
            number = 2;
        }
        msigma.number = number + 1;
    }
    /* C: msigma->symbol = sigma->symbol (aliased, no copy) — owned clone
    here, see the Mergesigma type comment */
    msigma.symbol = sigma.symbol.clone();
    msigma.presence = presence as u8;
    msigma
}

// [spec:foma:def:constructions.copy-mergesigma-fn]
// [spec:foma:sem:constructions.copy-mergesigma-fn]
pub fn copy_mergesigma(mergesigma: Option<&Mergesigma>) -> Option<Box<Sigma>> {
    let mut new_sigma: Option<Box<Sigma>> = None;

    /* C: tail-pointer append (sigma cursor trails the freshly malloc'd
    node); a tail cursor into the owned chain here */
    let mut tail: &mut Option<Box<Sigma>> = &mut new_sigma;
    let mut mergesigma = mergesigma;
    while let Some(m) = mergesigma {
        *tail = Some(Box::new(Sigma {
            number: m.number,
            /* sigma->symbol = NULL; if (mergesigma->symbol != NULL)
            sigma->symbol = strdup(mergesigma->symbol); */
            symbol: m.symbol.clone(),
            next: None,
        }));
        tail = &mut tail.as_deref_mut().unwrap().next;
        mergesigma = m.next.as_deref();
    }
    new_sigma
}

// [spec:foma:def:constructions.fsm-merge-sigma-fn]
// [spec:foma:sem:constructions.fsm-merge-sigma-fn]
// [spec:foma:def:fomalib.fsm-merge-sigma-fn]
// [spec:foma:sem:fomalib.fsm-merge-sigma-fn]
pub fn fsm_merge_sigma(net1: &mut Fsm, net2: &mut Fsm) {
    let mut end_1 = 0;
    let mut end_2 = 0;
    let mut equal = 1;
    let mut unknown_1 = 0;
    let mut unknown_2 = 0;
    let mut net_unk = 0;

    if !FSM_OPTIONS.with(|o| o.borrow().skip_word_boundary_marker) {
        let i = sigma_find(".#.", net1.sigma.as_deref());
        let j = sigma_find(".#.", net2.sigma.as_deref());
        if i != -1 && j == -1 {
            sigma_add(".#.", net2.sigma.as_deref_mut().unwrap());
            sigma_sort(net2);
        }
        if j != -1 && i == -1 {
            sigma_add(".#.", net1.sigma.as_deref_mut().unwrap());
            sigma_sort(net1);
        }
    }

    let sigmasizes = sigma_max(net1.sigma.as_deref()) + sigma_max(net2.sigma.as_deref()) + 3;

    /* C: malloc'd (uninitialized); zero-filled here — entries are always
    written before being read for well-formed nets */
    let mut mapping_1: Vec<i32> = vec![0; sigmasizes as usize];
    let mut mapping_2: Vec<i32> = vec![0; sigmasizes as usize];

    /* Fill mergesigma */

    let mut start_mergesigma = Box::new(Mergesigma {
        number: -1,
        symbol: None,
        presence: 0,
        next: None,
    });

    /* Loop over sigma 1, sigma 2 */
    {
        let mut sigma_1 = net1.sigma.as_deref();
        let mut sigma_2 = net2.sigma.as_deref();
        let mut mergesigma: &mut Mergesigma = &mut start_mergesigma;
        loop {
            if sigma_1.is_none() {
                end_1 = 1;
            }
            if sigma_2.is_none() {
                end_2 = 1;
            }
            if end_1 != 0 && end_2 != 0 {
                break;
            }
            if end_2 != 0 {
                /* Treating only 1 now */
                let s1 = sigma_1.unwrap();
                mergesigma = add_to_mergesigma(mergesigma, s1, 1);
                mapping_1[s1.number as usize] = mergesigma.number;
                sigma_1 = s1.next.as_deref();
                equal = 0;
                continue;
            } else if end_1 != 0 {
                /* Treating only 2 now */
                let s2 = sigma_2.unwrap();
                mergesigma = add_to_mergesigma(mergesigma, s2, 2);
                mapping_2[s2.number as usize] = mergesigma.number;
                sigma_2 = s2.next.as_deref();
                equal = 0;
                continue;
            } else {
                /* Both alive */

                let s1 = sigma_1.unwrap();
                let s2 = sigma_2.unwrap();

                /* 1 or 2 contains special characters */
                if s1.number <= IDENTITY || s2.number <= IDENTITY {
                    /* Treating zeros or unknowns */

                    if s1.number == UNKNOWN || s1.number == IDENTITY {
                        unknown_1 = 1;
                    }
                    if s2.number == UNKNOWN || s2.number == IDENTITY {
                        unknown_2 = 1;
                    }

                    if s1.number == s2.number {
                        mergesigma = add_to_mergesigma(mergesigma, s1, 3);
                        sigma_1 = s1.next.as_deref();
                        sigma_2 = s2.next.as_deref();
                    } else if s1.number < s2.number {
                        mergesigma = add_to_mergesigma(mergesigma, s1, 1);
                        sigma_1 = s1.next.as_deref();
                        equal = 0;
                    } else {
                        mergesigma = add_to_mergesigma(mergesigma, s2, 2);
                        sigma_2 = s2.next.as_deref();
                        equal = 0;
                    }
                    continue;
                }
                /* Both contain non-special chars */
                /* strcmp — Rust str comparison is bytewise, as strcmp */
                let cmp = s1
                    .symbol
                    .as_deref()
                    .unwrap()
                    .cmp(s2.symbol.as_deref().unwrap());
                if cmp == std::cmp::Ordering::Equal {
                    mergesigma = add_to_mergesigma(mergesigma, s1, 3);
                    /* Add symbol numbers to mapping */
                    mapping_1[s1.number as usize] = mergesigma.number;
                    mapping_2[s2.number as usize] = mergesigma.number;

                    sigma_1 = s1.next.as_deref();
                    sigma_2 = s2.next.as_deref();
                } else if cmp == std::cmp::Ordering::Less {
                    mergesigma = add_to_mergesigma(mergesigma, s1, 1);
                    mapping_1[s1.number as usize] = mergesigma.number;
                    sigma_1 = s1.next.as_deref();
                    equal = 0;
                } else {
                    mergesigma = add_to_mergesigma(mergesigma, s2, 2);
                    mapping_2[s2.number as usize] = mergesigma.number;
                    sigma_2 = s2.next.as_deref();
                    equal = 0;
                }
                continue;
            }
        }
    }

    /* Go over both net1 and net2 and replace arc numbers with new mappings */

    let mut i = 0usize;
    while net1.states[i].state_no != -1 {
        if net1.states[i].r#in > 2 {
            net1.states[i].r#in = mapping_1[net1.states[i].r#in as usize] as i16;
        }
        if net1.states[i].out > 2 {
            net1.states[i].out = mapping_1[net1.states[i].out as usize] as i16;
        }
        i += 1;
    }
    let mut i = 0usize;
    while net2.states[i].state_no != -1 {
        if net2.states[i].r#in > 2 {
            net2.states[i].r#in = mapping_2[net2.states[i].r#in as usize] as i16;
        }
        if net2.states[i].out > 2 {
            net2.states[i].out = mapping_2[net2.states[i].out as usize] as i16;
        }
        i += 1;
    }

    /* Copy mergesigma to net1, net2 */

    let new_sigma_1 = copy_mergesigma(Some(&start_mergesigma));
    let new_sigma_2 = copy_mergesigma(Some(&start_mergesigma));

    fsm_sigma_destroy(net1.sigma.take());
    fsm_sigma_destroy(net2.sigma.take());

    net1.sigma = new_sigma_1;
    net2.sigma = new_sigma_2;

    /* Expand on ?, ?:x, y:? */

    if unknown_1 != 0 && equal == 0 {
        /* Expand net 1 */
        let net_lines = find_arccount(&net1.states);
        /* C: net_unk carries its function-entry 0 here (only net 2's
        branch re-zeroes it) */
        let mut ms = Some(&*start_mergesigma);
        while let Some(m) = ms {
            if m.presence == 2 {
                net_unk += 1;
            }
            ms = m.next.as_deref();
        }
        let mut net_adds = 0;
        let mut i = 0usize;
        while net1.states[i].state_no != -1 {
            let (line_in, line_out) = (net1.states[i].r#in as i32, net1.states[i].out as i32);
            if line_in == IDENTITY {
                net_adds += net_unk;
            }
            if line_in == UNKNOWN && line_out != UNKNOWN {
                net_adds += net_unk;
            }
            if line_out == UNKNOWN && line_in != UNKNOWN {
                net_adds += net_unk;
            }
            if line_in == UNKNOWN && line_out == UNKNOWN {
                net_adds += net_unk * net_unk - net_unk + 2 * net_unk;
            }
            i += 1;
        }

        /* C: malloc'd (uninitialized); zeroed lines here */
        let mut new_1_state: Vec<FsmState> = vec![
            FsmState {
                state_no: 0,
                r#in: 0,
                out: 0,
                target: 0,
                final_state: 0,
                start_state: 0,
            };
            (net_adds + net_lines + 1) as usize
        ];
        let mut j: i32 = 0;
        let mut i = 0usize;
        while net1.states[i].state_no != -1 {
            let state_no = net1.states[i].state_no;
            let line_in = net1.states[i].r#in as i32;
            let line_out = net1.states[i].out as i32;
            let target = net1.states[i].target;
            let final_state = net1.states[i].final_state as i32;
            let start_state = net1.states[i].start_state as i32;

            if line_in == IDENTITY {
                add_fsm_arc(
                    &mut new_1_state,
                    j,
                    state_no,
                    line_in,
                    line_out,
                    target,
                    final_state,
                    start_state,
                );
                j += 1;
                let mut ms = Some(&*start_mergesigma);
                while let Some(m) = ms {
                    if m.presence == 2 && m.number > IDENTITY {
                        add_fsm_arc(
                            &mut new_1_state,
                            j,
                            state_no,
                            m.number,
                            m.number,
                            target,
                            final_state,
                            start_state,
                        );
                        j += 1;
                    }
                    ms = m.next.as_deref();
                }
            }

            if line_in == UNKNOWN && line_out != UNKNOWN {
                add_fsm_arc(
                    &mut new_1_state,
                    j,
                    state_no,
                    line_in,
                    line_out,
                    target,
                    final_state,
                    start_state,
                );
                j += 1;
                let mut ms = Some(&*start_mergesigma);
                while let Some(m) = ms {
                    if m.presence == 2 && m.number > IDENTITY {
                        add_fsm_arc(
                            &mut new_1_state,
                            j,
                            state_no,
                            m.number,
                            line_out,
                            target,
                            final_state,
                            start_state,
                        );
                        j += 1;
                    }
                    ms = m.next.as_deref();
                }
            }

            if line_in != UNKNOWN && line_out == UNKNOWN {
                add_fsm_arc(
                    &mut new_1_state,
                    j,
                    state_no,
                    line_in,
                    line_out,
                    target,
                    final_state,
                    start_state,
                );
                j += 1;
                let mut ms = Some(&*start_mergesigma);
                while let Some(m) = ms {
                    if m.presence == 2 && m.number > IDENTITY {
                        add_fsm_arc(
                            &mut new_1_state,
                            j,
                            state_no,
                            line_in,
                            m.number,
                            target,
                            final_state,
                            start_state,
                        );
                        j += 1;
                    }
                    ms = m.next.as_deref();
                }
            }

            /* Replace ?:? with ?:[all unknowns] [all unknowns]:? and [all unknowns]:[all unknowns] where a != b */
            if line_in == UNKNOWN && line_out == UNKNOWN {
                add_fsm_arc(
                    &mut new_1_state,
                    j,
                    state_no,
                    line_in,
                    line_out,
                    target,
                    final_state,
                    start_state,
                );
                j += 1;
                let mut ms2 = Some(&*start_mergesigma);
                while let Some(m2) = ms2 {
                    let mut ms = Some(&*start_mergesigma);
                    while let Some(m) = ms {
                        if ((m.presence == 2
                            && m2.presence == 2
                            && m.number > IDENTITY
                            && m2.number > IDENTITY)
                            || (m.number == UNKNOWN && m2.number > IDENTITY && m2.presence == 2)
                            || (m2.number == UNKNOWN && m.number > IDENTITY && m.presence == 2))
                            && m.number != m2.number
                        {
                            add_fsm_arc(
                                &mut new_1_state,
                                j,
                                state_no,
                                m.number,
                                m2.number,
                                target,
                                final_state,
                                start_state,
                            );
                            j += 1;
                        }
                        ms = m.next.as_deref();
                    }
                    ms2 = m2.next.as_deref();
                }
            }

            /* Simply copy arcs that are not IDENTITY or UNKNOWN */
            if (line_in > IDENTITY || line_in == EPSILON)
                && (line_out > IDENTITY || line_out == EPSILON)
            {
                add_fsm_arc(
                    &mut new_1_state,
                    j,
                    state_no,
                    line_in,
                    line_out,
                    target,
                    final_state,
                    start_state,
                );
                j += 1;
            }

            if line_in == -1 {
                add_fsm_arc(
                    &mut new_1_state,
                    j,
                    state_no,
                    line_in,
                    line_out,
                    target,
                    final_state,
                    start_state,
                );
                j += 1;
            }
            i += 1;
        }

        add_fsm_arc(&mut new_1_state, j, -1, -1, -1, -1, -1, -1);
        /* free(net1->states); net1->states = new_1_state */
        net1.states = new_1_state;
    }

    if unknown_2 != 0 && equal == 0 {
        /* Expand net 2 */
        let net_lines = find_arccount(&net2.states);
        net_unk = 0;
        let mut ms = Some(&*start_mergesigma);
        while let Some(m) = ms {
            if m.presence == 1 {
                net_unk += 1;
            }
            ms = m.next.as_deref();
        }

        let mut net_adds = 0;
        let mut i = 0usize;
        while net2.states[i].state_no != -1 {
            let (line_in, line_out) = (net2.states[i].r#in as i32, net2.states[i].out as i32);
            if line_in == IDENTITY {
                net_adds += net_unk;
            }
            if line_in == UNKNOWN && line_out != UNKNOWN {
                net_adds += net_unk;
            }
            if line_out == UNKNOWN && line_in != UNKNOWN {
                net_adds += net_unk;
            }
            if line_in == UNKNOWN && line_out == UNKNOWN {
                net_adds += net_unk * net_unk - net_unk + 2 * net_unk;
            }
            i += 1;
        }

        /* We need net_add new lines in fsm_state */
        let mut new_2_state: Vec<FsmState> = vec![
            FsmState {
                state_no: 0,
                r#in: 0,
                out: 0,
                target: 0,
                final_state: 0,
                start_state: 0,
            };
            (net_adds + net_lines + 1) as usize
        ];
        let mut j: i32 = 0;
        let mut i = 0usize;
        while net2.states[i].state_no != -1 {
            let state_no = net2.states[i].state_no;
            let line_in = net2.states[i].r#in as i32;
            let line_out = net2.states[i].out as i32;
            let target = net2.states[i].target;
            let final_state = net2.states[i].final_state as i32;
            let start_state = net2.states[i].start_state as i32;

            if line_in == IDENTITY {
                add_fsm_arc(
                    &mut new_2_state,
                    j,
                    state_no,
                    line_in,
                    line_out,
                    target,
                    final_state,
                    start_state,
                );
                j += 1;
                let mut ms = Some(&*start_mergesigma);
                while let Some(m) = ms {
                    if m.presence == 1 && m.number > IDENTITY {
                        add_fsm_arc(
                            &mut new_2_state,
                            j,
                            state_no,
                            m.number,
                            m.number,
                            target,
                            final_state,
                            start_state,
                        );
                        j += 1;
                    }
                    ms = m.next.as_deref();
                }
            }

            if line_in == UNKNOWN && line_out != UNKNOWN {
                add_fsm_arc(
                    &mut new_2_state,
                    j,
                    state_no,
                    line_in,
                    line_out,
                    target,
                    final_state,
                    start_state,
                );
                j += 1;
                let mut ms = Some(&*start_mergesigma);
                while let Some(m) = ms {
                    if m.presence == 1 && m.number > IDENTITY {
                        add_fsm_arc(
                            &mut new_2_state,
                            j,
                            state_no,
                            m.number,
                            line_out,
                            target,
                            final_state,
                            start_state,
                        );
                        j += 1;
                    }
                    ms = m.next.as_deref();
                }
            }

            if line_in != UNKNOWN && line_out == UNKNOWN {
                add_fsm_arc(
                    &mut new_2_state,
                    j,
                    state_no,
                    line_in,
                    line_out,
                    target,
                    final_state,
                    start_state,
                );
                j += 1;
                let mut ms = Some(&*start_mergesigma);
                while let Some(m) = ms {
                    if m.presence == 1 && m.number > IDENTITY {
                        add_fsm_arc(
                            &mut new_2_state,
                            j,
                            state_no,
                            line_in,
                            m.number,
                            target,
                            final_state,
                            start_state,
                        );
                        j += 1;
                    }
                    ms = m.next.as_deref();
                }
            }

            if line_in == UNKNOWN && line_out == UNKNOWN {
                add_fsm_arc(
                    &mut new_2_state,
                    j,
                    state_no,
                    line_in,
                    line_out,
                    target,
                    final_state,
                    start_state,
                );
                j += 1;
                let mut ms2 = Some(&*start_mergesigma);
                while let Some(m2) = ms2 {
                    let mut ms = Some(&*start_mergesigma);
                    while let Some(m) = ms {
                        if ((m.presence == 1
                            && m2.presence == 1
                            && m.number > IDENTITY
                            && m2.number > IDENTITY)
                            || (m.number == UNKNOWN && m2.number > IDENTITY && m2.presence == 1)
                            || (m2.number == UNKNOWN && m.number > IDENTITY && m.presence == 1))
                            && m.number != m2.number
                        {
                            add_fsm_arc(
                                &mut new_2_state,
                                j,
                                state_no,
                                m.number,
                                m2.number,
                                target,
                                final_state,
                                start_state,
                            );
                            j += 1;
                        }
                        ms = m.next.as_deref();
                    }
                    ms2 = m2.next.as_deref();
                }
            }

            /* Simply copy arcs that are not IDENTITY or UNKNOWN */
            if (line_in > IDENTITY || line_in == EPSILON)
                && (line_out > IDENTITY || line_out == EPSILON)
            {
                add_fsm_arc(
                    &mut new_2_state,
                    j,
                    state_no,
                    line_in,
                    line_out,
                    target,
                    final_state,
                    start_state,
                );
                j += 1;
            }

            if line_in == -1 {
                add_fsm_arc(
                    &mut new_2_state,
                    j,
                    state_no,
                    line_in,
                    line_out,
                    target,
                    final_state,
                    start_state,
                );
                j += 1;
            }
            i += 1;
        }

        add_fsm_arc(&mut new_2_state, j, -1, -1, -1, -1, -1, -1);
        /* free(net2->states); net2->states = new_2_state */
        net2.states = new_2_state;
    }
    /* free(mapping_1); free(mapping_2) */
    drop(mapping_1);
    drop(mapping_2);

    /* Free structure */
    drop(start_mergesigma);
}

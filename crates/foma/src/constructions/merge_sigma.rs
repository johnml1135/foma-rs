//! Split out of constructions.c (see mod.rs). Cross-module and
//! external names come via `use super::*` (re-exported by mod.rs).
use super::*;
use smol_str::SmolStr;

/* C: #define STACK_3_PUSH(a,b,c) / STACK_2_PUSH(a,b) — expanded inline at
each use site below (int_stack_push calls in the same order) */

// [spec:foma:def:constructions.mergesigma]
#[derive(Debug, Clone)]
pub struct Mergesigma {
    /* C: char *symbol aliases the source sigma node's string (no copy);
    owned clone here — observably equivalent (copy_mergesigma deep-copies
    again and the C list is freed without freeing the symbols). The C
    singly-linked list is a `Vec<Mergesigma>` in append order here. */
    pub symbol: Option<SmolStr>,
    /// 1 = in net 1, 2 = in net 2, 3 = in both
    pub presence: u8,
    pub number: i32,
}

// [spec:foma:def:constructions.add-to-mergesigma-fn]
// [spec:foma:sem:constructions.add-to-mergesigma-fn]
pub fn add_to_mergesigma(msigma: &mut Vec<Mergesigma>, sigma: &Sigma, presence: i16) -> i32 {
    /* the C sentinel head (number == -1, reused for the first node) is the
    empty Vec here: an empty list numbers its first node as if from 2. */
    let mut number = msigma.last().map_or(2, |m| m.number);

    let assigned = if sigma.number < 3 {
        sigma.number
    } else {
        if number < 3 {
            number = 2;
        }
        number + 1
    };
    /* C: msigma->symbol = sigma->symbol (aliased, no copy) — owned clone
    here, see the Mergesigma type comment */
    msigma.push(Mergesigma {
        symbol: Some(sigma.symbol.clone()),
        presence: presence as u8,
        number: assigned,
    });
    assigned
}

// [spec:foma:def:constructions.copy-mergesigma-fn]
// [spec:foma:sem:constructions.copy-mergesigma-fn]
pub fn copy_mergesigma(mergesigma: &[Mergesigma]) -> Vec<Sigma> {
    /* append each mergesigma node in order; a NULL mergesigma symbol becomes
    an empty string (the merge always fills symbols for well-formed nets) */
    mergesigma
        .iter()
        .map(|m| Sigma {
            number: m.number,
            symbol: m.symbol.clone().unwrap_or_default(),
        })
        .collect()
}

/// Expand `?`/`@`/`?:x`/`x:?`/`?:?` arcs in `net` into explicit arcs over the
/// symbols the *other* net contributed — the mergesigma nodes carrying
/// `presence` (2 when expanding net1, 1 when expanding net2). Shared body of the
/// two fsm_merge_sigma expansion passes, which C kept as a copy-paste pair.
fn expand_unknowns(net: &mut Fsm, mergesigma: &[Mergesigma], presence: u8) {
    let net_lines = find_arccount(&net.states.rows());
    let mut net_unk = 0;
    for m in mergesigma {
        if m.presence == presence {
            net_unk += 1;
        }
    }

    let fsm = net.states.rows();
    let mut net_adds = 0;
    let mut i = 0usize;
    while fsm[i].state_no != -1 {
        let (line_in, line_out) = (fsm[i].r#in as i32, fsm[i].out as i32);
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
    let mut new_state: Vec<FsmState> = vec![
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
    while fsm[i].state_no != -1 {
        let state_no = fsm[i].state_no;
        let line_in = fsm[i].r#in as i32;
        let line_out = fsm[i].out as i32;
        let target = fsm[i].target;
        let final_state = fsm[i].final_state as i32;
        let start_state = fsm[i].start_state as i32;

        if line_in == IDENTITY {
            add_fsm_arc(
                &mut new_state,
                j,
                state_no,
                line_in,
                line_out,
                target,
                final_state,
                start_state,
            );
            j += 1;
            for m in mergesigma {
                if m.presence == presence && m.number > IDENTITY {
                    add_fsm_arc(
                        &mut new_state,
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
            }
        }

        if line_in == UNKNOWN && line_out != UNKNOWN {
            add_fsm_arc(
                &mut new_state,
                j,
                state_no,
                line_in,
                line_out,
                target,
                final_state,
                start_state,
            );
            j += 1;
            for m in mergesigma {
                if m.presence == presence && m.number > IDENTITY {
                    add_fsm_arc(
                        &mut new_state,
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
            }
        }

        if line_in != UNKNOWN && line_out == UNKNOWN {
            add_fsm_arc(
                &mut new_state,
                j,
                state_no,
                line_in,
                line_out,
                target,
                final_state,
                start_state,
            );
            j += 1;
            for m in mergesigma {
                if m.presence == presence && m.number > IDENTITY {
                    add_fsm_arc(
                        &mut new_state,
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
            }
        }

        /* Replace ?:? with ?:[all unknowns] [all unknowns]:? and [all unknowns]:[all unknowns] where a != b */
        if line_in == UNKNOWN && line_out == UNKNOWN {
            add_fsm_arc(
                &mut new_state,
                j,
                state_no,
                line_in,
                line_out,
                target,
                final_state,
                start_state,
            );
            j += 1;
            for m2 in mergesigma {
                for m in mergesigma {
                    if ((m.presence == presence
                        && m2.presence == presence
                        && m.number > IDENTITY
                        && m2.number > IDENTITY)
                        || (m.number == UNKNOWN && m2.number > IDENTITY && m2.presence == presence)
                        || (m2.number == UNKNOWN && m.number > IDENTITY && m.presence == presence))
                        && m.number != m2.number
                    {
                        add_fsm_arc(
                            &mut new_state,
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
                }
            }
        }

        /* Simply copy arcs that are not IDENTITY or UNKNOWN */
        if (line_in > IDENTITY || line_in == EPSILON)
            && (line_out > IDENTITY || line_out == EPSILON)
        {
            add_fsm_arc(
                &mut new_state,
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
                &mut new_state,
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

    add_fsm_arc(&mut new_state, j, -1, -1, -1, -1, -1, -1);
    drop(fsm);
    /* free(net->states); net->states = new_state */
    net.states = new_state.into();
}

// [spec:foma:def:constructions.fsm-merge-sigma-fn]
// [spec:foma:sem:constructions.fsm-merge-sigma-fn]
// [spec:foma:def:fomalib.fsm-merge-sigma-fn]
// [spec:foma:sem:fomalib.fsm-merge-sigma-fn]
pub fn fsm_merge_sigma(opts: &FomaOptions, net1: &mut Fsm, net2: &mut Fsm) {
    // Fast path: identical alphabets need no remapping, sigma rebuild, or
    // unknown-expansion — the full merge below would reproduce the same sigma
    // with an identity arc mapping. (DEVIATION from C, which always rebuilds;
    // observably identical, and both sigmas are kept sorted so a positional
    // compare is exact.) This is the common case when combining machines over a
    // shared alphabet (e.g. unioning many words into a lexicon).
    if net1.sigma == net2.sigma {
        return;
    }

    let mut end_1 = 0;
    let mut end_2 = 0;
    let mut equal = 1;
    let mut unknown_1 = 0;
    let mut unknown_2 = 0;

    if !opts.skip_word_boundary_marker {
        let in_1 = sigma_contains(".#.", &net1.sigma);
        let in_2 = sigma_contains(".#.", &net2.sigma);
        if in_1 && !in_2 {
            sigma_add(".#.", &mut net2.sigma);
            sigma_sort(net2);
        }
        if in_2 && !in_1 {
            sigma_add(".#.", &mut net1.sigma);
            sigma_sort(net1);
        }
    }

    let sigmasizes = sigma_max(&net1.sigma) + sigma_max(&net2.sigma) + 3;

    /* C: malloc'd (uninitialized); zero-filled here — entries are always
    written before being read for well-formed nets */
    let mut mapping_1: Vec<i32> = vec![0; sigmasizes as usize];
    let mut mapping_2: Vec<i32> = vec![0; sigmasizes as usize];

    /* Fill mergesigma */

    let mut mergesigma: Vec<Mergesigma> = Vec::new();

    /* Loop over sigma 1, sigma 2 — index cursors over each alphabet Vec; the
    cursor being past the end plays the role of C's NULL. */
    {
        let mut i1 = 0usize;
        let mut i2 = 0usize;
        loop {
            if i1 >= net1.sigma.len() {
                end_1 = 1;
            }
            if i2 >= net2.sigma.len() {
                end_2 = 1;
            }
            if end_1 != 0 && end_2 != 0 {
                break;
            }
            if end_2 != 0 {
                /* Treating only 1 now */
                let s1 = &net1.sigma[i1];
                mapping_1[s1.number as usize] = add_to_mergesigma(&mut mergesigma, s1, 1);
                i1 += 1;
                equal = 0;
                continue;
            } else if end_1 != 0 {
                /* Treating only 2 now */
                let s2 = &net2.sigma[i2];
                mapping_2[s2.number as usize] = add_to_mergesigma(&mut mergesigma, s2, 2);
                i2 += 1;
                equal = 0;
                continue;
            } else {
                /* Both alive */

                let s1 = &net1.sigma[i1];
                let s2 = &net2.sigma[i2];

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
                        add_to_mergesigma(&mut mergesigma, s1, 3);
                        i1 += 1;
                        i2 += 1;
                    } else if s1.number < s2.number {
                        add_to_mergesigma(&mut mergesigma, s1, 1);
                        i1 += 1;
                        equal = 0;
                    } else {
                        add_to_mergesigma(&mut mergesigma, s2, 2);
                        i2 += 1;
                        equal = 0;
                    }
                    continue;
                }
                /* Both contain non-special chars */
                /* strcmp — Rust str comparison is bytewise, as strcmp */
                let cmp = s1.symbol.cmp(&s2.symbol);
                if cmp == std::cmp::Ordering::Equal {
                    let mnum = add_to_mergesigma(&mut mergesigma, s1, 3);
                    /* Add symbol numbers to mapping */
                    mapping_1[s1.number as usize] = mnum;
                    mapping_2[s2.number as usize] = mnum;

                    i1 += 1;
                    i2 += 1;
                } else if cmp == std::cmp::Ordering::Less {
                    mapping_1[s1.number as usize] = add_to_mergesigma(&mut mergesigma, s1, 1);
                    i1 += 1;
                    equal = 0;
                } else {
                    mapping_2[s2.number as usize] = add_to_mergesigma(&mut mergesigma, s2, 2);
                    i2 += 1;
                    equal = 0;
                }
                continue;
            }
        }
    }

    /* Go over both net1 and net2 and replace arc numbers with new mappings */

    {
        let mut fsm1 = net1.states.rows_mut();
        let mut i = 0usize;
        while fsm1[i].state_no != -1 {
            if fsm1[i].r#in > 2 {
                fsm1[i].r#in = mapping_1[fsm1[i].r#in as usize] as i16;
            }
            if fsm1[i].out > 2 {
                fsm1[i].out = mapping_1[fsm1[i].out as usize] as i16;
            }
            i += 1;
        }
    }
    {
        let mut fsm2 = net2.states.rows_mut();
        let mut i = 0usize;
        while fsm2[i].state_no != -1 {
            if fsm2[i].r#in > 2 {
                fsm2[i].r#in = mapping_2[fsm2[i].r#in as usize] as i16;
            }
            if fsm2[i].out > 2 {
                fsm2[i].out = mapping_2[fsm2[i].out as usize] as i16;
            }
            i += 1;
        }
    }

    /* Copy mergesigma to net1, net2 */

    /* Both nets get the same merged alphabet; build it once and clone. */
    let new_sigma_1 = copy_mergesigma(&mergesigma);
    let new_sigma_2 = new_sigma_1.clone();

    fsm_sigma_destroy(core::mem::take(&mut net1.sigma));
    fsm_sigma_destroy(core::mem::take(&mut net2.sigma));

    net1.sigma = new_sigma_1;
    net2.sigma = new_sigma_2;

    /* Expand on ?, ?:x, y:? */

    if unknown_1 != 0 && equal == 0 {
        expand_unknowns(net1, &mergesigma, 2);
    }

    if unknown_2 != 0 && equal == 0 {
        expand_unknowns(net2, &mergesigma, 1);
    }
}

//! Wave-4 split of constructions.c (see mod.rs). Cross-module and
//! external names come via `use super::*` (re-exported by mod.rs).
use super::*;

// [spec:foma:def:constructions.sort-cmp-fn+1]
// [spec:foma:sem:constructions.sort-cmp-fn+1]
// [spec:foma:def:fomalibconf.sort-cmp-fn+1]
// [spec:foma:sem:fomalibconf.sort-cmp-fn+1]
pub fn sort_cmp(a: &FsmState, b: &FsmState) -> core::cmp::Ordering {
    /* C: qsort comparator returning a->state_no - b->state_no (ascending) */
    a.state_no.cmp(&b.state_no)
}

// [spec:foma:def:constructions.fsm-sort-lines-fn]
// [spec:foma:sem:constructions.fsm-sort-lines-fn]
// [spec:foma:def:fomalibconf.fsm-sort-lines-fn]
// [spec:foma:sem:fomalibconf.fsm-sort-lines-fn]
pub fn fsm_sort_lines(net: &mut Fsm) {
    let mut fsm = net.states.rows_mut();
    let count = find_arccount(&fsm);
    /* C: qsort (unstable) over the lines before the sentinel; a slice
    sort_unstable is an admissible qsort behavior */
    fsm[..count as usize].sort_unstable_by(sort_cmp);
}

// [spec:foma:def:constructions.fsm-update-flags-fn]
// [spec:foma:sem:constructions.fsm-update-flags-fn]
// [spec:foma:def:fomalibconf.fsm-update-flags-fn]
// [spec:foma:sem:fomalibconf.fsm-update-flags-fn]
pub fn fsm_update_flags(
    net: &mut Fsm,
    det: i32,
    pru: i32,
    min: i32,
    eps: i32,
    r#loop: i32,
    completed: i32,
) {
    net.is_deterministic = Tern::from_wire(det);
    net.is_pruned = Tern::from_wire(pru);
    net.is_minimized = Tern::from_wire(min);
    net.is_epsilon_free = Tern::from_wire(eps);
    net.is_loop_free = Tern::from_wire(r#loop);
    net.is_completed = Tern::from_wire(completed);
    net.arcs_sorted_in = false;
    net.arcs_sorted_out = false;
}

// [spec:foma:def:constructions.fsm-count-states-fn]
// [spec:foma:sem:constructions.fsm-count-states-fn]
pub fn fsm_count_states(fsm: &[FsmState]) -> i32 {
    let mut temp = -1;
    let mut states = 0;
    let mut i = 0usize;
    while fsm[i].state_no != -1 {
        if temp != fsm[i].state_no {
            states += 1;
            temp = fsm[i].state_no;
        }
        i += 1;
    }
    states
}

// [spec:foma:def:constructions.state-arr]
#[derive(Debug, Clone)]
pub struct StateArr {
    pub r#final: i32,
    pub start: i32,
    /* C: struct fsm_state *transitions — pointer to the state's first line;
    an index into the same line table here (interior pointer convention) */
    pub transitions: usize,
}

// [spec:foma:def:constructions.init-state-pointers-fn]
// [spec:foma:sem:constructions.init-state-pointers-fn]
pub fn init_state_pointers(fsm_state: &[FsmState]) -> Vec<StateArr> {
    /* Create an array for quick lookup of whether states are final, and a pointer to the first line regarding each state */

    let mut sold = -1;
    let states = fsm_count_states(fsm_state);
    /* C: malloc((states+1) entries) — uninitialized; the spare entry and the
    transitions fields start zeroed here */
    let mut state_arr: Vec<StateArr> = vec![
        StateArr {
            r#final: 0,
            start: 0,
            transitions: 0,
        };
        (states + 1) as usize
    ];
    for i in 0..states {
        state_arr[i as usize].r#final = 0;
        state_arr[i as usize].start = 0;
    }

    let mut i = 0usize;
    while fsm_state[i].state_no != -1 {
        if fsm_state[i].final_state == 1 {
            state_arr[fsm_state[i].state_no as usize].r#final = 1;
        }
        if fsm_state[i].start_state == 1 {
            state_arr[fsm_state[i].state_no as usize].start = 1;
        }
        if fsm_state[i].state_no != sold {
            state_arr[fsm_state[i].state_no as usize].transitions = i;
            sold = fsm_state[i].state_no;
        }
        i += 1;
    }
    state_arr
}

// [spec:foma:def:constructions.add-fsm-arc-fn]
// [spec:foma:sem:constructions.add-fsm-arc-fn]
// [spec:foma:def:fomalibconf.add-fsm-arc-fn]
// [spec:foma:sem:fomalibconf.add-fsm-arc-fn]
#[allow(clippy::too_many_arguments)]
pub fn add_fsm_arc(
    fsm: &mut [FsmState],
    offset: i32,
    state_no: i32,
    r#in: i32,
    out: i32,
    target: i32,
    final_state: i32,
    start_state: i32,
) -> i32 {
    let mut offset = offset;
    let line = &mut fsm[offset as usize];
    line.state_no = state_no;
    /* int→short / int→char truncation as in C */
    line.r#in = r#in as i16;
    line.out = out as i16;
    line.target = target;
    line.final_state = final_state as i8;
    line.start_state = start_state as i8;
    offset += 1;
    offset
}

// [spec:foma:def:constructions.fsm-count-fn]
// [spec:foma:sem:constructions.fsm-count-fn]
// [spec:foma:def:fomalibconf.fsm-count-fn]
// [spec:foma:sem:fomalibconf.fsm-count-fn]
pub fn fsm_count(net: &mut Fsm) {
    /* Counts read straight off the compressed blocks: one flat row per arc, one
    marker row per arc-less state, plus the terminator; final/start is a per-
    state property (a block's first-row value), so finalcount is just the count
    of final blocks — exactly what the old per-state-boundary walk computed. */
    let mut linecount = 0;
    let mut arccount = 0;
    let mut finalcount = 0;
    let mut maxstate = 0;

    for b in net.states.blocks() {
        if b.state_no > maxstate {
            maxstate = b.state_no;
        }
        linecount += (b.arc_len as i32).max(1);
        arccount += b.arc_len as i32;
        if b.final_state != 0 {
            finalcount += 1;
        }
    }

    linecount += 1;
    net.statecount = maxstate + 1;
    net.linecount = linecount;
    net.arccount = arccount;
    net.finalcount = finalcount;
}

// [spec:foma:def:constructions.fsm-add-to-states-fn]
// [spec:foma:sem:constructions.fsm-add-to-states-fn]
pub(crate) fn fsm_add_to_states(net: &mut Fsm, add: i32) {
    let mut fsm = net.states.rows_mut();
    let mut i = 0usize;
    while fsm[i].state_no != -1 {
        fsm[i].state_no += add;
        if fsm[i].target != -1 {
            fsm[i].target += add;
        }
        i += 1;
    }
}

/* _marktail(?* L, 0:x) does ~$x .o. [..] -> x || L _ ;   */
/* _marktail(?* R.r, 0:x).r does ~$x .o. [..] -> x || _ R */

// [spec:foma:def:constructions.fsm-mark-fsm-tail-fn]
// [spec:foma:sem:constructions.fsm-mark-fsm-tail-fn]
// [spec:foma:def:fomalib.fsm-mark-fsm-tail-fn]
// [spec:foma:sem:fomalib.fsm-mark-fsm-tail-fn]
pub fn fsm_mark_fsm_tail(net: Fsm, marker: &Fsm) -> Fsm {
    let mut inh = fsm_read_init(net);
    /* C: the read handle borrows marker (which is NOT destroyed); the
    Rust handle owns its net, so it reads a deep copy of marker —
    read-only, observably equivalent */
    let mut minh = fsm_read_init(marker.clone());

    let name = inh
        .net
        .as_ref()
        .expect("net present until fsm_read_done")
        .name
        .clone();
    let mut outh = fsm_construct_init(&name);
    fsm_construct_copy_sigma(
        &mut outh,
        &inh.net
            .as_ref()
            .expect("net present until fsm_read_done")
            .sigma,
    );

    let statecount = inh
        .net
        .as_ref()
        .expect("net present until fsm_read_done")
        .statecount;
    /* calloc — zeroed; 0 means "unset" (fresh numbers start at
    statecount >= 1) */
    let mut mappings: Vec<i32> = vec![0; statecount as usize];
    let mut maxstate = statecount;

    while fsm_get_next_arc(&mut inh) != 0 {
        let target = fsm_get_arc_target(&inh);
        if fsm_read_is_final(&inh, target) {
            let newtarget;
            if mappings[target as usize] == 0 {
                newtarget = maxstate;
                mappings[target as usize] = newtarget;
                fsm_read_reset(Some(&mut minh));
                while fsm_get_next_arc(&mut minh) != 0 {
                    let min_in = fsm_get_arc_in(&minh)
                        .expect("arc label present on the positioned cursor")
                        .to_string();
                    let min_out = fsm_get_arc_out(&minh)
                        .expect("arc label present on the positioned cursor")
                        .to_string();
                    fsm_construct_add_arc(&mut outh, newtarget, target, &min_in, &min_out);
                }
                maxstate += 1;
            } else {
                newtarget = mappings[target as usize];
            }
            let (source, num_in, num_out) = (
                fsm_get_arc_source(&inh),
                fsm_get_arc_num_in(&inh),
                fsm_get_arc_num_out(&inh),
            );
            fsm_construct_add_arc_nums(&mut outh, source, newtarget, num_in, num_out);
        } else {
            let (source, num_in, num_out) = (
                fsm_get_arc_source(&inh),
                fsm_get_arc_num_in(&inh),
                fsm_get_arc_num_out(&inh),
            );
            fsm_construct_add_arc_nums(&mut outh, source, target, num_in, num_out);
        }
    }
    for i in 0..statecount {
        fsm_construct_set_final(&mut outh, i);
    }

    fsm_construct_set_initial(&mut outh, 0);
    let net = fsm_read_done(inh);
    /* fsm_read_done(minh) — frees the handle; the marker copy is dropped
    with it (the C caller keeps the original marker) */
    let marker_copy = fsm_read_done(minh);
    drop(marker_copy);
    let newnet = fsm_construct_done(outh);
    fsm_destroy(net);
    /* free(mappings) */
    drop(mappings);
    newnet
}

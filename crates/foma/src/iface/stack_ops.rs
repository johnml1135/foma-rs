//! foma/iface.c Wave-4 split: stack commands (pop/turn/rotate/load/save/
//! name/print-name/quit). See iface/mod.rs.
use super::*;

// [spec:foma:def:iface.iface-load-stack-fn]
// [spec:foma:sem:iface.iface-load-stack-fn]
// [spec:foma:def:foma.iface-load-stack-fn]
// [spec:foma:sem:foma.iface-load-stack-fn]
pub fn iface_load_stack(session: &mut Session, filename: &str) {
    let mut fsrh = fsm_read_binary_file_multiple_init(filename);
    if fsrh.is_none() {
        eprint!("{}: ", filename);
        perror("File error");
        return;
    }
    while let Some(net) = fsm_read_binary_file_multiple(&mut fsrh) {
        session.stack_add(net);
    }
}

// [spec:foma:def:iface.iface-pop-fn]
// [spec:foma:sem:iface.iface-pop-fn]
// [spec:foma:def:foma.iface-pop-fn]
// [spec:foma:sem:foma.iface-pop-fn]
pub fn iface_pop(session: &mut Session) {
    if session.stack_size() < 1 {
        print!("Stack is empty.\n");
    } else {
        let net = session.stack_pop().unwrap();
        fsm_destroy(net);
    }
}

// [spec:foma:def:iface.iface-name-net-fn]
// [spec:foma:sem:iface.iface-name-net-fn+1]
// [spec:foma:def:foma.iface-name-net-fn]
// [spec:foma:sem:foma.iface-name-net-fn+1]
pub fn iface_name_net(session: &mut Session, name: &str) {
    if iface_stack_check(session, 1) {
        let top = session.stack_find_top().unwrap();
        session.stack_entry_fsm(top, |f| {
            // [spec:foma:sem:iface.iface-name-net-fn+1] store the name in full. C
            // used a fixed char[40] field (strncpy without a NUL terminator for
            // names >= 40 bytes), truncating longer names.
            f.name = name.to_string();
        });
        iface_print_name(session);
    }
}

// [spec:foma:def:iface.iface-print-name-fn]
// [spec:foma:sem:iface.iface-print-name-fn]
// [spec:foma:def:foma.iface-print-name-fn]
// [spec:foma:sem:foma.iface-print-name-fn]
pub fn iface_print_name(session: &mut Session) {
    if iface_stack_check(session, 1) {
        let top = session.stack_find_top().unwrap();
        let name = session.stack_entry_fsm(top, |f| f.name.clone());
        print!("{}\n", name);
    }
}

// [spec:foma:def:iface.iface-quit-fn]
// [spec:foma:sem:iface.iface-quit-fn]
// [spec:foma:def:foma.iface-quit-fn]
// [spec:foma:sem:foma.iface-quit-fn]
pub fn iface_quit(session: &mut Session) {
    // remove_defined(g_defines, NULL) — NULL name destroys every defined net.
    remove_defined(&mut session.defines, None);
    while !session.stack_isempty() {
        let net = session.stack_pop().unwrap();
        fsm_destroy(net);
    }
    std::process::exit(0);
}

// [spec:foma:def:iface.iface-rotate-fn]
// [spec:foma:sem:iface.iface-rotate-fn]
// [spec:foma:def:foma.iface-rotate-fn]
// [spec:foma:sem:foma.iface-rotate-fn]
pub fn iface_rotate(session: &mut Session) {
    if iface_stack_check(session, 1) {
        session.stack_rotate();
    }
}

// [spec:foma:def:iface.iface-save-stack-fn]
// [spec:foma:sem:iface.iface-save-stack-fn]
// [spec:foma:def:foma.iface-save-stack-fn]
// [spec:foma:sem:foma.iface-save-stack-fn]
pub fn iface_save_stack(session: &mut Session, filename: &str) {
    if iface_stack_check(session, 1) {
        // gzopen(filename, "wb") — File::create + GzEncoder.
        let file = match File::create(filename) {
            Ok(f) => f,
            Err(_) => {
                print!("Error opening file {} for writing.\n", filename);
                return;
            }
        };
        print!("Writing to file {}.\n", filename);
        let mut outfile = GzEncoder::new(file, Compression::default());
        // for (stack_ptr = stack_find_bottom(); stack_ptr->next != NULL; stack_ptr = stack_ptr->next)
        let mut stack_ptr = session.stack_find_bottom().unwrap();
        while session.stack_entry_next(stack_ptr).is_some() {
            session.stack_entry_fsm(stack_ptr, |f| foma_net_print(f, &mut outfile));
            stack_ptr = session.stack_entry_next(stack_ptr).unwrap();
        }
        // gzclose(outfile)
        let _ = outfile.finish();
    }
}

// [spec:foma:def:iface.iface-turn-fn]
// [spec:foma:sem:iface.iface-turn-fn+1]
// [spec:foma:def:foma.iface-turn-fn]
// [spec:foma:sem:foma.iface-turn-fn+1]
pub fn iface_turn(session: &mut Session) {
    // [spec:foma:sem:iface.iface-turn-fn+1] "turn stack" reverses the whole stack
    // via stack_turn(). C wired it to stack_rotate() (a top/bottom swap), which
    // contradicted the "turns stack upside down" help text.
    if iface_stack_check(session, 1) {
        session.stack_turn();
    }
}

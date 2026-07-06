//! foma/iface.c Wave-4 split: stack commands (pop/turn/rotate/load/save/
//! name/print-name/quit). See iface/mod.rs.
use super::*;

// [spec:foma:def:iface.iface-load-stack-fn]
// [spec:foma:sem:iface.iface-load-stack-fn]
// [spec:foma:def:foma.iface-load-stack-fn]
// [spec:foma:sem:foma.iface-load-stack-fn]
pub fn iface_load_stack(filename: &str) {
    let mut fsrh = fsm_read_binary_file_multiple_init(filename);
    if fsrh.is_none() {
        eprint!("{}: ", filename);
        perror("File error");
        return;
    }
    while let Some(net) = fsm_read_binary_file_multiple(&mut fsrh) {
        stack_add(net);
    }
}

// [spec:foma:def:iface.iface-pop-fn]
// [spec:foma:sem:iface.iface-pop-fn]
// [spec:foma:def:foma.iface-pop-fn]
// [spec:foma:sem:foma.iface-pop-fn]
pub fn iface_pop() {
    if stack_size() < 1 {
        print!("Stack is empty.\n");
    } else {
        let net = stack_pop().unwrap();
        fsm_destroy(net);
    }
}

// [spec:foma:def:iface.iface-name-net-fn]
// [spec:foma:sem:iface.iface-name-net-fn]
// [spec:foma:def:foma.iface-name-net-fn]
// [spec:foma:sem:foma.iface-name-net-fn]
pub fn iface_name_net(name: &str) {
    if iface_stack_check(1) != 0 {
        let top = stack_find_top().unwrap();
        stack_entry_fsm(top, |f| {
            // strncpy(top->fsm->name, name, 40): copy <= 40 bytes; when
            // strlen(name) >= 40 the field is left WITHOUT a NUL terminator, i.e.
            // truncated to 40 bytes (latent bug — reproduced literally).
            let bytes = name.as_bytes();
            let n = if bytes.len() < 40 { bytes.len() } else { 40 };
            f.name = String::from_utf8_lossy(&bytes[..n]).into_owned();
        });
        iface_print_name();
    }
}

// [spec:foma:def:iface.iface-print-name-fn]
// [spec:foma:sem:iface.iface-print-name-fn]
// [spec:foma:def:foma.iface-print-name-fn]
// [spec:foma:sem:foma.iface-print-name-fn]
pub fn iface_print_name() {
    if iface_stack_check(1) != 0 {
        let top = stack_find_top().unwrap();
        let name = stack_entry_fsm(top, |f| f.name.clone());
        print!("{}\n", name);
    }
}

// [spec:foma:def:iface.iface-quit-fn]
// [spec:foma:sem:iface.iface-quit-fn]
// [spec:foma:def:foma.iface-quit-fn]
// [spec:foma:sem:foma.iface-quit-fn]
pub fn iface_quit() {
    G_DEFINES.with(|g| {
        let mut g = g.borrow_mut();
        // remove_defined(g_defines, NULL) — NULL name destroys every defined net.
        if let Some(d) = g.as_deref_mut() {
            remove_defined(d, None);
        }
    });
    while stack_isempty() == 0 {
        let net = stack_pop().unwrap();
        fsm_destroy(net);
    }
    std::process::exit(0);
}

// [spec:foma:def:iface.iface-rotate-fn]
// [spec:foma:sem:iface.iface-rotate-fn]
// [spec:foma:def:foma.iface-rotate-fn]
// [spec:foma:sem:foma.iface-rotate-fn]
pub fn iface_rotate() {
    if iface_stack_check(1) != 0 {
        stack_rotate();
    }
}

// [spec:foma:def:iface.iface-save-stack-fn]
// [spec:foma:sem:iface.iface-save-stack-fn]
// [spec:foma:def:foma.iface-save-stack-fn]
// [spec:foma:sem:foma.iface-save-stack-fn]
pub fn iface_save_stack(filename: &str) {
    if iface_stack_check(1) != 0 {
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
        let mut stack_ptr = stack_find_bottom().unwrap();
        while stack_entry_next(stack_ptr).is_some() {
            stack_entry_fsm(stack_ptr, |f| foma_net_print(f, &mut outfile));
            stack_ptr = stack_entry_next(stack_ptr).unwrap();
        }
        // gzclose(outfile)
        let _ = outfile.finish();
    }
}

// [spec:foma:def:iface.iface-turn-fn]
// [spec:foma:sem:iface.iface-turn-fn]
// [spec:foma:def:foma.iface-turn-fn]
// [spec:foma:sem:foma.iface-turn-fn]
pub fn iface_turn() {
    // Latent bug reproduced: "turn stack" calls stack_rotate() (byte-for-byte the
    // same as iface_rotate), NOT stack_turn(); it only swaps top/bottom fsms.
    if iface_stack_check(1) != 0 {
        stack_rotate();
    }
}

//! foma/iface.c Wave-4 split: read/write/save/load file commands.
//! See iface/mod.rs.
use super::*;

// [spec:foma:def:iface.iface-load-defined-fn]
// [spec:foma:sem:iface.iface-load-defined-fn]
// [spec:foma:def:foma.iface-load-defined-fn]
// [spec:foma:sem:foma.iface-load-defined-fn]
pub fn iface_load_defined(filename: &str) {
    G_DEFINES.with(|g| {
        let mut g = g.borrow_mut();
        // C: load_defined(g_defines, filename); g_defines is the init'd dummy head.
        if let Some(d) = g.as_deref_mut() {
            load_defined(d, filename);
        }
    });
}

// [spec:foma:def:iface.iface-read-att-fn]
// [spec:foma:sem:iface.iface-read-att-fn]
// [spec:foma:def:foma.iface-read-att-fn]
// [spec:foma:sem:foma.iface-read-att-fn]
pub fn iface_read_att(filename: &str) -> i32 {
    print!("Reading AT&T file: {}\n", filename);
    match read_att(filename) {
        None => {
            eprint!("{}: ", filename);
            perror("Error opening file");
            1
        }
        Some(tempnet) => {
            stack_add(tempnet);
            0
        }
    }
}

// [spec:foma:def:iface.iface-read-prolog-fn]
// [spec:foma:sem:iface.iface-read-prolog-fn]
// [spec:foma:def:foma.iface-read-prolog-fn]
// [spec:foma:sem:foma.iface-read-prolog-fn]
pub fn iface_read_prolog(filename: &str) -> i32 {
    print!("Reading prolog: {}\n", filename);
    match fsm_read_prolog(filename) {
        None => {
            eprint!("{}: ", filename);
            perror("Error opening file");
            1
        }
        Some(tempnet) => {
            stack_add(tempnet);
            0
        }
    }
}

// [spec:foma:def:iface.iface-read-spaced-text-fn]
// [spec:foma:sem:iface.iface-read-spaced-text-fn]
// [spec:foma:def:foma.iface-read-spaced-text-fn]
// [spec:foma:sem:foma.iface-read-spaced-text-fn]
pub fn iface_read_spaced_text(filename: &str) -> i32 {
    match fsm_read_spaced_text_file(filename) {
        None => {
            eprint!("{}: ", filename);
            perror("File error");
            1
        }
        Some(net) => {
            stack_add(fsm_topsort(fsm_minimize(net)));
            0
        }
    }
}

// [spec:foma:def:iface.iface-read-text-fn]
// [spec:foma:sem:iface.iface-read-text-fn]
// [spec:foma:def:foma.iface-read-text-fn]
// [spec:foma:sem:foma.iface-read-text-fn]
pub fn iface_read_text(filename: &str) -> i32 {
    match fsm_read_text_file(filename) {
        None => {
            eprint!("{}: ", filename);
            perror("File error");
            1
        }
        Some(net) => {
            stack_add(fsm_topsort(fsm_minimize(net)));
            0
        }
    }
}

// [spec:foma:def:iface.iface-save-defined-fn]
// [spec:foma:sem:iface.iface-save-defined-fn]
// [spec:foma:def:foma.iface-save-defined-fn]
// [spec:foma:sem:foma.iface-save-defined-fn]
pub fn iface_save_defined(filename: &str) {
    G_DEFINES.with(|g| {
        let mut g = g.borrow_mut();
        // save_defined(g_defines, filename): the C helper prints "No defined
        // networks.\n" to stderr when g_defines is NULL; a &mut can't be NULL, so
        // (per io.rs save_defined) that check lives at this call site.
        match g.as_deref_mut() {
            None => {
                eprint!("No defined networks.\n");
            }
            Some(d) => {
                save_defined(d, filename);
            }
        }
    });
}

// [spec:foma:def:iface.iface-write-att-fn]
// [spec:foma:sem:iface.iface-write-att-fn]
// [spec:foma:def:foma.iface-write-att-fn]
// [spec:foma:sem:foma.iface-write-att-fn]
pub fn iface_write_att(filename: Option<&str>) -> i32 {
    if iface_stack_check(1) == 0 {
        return 1;
    }
    let top = stack_find_top().unwrap();
    let mut outfile: Output = match filename {
        None => Output::Stdout(std::io::stdout()),
        Some(name) => {
            print!("Writing AT&T file: {}\n", name);
            match File::create(name) {
                Ok(f) => Output::File(f),
                Err(_) => {
                    eprint!("{}: ", name);
                    perror("File error opening.");
                    return 1;
                }
            }
        }
    };
    stack_entry_fsm(top, |f| net_print_att(f, &mut outfile));
    // fclose only when filename != NULL; stdout is not closed. Both drop here.
    0
}

// [spec:foma:def:iface.iface-write-prolog-fn]
// [spec:foma:sem:iface.iface-write-prolog-fn]
// [spec:foma:def:foma.iface-write-prolog-fn]
// [spec:foma:sem:foma.iface-write-prolog-fn]
pub fn iface_write_prolog(filename: Option<&str>) {
    if iface_stack_check(1) != 0 {
        let top = stack_find_top().unwrap();
        stack_entry_fsm(top, |f| foma_write_prolog(f, filename));
    }
}

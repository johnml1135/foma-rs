//! foma/iface.c Wave-4 split: shared static helpers (sigptr / print_* /
//! view_net / print_stats / perror). `use super::*` resolves to the iface
//! module, which re-exports every submodule's surface plus the external deps.
use super::*;

// DEVIATION from C: perror(s) prints "s: <strerror(errno)>\n" to stderr. Rust has
// no libc errno; `std::io::Error::last_os_error()` reads the current thread's
// errno (set by the preceding failed syscall) and its Display is close to
// strerror (adds "(os error N)"). Unannotated plumbing shared across submodules.
pub(crate) fn perror(s: &str) {
    eprint!("{}: {}\n", s, std::io::Error::last_os_error());
}

// [spec:foma:def:iface.sigptr-fn]
// [spec:foma:sem:iface.sigptr-fn]
// Static helper (C: `static char *sigptr`). Returns an owned display string; C
// returns borrowed static/sigma pointers or a leaked malloc'd "NONE(%i)", but
// the owned String here is observably identical.
pub(crate) fn sigptr(sigma: &[Sigma], number: i32) -> String {
    if number == EPSILON {
        return "0".to_string();
    }
    if number == UNKNOWN {
        return "?".to_string();
    }
    if number == IDENTITY {
        return "@".to_string();
    }
    for node in sigma {
        if node.number == number {
            let sym = node.symbol.as_str();
            if sym == "0" {
                return "\"0\"".to_string();
            }
            if sym == "?" {
                return "\"?\"".to_string();
            }
            if sym == "\n" {
                return "\\n".to_string();
            }
            if sym == "\r" {
                return "\\r".to_string();
            }
            return sym.to_string();
        }
    }
    // malloc(40) + snprintf "NONE(%i)" — leaked in C.
    format!("NONE({})", number)
}

// [spec:foma:def:iface.print-net-fn]
// [spec:foma:sem:iface.print-net-fn]
pub(crate) fn print_net(net: &mut Fsm, filename: Option<&str>) -> i32 {
    let mut out: Output = match filename {
        None => Output::Stdout(std::io::stdout()),
        Some(name) => match File::create(name) {
            Ok(f) => Output::File(f),
            Err(_) => {
                print!("Error writing to file {}. Using stdout.\n", name);
                Output::Stdout(std::io::stdout())
            }
        },
    };
    // C prints this unconditionally after the fopen block (even after fallback).
    if let Some(name) = filename {
        print!("Writing network to file {}.\n", name);
    }
    fsm_count(net);
    let mut finals = vec![0i32; net.statecount as usize];
    let mut i = 0usize;
    loop {
        let state_no = net.states[i].state_no;
        if state_no == -1 {
            break;
        }
        let final_state = net.states[i].final_state;
        let in_ = net.states[i].r#in;
        let out_ = net.states[i].out;
        finals[state_no as usize] = if final_state == 1 { 1 } else { 0 };
        if in_ != out_ {
            net.arity = 2;
        }
        i += 1;
    }
    print_sigma(&net.sigma, &mut out);
    let _ = write!(out, "Net: {}\n", net.name);
    let _ = write!(out, "Flags: ");
    if net.is_deterministic == YES {
        let _ = write!(out, "deterministic ");
    }
    if net.is_pruned == YES {
        let _ = write!(out, "pruned ");
    }
    if net.is_minimized == YES {
        let _ = write!(out, "minimized ");
    }
    if net.is_epsilon_free == YES {
        let _ = write!(out, "epsilon_free ");
    }
    if net.is_loop_free != 0 {
        let _ = write!(out, "loop_free ");
    }
    if net.arcs_sorted_in != 0 {
        let _ = write!(out, "arcs_sorted_in ");
    }
    if net.arcs_sorted_out != 0 {
        let _ = write!(out, "arcs_sorted_out ");
    }
    let _ = write!(out, "\n");
    let _ = write!(out, "Arity: {}\n", net.arity);
    let mut previous_state: i32 = -1;
    let mut i = 0usize;
    loop {
        let state_no = net.states[i].state_no;
        if state_no == -1 {
            break;
        }
        let start_state = net.states[i].start_state;
        let final_state = net.states[i].final_state;
        let in_ = net.states[i].r#in as i32;
        let out_ = net.states[i].out as i32;
        let target = net.states[i].target;
        if state_no != previous_state {
            if start_state != 0 {
                let _ = write!(out, "S");
            }
            if final_state != 0 {
                let _ = write!(out, "f");
            }
            if in_ == -1 {
                let _ = write!(out, "s{}:\t(no arcs).\n", state_no);
                i += 1;
                continue;
            } else {
                let _ = write!(out, "s{}:\t", state_no);
            }
        }
        previous_state = state_no;
        if in_ == out_ {
            if in_ == IDENTITY {
                let _ = write!(out, "@ -> ");
            } else if in_ == UNKNOWN {
                let _ = write!(out, "?:? -> ");
            } else {
                let _ = write!(out, "{} -> ", sigptr(&net.sigma, in_));
            }
        } else {
            let _ = write!(
                out,
                "<{}:{}> -> ",
                sigptr(&net.sigma, in_),
                sigptr(&net.sigma, out_)
            );
        }
        if finals[target as usize] == 1 {
            let _ = write!(out, "f");
        }
        let _ = write!(out, "s{}", target);
        if net.states[i + 1].state_no == state_no {
            let _ = write!(out, ", ");
        } else {
            let _ = write!(out, ".\n");
        }
        i += 1;
    }
    // fclose only when filename != NULL; free finals. All drop at scope end.
    0
}

// [spec:foma:def:iface.print-mem-size-fn]
// [spec:foma:sem:iface.print-mem-size-fn]
pub(crate) fn print_mem_size(net: &Fsm) {
    // DEVIATION from C: the byte total uses C's LP64 sizeof(struct sigma/fsm/
    // fsm_state) = 24 / 128 / 16. Rust's own struct layouts differ (String/Vec/
    // Option<Box>), so the C ABI sizes are hardcoded to keep the printed size
    // byte-identical on a 64-bit build.
    const SIZEOF_SIGMA: u32 = 24;
    const SIZEOF_FSM: u32 = 128;
    const SIZEOF_FSM_STATE: u32 = 16;
    let mut s: u32 = 0;
    for node in &net.sigma {
        let symlen = node.symbol.len() as u32;
        s = s
            .wrapping_add(symlen)
            .wrapping_add(1)
            .wrapping_add(SIZEOF_SIGMA);
    }
    s = s.wrapping_add(SIZEOF_FSM);
    s = s.wrapping_add(SIZEOF_FSM_STATE.wrapping_mul(net.linecount as u32));
    let sf = s as f32;
    let size: String;
    if s < 1024 {
        size = format!("{} bytes. ", s);
    } else if s >= 1024 && s < 1048576 {
        size = format!("{:.1} kB. ", (sf / 1024.0f32) as f64);
    } else if s >= 1048576 && s < 1073741824 {
        size = format!("{:.1} MB. ", (sf / 1048576.0f32) as f64);
    } else {
        size = format!("{:.1} GB. ", (sf / 1073741824.0f32) as f64);
    }
    print!("{}", size);
    let _ = std::io::stdout().flush();
}

// [spec:foma:def:iface.print-stats-fn]
// [spec:foma:sem:iface.print-stats-fn]
// [spec:foma:def:foma.print-stats-fn]
// [spec:foma:sem:foma.print-stats-fn]
pub fn print_stats(net: &Fsm) -> i32 {
    print_mem_size(net);
    if net.statecount == 1 {
        print!("1 state, ");
    } else {
        print!("{} states, ", net.statecount);
    }
    if net.arccount == 1 {
        print!("1 arc, ");
    } else {
        print!("{} arcs, ", net.arccount);
    }
    if net.pathcount == 1 {
        print!("1 path");
    } else if net.pathcount == -1 {
        print!("Cyclic");
    } else if net.pathcount == -2 {
        // more than %lld paths with LLONG_MAX
        print!("more than {} paths", i64::MAX);
    } else if net.pathcount == -3 {
        print!("unknown number of paths");
    } else {
        print!("{} paths", net.pathcount);
    }
    print!(".\n");
    0
}

// [spec:foma:def:iface.print-sigma-fn]
// [spec:foma:sem:iface.print-sigma-fn]
pub(crate) fn print_sigma<W: std::io::Write + ?Sized>(sigma: &[Sigma], out: &mut W) -> i32 {
    let mut size = 0;
    let _ = write!(out, "Sigma:");
    for node in sigma {
        if node.number > 2 {
            let _ = write!(out, " {}", node.symbol);
            size += 1;
        }
        if node.number == IDENTITY {
            let _ = write!(out, " {}", "@");
        }
        if node.number == UNKNOWN {
            let _ = write!(out, " {}", "?");
        }
    }
    let _ = write!(out, "\n");
    let _ = write!(out, "Size: {}.\n", size);
    1
}

// [spec:foma:def:iface.print-dot-fn]
// [spec:foma:sem:iface.print-dot-fn+1]
pub(crate) fn print_dot(net: &mut Fsm, filename: Option<&str>) -> i32 {
    fsm_count(net);
    let mut finals = vec![0i16; net.statecount as usize];
    let mut i = 0usize;
    loop {
        let state_no = net.states[i].state_no;
        if state_no == -1 {
            break;
        }
        finals[state_no as usize] = if net.states[i].final_state == 1 { 1 } else { 0 };
        i += 1;
    }
    let mut dotfile: Output = match filename {
        // C: `dotfile = fopen(filename,"w");` with NO NULL check (latent crash on
        // failure). Report the error and return instead of crashing, matching the
        // other file-writing commands (e.g. iface_write_att).
        Some(name) => match File::create(name) {
            Ok(f) => Output::File(f),
            Err(_) => {
                eprint!("{}: ", name);
                perror("Error opening dot file.");
                return 1;
            }
        },
        None => Output::Stdout(std::io::stdout()),
    };
    let _ = write!(dotfile, "digraph A {{\nrankdir = LR;\n");
    for i in 0..net.statecount {
        if finals[i as usize] != 0 {
            let _ = write!(dotfile, "node [shape=doublecircle,style=filled] {}\n", i);
        } else {
            let _ = write!(dotfile, "node [shape=circle,style=filled] {}\n", i);
        }
    }
    // C: calloc(linecount, sizeof(printed)) allocates sizeof(POINTER) per line
    // (over-allocation bug, harmless); here a per-line flag Vec of linecount.
    let mut printed = vec![0i16; net.linecount as usize];
    let mut i = 0usize;
    loop {
        let state_no_i = net.states[i].state_no;
        if state_no_i == -1 {
            break;
        }
        let target_i = net.states[i].target;
        if target_i == -1 || printed[i] == 1 {
            i += 1;
            continue;
        }
        let _ = write!(dotfile, "{} -> {} [label=\"", state_no_i, target_i);
        let mut linelen = 0i32;
        let mut j = i;
        while net.states[j].state_no == state_no_i {
            let target_j = net.states[j].target;
            if target_i == target_j && printed[j] == 0 {
                printed[j] = 1;
                let in_j = net.states[j].r#in as i32;
                let out_j = net.states[j].out as i32;
                if in_j == out_j && out_j != UNKNOWN {
                    let sig = sigptr(&net.sigma, in_j);
                    let _ = dotfile.write_all(&escape_string(sig.as_bytes(), b'"'));
                    linelen += sig.len() as i32;
                } else {
                    let sig_in = sigptr(&net.sigma, in_j);
                    let sig_out = sigptr(&net.sigma, out_j);
                    let _ = dotfile.write_all(b"<");
                    let _ = dotfile.write_all(&escape_string(sig_in.as_bytes(), b'"'));
                    let _ = dotfile.write_all(b":");
                    let _ = dotfile.write_all(&escape_string(sig_out.as_bytes(), b'"'));
                    let _ = dotfile.write_all(b">");
                    linelen += sig_in.len() as i32 + sig_out.len() as i32 + 3;
                }
                if linelen > 12 {
                    let _ = write!(dotfile, "\\n");
                    linelen = 0;
                } else {
                    let _ = write!(dotfile, " ");
                }
            }
            j += 1;
        }
        let _ = write!(dotfile, "\"];\n");
        i += 1;
    }
    // free(finals); free(printed).
    let _ = write!(dotfile, "}}\n");
    // fclose only when filename != NULL — dropped at scope end.
    1
}

// [spec:foma:def:iface.view-net-fn]
// [spec:foma:sem:iface.view-net-fn]
// [spec:foma:def:foma.view-net-fn]
// [spec:foma:sem:foma.view-net-fn]
pub(crate) fn view_net(net: &mut Fsm) -> i32 {
    // DEVIATION from C: no tempnam(); a unique temp path is built under the system
    // temp dir from the pid + a per-thread counter (observably a unique file).
    fn tempnam_foma() -> String {
        thread_local! { static CTR: Cell<u64> = const { Cell::new(0) }; }
        let n = CTR.with(|c| {
            let v = c.get();
            c.set(v + 1);
            v
        });
        std::env::temp_dir()
            .join(format!("foma{}_{}", std::process::id(), n))
            .to_string_lossy()
            .into_owned()
    }
    let dotname = format!("{}.dot", tempnam_foma());
    print_dot(net, Some(&dotname));
    let pngname = tempnam_foma();
    // DEVIATION from C: system(cmd) → `/bin/sh -c "<cmd>"` via std::process::Command
    // (a spawn failure ↔ C's system() == -1; the exit status is otherwise ignored).
    let cmd1 = if cfg!(target_os = "macos") {
        format!("dot -Tpng {} > {}.png ", dotname, pngname)
    } else {
        format!("dot -Tpng {} > {} ", dotname, pngname)
    };
    if std::process::Command::new("sh")
        .arg("-c")
        .arg(&cmd1)
        .status()
        .is_err()
    {
        print!("Error writing tempfile.\n");
    }
    let cmd2 = if cfg!(target_os = "macos") {
        format!("/usr/bin/open {}.png 2>/dev/null &", pngname)
    } else {
        format!("/usr/bin/xdg-open {} 2>/dev/null &", pngname)
    };
    if std::process::Command::new("sh")
        .arg("-c")
        .arg(&cmd2)
        .status()
        .is_err()
    {
        print!("Error opening viewer.\n");
    }
    // free(pngname); free(dotname) — temp files are never deleted (as in C).
    1
}

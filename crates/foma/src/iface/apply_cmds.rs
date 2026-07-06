//! foma/iface.c Wave-4 split: apply/enumeration commands (apply up/down/med/
//! file, random-*, words, pairs, shortest-string). See iface/mod.rs.
use super::*;

/// C: `#define LINE_LIMIT 8192` — fgets buffer size in iface_apply_file.
const LINE_LIMIT: usize = 8192;

// [spec:foma:def:iface.iface-apply-set-params-fn]
// [spec:foma:sem:iface.iface-apply-set-params-fn]
// [spec:foma:def:foma.iface-apply-set-params-fn]
// [spec:foma:sem:foma.iface-apply-set-params-fn]
pub fn iface_apply_set_params(h: &mut ApplyHandle) {
    apply_set_print_space(h, G_PRINT_SPACE.with(|v| v.get()));
    apply_set_print_pairs(h, G_PRINT_PAIRS.with(|v| v.get()));
    apply_set_show_flags(h, G_SHOW_FLAGS.with(|v| v.get()));
    apply_set_obey_flags(h, G_OBEY_FLAGS.with(|v| v.get()));
}

// [spec:foma:def:iface.iface-apply-med-fn]
// [spec:foma:sem:iface.iface-apply-med-fn]
// [spec:foma:def:foma.iface-apply-med-fn]
// [spec:foma:sem:foma.iface-apply-med-fn]
pub fn iface_apply_med(word: &str) {
    if iface_stack_check(1) == 0 {
        return;
    }
    // amedh = stack_get_med_ah() — arena index of the top entry (see module notes)
    let amedh = stack_get_med_ah().unwrap();

    stack_entry_amedh(amedh, |h| {
        apply_med_set_heap_max(h, 4194304 + 1);
        apply_med_set_med_limit(h, G_MED_LIMIT.with(|v| v.get()));
        apply_med_set_med_cutoff(h, G_MED_CUTOFF.with(|v| v.get()));
    });

    let result = stack_entry_amedh(amedh, |h| apply_med(h, Some(word)));
    match result {
        None => {
            print!("???\n");
            return;
        }
        Some(r) => {
            print!("{}\n", r);
            let (instr, cost) =
                stack_entry_amedh(amedh, |h| (apply_med_get_instring(h), apply_med_get_cost(h)));
            print!("{}\n", instr.unwrap_or_default());
            print!("Cost[f]: {}\n\n", cost);
        }
    }
    loop {
        let result = stack_entry_amedh(amedh, |h| apply_med(h, None));
        match result {
            None => break,
            Some(r) => {
                print!("{}\n", r);
                let (instr, cost) =
                    stack_entry_amedh(amedh, |h| (apply_med_get_instring(h), apply_med_get_cost(h)));
                print!("{}\n", instr.unwrap_or_default());
                print!("Cost[f]: {}\n\n", cost);
            }
        }
    }
}

// [spec:foma:def:iface.iface-apply-file-fn]
// [spec:foma:sem:iface.iface-apply-file-fn]
// [spec:foma:def:foma.iface-apply-file-fn]
// [spec:foma:sem:foma.iface-apply-file-fn]
pub fn iface_apply_file(infilename: &str, outfilename: Option<&str>, direction: i32) -> i32 {
    let _ = LINE_LIMIT; // fgets buffer size; read_line reads whole lines here.
    if direction != AP_D && direction != AP_U {
        perror("Invalid direction in iface_apply_file().\n");
        return 1;
    }
    if iface_stack_check(1) == 0 {
        return 0;
    }
    let infile = match File::open(infilename) {
        Ok(f) => f,
        Err(_) => {
            eprint!("{}: ", infilename);
            perror("Error opening file");
            return 1;
        }
    };

    // C: OUTFILE = fopen(outfilename, "w") happens BEFORE the "Writing output to
    // file" message, which is BEFORE the NULL check — so the message prints even
    // when the open fails.
    let mut outfile: Box<dyn Write> = match outfilename {
        None => Box::new(std::io::stdout()),
        Some(name) => {
            let res = File::create(name);
            print!("Writing output to file {}.\n", name);
            match res {
                Ok(f) => Box::new(f),
                Err(_) => {
                    eprint!("{}: ", name);
                    perror("Error opening output file.");
                    return 1;
                }
            }
        }
    };

    let ah = stack_get_ah().unwrap();
    stack_entry_ah(ah, |h| iface_apply_set_params(h));

    let mut reader = BufReader::new(infile);
    let mut inword = String::new();
    loop {
        inword.clear();
        // fgets: NULL at EOF. read_line returns Ok(0) at EOF.
        // DEVIATION from C: read_line requires UTF-8; a decode error is treated as
        // end-of-input here (C reads raw bytes).
        let n = reader.read_line(&mut inword).unwrap_or(0);
        if n == 0 {
            break;
        }
        // C: if (inword[strlen(inword)-1] == '\n') inword[strlen-1] = '\0';
        // DEVIATION from C (on a line whose first byte is NUL, strlen==0 and the C
        // indexes inword[-1] — OOB; guard non-empty and strip a trailing '\n').
        if !inword.is_empty() && inword.as_bytes()[inword.len() - 1] == b'\n' {
            inword.pop();
        }

        let _ = write!(outfile, "\n{}\n", inword);
        let result = if direction == AP_D {
            stack_entry_ah(ah, |h| apply_down(h, Some(&inword)))
        } else {
            stack_entry_ah(ah, |h| apply_up(h, Some(&inword)))
        };

        let result = match result {
            None => {
                let _ = write!(outfile, "???\n");
                continue;
            }
            Some(r) => r,
        };
        let _ = write!(outfile, "{}\n", result);
        loop {
            let result = if direction == AP_D {
                stack_entry_ah(ah, |h| apply_down(h, None))
            } else {
                stack_entry_ah(ah, |h| apply_up(h, None))
            };
            match result {
                None => break,
                Some(r) => {
                    let _ = write!(outfile, "{}\n", r);
                }
            }
        }
    }
    // C: fclose(OUTFILE) only when outfilename != NULL; the input file is never
    // fclose'd (latent leak). Rust drops (closes) both at scope end; stdout is not
    // closed. The observable difference (leak vs. drop) is unrepresentable safely.
    0
}

// [spec:foma:def:iface.iface-apply-down-fn]
// [spec:foma:sem:iface.iface-apply-down-fn]
// [spec:foma:def:foma.iface-apply-down-fn]
// [spec:foma:sem:foma.iface-apply-down-fn]
pub fn iface_apply_down(word: &str) {
    if iface_stack_check(1) == 0 {
        return;
    }
    let ah = stack_get_ah().unwrap();
    stack_entry_ah(ah, |h| iface_apply_set_params(h));
    let result = stack_entry_ah(ah, |h| apply_down(h, Some(word)));
    match result {
        None => {
            print!("???\n");
            return;
        }
        Some(r) => {
            print!("{}\n", r);
        }
    }
    let mut i = G_LIST_LIMIT.with(|v| v.get());
    while i > 0 {
        let result = stack_entry_ah(ah, |h| apply_down(h, None));
        match result {
            None => break,
            Some(r) => print!("{}\n", r),
        }
        i -= 1;
    }
}

// [spec:foma:def:iface.iface-apply-up-fn]
// [spec:foma:sem:iface.iface-apply-up-fn]
// [spec:foma:def:foma.iface-apply-up-fn]
// [spec:foma:sem:foma.iface-apply-up-fn]
pub fn iface_apply_up(word: &str) {
    if iface_stack_check(1) == 0 {
        return;
    }
    let ah = stack_get_ah().unwrap();
    stack_entry_ah(ah, |h| iface_apply_set_params(h));
    let result = stack_entry_ah(ah, |h| apply_up(h, Some(word)));
    match result {
        None => {
            print!("???\n");
            return;
        }
        Some(r) => {
            print!("{}\n", r);
        }
    }
    let mut i = G_LIST_LIMIT.with(|v| v.get());
    while i > 0 {
        let result = stack_entry_ah(ah, |h| apply_up(h, None));
        match result {
            None => break,
            Some(r) => print!("{}\n", r),
        }
        i -= 1;
    }
}

// [spec:foma:def:iface.iface-lower-words-fn]
// [spec:foma:sem:iface.iface-lower-words-fn]
// [spec:foma:def:foma.iface-lower-words-fn]
// [spec:foma:sem:foma.iface-lower-words-fn]
pub fn iface_lower_words(limit: i32) {
    if iface_stack_check(1) == 0 {
        return;
    }
    let limit = if limit == -1 {
        G_LIST_LIMIT.with(|v| v.get())
    } else {
        limit
    };
    if iface_stack_check(1) != 0 {
        let ah = stack_get_ah().unwrap();
        stack_entry_ah(ah, |h| iface_apply_set_params(h));
        let mut i = limit;
        while i > 0 {
            let result = stack_entry_ah(ah, |h| apply_lower_words(h));
            match result {
                None => break,
                Some(r) => print!("{}\n", r),
            }
            i -= 1;
        }
        stack_entry_ah(ah, |h| apply_reset_enumerator(h));
    }
}

// [spec:foma:def:iface.iface-random-lower-fn]
// [spec:foma:sem:iface.iface-random-lower-fn]
// [spec:foma:def:foma.iface-random-lower-fn]
// [spec:foma:sem:foma.iface-random-lower-fn]
pub fn iface_random_lower(limit: i32) {
    iface_apply_random(apply_random_lower, limit);
}

// [spec:foma:def:iface.iface-random-upper-fn]
// [spec:foma:sem:iface.iface-random-upper-fn]
// [spec:foma:def:foma.iface-random-upper-fn]
// [spec:foma:sem:foma.iface-random-upper-fn]
pub fn iface_random_upper(limit: i32) {
    iface_apply_random(apply_random_upper, limit);
}

// [spec:foma:def:iface.iface-random-words-fn]
// [spec:foma:sem:iface.iface-random-words-fn]
// [spec:foma:def:foma.iface-random-words-fn]
// [spec:foma:sem:foma.iface-random-words-fn]
pub fn iface_random_words(limit: i32) {
    iface_apply_random(apply_random_words, limit);
}

// [spec:foma:def:iface.iface-apply-random-fn]
// [spec:foma:sem:iface.iface-apply-random-fn]
// [spec:foma:def:foma.iface-apply-random-fn]
// [spec:foma:sem:foma.iface-apply-random-fn]
// C: `void iface_apply_random(char *(*applyer)(struct apply_handle *h), int limit)` —
// the applyer function pointer becomes a Rust fn pointer of the same shape.
pub fn iface_apply_random(applyer: fn(&mut ApplyHandle) -> Option<String>, limit: i32) {
    let limit = if limit == -1 {
        G_LIST_RANDOM_LIMIT.with(|v| v.get())
    } else {
        limit
    };
    if iface_stack_check(1) != 0 {
        // calloc(limit, sizeof(struct apply_results {char *string; int count;}))
        let mut results: Vec<(Option<String>, i32)> = vec![(None, 0); limit as usize];
        let ah = stack_get_ah().unwrap();
        stack_entry_ah(ah, |h| iface_apply_set_params(h));
        let mut i = limit;
        while i > 0 {
            let result = stack_entry_ah(ah, |h| applyer(h));
            if let Some(result) = result {
                for slot in results.iter_mut() {
                    if slot.0.is_none() {
                        // strdup(result)
                        slot.0 = Some(result.clone());
                        slot.1 = 1;
                        break;
                    } else if slot.0.as_deref() == Some(result.as_str()) {
                        slot.1 += 1;
                        break;
                    }
                }
            }
            i -= 1;
        }
        for slot in results.iter() {
            if let Some(s) = &slot.0 {
                print!("[{}] {}\n", slot.1, s);
                // free(tempresults->string) — String dropped at scope end.
            }
        }
        // free(results) — Vec dropped at scope end.
        stack_entry_ah(ah, |h| apply_reset_enumerator(h));
    }
}

// [spec:foma:def:iface.iface-print-shortest-string-fn]
// [spec:foma:sem:iface.iface-print-shortest-string-fn]
// [spec:foma:def:foma.iface-print-shortest-string-fn]
// [spec:foma:sem:foma.iface-print-shortest-string-fn]
pub fn iface_print_shortest_string() {
    /* L -  ?+  [[L .o. [?:"@TMP@"]*].l .o. "@TMP@":?*].l; */
    if iface_stack_check(1) != 0 {
        let top = stack_find_top().unwrap();
        let mut one = stack_entry_fsm(top, |f| fsm_copy(f));
        if stack_entry_fsm(top, |f| f.arity) == 1 {
            let result = fsm_minimize(fsm_minus(
                fsm_copy(&mut one),
                fsm_concat(
                    fsm_kleene_plus(fsm_identity()),
                    fsm_lower(fsm_compose(
                        fsm_lower(fsm_compose(
                            fsm_copy(&mut one),
                            fsm_kleene_star(fsm_cross_product(fsm_identity(), fsm_symbol("@TMP@"))),
                        )),
                        fsm_kleene_star(fsm_cross_product(fsm_symbol("@TMP@"), fsm_identity())),
                    )),
                ),
            ));
            let mut ah = apply_init(&result);
            let word = apply_words(&mut ah);
            if let Some(w) = &word {
                print!("{}\n", w);
            }
            apply_clear(ah);
            fsm_destroy(result);
            // C leaks the initial fsm_copy `one` here; dropped (freed) at scope end.
        } else {
            let mut onel = fsm_lower(fsm_copy(&mut one));
            let mut oneu = fsm_upper(one);
            let result_u = fsm_minimize(fsm_minus(
                fsm_copy(&mut oneu),
                fsm_concat(
                    fsm_kleene_plus(fsm_identity()),
                    fsm_lower(fsm_compose(
                        fsm_lower(fsm_compose(
                            fsm_copy(&mut oneu),
                            fsm_kleene_star(fsm_cross_product(fsm_identity(), fsm_symbol("@TMP@"))),
                        )),
                        fsm_kleene_star(fsm_cross_product(fsm_symbol("@TMP@"), fsm_identity())),
                    )),
                ),
            ));
            let result_l = fsm_minimize(fsm_minus(
                fsm_copy(&mut onel),
                fsm_concat(
                    fsm_kleene_plus(fsm_identity()),
                    fsm_lower(fsm_compose(
                        fsm_lower(fsm_compose(
                            fsm_copy(&mut onel),
                            fsm_kleene_star(fsm_cross_product(fsm_identity(), fsm_symbol("@TMP@"))),
                        )),
                        fsm_kleene_star(fsm_cross_product(fsm_symbol("@TMP@"), fsm_identity())),
                    )),
                ),
            ));
            fsm_destroy(oneu);
            fsm_destroy(onel);
            let mut ah = apply_init(&result_u);
            let word = apply_words(&mut ah);
            // C: if (word == NULL) word = ""; printf("Upper: %s\n", word);
            print!("Upper: {}\n", word.as_deref().unwrap_or(""));
            apply_clear(ah);
            fsm_destroy(result_u);
            let mut ah = apply_init(&result_l);
            let word = apply_words(&mut ah);
            print!("Lower: {}\n", word.as_deref().unwrap_or(""));
            apply_clear(ah);
            fsm_destroy(result_l);
        }
    }
}

// [spec:foma:def:iface.iface-print-shortest-string-size-fn]
// [spec:foma:sem:iface.iface-print-shortest-string-size-fn]
// [spec:foma:def:foma.iface-print-shortest-string-size-fn]
// [spec:foma:sem:foma.iface-print-shortest-string-size-fn]
pub fn iface_print_shortest_string_size() {
    if iface_stack_check(1) != 0 {
        let top = stack_find_top().unwrap();
        let mut one = stack_entry_fsm(top, |f| fsm_copy(f));
        /* [L .o. [?:a]*].l; */
        if stack_entry_fsm(top, |f| f.arity) == 1 {
            let result = fsm_minimize(fsm_lower(fsm_compose(
                one,
                fsm_kleene_star(fsm_cross_product(fsm_identity(), fsm_symbol("a"))),
            )));
            print!("Shortest acyclic path length: {}\n", result.statecount - 1);
            // Result net never fsm_destroy'd in C (leak); dropped at scope end.
        } else {
            let onel = fsm_lower(fsm_copy(&mut one));
            let oneu = fsm_upper(one);
            let result_u = fsm_minimize(fsm_lower(fsm_compose(
                oneu,
                fsm_kleene_star(fsm_cross_product(fsm_identity(), fsm_symbol("a"))),
            )));
            let result_l = fsm_minimize(fsm_lower(fsm_compose(
                onel,
                fsm_kleene_star(fsm_cross_product(fsm_identity(), fsm_symbol("a"))),
            )));
            print!("Shortest acyclic upper path length: {}\n", result_u.statecount - 1);
            print!("Shortest acyclic lower path length: {}\n", result_l.statecount - 1);
        }
    }
}

// [spec:foma:def:iface.iface-upper-words-fn]
// [spec:foma:sem:iface.iface-upper-words-fn]
// [spec:foma:def:foma.iface-upper-words-fn]
// [spec:foma:sem:foma.iface-upper-words-fn]
pub fn iface_upper_words(limit: i32) {
    let limit = if limit == -1 {
        G_LIST_LIMIT.with(|v| v.get())
    } else {
        limit
    };
    if iface_stack_check(1) != 0 {
        let ah = stack_get_ah().unwrap();
        stack_entry_ah(ah, |h| iface_apply_set_params(h));
        let mut i = limit;
        while i > 0 {
            let result = stack_entry_ah(ah, |h| apply_upper_words(h));
            match result {
                None => break,
                Some(r) => print!("{}\n", r),
            }
            i -= 1;
        }
        stack_entry_ah(ah, |h| apply_reset_enumerator(h));
    }
}

// [spec:foma:def:iface.iface-words-file-fn]
// [spec:foma:sem:iface.iface-words-file-fn+1]
// [spec:foma:def:foma.iface-words-file-fn]
// [spec:foma:sem:foma.iface-words-file-fn+1]
pub fn iface_words_file(filename: &str, r#type: i32) {
    /* type 0 (words), 1 (upper-words), 2 (lower-words) */
    // Wave 4 fix: the C kept the applyer in a function-local `static`, so a type-0
    // call after any type-1/2 call reused the stale upper/lower enumerator. Select
    // the enumerator fresh from `type` on every call instead.
    let applyer: fn(&mut ApplyHandle) -> Option<String> = if r#type == 1 {
        apply_upper_words
    } else if r#type == 2 {
        apply_lower_words
    } else {
        apply_words
    };
    if iface_stack_check(1) != 0 {
        let top = stack_find_top().unwrap();
        if stack_entry_fsm(top, |f| f.pathcount) == PATHCOUNT_CYCLIC {
            print!("FSM is cyclic: can't write all words to file.\n");
            return;
        }
        print!("Writing to {}.\n", filename);
        let mut outfile = match File::create(filename) {
            Ok(f) => f,
            Err(_) => {
                perror("Error opening file");
                return;
            }
        };
        let ah = stack_get_ah().unwrap();
        stack_entry_ah(ah, |h| iface_apply_set_params(h));
        loop {
            let result = stack_entry_ah(ah, |h| applyer(h));
            match result {
                None => break,
                Some(r) => {
                    let _ = write!(outfile, "{}\n", r);
                }
            }
        }
        stack_entry_ah(ah, |h| apply_reset_enumerator(h));
        // fclose(outfile) — dropped at scope end.
    }
}

// [spec:foma:def:iface.iface-words-fn]
// [spec:foma:sem:iface.iface-words-fn]
// [spec:foma:def:foma.iface-words-fn]
// [spec:foma:sem:foma.iface-words-fn]
pub fn iface_words(limit: i32) {
    let limit = if limit == -1 {
        G_LIST_LIMIT.with(|v| v.get())
    } else {
        limit
    };
    if iface_stack_check(1) != 0 {
        let ah = stack_get_ah().unwrap();
        stack_entry_ah(ah, |h| iface_apply_set_params(h));
        let mut i = limit;
        while i > 0 {
            let result = stack_entry_ah(ah, |h| apply_words(h));
            match result {
                None => break,
                Some(r) => print!("{}\n", r),
            }
            i -= 1;
        }
        stack_entry_ah(ah, |h| apply_reset_enumerator(h));
    }
}

// [spec:foma:def:iface.iface-pairs-call-fn]
// [spec:foma:sem:iface.iface-pairs-call-fn]
pub fn iface_pairs_call(limit: i32, random: i32) {
    let limit = if limit == -1 {
        G_LIST_LIMIT.with(|v| v.get())
    } else {
        limit
    };
    if iface_stack_check(1) != 0 {
        let ah = stack_get_ah().unwrap();
        stack_entry_ah(ah, |h| apply_set_show_flags(h, G_SHOW_FLAGS.with(|v| v.get())));
        stack_entry_ah(ah, |h| apply_set_obey_flags(h, G_OBEY_FLAGS.with(|v| v.get())));
        stack_entry_ah(ah, |h| apply_set_space_symbol(h, "\u{1}"));
        stack_entry_ah(ah, |h| apply_set_epsilon(h, "\u{2}"));
        stack_entry_ah(ah, |h| apply_set_separator(h, "\u{3}"));
        let mut i = limit;
        while i > 0 {
            let result = if random == 1 {
                stack_entry_ah(ah, |h| apply_random_words(h))
            } else {
                stack_entry_ah(ah, |h| apply_words(h))
            };
            let result = match result {
                None => break,
                Some(r) => r,
            };
            let mut result = result.into_bytes();
            let mut upper = Vec::new();
            let mut lower = Vec::new();
            iface_split_result(&mut result, &mut upper, &mut lower);
            // printf("%s\t%s\n", upper, lower) — raw bytes (may be UTF-8-corrupted).
            let mut out = std::io::stdout();
            let _ = out.write_all(&upper);
            let _ = out.write_all(b"\t");
            let _ = out.write_all(&lower);
            let _ = out.write_all(b"\n");
            i -= 1;
        }
        stack_entry_ah(ah, |h| apply_set_space_symbol(h, " "));
        stack_entry_ah(ah, |h| apply_set_epsilon(h, "0"));
        stack_entry_ah(ah, |h| apply_set_separator(h, ":"));
        stack_entry_ah(ah, |h| apply_reset_enumerator(h));
    }
}

// [spec:foma:def:iface.iface-random-pairs-fn]
// [spec:foma:sem:iface.iface-random-pairs-fn+1]
// [spec:foma:def:foma.iface-random-pairs-fn]
// [spec:foma:sem:foma.iface-random-pairs-fn+1]
pub fn iface_random_pairs(limit: i32) {
    // Wave 4 fix: the C passed limit straight through, so limit == -1 resolved to
    // g_list_limit inside iface_pairs_call instead of g_list_random_limit like the
    // other random commands. Resolve -1 to g_list_random_limit here first.
    let limit = if limit == -1 {
        G_LIST_RANDOM_LIMIT.with(|v| v.get())
    } else {
        limit
    };
    iface_pairs_call(limit, 1);
}

// [spec:foma:def:iface.iface-pairs-fn]
// [spec:foma:sem:iface.iface-pairs-fn]
// [spec:foma:def:foma.iface-pairs-fn]
// [spec:foma:sem:foma.iface-pairs-fn]
pub fn iface_pairs(limit: i32) {
    iface_pairs_call(limit, 0);
}

// [spec:foma:def:iface.iface-pairs-file-fn]
// [spec:foma:sem:iface.iface-pairs-file-fn]
// [spec:foma:def:foma.iface-pairs-file-fn]
// [spec:foma:sem:foma.iface-pairs-file-fn]
pub fn iface_pairs_file(filename: &str) {
    if iface_stack_check(1) != 0 {
        let top = stack_find_top().unwrap();
        if stack_entry_fsm(top, |f| f.pathcount) == PATHCOUNT_CYCLIC {
            print!("FSM is cyclic: can't write all pairs to file.\n");
            return;
        }
        print!("Writing to {}.\n", filename);
        let mut outfile = match File::create(filename) {
            Ok(f) => f,
            Err(_) => {
                perror("Error opening file");
                return;
            }
        };
        let ah = stack_get_ah().unwrap();
        stack_entry_ah(ah, |h| apply_set_show_flags(h, G_SHOW_FLAGS.with(|v| v.get())));
        stack_entry_ah(ah, |h| apply_set_obey_flags(h, G_OBEY_FLAGS.with(|v| v.get())));
        stack_entry_ah(ah, |h| apply_set_space_symbol(h, "\u{1}"));
        stack_entry_ah(ah, |h| apply_set_epsilon(h, "\u{2}"));
        stack_entry_ah(ah, |h| apply_set_separator(h, "\u{3}"));
        loop {
            let result = stack_entry_ah(ah, |h| apply_words(h));
            let result = match result {
                None => break,
                Some(r) => r,
            };
            let mut result = result.into_bytes();
            let mut upper = Vec::new();
            let mut lower = Vec::new();
            iface_split_result(&mut result, &mut upper, &mut lower);
            let _ = outfile.write_all(&upper);
            let _ = outfile.write_all(b"\t");
            let _ = outfile.write_all(&lower);
            let _ = outfile.write_all(b"\n");
        }
        stack_entry_ah(ah, |h| apply_set_space_symbol(h, " "));
        stack_entry_ah(ah, |h| apply_set_epsilon(h, "0"));
        stack_entry_ah(ah, |h| apply_set_separator(h, ":"));
        stack_entry_ah(ah, |h| apply_reset_enumerator(h));
        // fclose(outfile) — dropped at scope end.
    }
}

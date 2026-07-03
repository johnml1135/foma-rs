//! foma/flags.c — literal Wave-2 (bug-for-bug) port per
//! docs/port/rust-conventions.md. Sem rules: docs/spec/port/foma/flags.md.

use crate::constructions::{
    add_fsm_arc, fsm_complement, fsm_compose, fsm_concat, fsm_contains, fsm_intersect,
    fsm_optionality, fsm_symbol, fsm_union, fsm_universal,
};
use crate::mem::{G_FLAG_IS_EPSILON, G_VERBOSE, xxstrndup};
use crate::minimize::fsm_minimize;
use crate::sigma::{sigma_cleanup, sigma_max, sigma_remove_num, sigma_sort};
use crate::structures::{fsm_copy, fsm_empty_set};
use crate::topsort::fsm_topsort;
use crate::types::{EPSILON, Fsm, FsmState, NO, UNK};
use crate::types::{
    FLAG_CLEAR, FLAG_DISALLOW, FLAG_EQUAL, FLAG_NEGATIVE, FLAG_POSITIVE, FLAG_REQUIRE, FLAG_UNIFY,
};
use crate::utf8::utf8skip;

/* File-local constants (C #defines; distinct from apply.c's FAIL/SUCCEED) */
const FAIL: i32 = 1;
const SUCCEED: i32 = 2;
const NONE: i32 = 3;

// [spec:foma:def:flags.flags]
pub struct Flags {
    pub r#type: i32,
    pub name: Option<String>,
    pub value: Option<String>,
    pub next: Option<Box<Flags>>,
}

/* We eliminate all flags by creating a list of them and building a regex filter   */
/* that successively removes unwanted paths.  NB: flag_eliminate() called with the */
/* second argument NULL eliminates all flags.                                      */
/* The regexes we build for each flag symbol are of the format:                    */
/* ~[?* FAIL ~$SUCCEED THISFLAG ?*] for U,P,D                                      */
/* or                                                                              */
/* ~[(?* FAIL) ~$SUCCEED THISFLAG ?*] for the R flag                               */
/* The function flag_build() determines, depending on the flag at hand for each    */
/* of the other flags occurring in the network if it belongs in FAIL, SUCCEED,     */
/* or neither.                                                                     */
/* The languages FAIL, SUCCEED is then the union of all symbols that cause         */
/* compatibility or incompatibility.                                               */
/* We intersect all these filters, creating a large filter that we compose both on */
/* the upper side of the network and the lower side:                               */
/* RESULT = FILTER .o. ORIGINAL .o. FILTER                                         */
/* We can't simply intersect the language with FILTER because the lower side flags */
/* are independent of the upper side ones, and the network may be a transducer.    */
/* Finally, we replace the affected arcs with EPSILON arcs, and call               */
/* sigma_cleanup() to purge the symbols not occurring on arcs.                     */

///
///Eliminate a flag from a network. If called with name = NULL, eliminate all flags.
///
// [spec:foma:def:flags.flag-eliminate-fn]
// [spec:foma:sem:flags.flag-eliminate-fn]
// [spec:foma:def:fomalib.flag-eliminate-fn]
// [spec:foma:sem:fomalib.flag-eliminate-fn]
pub fn flag_eliminate(net: Box<Fsm>, name: Option<&str>) -> Box<Fsm> {
    let mut filter: Option<Box<Fsm>> = None;

    if net.pathcount == 0 {
        if G_VERBOSE.with(|v| v.get()) != 0 {
            eprint!("Skipping flag elimination since there are no paths in network.\n");
            /* fflush(stderr) — stderr is unbuffered */
        }
        return net;
    }

    let flags = flag_extract(&net);
    /* Check that flag actually exists in net */
    if let Some(name) = name {
        let mut found = 0;
        let mut f = flags.as_deref();
        while let Some(fl) = f {
            /* strcmp(name, f->name) — C would segfault on a NULL f->name */
            if fl.name.as_deref() == Some(name) {
                found = 1;
            }
            f = fl.next.as_deref();
        }
        if found == 0 {
            if G_VERBOSE.with(|v| v.get()) != 0 {
                eprint!("Flag attribute '{}' does not occur in the network.\n", name);
                /* fflush(stderr) */
            }
            return net;
        }
    }

    let mut flag = 0;

    let mut f = flags.as_deref();
    while let Some(fl) = f {
        let mut succeed_flags: Option<Box<Fsm>> = None;
        let mut fail_flags: Option<Box<Fsm>> = None;
        let mut self_: Option<Box<Fsm>> = None;

        /* BUG in C, ported literally: `|` instead of `&` — the bitwise-or is
        always nonzero, so the intended U/R/D/E type restriction is a no-op
        and the body runs for every type (masked in practice: flag_build only
        classifies pairs when f's type is U, R, or D). */
        if (name.is_none() || fl.name.as_deref() == name)
            && (fl.r#type | FLAG_UNIFY | FLAG_REQUIRE | FLAG_DISALLOW | FLAG_EQUAL) != 0
        {
            succeed_flags = Some(fsm_empty_set());
            fail_flags = Some(fsm_empty_set());
            self_ = Some(flag_create_symbol(
                fl.r#type,
                fl.name.as_deref().unwrap(),
                fl.value.as_deref(),
            ));

            let mut ff = flags.as_deref();
            flag = 0;
            while let Some(ffl) = ff {
                let fstatus = flag_build(
                    fl.r#type,
                    fl.name.as_deref().unwrap(),
                    fl.value.as_deref(),
                    ffl.r#type,
                    ffl.name.as_deref().unwrap(),
                    ffl.value.as_deref(),
                );
                if fstatus == FAIL {
                    fail_flags = Some(fsm_minimize(fsm_union(
                        fail_flags.take().unwrap(),
                        flag_create_symbol(
                            ffl.r#type,
                            ffl.name.as_deref().unwrap(),
                            ffl.value.as_deref(),
                        ),
                    )));
                    flag = 1;
                }
                if fstatus == SUCCEED {
                    succeed_flags = Some(fsm_minimize(fsm_union(
                        succeed_flags.take().unwrap(),
                        flag_create_symbol(
                            ffl.r#type,
                            ffl.name.as_deref().unwrap(),
                            ffl.value.as_deref(),
                        ),
                    )));
                    flag = 1;
                }
                ff = ffl.next.as_deref();
            }
        }

        if flag != 0 {
            let newfilter = if fl.r#type == FLAG_REQUIRE {
                fsm_complement(fsm_concat(
                    fsm_optionality(fsm_concat(fsm_universal(), fail_flags.take().unwrap())),
                    fsm_concat(
                        fsm_complement(fsm_contains(succeed_flags.take().unwrap())),
                        fsm_concat(self_.take().unwrap(), fsm_universal()),
                    ),
                ))
            } else {
                fsm_complement(fsm_contains(fsm_concat(
                    fail_flags.take().unwrap(),
                    fsm_concat(
                        fsm_complement(fsm_contains(succeed_flags.take().unwrap())),
                        self_.take().unwrap(),
                    ),
                )))
            };

            filter = Some(match filter.take() {
                None => newfilter,
                Some(filter) => fsm_intersect(filter, newfilter),
            });
        }
        flag = 0;
        f = fl.next.as_deref();
    }

    let newnet = if let Some(mut filter) = filter {
        let old_g_flag_is_epsilon = G_FLAG_IS_EPSILON.with(|c| c.get());
        G_FLAG_IS_EPSILON.with(|c| c.set(0));
        let newnet = fsm_compose(fsm_copy(&mut filter), fsm_compose(net, fsm_copy(&mut filter)));
        G_FLAG_IS_EPSILON.with(|c| c.set(old_g_flag_is_epsilon));
        /* the filter itself is never destroyed in C (leak); dropped here */
        newnet
    } else {
        net
    };

    let mut newnet = newnet;
    flag_purge(&mut newnet, name);
    let mut newnet = fsm_minimize(newnet);
    sigma_cleanup(&mut newnet, 0);
    sigma_sort(&mut newnet);
    /* free(flags) — C frees only the list head (remaining nodes and their
    name/value strings leak); Rust drops the whole list */
    drop(flags);
    fsm_topsort(newnet)
}

// [spec:foma:def:flags.flag-create-symbol-fn]
// [spec:foma:sem:flags.flag-create-symbol-fn]
pub(crate) fn flag_create_symbol(r#type: i32, name: &str, value: Option<&str>) -> Box<Fsm> {
    let value = match value {
        None => "",
        Some(v) => v,
    };

    /* C: string = malloc(strlen(name)+strlen(value)+6), built with strcat and
    never freed (leak); an owned String here */
    let mut string = String::new();
    string.push_str("@");
    /* flag_type_to_char(type) — C would segfault on NULL for unknown types */
    string.push_str(flag_type_to_char(r#type).unwrap());
    string.push_str(".");
    string.push_str(name);
    if value != "" {
        string.push_str(".");
        string.push_str(value);
    }
    string.push_str("@");

    fsm_symbol(&string)
}

// [spec:foma:def:flags.flag-type-to-char-fn]
// [spec:foma:sem:flags.flag-type-to-char-fn]
pub(crate) fn flag_type_to_char(r#type: i32) -> Option<&'static str> {
    match r#type {
        FLAG_UNIFY => return Some("U"),
        FLAG_CLEAR => return Some("C"),
        FLAG_DISALLOW => return Some("D"),
        FLAG_NEGATIVE => return Some("N"),
        FLAG_POSITIVE => return Some("P"),
        FLAG_REQUIRE => return Some("R"),
        FLAG_EQUAL => return Some("E"),
        _ => {}
    }
    None
}

// [spec:foma:def:flags.flag-build-fn]
// [spec:foma:sem:flags.flag-build-fn]
// [spec:foma:def:fomalib.flag-build-fn]
// [spec:foma:sem:fomalib.flag-build-fn]
pub fn flag_build(
    ftype: i32,
    fname: &str,
    fvalue: Option<&str>,
    fftype: i32,
    ffname: &str,
    ffvalue: Option<&str>,
) -> i32 {
    let mut selfnull = 0; /* If current flag has no value, e.g. @R.A@ */
    if fname != ffname {
        return NONE;
    }

    let fvalue = match fvalue {
        None => {
            selfnull = 1;
            ""
        }
        Some(v) => v,
    };

    let ffvalue = ffvalue.unwrap_or("");

    /* valeq = strcmp(fvalue, ffvalue) — only ever compared against 0 below */
    let valeq = if fvalue == ffvalue { 0 } else { 1 };
    /* U flags */
    if ftype == FLAG_UNIFY && fftype == FLAG_POSITIVE && valeq == 0 {
        return SUCCEED;
    }
    if ftype == FLAG_UNIFY && fftype == FLAG_CLEAR {
        return SUCCEED;
    }
    if ftype == FLAG_UNIFY && fftype == FLAG_UNIFY && valeq != 0 {
        return FAIL;
    }
    if ftype == FLAG_UNIFY && fftype == FLAG_POSITIVE && valeq != 0 {
        return FAIL;
    }
    if ftype == FLAG_UNIFY && fftype == FLAG_NEGATIVE && valeq == 0 {
        return FAIL;
    }

    /* R flag with value = 0 */
    if ftype == FLAG_REQUIRE && fftype == FLAG_UNIFY && selfnull != 0 {
        return SUCCEED;
    }
    if ftype == FLAG_REQUIRE && fftype == FLAG_POSITIVE && selfnull != 0 {
        return SUCCEED;
    }
    if ftype == FLAG_REQUIRE && fftype == FLAG_NEGATIVE && selfnull != 0 {
        return SUCCEED;
    }
    if ftype == FLAG_REQUIRE && fftype == FLAG_CLEAR && selfnull != 0 {
        return FAIL;
    }

    /* R flag with value */
    if ftype == FLAG_REQUIRE && fftype == FLAG_POSITIVE && valeq == 0 && selfnull == 0 {
        return SUCCEED;
    }
    if ftype == FLAG_REQUIRE && fftype == FLAG_UNIFY && valeq == 0 && selfnull == 0 {
        return SUCCEED;
    }
    if ftype == FLAG_REQUIRE && fftype == FLAG_POSITIVE && valeq != 0 && selfnull == 0 {
        return FAIL;
    }
    if ftype == FLAG_REQUIRE && fftype == FLAG_UNIFY && valeq != 0 && selfnull == 0 {
        return FAIL;
    }
    if ftype == FLAG_REQUIRE && fftype == FLAG_NEGATIVE && selfnull == 0 {
        return FAIL;
    }
    if ftype == FLAG_REQUIRE && fftype == FLAG_CLEAR && selfnull == 0 {
        return FAIL;
    }

    /* D flag with value = 0 */
    if ftype == FLAG_DISALLOW && fftype == FLAG_CLEAR && selfnull != 0 {
        return SUCCEED;
    }
    if ftype == FLAG_DISALLOW && fftype == FLAG_POSITIVE && selfnull != 0 {
        return FAIL;
    }
    if ftype == FLAG_DISALLOW && fftype == FLAG_UNIFY && selfnull != 0 {
        return FAIL;
    }
    if ftype == FLAG_DISALLOW && fftype == FLAG_NEGATIVE && selfnull != 0 {
        return FAIL;
    }

    /* D flag with value */
    if ftype == FLAG_DISALLOW && fftype == FLAG_POSITIVE && valeq != 0 && selfnull == 0 {
        return SUCCEED;
    }
    if ftype == FLAG_DISALLOW && fftype == FLAG_CLEAR && selfnull == 0 {
        return SUCCEED;
    }
    if ftype == FLAG_DISALLOW && fftype == FLAG_NEGATIVE && valeq == 0 && selfnull == 0 {
        return SUCCEED;
    }
    if ftype == FLAG_DISALLOW && fftype == FLAG_POSITIVE && valeq == 0 && selfnull == 0 {
        return FAIL;
    }
    if ftype == FLAG_DISALLOW && fftype == FLAG_UNIFY && valeq == 0 && selfnull == 0 {
        return FAIL;
    }
    if ftype == FLAG_DISALLOW && fftype == FLAG_NEGATIVE && valeq != 0 && selfnull == 0 {
        return FAIL;
    }

    NONE
}

/* Remove flags that are being eliminated from arcs and sigma */

// [spec:foma:def:flags.flag-purge-fn]
// [spec:foma:sem:flags.flag-purge-fn]
pub(crate) fn flag_purge(net: &mut Fsm, name: Option<&str>) {
    let sigmasize = sigma_max(net.sigma.as_deref()) + 1;
    /* C: malloc'd int array, zeroed by the following loop */
    let mut ftable: Vec<i32> = vec![0; sigmasize as usize];

    let mut sigma = net.sigma.as_deref();
    while let Some(s) = sigma {
        if s.number == -1 {
            break;
        }
        let symbol = s.symbol.as_deref().unwrap_or("");
        if flag_check(symbol) != 0 {
            match name {
                None => {
                    ftable[s.number as usize] = 1;
                }
                Some(name) => {
                    /* csym = (sigma->symbol) + 3 — skip "@X." (one-byte operator) */
                    let csym = &symbol.as_bytes()[3..];
                    let name_b = name.as_bytes();
                    /* strncmp(csym,name,strlen(name)) == 0 && strlen(csym) > strlen(name)
                    && (csym[strlen(name)] == '.' || csym[strlen(name)] == '@') */
                    if csym.starts_with(name_b)
                        && csym.len() > name_b.len()
                        && (csym[name_b.len()] == b'.' || csym[name_b.len()] == b'@')
                    {
                        ftable[s.number as usize] = 1;
                    }
                }
            }
        }
        sigma = s.next.as_deref();
    }
    for i in 0..sigmasize {
        if ftable[i as usize] != 0 {
            net.sigma = sigma_remove_num(i, net.sigma.take());
        }
    }

    let fsm = &mut net.states;
    let mut i = 0usize;
    while fsm[i].state_no != -1 {
        if fsm[i].r#in >= 0 && fsm[i].out >= 0 {
            if ftable[fsm[i].r#in as usize] != 0 {
                fsm[i].r#in = EPSILON as i16;
            }
            if ftable[fsm[i].out as usize] != 0 {
                fsm[i].out = EPSILON as i16;
            }
        }
        i += 1;
    }

    /* free(ftable) — drop */
    net.is_deterministic = NO;
    net.is_minimized = NO;
    net.is_epsilon_free = NO;
}

/* Extract all flags from network and place them in struct flag linked list */

// [spec:foma:def:flags.flag-extract-fn]
// [spec:foma:sem:flags.flag-extract-fn]
pub(crate) fn flag_extract(net: &Fsm) -> Option<Box<Flags>> {
    let mut flags: Option<Box<Flags>> = None;
    let mut sigma = net.sigma.as_deref();
    while let Some(s) = sigma {
        let symbol = s.symbol.as_deref().unwrap_or("");
        if flag_check(symbol) != 0 {
            let flagst = Box::new(Flags {
                r#type: flag_get_type(symbol),
                name: flag_get_name(symbol),
                value: flag_get_value(symbol),
                next: flags,
            });
            flags = Some(flagst);
        }
        sigma = s.next.as_deref();
    }
    flags
}

// [spec:foma:def:flags.flag-check-fn]
// [spec:foma:sem:flags.flag-check-fn]
// [spec:foma:def:fomalibconf.flag-check-fn]
// [spec:foma:sem:fomalibconf.flag-check-fn]
pub fn flag_check(s: &str) -> i32 {
    /* We simply simulate this regex (where ND is not dot) */
    /* "@" [U|P|N|R|E|D] "." ND+ "." ND+ "@" | "@" [D|R|C] "." ND+ "@" */
    /* and return 1 if it matches */

    let s = s.as_bytes();
    /* *(s+i): the end of the &str stands in for the C NUL terminator */
    let at = |i: usize| -> u8 {
        if i < s.len() { s[i] } else { 0 }
    };

    let mut i = 0usize;

    /* C goto labels s0..s11 → state loop with the same targets */
    let mut state = 0;
    loop {
        match state {
            /* entry */
            0 => {
                if at(i) == b'@' {
                    i += 1;
                    state = 1;
                    continue;
                }
                return 0;
            }
            /* s1 */
            1 => {
                if at(i) == b'C' {
                    i += 1;
                    state = 4;
                    continue;
                }
                if at(i) == b'N' || at(i) == b'E' || at(i) == b'U' || at(i) == b'P' {
                    i += 1;
                    state = 2;
                    continue;
                }
                if at(i) == b'R' || at(i) == b'D' {
                    i += 1;
                    state = 3;
                    continue;
                }
                return 0;
            }
            /* s2 */
            2 => {
                if at(i) == b'.' {
                    i += 1;
                    state = 5;
                    continue;
                }
                return 0;
            }
            /* s3 */
            3 => {
                if at(i) == b'.' {
                    i += 1;
                    state = 6;
                    continue;
                }
                return 0;
            }
            /* s4 */
            4 => {
                if at(i) == b'.' {
                    i += 1;
                    state = 7;
                    continue;
                }
                return 0;
            }
            /* s5 */
            5 => {
                if at(i) != b'.' && at(i) != 0 {
                    i += 1;
                    state = 8;
                    continue;
                }
                return 0;
            }
            /* s6 */
            6 => {
                if at(i) != b'.' && at(i) != 0 {
                    i += 1;
                    state = 9;
                    continue;
                }
                return 0;
            }
            /* s7 */
            7 => {
                if at(i) != b'.' && at(i) != 0 {
                    i += 1;
                    state = 10;
                    continue;
                }
                return 0;
            }
            /* s8 — quirk: '@' is an ordinary ND byte here (first field of
            U/P/N/E flags), so e.g. "@U.a@b.c@" is accepted */
            8 => {
                if at(i) == b'.' {
                    i += 1;
                    state = 7;
                    continue;
                }
                if at(i) != b'.' && at(i) != 0 {
                    i += 1;
                    state = 8;
                    continue;
                }
                return 0;
            }
            /* s9 */
            9 => {
                if at(i) == b'@' {
                    i += 1;
                    state = 11;
                    continue;
                }
                if at(i) == b'.' {
                    i += 1;
                    state = 7;
                    continue;
                }
                if at(i) != b'.' && at(i) != 0 {
                    i += 1;
                    state = 9;
                    continue;
                }
                return 0;
            }
            /* s10 */
            10 => {
                if at(i) == b'@' {
                    i += 1;
                    state = 11;
                    continue;
                }
                if at(i) != b'.' && at(i) != 0 {
                    i += 1;
                    state = 10;
                    continue;
                }
                return 0;
            }
            /* s11 */
            11 => {
                if at(i) == 0 {
                    return 1;
                }
                return 0;
            }
            _ => unreachable!(),
        }
    }
}

// [spec:foma:def:flags.flag-get-type-fn]
// [spec:foma:sem:flags.flag-get-type-fn]
// [spec:foma:def:fomalibconf.flag-get-type-fn]
// [spec:foma:sem:fomalibconf.flag-get-type-fn]
pub fn flag_get_type(string: &str) -> i32 {
    let b = string.as_bytes();
    /* strncmp(string+1, "X.", 2) == 0 — a string shorter than 3 bytes can't
    match (the C strncmp stops at the NUL and compares unequal) */
    let strncmp2 = |pat: &[u8; 2]| -> bool { b.len() >= 3 && b[1] == pat[0] && b[2] == pat[1] };
    if strncmp2(b"U.") {
        return FLAG_UNIFY;
    }
    if strncmp2(b"C.") {
        return FLAG_CLEAR;
    }
    if strncmp2(b"D.") {
        return FLAG_DISALLOW;
    }
    if strncmp2(b"N.") {
        return FLAG_NEGATIVE;
    }
    if strncmp2(b"P.") {
        return FLAG_POSITIVE;
    }
    if strncmp2(b"R.") {
        return FLAG_REQUIRE;
    }
    if strncmp2(b"E.") {
        return FLAG_EQUAL;
    }
    0
}

// [spec:foma:def:flags.flag-get-name-fn]
// [spec:foma:sem:flags.flag-get-name-fn]
// [spec:foma:def:fomalibconf.flag-get-name-fn]
// [spec:foma:sem:fomalibconf.flag-get-name-fn]
pub fn flag_get_name(string: &str) -> Option<String> {
    let s = string.as_bytes();
    let mut start = 0usize;
    let mut end = 0usize;
    let len = s.len(); /* strlen(string) */

    /* for (i=0; i < len; i += (utf8skip(string+i) + 1)) */
    let mut i = 0usize;
    while i < len {
        if s[i] == b'.' && start == 0 {
            start = i + 1;
            i += utf8skip(&s[i..]) as usize + 1;
            continue;
        }
        if (s[i] == b'.' || s[i] == b'@') && start != 0 {
            end = i;
            break;
        }
        i += utf8skip(&s[i..]) as usize + 1;
    }
    if start > 0 && end > 0 {
        return Some(xxstrndup(&string[start..], end - start));
    }
    None
}

// [spec:foma:def:flags.flag-get-value-fn]
// [spec:foma:sem:flags.flag-get-value-fn]
// [spec:foma:def:fomalibconf.flag-get-value-fn]
// [spec:foma:sem:fomalibconf.flag-get-value-fn]
pub fn flag_get_value(string: &str) -> Option<String> {
    let s = string.as_bytes();
    let mut first = 0usize;
    let mut start = 0usize;
    let mut end = 0usize;
    let len = s.len(); /* strlen(string) */

    /* for (i=0; i < len; i += (utf8skip(string+i) + 1)) */
    let mut i = 0usize;
    while i < len {
        if s[i] == b'.' && first == 0 {
            first = i + 1;
            i += utf8skip(&s[i..]) as usize + 1;
            continue;
        }
        if s[i] == b'@' && start != 0 {
            end = i;
            break;
        }
        if s[i] == b'.' && first != 0 {
            start = i + 1;
            i += utf8skip(&s[i..]) as usize + 1;
            continue;
        }
        i += utf8skip(&s[i..]) as usize + 1;
    }
    if start > 0 && end > 0 {
        return Some(xxstrndup(&string[start..], end - start));
    }
    None
}

// [spec:foma:def:flags.flag-twosided-fn]
// [spec:foma:sem:flags.flag-twosided-fn]
// [spec:foma:def:fomalib.flag-twosided-fn]
// [spec:foma:sem:fomalib.flag-twosided-fn]
pub fn flag_twosided(mut net: Box<Fsm>) -> Box<Fsm> {
    /* Enforces twosided flag diacritics */

    /* Mark flag symbols */
    let maxsigma = sigma_max(net.sigma.as_deref());
    /* C: calloc(maxsigma+1, sizeof(int)) */
    let mut isflag: Vec<i32> = vec![0; (maxsigma + 1) as usize];
    let mut sigma = net.sigma.as_deref();
    while let Some(s) = sigma {
        /* DEVIATION from C (an empty-sigma sentinel node has number == -1;
        the C writes isflag[-1], an out-of-bounds write — skipped here) */
        if s.number != -1 {
            if flag_check(s.symbol.as_deref().unwrap_or("")) != 0 {
                isflag[s.number as usize] = 1;
            } else {
                isflag[s.number as usize] = 0;
            }
        }
        sigma = s.next.as_deref();
    }
    let mut maxstate = 0;
    let mut change = 0;
    let mut i = 0usize;
    let mut newarcs = 0usize;
    while net.states[i].state_no != -1 {
        maxstate = if net.states[i].state_no > maxstate {
            net.states[i].state_no
        } else {
            maxstate
        };
        if net.states[i].target == -1 {
            i += 1;
            continue;
        }
        if isflag[net.states[i].r#in as usize] != 0 && net.states[i].out == EPSILON as i16 {
            change = 1;
            net.states[i].out = net.states[i].r#in;
        } else if isflag[net.states[i].out as usize] != 0 && net.states[i].r#in == EPSILON as i16 {
            change = 1;
            net.states[i].r#in = net.states[i].out;
        }
        if (isflag[net.states[i].r#in as usize] != 0 || isflag[net.states[i].out as usize] != 0)
            && net.states[i].r#in != net.states[i].out
        {
            newarcs += 1;
        }
        i += 1;
    }

    if newarcs == 0 {
        if change == 1 {
            net.is_deterministic = UNK;
            net.is_minimized = UNK;
            net.is_pruned = UNK;
            return fsm_topsort(fsm_minimize(net));
        }
        return net;
    }
    /* C: realloc(net->states, sizeof(struct fsm)*(i+newarcs)) — BUG kept as
    documented: sized with sizeof(struct fsm) instead of sizeof(struct
    fsm_state), a huge over-allocation that also hides that i+newarcs+1 slots
    are needed for the new sentinel. DEVIATION from C (the Vec grows to
    exactly the i+newarcs+1 lines actually written). */
    net.states.resize(
        i + newarcs + 1,
        FsmState {
            state_no: 0,
            r#in: 0,
            out: 0,
            target: 0,
            final_state: 0,
            start_state: 0,
        },
    );
    let tail = i;
    let mut j = i as i32;
    maxstate += 1;
    for i in 0..tail {
        if net.states[i].target == -1 {
            continue;
        }
        let in_i = net.states[i].r#in;
        let out_i = net.states[i].out;
        let target_i = net.states[i].target;
        if (isflag[in_i as usize] != 0 || isflag[out_i as usize] != 0) && in_i != out_i {
            if isflag[in_i as usize] != 0 && isflag[out_i as usize] == 0 {
                j = add_fsm_arc(
                    &mut net.states,
                    j,
                    maxstate,
                    EPSILON,
                    out_i as i32,
                    target_i,
                    0,
                    0,
                );
                net.states[i].out = net.states[i].r#in;
                net.states[i].target = maxstate;
                maxstate += 1;
            } else if isflag[out_i as usize] != 0 && isflag[in_i as usize] == 0 {
                j = add_fsm_arc(
                    &mut net.states,
                    j,
                    maxstate,
                    out_i as i32,
                    out_i as i32,
                    target_i,
                    0,
                    0,
                );
                net.states[i].out = EPSILON as i16;
                net.states[i].target = maxstate;
                maxstate += 1;
            } else if isflag[in_i as usize] != 0 && isflag[out_i as usize] != 0 {
                j = add_fsm_arc(
                    &mut net.states,
                    j,
                    maxstate,
                    out_i as i32,
                    out_i as i32,
                    target_i,
                    0,
                    0,
                );
                net.states[i].out = net.states[i].r#in;
                net.states[i].target = maxstate;
                maxstate += 1;
            }
        }
    }
    /* Add sentinel */
    add_fsm_arc(&mut net.states, j, -1, -1, -1, -1, -1, -1);
    net.is_deterministic = UNK;
    net.is_minimized = UNK;
    fsm_topsort(fsm_minimize(net))
}

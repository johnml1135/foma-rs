//! foma/flags.c — literal Wave-2 (bug-for-bug) port per
//! docs/port/rust-conventions.md. Sem rules: docs/spec/port/foma/flags.md.

use crate::constructions::{
    add_fsm_arc, fsm_complement, fsm_compose, fsm_concat, fsm_contains, fsm_intersect,
    fsm_optionality, fsm_symbol, fsm_union, fsm_universal,
};
use crate::minimize::fsm_minimize;
use crate::options::FomaOptions;
use crate::sigma::{sigma_cleanup, sigma_max, sigma_remove_num, sigma_sort};
use crate::structures::{fsm_copy, fsm_empty_set};
use crate::topsort::fsm_topsort;
use crate::types::FlagType;
use crate::types::{EPSILON, Fsm, FsmState, Tern};
use smol_str::SmolStr;

/// Pairwise flag-compatibility verdict from `flag_build` (C #defines FAIL=1 /
/// SUCCEED=2 / NONE=3; distinct from apply.c's FAIL/SUCCEED).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlagBuildResult {
    Fail,
    Succeed,
    None,
}

// [spec:foma:def:flags.flags]
pub struct Flags {
    pub r#type: FlagType,
    pub name: Option<SmolStr>,
    pub value: Option<SmolStr>,
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
// [spec:foma:sem:flags.flag-eliminate-fn+1]
// [spec:foma:def:fomalib.flag-eliminate-fn]
// [spec:foma:sem:fomalib.flag-eliminate-fn+1]
pub fn flag_eliminate(opts: &FomaOptions, net: Box<Fsm>, name: Option<&str>) -> Box<Fsm> {
    let mut filter: Option<Box<Fsm>> = None;

    if net.pathcount == 0 {
        if opts.verbose {
            tracing::warn!("Skipping flag elimination since there are no paths in network.");
        }
        return net;
    }

    let flags = flag_extract(&net);
    /* Check that flag actually exists in net */
    if let Some(name) = name {
        let mut found = false;
        let mut f = flags.as_deref();
        while let Some(fl) = f {
            /* strcmp(name, f->name) — C would segfault on a NULL f->name */
            if fl.name.as_deref() == Some(name) {
                found = true;
            }
            f = fl.next.as_deref();
        }
        if !found {
            if opts.verbose {
                tracing::warn!("Flag attribute '{}' does not occur in the network.", name);
            }
            return net;
        }
    }

    let mut flag = false;

    let mut f = flags.as_deref();
    while let Some(fl) = f {
        let mut succeed_flags: Option<Box<Fsm>> = None;
        let mut fail_flags: Option<Box<Fsm>> = None;
        let mut self_: Option<Box<Fsm>> = None;

        /* DEVIATION from C: the C ORed the type mask (`f->type | U|R|D|E`),
        which is always nonzero, so the intended restriction to U/R/D/E flags
        was a no-op and the body ran for every type. `intersects` actually
        restricts it. Observable language is unchanged: flag_build classifies
        pairs only when f's type is U, R, or D, so the P/N/C/E iterations the
        bug allowed never built a filter anyway. */
        if (name.is_none() || fl.name.as_deref() == name)
            && fl.r#type.intersects(
                FlagType::UNIFY | FlagType::REQUIRE | FlagType::DISALLOW | FlagType::EQUAL,
            )
        {
            succeed_flags = Some(fsm_empty_set());
            fail_flags = Some(fsm_empty_set());
            self_ = Some(flag_create_symbol(
                fl.r#type,
                fl.name.as_deref().expect("flag list node has a name"),
                fl.value.as_deref(),
            ));

            let mut ff = flags.as_deref();
            flag = false;
            while let Some(ffl) = ff {
                let fstatus = flag_build(
                    fl.r#type,
                    fl.name.as_deref().expect("flag list node has a name"),
                    fl.value.as_deref(),
                    ffl.r#type,
                    ffl.name.as_deref().expect("flag list node has a name"),
                    ffl.value.as_deref(),
                );
                if fstatus == FlagBuildResult::Fail {
                    fail_flags = Some(fsm_minimize(
                        opts,
                        fsm_union(
                            opts,
                            fail_flags
                                .take()
                                .expect("fail_flags populated when flag was set"),
                            flag_create_symbol(
                                ffl.r#type,
                                ffl.name.as_deref().expect("flag list node has a name"),
                                ffl.value.as_deref(),
                            ),
                        ),
                    ));
                    flag = true;
                }
                if fstatus == FlagBuildResult::Succeed {
                    succeed_flags = Some(fsm_minimize(
                        opts,
                        fsm_union(
                            opts,
                            succeed_flags
                                .take()
                                .expect("succeed_flags populated when flag was set"),
                            flag_create_symbol(
                                ffl.r#type,
                                ffl.name.as_deref().expect("flag list node has a name"),
                                ffl.value.as_deref(),
                            ),
                        ),
                    ));
                    flag = true;
                }
                ff = ffl.next.as_deref();
            }
        }

        if flag {
            let newfilter = if fl.r#type == FlagType::REQUIRE {
                fsm_complement(
                    opts,
                    fsm_concat(
                        opts,
                        fsm_optionality(
                            opts,
                            fsm_concat(
                                opts,
                                fsm_universal(),
                                fail_flags
                                    .take()
                                    .expect("fail_flags populated when flag was set"),
                            ),
                        ),
                        fsm_concat(
                            opts,
                            fsm_complement(
                                opts,
                                fsm_contains(
                                    opts,
                                    succeed_flags
                                        .take()
                                        .expect("succeed_flags populated when flag was set"),
                                ),
                            ),
                            fsm_concat(
                                opts,
                                self_.take().expect("self_ populated when flag was set"),
                                fsm_universal(),
                            ),
                        ),
                    ),
                )
            } else {
                fsm_complement(
                    opts,
                    fsm_contains(
                        opts,
                        fsm_concat(
                            opts,
                            fail_flags
                                .take()
                                .expect("fail_flags populated when flag was set"),
                            fsm_concat(
                                opts,
                                fsm_complement(
                                    opts,
                                    fsm_contains(
                                        opts,
                                        succeed_flags
                                            .take()
                                            .expect("succeed_flags populated when flag was set"),
                                    ),
                                ),
                                self_.take().expect("self_ populated when flag was set"),
                            ),
                        ),
                    ),
                )
            };

            filter = Some(match filter.take() {
                None => newfilter,
                Some(filter) => fsm_intersect(opts, filter, newfilter),
            });
        }
        flag = false;
        f = fl.next.as_deref();
    }

    let newnet = if let Some(mut filter) = filter {
        /* C saved g_flag_is_epsilon, forced it to 0 around the two composes,
        and restored it; here the composes get an option copy with it off. */
        let compose_opts = FomaOptions {
            flag_is_epsilon: false,
            ..opts.clone()
        };
        let newnet = fsm_compose(
            &compose_opts,
            fsm_copy(&mut filter),
            fsm_compose(&compose_opts, net, fsm_copy(&mut filter)),
        );
        /* the filter itself is never destroyed in C (leak); dropped here */
        newnet
    } else {
        net
    };

    let mut newnet = newnet;
    flag_purge(&mut newnet, name);
    let mut newnet = fsm_minimize(opts, newnet);
    sigma_cleanup(&mut newnet, 0);
    sigma_sort(&mut newnet);
    /* free(flags) — C frees only the list head (remaining nodes and their
    name/value strings leak); Rust drops the whole list */
    drop(flags);
    fsm_topsort(newnet)
}

// [spec:foma:def:flags.flag-create-symbol-fn]
// [spec:foma:sem:flags.flag-create-symbol-fn]
pub(crate) fn flag_create_symbol(r#type: FlagType, name: &str, value: Option<&str>) -> Box<Fsm> {
    let value = value.unwrap_or_default();

    /* C: string = malloc(strlen(name)+strlen(value)+6), built with strcat and
    never freed (leak); an owned String here */
    let mut string = String::new();
    string.push('@');
    /* flag_type_to_char(type) — C would segfault on NULL for unknown types */
    string.push_str(flag_type_to_char(r#type).expect("known flag type has a char"));
    string.push('.');
    string.push_str(name);
    if !value.is_empty() {
        string.push('.');
        string.push_str(value);
    }
    string.push('@');

    fsm_symbol(&string)
}

// [spec:foma:def:flags.flag-type-to-char-fn]
// [spec:foma:sem:flags.flag-type-to-char-fn]
pub(crate) fn flag_type_to_char(r#type: FlagType) -> Option<&'static str> {
    if r#type == FlagType::UNIFY {
        Some("U")
    } else if r#type == FlagType::CLEAR {
        Some("C")
    } else if r#type == FlagType::DISALLOW {
        Some("D")
    } else if r#type == FlagType::NEGATIVE {
        Some("N")
    } else if r#type == FlagType::POSITIVE {
        Some("P")
    } else if r#type == FlagType::REQUIRE {
        Some("R")
    } else if r#type == FlagType::EQUAL {
        Some("E")
    } else {
        None
    }
}

// [spec:foma:def:flags.flag-build-fn]
// [spec:foma:sem:flags.flag-build-fn]
// [spec:foma:def:fomalib.flag-build-fn]
// [spec:foma:sem:fomalib.flag-build-fn]
pub fn flag_build(
    ftype: FlagType,
    fname: &str,
    fvalue: Option<&str>,
    fftype: FlagType,
    ffname: &str,
    ffvalue: Option<&str>,
) -> FlagBuildResult {
    if fname != ffname {
        return FlagBuildResult::None;
    }
    /* selfnull: the eliminated flag is valueless, e.g. @R.A@ or @D.A@ */
    let selfnull = fvalue.is_none();
    let fvalue = fvalue.unwrap_or("");
    let ffvalue = ffvalue.unwrap_or("");
    /* eq mirrors the C's `strcmp(fvalue, ffvalue) == 0` */
    let eq = fvalue == ffvalue;

    /* Pairwise compatibility decision table (see the sem rule); first matching
    row wins, anything unlisted is FlagBuildResult::None. Columns: eliminated flag type, other
    flag type, required `eq` (None = don't care), required `selfnull`, result. */
    type Row = (
        FlagType,
        FlagType,
        Option<bool>,
        Option<bool>,
        FlagBuildResult,
    );
    #[rustfmt::skip]
    let rows: [Row; 25] = [
        /* U flags */
        (FlagType::UNIFY,    FlagType::POSITIVE, Some(true),  None,        FlagBuildResult::Succeed),
        (FlagType::UNIFY,    FlagType::CLEAR,    None,        None,        FlagBuildResult::Succeed),
        (FlagType::UNIFY,    FlagType::UNIFY,    Some(false), None,        FlagBuildResult::Fail),
        (FlagType::UNIFY,    FlagType::POSITIVE, Some(false), None,        FlagBuildResult::Fail),
        (FlagType::UNIFY,    FlagType::NEGATIVE, Some(true),  None,        FlagBuildResult::Fail),
        /* R flag, valueless */
        (FlagType::REQUIRE,  FlagType::UNIFY,    None,        Some(true),  FlagBuildResult::Succeed),
        (FlagType::REQUIRE,  FlagType::POSITIVE, None,        Some(true),  FlagBuildResult::Succeed),
        (FlagType::REQUIRE,  FlagType::NEGATIVE, None,        Some(true),  FlagBuildResult::Succeed),
        (FlagType::REQUIRE,  FlagType::CLEAR,    None,        Some(true),  FlagBuildResult::Fail),
        /* R flag, with value */
        (FlagType::REQUIRE,  FlagType::POSITIVE, Some(true),  Some(false), FlagBuildResult::Succeed),
        (FlagType::REQUIRE,  FlagType::UNIFY,    Some(true),  Some(false), FlagBuildResult::Succeed),
        (FlagType::REQUIRE,  FlagType::POSITIVE, Some(false), Some(false), FlagBuildResult::Fail),
        (FlagType::REQUIRE,  FlagType::UNIFY,    Some(false), Some(false), FlagBuildResult::Fail),
        (FlagType::REQUIRE,  FlagType::NEGATIVE, None,        Some(false), FlagBuildResult::Fail),
        (FlagType::REQUIRE,  FlagType::CLEAR,    None,        Some(false), FlagBuildResult::Fail),
        /* D flag, valueless */
        (FlagType::DISALLOW, FlagType::CLEAR,    None,        Some(true),  FlagBuildResult::Succeed),
        (FlagType::DISALLOW, FlagType::POSITIVE, None,        Some(true),  FlagBuildResult::Fail),
        (FlagType::DISALLOW, FlagType::UNIFY,    None,        Some(true),  FlagBuildResult::Fail),
        (FlagType::DISALLOW, FlagType::NEGATIVE, None,        Some(true),  FlagBuildResult::Fail),
        /* D flag, with value */
        (FlagType::DISALLOW, FlagType::POSITIVE, Some(false), Some(false), FlagBuildResult::Succeed),
        (FlagType::DISALLOW, FlagType::CLEAR,    None,        Some(false), FlagBuildResult::Succeed),
        (FlagType::DISALLOW, FlagType::NEGATIVE, Some(true),  Some(false), FlagBuildResult::Succeed),
        (FlagType::DISALLOW, FlagType::POSITIVE, Some(true),  Some(false), FlagBuildResult::Fail),
        (FlagType::DISALLOW, FlagType::UNIFY,    Some(true),  Some(false), FlagBuildResult::Fail),
        (FlagType::DISALLOW, FlagType::NEGATIVE, Some(false), Some(false), FlagBuildResult::Fail),
    ];
    for &(ft, fft, eq_req, null_req, result) in &rows {
        if ftype == ft
            && fftype == fft
            && eq_req.is_none_or(|r| r == eq)
            && null_req.is_none_or(|r| r == selfnull)
        {
            return result;
        }
    }
    FlagBuildResult::None
}

/* Remove flags that are being eliminated from arcs and sigma */

// [spec:foma:def:flags.flag-purge-fn]
// [spec:foma:sem:flags.flag-purge-fn]
pub(crate) fn flag_purge(net: &mut Fsm, name: Option<&str>) {
    let sigmasize = sigma_max(&net.sigma) + 1;
    /* C: malloc'd int array, zeroed by the following loop */
    let mut ftable: Vec<bool> = vec![false; sigmasize as usize];

    for s in &net.sigma {
        let symbol = s.symbol.as_str();
        if flag_check(symbol) {
            match name {
                None => {
                    ftable[s.number as usize] = true;
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
                        ftable[s.number as usize] = true;
                    }
                }
            }
        }
    }
    for i in 0..sigmasize {
        if ftable[i as usize] {
            sigma_remove_num(i, &mut net.sigma);
        }
    }

    let fsm = &mut net.states;
    let mut i = 0usize;
    while fsm[i].state_no != -1 {
        if fsm[i].r#in >= 0 && fsm[i].out >= 0 {
            if ftable[fsm[i].r#in as usize] {
                fsm[i].r#in = EPSILON as i16;
            }
            if ftable[fsm[i].out as usize] {
                fsm[i].out = EPSILON as i16;
            }
        }
        i += 1;
    }

    /* free(ftable) — drop */
    net.is_deterministic = Tern::No;
    net.is_minimized = Tern::No;
    net.is_epsilon_free = Tern::No;
}

/* Extract all flags from network and place them in struct flag linked list */

// [spec:foma:def:flags.flag-extract-fn]
// [spec:foma:sem:flags.flag-extract-fn]
pub(crate) fn flag_extract(net: &Fsm) -> Option<Box<Flags>> {
    let mut flags: Option<Box<Flags>> = None;
    for s in &net.sigma {
        let symbol = s.symbol.as_str();
        if flag_check(symbol) {
            let flagst = Box::new(Flags {
                r#type: flag_get_type(symbol),
                name: flag_get_name(symbol),
                value: flag_get_value(symbol),
                next: flags,
            });
            flags = Some(flagst);
        }
    }
    flags
}

// [spec:foma:def:flags.flag-check-fn]
// [spec:foma:sem:flags.flag-check-fn]
// [spec:foma:def:fomalibconf.flag-check-fn]
// [spec:foma:sem:fomalibconf.flag-check-fn]
pub fn flag_check(s: &str) -> bool {
    /* Byte-level DFA for the flag-diacritic grammar (ND = any byte that is
    neither '.' nor NUL): return true iff s matches
        "@" [U|P|N|E] "." ND+ "." ND+ "@"   (U/P/N/E: attribute AND value)
      | "@" [R|D]     "." ND+ ["." ND+] "@" (R/D: value optional)
      | "@" C         "." ND+ "@"           (C: never a value)
    The C used numeric goto labels s0..s11; here each is a `State` variant with
    the same transitions. The end of the &str stands in for the C NUL. */
    #[derive(Clone, Copy)]
    enum State {
        Start,
        S1,
        S2,
        S3,
        S4,
        S5,
        S6,
        S7,
        S8,
        S9,
        S10,
        S11,
    }
    use State::*;

    let s = s.as_bytes();
    let at = |i: usize| -> u8 { if i < s.len() { s[i] } else { 0 } };

    let mut i = 0usize;
    let mut state = Start;
    loop {
        state = match state {
            Start => match at(i) {
                b'@' => {
                    i += 1;
                    S1
                }
                _ => return false,
            },
            S1 => match at(i) {
                b'C' => {
                    i += 1;
                    S4
                }
                b'N' | b'E' | b'U' | b'P' => {
                    i += 1;
                    S2
                }
                b'R' | b'D' => {
                    i += 1;
                    S3
                }
                _ => return false,
            },
            /* operator letter '.': into the first (mandatory) field */
            S2 if at(i) == b'.' => {
                i += 1;
                S5
            }
            S3 if at(i) == b'.' => {
                i += 1;
                S6
            }
            S4 if at(i) == b'.' => {
                i += 1;
                S7
            }
            S2 | S3 | S4 => return false,
            /* first ND byte of each field must exist */
            S5 => match at(i) {
                b'.' | 0 => return false,
                _ => {
                    i += 1;
                    S8
                }
            },
            S6 => match at(i) {
                b'.' | 0 => return false,
                _ => {
                    i += 1;
                    S9
                }
            },
            S7 => match at(i) {
                b'.' | 0 => return false,
                _ => {
                    i += 1;
                    S10
                }
            },
            /* S8: first field of U/P/N/E — quirk: '@' is an ordinary ND byte
            here, so e.g. "@U.a@b.c@" is accepted; only '.' ends the field */
            S8 => match at(i) {
                b'.' => {
                    i += 1;
                    S7
                }
                0 => return false,
                _ => {
                    i += 1;
                    S8
                }
            },
            /* S9: single field of R/D — '@' ends the string, '.' opens a value */
            S9 => match at(i) {
                b'@' => {
                    i += 1;
                    S11
                }
                b'.' => {
                    i += 1;
                    S7
                }
                0 => return false,
                _ => {
                    i += 1;
                    S9
                }
            },
            /* S10: last field — only '@' ends it */
            S10 => match at(i) {
                b'@' => {
                    i += 1;
                    S11
                }
                b'.' | 0 => return false,
                _ => {
                    i += 1;
                    S10
                }
            },
            /* S11: accept iff the '@' was the final byte */
            S11 => return at(i) == 0,
        };
    }
}

// [spec:foma:def:flags.flag-get-type-fn]
// [spec:foma:sem:flags.flag-get-type-fn]
// [spec:foma:def:fomalibconf.flag-get-type-fn]
// [spec:foma:sem:fomalibconf.flag-get-type-fn]
pub fn flag_get_type(string: &str) -> FlagType {
    let b = string.as_bytes();
    /* strncmp(string+1, "X.", 2) == 0 — a string shorter than 3 bytes can't
    match (the C strncmp stops at the NUL and compares unequal) */
    let strncmp2 = |pat: &[u8; 2]| -> bool { b.len() >= 3 && b[1] == pat[0] && b[2] == pat[1] };
    if strncmp2(b"U.") {
        return FlagType::UNIFY;
    }
    if strncmp2(b"C.") {
        return FlagType::CLEAR;
    }
    if strncmp2(b"D.") {
        return FlagType::DISALLOW;
    }
    if strncmp2(b"N.") {
        return FlagType::NEGATIVE;
    }
    if strncmp2(b"P.") {
        return FlagType::POSITIVE;
    }
    if strncmp2(b"R.") {
        return FlagType::REQUIRE;
    }
    if strncmp2(b"E.") {
        return FlagType::EQUAL;
    }
    FlagType::empty()
}

// [spec:foma:def:flags.flag-get-name-fn]
// [spec:foma:sem:flags.flag-get-name-fn]
// [spec:foma:def:fomalibconf.flag-get-name-fn]
// [spec:foma:sem:fomalibconf.flag-get-name-fn]
pub fn flag_get_name(string: &str) -> Option<SmolStr> {
    // A flag diacritic is `@X.name.value@` / `@X.name@`: the name is the run
    // between the first '.' and the next '.' or '@'. Those delimiters are ASCII,
    // so they never occur inside a multi-byte char — walk characters directly.
    let mut start: Option<usize> = None;
    for (i, c) in string.char_indices() {
        match (c, start) {
            ('.', None) => start = Some(i + 1),
            ('.' | '@', Some(s)) => return Some(string[s..i].into()),
            _ => {}
        }
    }
    None
}

// [spec:foma:def:flags.flag-get-value-fn]
// [spec:foma:sem:flags.flag-get-value-fn]
// [spec:foma:def:fomalibconf.flag-get-value-fn]
// [spec:foma:sem:fomalibconf.flag-get-value-fn]
pub fn flag_get_value(string: &str) -> Option<SmolStr> {
    // The value is the run between the SECOND '.' and the closing '@'
    // (`@X.name.value@`); a one-argument flag `@X.name@` has no second '.', so
    // `start` stays unset and the closing '@' yields nothing. ASCII delimiters,
    // so walk characters directly.
    let mut seen_first_dot = false;
    let mut start: Option<usize> = None;
    for (i, c) in string.char_indices() {
        match (c, seen_first_dot, start) {
            ('.', false, _) => seen_first_dot = true,
            ('@', _, Some(s)) => return Some(string[s..i].into()),
            ('.', true, _) => start = Some(i + 1),
            _ => {}
        }
    }
    None
}

// [spec:foma:def:flags.flag-twosided-fn]
// [spec:foma:sem:flags.flag-twosided-fn]
// [spec:foma:def:fomalib.flag-twosided-fn]
// [spec:foma:sem:fomalib.flag-twosided-fn]
pub fn flag_twosided(opts: &FomaOptions, mut net: Box<Fsm>) -> Box<Fsm> {
    /* Enforces twosided flag diacritics */

    /* Mark flag symbols */
    let maxsigma = sigma_max(&net.sigma);
    /* C: calloc(maxsigma+1, sizeof(int)) */
    let mut isflag: Vec<bool> = vec![false; (maxsigma + 1) as usize];
    for s in &net.sigma {
        isflag[s.number as usize] = flag_check(&s.symbol);
    }
    let mut maxstate = 0;
    let mut change = false;
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
        if isflag[net.states[i].r#in as usize] && net.states[i].out == EPSILON as i16 {
            change = true;
            net.states[i].out = net.states[i].r#in;
        } else if isflag[net.states[i].out as usize] && net.states[i].r#in == EPSILON as i16 {
            change = true;
            net.states[i].r#in = net.states[i].out;
        }
        if (isflag[net.states[i].r#in as usize] || isflag[net.states[i].out as usize])
            && net.states[i].r#in != net.states[i].out
        {
            newarcs += 1;
        }
        i += 1;
    }

    if newarcs == 0 {
        if change {
            net.is_deterministic = Tern::Unk;
            net.is_minimized = Tern::Unk;
            net.is_pruned = Tern::Unk;
            return fsm_topsort(fsm_minimize(opts, net));
        }
        return net;
    }
    /* Grow the line table to hold the i original lines, newarcs split arcs,
    and one new sentinel. */
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
        if (isflag[in_i as usize] || isflag[out_i as usize]) && in_i != out_i {
            if isflag[in_i as usize] && !isflag[out_i as usize] {
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
            } else if isflag[out_i as usize] && !isflag[in_i as usize] {
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
            } else if isflag[in_i as usize] && isflag[out_i as usize] {
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
    net.is_deterministic = Tern::Unk;
    net.is_minimized = Tern::Unk;
    fsm_topsort(fsm_minimize(opts, net))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apply::{apply_init, apply_words};
    use crate::regex::fsm_parse_regex;
    use crate::types::FlagType;

    /* All symbols in a net's sigma (excluding the -1 sentinel), by symbol text. */
    fn sigma_syms(net: &Fsm) -> Vec<String> {
        let mut v: Vec<String> = net
            .sigma
            .iter()
            .map(|node| node.symbol.to_string())
            .collect();
        v.sort();
        v
    }

    /* Map a sigma number to its symbol text; EPSILON prints as "0". */
    fn num_to_sym(net: &Fsm, n: i16) -> String {
        if n as i32 == EPSILON {
            return "0".to_string();
        }
        for node in &net.sigma {
            if node.number == n as i32 {
                return node.symbol.to_string();
            }
        }
        format!("#{}", n)
    }

    /* Multiset of (in,out) arc labels as symbol text. */
    fn arc_labels(net: &Fsm) -> Vec<(String, String)> {
        let mut v: Vec<(String, String)> = net
            .states
            .iter()
            .take_while(|l| l.state_no != -1)
            .filter(|l| l.target != -1)
            .map(|l| (num_to_sym(net, l.r#in), num_to_sym(net, l.out)))
            .collect();
        v.sort();
        v
    }

    /* Enumerate the whole (finite) language via apply_words. */
    fn all_words(net: &Fsm) -> Vec<String> {
        let mut h = apply_init(net);
        let mut v = Vec::new();
        let mut r = apply_words(&mut h);
        while let Some(s) = r {
            v.push(s);
            r = apply_words(&mut h);
        }
        v.sort();
        v.dedup();
        v
    }

    // [spec:foma:sem:flags.flag-check-fn/test]
    // [spec:foma:sem:fomalibconf.flag-check-fn/test]
    #[test]
    fn flag_check_dfa() {
        /* U/P/N/E require both attribute and value */
        assert!(flag_check("@U.F.V@"));
        assert!(flag_check("@P.F.V@"));
        assert!(flag_check("@N.F.V@"));
        assert!(flag_check("@E.F.V@"));
        /* R/D value optional */
        assert!(flag_check("@R.F@"));
        assert!(flag_check("@D.F@"));
        assert!(flag_check("@R.F.V@"));
        assert!(flag_check("@D.F.V@"));
        /* C never takes a value */
        assert!(flag_check("@C.X@"));
        /* Quirk: '@' is an ordinary ND byte in the mandatory first field of
        U/P/N/E, so this is accepted despite the interior '@' */
        assert!(flag_check("@U.a@b.c@"));

        /* Rejections */
        assert!(!(flag_check("")));
        assert!(!(flag_check("a")));
        assert!(!(flag_check("@Z.F.V@")), "bad operator letter");
        assert!(!(flag_check("@U.F@")), "U needs a value field");
        assert!(!(flag_check("@C.X.Y@")), "C may not take a value");
        assert!(!(flag_check("@R.F.V.W@")), "at most two fields");
        assert!(!(flag_check("@U.F.V@x")), "must end right after '@'");
    }

    // [spec:foma:sem:flags.flag-get-type-fn/test]
    // [spec:foma:sem:fomalibconf.flag-get-type-fn/test]
    // [spec:foma:sem:flags.flag-get-name-fn/test]
    // [spec:foma:sem:fomalibconf.flag-get-name-fn/test]
    // [spec:foma:sem:flags.flag-get-value-fn/test]
    // [spec:foma:sem:fomalibconf.flag-get-value-fn/test]
    #[test]
    fn flag_field_extractors() {
        assert_eq!(flag_get_type("@U.F.V@"), FlagType::UNIFY);
        assert_eq!(flag_get_type("@C.X@"), FlagType::CLEAR);
        assert_eq!(flag_get_type("@D.F@"), FlagType::DISALLOW);
        assert_eq!(flag_get_type("@N.F.V@"), FlagType::NEGATIVE);
        assert_eq!(flag_get_type("@P.F.V@"), FlagType::POSITIVE);
        assert_eq!(flag_get_type("@R.A@"), FlagType::REQUIRE);
        assert_eq!(flag_get_type("@E.F.V@"), FlagType::EQUAL);
        assert_eq!(flag_get_type("@Z.x@"), FlagType::empty());

        assert_eq!(flag_get_name("@U.FEAT.VAL@").as_deref(), Some("FEAT"));
        assert_eq!(flag_get_name("@R.A@").as_deref(), Some("A"));

        assert_eq!(flag_get_value("@U.FEAT.VAL@").as_deref(), Some("VAL"));
        /* valueless flags yield NULL */
        assert_eq!(flag_get_value("@R.A@"), None);
        assert_eq!(flag_get_value("@C.X@"), None);
        assert_eq!(flag_get_value("@D.F@"), None);
    }

    // [spec:foma:sem:flags.flag-type-to-char-fn/test]
    // [spec:foma:sem:flags.flag-create-symbol-fn/test]
    #[test]
    fn flag_create_symbol_builds_symbol() {
        assert_eq!(flag_type_to_char(FlagType::UNIFY), Some("U"));
        assert_eq!(flag_type_to_char(FlagType::CLEAR), Some("C"));
        assert_eq!(flag_type_to_char(FlagType::DISALLOW), Some("D"));
        assert_eq!(flag_type_to_char(FlagType::NEGATIVE), Some("N"));
        assert_eq!(flag_type_to_char(FlagType::POSITIVE), Some("P"));
        assert_eq!(flag_type_to_char(FlagType::REQUIRE), Some("R"));
        assert_eq!(flag_type_to_char(FlagType::EQUAL), Some("E"));
        assert_eq!(flag_type_to_char(FlagType::from_bits_retain(999)), None);

        /* "@" + type + "." + name + "." + value + "@" */
        let n = flag_create_symbol(FlagType::UNIFY, "F", Some("V"));
        assert_eq!(sigma_syms(&n), vec!["@U.F.V@".to_string()]);
        /* value omitted when NULL */
        let n = flag_create_symbol(FlagType::REQUIRE, "A", None);
        assert_eq!(sigma_syms(&n), vec!["@R.A@".to_string()]);
        /* value omitted when empty */
        let n = flag_create_symbol(FlagType::POSITIVE, "F", Some(""));
        assert_eq!(sigma_syms(&n), vec!["@P.F@".to_string()]);
    }

    // [spec:foma:sem:flags.flag-build-fn/test]
    // [spec:foma:sem:fomalib.flag-build-fn/test]
    #[test]
    fn flag_build_rows() {
        /* Different attribute names -> NONE immediately */
        assert_eq!(
            flag_build(
                FlagType::UNIFY,
                "F",
                Some("1"),
                FlagType::UNIFY,
                "G",
                Some("1")
            ),
            FlagBuildResult::None
        );

        /* U rows (first matching row wins) */
        assert_eq!(
            flag_build(
                FlagType::UNIFY,
                "F",
                Some("1"),
                FlagType::POSITIVE,
                "F",
                Some("1")
            ),
            FlagBuildResult::Succeed
        );
        assert_eq!(
            flag_build(FlagType::UNIFY, "F", Some("1"), FlagType::CLEAR, "F", None),
            FlagBuildResult::Succeed
        );
        assert_eq!(
            flag_build(
                FlagType::UNIFY,
                "F",
                Some("1"),
                FlagType::UNIFY,
                "F",
                Some("2")
            ),
            FlagBuildResult::Fail
        );
        assert_eq!(
            flag_build(
                FlagType::UNIFY,
                "F",
                Some("1"),
                FlagType::POSITIVE,
                "F",
                Some("2")
            ),
            FlagBuildResult::Fail
        );
        assert_eq!(
            flag_build(
                FlagType::UNIFY,
                "F",
                Some("1"),
                FlagType::NEGATIVE,
                "F",
                Some("1")
            ),
            FlagBuildResult::Fail
        );
        /* U vs U equal value is explicitly NONE */
        assert_eq!(
            flag_build(
                FlagType::UNIFY,
                "F",
                Some("1"),
                FlagType::UNIFY,
                "F",
                Some("1")
            ),
            FlagBuildResult::None
        );

        /* R valueless */
        assert_eq!(
            flag_build(
                FlagType::REQUIRE,
                "F",
                None,
                FlagType::UNIFY,
                "F",
                Some("1")
            ),
            FlagBuildResult::Succeed
        );
        assert_eq!(
            flag_build(
                FlagType::REQUIRE,
                "F",
                None,
                FlagType::POSITIVE,
                "F",
                Some("1")
            ),
            FlagBuildResult::Succeed
        );
        assert_eq!(
            flag_build(
                FlagType::REQUIRE,
                "F",
                None,
                FlagType::NEGATIVE,
                "F",
                Some("1")
            ),
            FlagBuildResult::Succeed
        );
        assert_eq!(
            flag_build(FlagType::REQUIRE, "F", None, FlagType::CLEAR, "F", None),
            FlagBuildResult::Fail
        );
        /* R with value */
        assert_eq!(
            flag_build(
                FlagType::REQUIRE,
                "F",
                Some("1"),
                FlagType::POSITIVE,
                "F",
                Some("1")
            ),
            FlagBuildResult::Succeed
        );
        assert_eq!(
            flag_build(
                FlagType::REQUIRE,
                "F",
                Some("1"),
                FlagType::POSITIVE,
                "F",
                Some("2")
            ),
            FlagBuildResult::Fail
        );

        /* D valueless */
        assert_eq!(
            flag_build(FlagType::DISALLOW, "F", None, FlagType::CLEAR, "F", None),
            FlagBuildResult::Succeed
        );
        assert_eq!(
            flag_build(
                FlagType::DISALLOW,
                "F",
                None,
                FlagType::POSITIVE,
                "F",
                Some("1")
            ),
            FlagBuildResult::Fail
        );
        /* D with value */
        assert_eq!(
            flag_build(
                FlagType::DISALLOW,
                "F",
                Some("1"),
                FlagType::POSITIVE,
                "F",
                Some("2")
            ),
            FlagBuildResult::Succeed
        );
        assert_eq!(
            flag_build(
                FlagType::DISALLOW,
                "F",
                Some("1"),
                FlagType::NEGATIVE,
                "F",
                Some("1")
            ),
            FlagBuildResult::Succeed
        );
        assert_eq!(
            flag_build(
                FlagType::DISALLOW,
                "F",
                Some("1"),
                FlagType::POSITIVE,
                "F",
                Some("1")
            ),
            FlagBuildResult::Fail
        );

        /* Any ftype of C/N/P/E yields NONE (masks the flag_eliminate `|` bug) */
        assert_eq!(
            flag_build(
                FlagType::POSITIVE,
                "F",
                Some("1"),
                FlagType::UNIFY,
                "F",
                Some("1")
            ),
            FlagBuildResult::None
        );
        assert_eq!(
            flag_build(
                FlagType::NEGATIVE,
                "F",
                Some("1"),
                FlagType::UNIFY,
                "F",
                Some("1")
            ),
            FlagBuildResult::None
        );
        assert_eq!(
            flag_build(
                FlagType::EQUAL,
                "F",
                Some("1"),
                FlagType::UNIFY,
                "F",
                Some("1")
            ),
            FlagBuildResult::None
        );
        assert_eq!(
            flag_build(FlagType::CLEAR, "F", None, FlagType::UNIFY, "F", Some("1")),
            FlagBuildResult::None
        );
    }

    // [spec:foma:sem:flags.flag-extract-fn/test]
    #[test]
    fn flag_extract_from_sigma() {
        let opts = &FomaOptions::default();
        let net = fsm_parse_regex(opts, r#""@U.F.1@" a "@R.G@""#, None, None).unwrap();
        let flags = flag_extract(&net);
        /* Collect (type, name, value) triples; non-flag "a" is excluded. */
        let mut got: Vec<(FlagType, Option<String>, Option<String>)> = Vec::new();
        let mut f = flags.as_deref();
        while let Some(fl) = f {
            got.push((
                fl.r#type,
                fl.name.as_deref().map(String::from),
                fl.value.as_deref().map(String::from),
            ));
            f = fl.next.as_deref();
        }
        got.sort();
        assert_eq!(
            got,
            vec![
                (
                    FlagType::UNIFY,
                    Some("F".to_string()),
                    Some("1".to_string())
                ),
                (FlagType::REQUIRE, Some("G".to_string()), None),
            ]
        );
        /* A net with no flag symbols yields the empty list. */
        let plain = fsm_parse_regex(opts, "a b c", None, None).unwrap();
        assert!(flag_extract(&plain).is_none());
    }

    // [spec:foma:sem:flags.flag-purge-fn/test]
    #[test]
    fn flag_purge_targeted() {
        let opts = &FomaOptions::default();
        /* Purge only attribute F; G survives. */
        let mut net = fsm_parse_regex(opts, r#""@U.F.1@" a "@U.G.1@""#, None, None).unwrap();
        flag_purge(&mut net, Some("F"));
        let syms = sigma_syms(&net);
        assert!(
            !syms.contains(&"@U.F.1@".to_string()),
            "F symbol removed from sigma"
        );
        assert!(syms.contains(&"@U.G.1@".to_string()), "G symbol kept");
        /* The F arc became epsilon; the G arc is unchanged. */
        let labels = arc_labels(&net);
        assert!(labels.iter().any(|(i, o)| i == "@U.G.1@" && o == "@U.G.1@"));
        assert!(!labels.iter().any(|(i, _)| i == "@U.F.1@"));
        assert_eq!(net.is_deterministic, Tern::No);
        assert_eq!(net.is_minimized, Tern::No);
        assert_eq!(net.is_epsilon_free, Tern::No);

        /* name == None purges every flag. */
        let mut net2 = fsm_parse_regex(opts, r#""@U.F.1@" a "@U.G.1@""#, None, None).unwrap();
        flag_purge(&mut net2, None);
        let syms2 = sigma_syms(&net2);
        assert!(!syms2.contains(&"@U.F.1@".to_string()));
        assert!(!syms2.contains(&"@U.G.1@".to_string()));
        assert!(syms2.contains(&"a".to_string()));
    }

    // [spec:foma:sem:flags.flag-eliminate-fn+1/test]
    // [spec:foma:sem:fomalib.flag-eliminate-fn+1/test]
    #[test]
    fn flag_eliminate_end_to_end() {
        let opts = &FomaOptions::default();
        /* U/R/D flags. Surviving paths (verified against C foma):
        @P.F.1@ a @U.F.1@ -> "a" (U unify equal), the @P.F.2@ b @U.F.1@ path
        fails (U unify unequal); @P.G.1@ c @R.G@ -> "c" (R require satisfied),
        d @R.G@ fails (nothing set G); e @D.H@ -> "e" (D disallow, H unset),
        @P.H.1@ f @D.H@ fails (D disallow but H set). */
        let src = r#"["@P.F.1@" a "@U.F.1@"] | ["@P.F.2@" b "@U.F.1@"] | ["@P.G.1@" c "@R.G@"] | [d "@R.G@"] | [e "@D.H@"] | ["@P.H.1@" f "@D.H@"]"#;
        let net = fsm_parse_regex(opts, src, None, None).unwrap();
        let result = flag_eliminate(opts, net, None);
        /* Wave 4 fix (`&` type mask): the body now runs only for U/R/D/E flags.
        Because flag_build already classified nothing for the other types, the
        observable flag-filtered language is identical to the pre-fix behavior. */
        assert_eq!(all_words(&result), vec!["a", "c", "e"]);
        /* No flag symbols remain in sigma. */
        for s in sigma_syms(&result) {
            assert!(!(flag_check(&s)), "no flag symbols remain: {}", s);
        }

        /* Eliminating a single named attribute leaves the other flags' effect. */
        let net2 = fsm_parse_regex(
            opts,
            r#"["@P.F.1@" a "@U.F.1@"] | ["@P.F.2@" b "@U.F.1@"]"#,
            None,
            None,
        )
        .unwrap();
        let result2 = flag_eliminate(opts, net2, Some("F"));
        assert_eq!(all_words(&result2), vec!["a"]);
    }

    // [spec:foma:sem:flags.flag-twosided-fn/test]
    // [spec:foma:sem:fomalib.flag-twosided-fn/test]
    #[test]
    fn flag_twosided_arc_split() {
        let opts = &FomaOptions::default();
        /* Pure repair: a flag:epsilon arc gains the flag on the output tape
        (newarcs == 0, change == 1). */
        let net = fsm_parse_regex(opts, r#""@U.F.1@":0"#, None, None).unwrap();
        let net = flag_twosided(opts, net);
        assert_eq!(
            arc_labels(&net),
            vec![("@U.F.1@".to_string(), "@U.F.1@".to_string())]
        );

        /* Arc splitting: flag:real (in is a flag, out a real symbol, in != out)
        splits into flag:flag then epsilon:real via a fresh intermediate state. */
        let net = fsm_parse_regex(opts, r#""@U.F.1@":a"#, None, None).unwrap();
        let net = flag_twosided(opts, net);
        assert_eq!(
            arc_labels(&net),
            vec![
                ("0".to_string(), "a".to_string()),
                ("@U.F.1@".to_string(), "@U.F.1@".to_string()),
            ]
        );
        /* Three states on the split path (start -> S -> final). */
        assert_eq!(net.statecount, 3);
    }
}

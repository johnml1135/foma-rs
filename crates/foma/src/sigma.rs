//! foma/sigma.c — literal (bug-for-bug) Wave-2 port.
//!
//! The sigma alphabet is the C singly linked list modeled as
//! `Option<Box<Sigma>>` chains (see types.rs). An empty sigma is a single
//! sentinel node with number == -1, symbol NULL. Functions that consume the
//! list head in C (returning a possibly-new head) take/return
//! `Option<Box<Sigma>>`; functions that mutate through a non-NULL head take
//! `&mut Sigma`; read-only walks take `Option<&Sigma>` (NULL ↔ None).

use crate::types::{EPSILON, Fsm, FsmSigmaList, IDENTITY, Sigma, UNKNOWN};

// [spec:foma:def:sigma.sigma-remove-fn]
// [spec:foma:sem:sigma.sigma-remove-fn]
// [spec:foma:def:fomalibconf.sigma-remove-fn]
// [spec:foma:sem:fomalibconf.sigma-remove-fn]
pub fn sigma_remove(symbol: &str, sigma: Option<Box<Sigma>>) -> Option<Box<Sigma>> {
    let mut sigma_start = sigma;
    /* head node: C's loop condition (sigma != NULL && sigma->number != -1) */
    match sigma_start.as_deref() {
        None => return sigma_start,
        Some(head) if head.number == -1 => return sigma_start,
        _ => {}
    }
    if sigma_start.as_deref().unwrap().symbol.as_deref() == Some(symbol) {
        /* sigma_prev == NULL: new head is sigma->next; node and symbol freed (drop) */
        let head = sigma_start.take().unwrap();
        return head.next;
    }
    let mut sigma_prev: &mut Sigma = sigma_start.as_deref_mut().unwrap();
    loop {
        let matched = match sigma_prev.next.as_deref() {
            Some(sigma) if sigma.number != -1 => sigma.symbol.as_deref() == Some(symbol),
            _ => break,
        };
        if matched {
            /* sigma_prev->next = sigma->next; node and symbol freed (drop) */
            let node = sigma_prev.next.take().unwrap();
            sigma_prev.next = node.next;
            break;
        }
        sigma_prev = sigma_prev.next.as_deref_mut().unwrap();
    }
    sigma_start
}

// [spec:foma:def:sigma.sigma-remove-num-fn]
// [spec:foma:sem:sigma.sigma-remove-num-fn]
// [spec:foma:def:fomalibconf.sigma-remove-num-fn]
// [spec:foma:sem:fomalibconf.sigma-remove-num-fn]
pub fn sigma_remove_num(num: i32, sigma: Option<Box<Sigma>>) -> Option<Box<Sigma>> {
    let mut sigma_start = sigma;
    /* head node: C's loop condition (sigma != NULL && sigma->number != -1) */
    match sigma_start.as_deref() {
        None => return sigma_start,
        Some(head) if head.number == -1 => return sigma_start,
        _ => {}
    }
    if sigma_start.as_deref().unwrap().number == num {
        /* sigma_prev == NULL: new head is sigma->next; node and symbol freed (drop) */
        let head = sigma_start.take().unwrap();
        return head.next;
    }
    let mut sigma_prev: &mut Sigma = sigma_start.as_deref_mut().unwrap();
    loop {
        let matched = match sigma_prev.next.as_deref() {
            Some(sigma) if sigma.number != -1 => sigma.number == num,
            _ => break,
        };
        if matched {
            /* sigma_prev->next = sigma->next; node and symbol freed (drop) */
            let node = sigma_prev.next.take().unwrap();
            sigma_prev.next = node.next;
            break;
        }
        sigma_prev = sigma_prev.next.as_deref_mut().unwrap();
    }
    sigma_start
}

// [spec:foma:def:sigma.sigma-add-special-fn]
// [spec:foma:sem:sigma.sigma-add-special-fn+1]
// [spec:foma:def:fomalibconf.sigma-add-special-fn]
// [spec:foma:sem:fomalibconf.sigma-add-special-fn+1]
pub fn sigma_add_special(symbol: i32, sigma: &mut Sigma) -> i32 {
    // [spec:foma:sem:sigma.sigma-add-special-fn+1] map the reserved code to its
    // symbol string. A non-reserved code is a caller error; insert a well-formed
    // placeholder rather than the symbol-less (NULL) node C created, which later
    // panicked when the sigma was read back.
    let str: Option<String> = Some(match symbol {
        EPSILON => "@_EPSILON_SYMBOL_@".to_string(),
        UNKNOWN => "@_UNKNOWN_SYMBOL_@".to_string(),
        IDENTITY => "@_IDENTITY_SYMBOL_@".to_string(),
        other => format!("@_SPECIAL_{other}_@"),
    });

    /* Insert special symbols pre-sorted */
    if sigma.number == -1 {
        sigma.number = symbol;
    } else {
        /* C scans with a (sigma_previous, sigma) pointer pair while
        (sigma != NULL && sigma->number < symbol && sigma->number != -1);
        safe-Rust equivalent: peek-ahead walk from the head */
        if !(sigma.number < symbol && sigma.number != -1) {
            /* sigma_previous == NULL: head insertion — copy the head's fields
            into the splice node, overwrite the head in place (the head
            pointer never changes) */
            let sigma_splice = Box::new(Sigma {
                symbol: sigma.symbol.take(),
                number: sigma.number,
                next: sigma.next.take(),
            });
            sigma.number = symbol;
            sigma.symbol = str;
            sigma.next = Some(sigma_splice);
            return symbol;
        }
        let mut sigma_previous: &mut Sigma = sigma;
        loop {
            let advance = match sigma_previous.next.as_deref() {
                Some(s) => s.number < symbol && s.number != -1,
                None => false,
            };
            if !advance {
                break;
            }
            sigma_previous = sigma_previous.next.as_deref_mut().unwrap();
        }
        /* sigma_previous != NULL: splice between previous and the scan node */
        let sigma_splice = Box::new(Sigma {
            number: symbol,
            symbol: str,
            next: sigma_previous.next.take(),
        });
        sigma_previous.next = Some(sigma_splice);
        return symbol;
    }
    sigma.next = None;
    sigma.symbol = str;
    symbol
}

/* WARNING: this function will indeed add a symbol to sigma */
/* but it's up to the user to sort the sigma (affecting arc numbers in the network) */
/* before merge_sigma() is ever called */

// [spec:foma:def:sigma.sigma-add-fn]
// [spec:foma:sem:sigma.sigma-add-fn]
// [spec:foma:def:fomalibconf.sigma-add-fn]
// [spec:foma:sem:fomalibconf.sigma-add-fn]
pub fn sigma_add(symbol: &str, sigma: &mut Sigma) -> i32 {
    let mut assert = -1;

    /* Special characters */
    if symbol == "@_EPSILON_SYMBOL_@" {
        assert = EPSILON;
    }
    if symbol == "@_IDENTITY_SYMBOL_@" {
        assert = IDENTITY;
    }
    if symbol == "@_UNKNOWN_SYMBOL_@" {
        assert = UNKNOWN;
    }

    /* Insert non-special in any order */
    if assert == -1 {
        if sigma.number == -1 {
            sigma.number = 3;
            sigma.next = None;
            sigma.symbol = Some(symbol.to_string());
            sigma.number
        } else {
            /* walk to the tail node */
            let mut tail: &mut Sigma = sigma;
            while tail.next.is_some() {
                tail = tail.next.as_deref_mut().unwrap();
            }
            /* new number = tail->number + 1, clamped up to 3 (the number comes
            from the tail node, not the list maximum) */
            let number = if (tail.number) + 1 < 3 {
                3
            } else {
                (tail.number) + 1
            };
            tail.next = Some(Box::new(Sigma {
                number,
                symbol: Some(symbol.to_string()),
                next: None,
            }));
            number
        }
    } else {
        /* Insert special symbols pre-sorted */
        if sigma.number == -1 {
            sigma.number = assert;
        } else {
            /* C scans with a (sigma_previous, sigma) pointer pair while
            (sigma != NULL && sigma->number < assert && sigma->number != -1) */
            if !(sigma.number < assert && sigma.number != -1) {
                /* sigma_previous == NULL: head insertion — copy the head's
                fields into the splice node, overwrite the head in place */
                let sigma_splice = Box::new(Sigma {
                    symbol: sigma.symbol.take(),
                    number: sigma.number,
                    next: sigma.next.take(),
                });
                sigma.number = assert;
                sigma.symbol = Some(symbol.to_string());
                sigma.next = Some(sigma_splice);
                return assert;
            }
            let mut sigma_previous: &mut Sigma = sigma;
            loop {
                let advance = match sigma_previous.next.as_deref() {
                    Some(s) => s.number < assert && s.number != -1,
                    None => false,
                };
                if !advance {
                    break;
                }
                sigma_previous = sigma_previous.next.as_deref_mut().unwrap();
            }
            /* sigma_previous != NULL: splice between previous and the scan node */
            let sigma_splice = Box::new(Sigma {
                number: assert,
                symbol: Some(symbol.to_string()),
                next: sigma_previous.next.take(),
            });
            sigma_previous.next = Some(sigma_splice);
            return assert;
        }
        sigma.next = None;
        sigma.symbol = Some(symbol.to_string());
        assert
    }
}

/* Remove symbols that are never used from sigma and renumber   */
/* The variable force controls whether to remove even though    */
/* @ or ? is present                                            */
/* If force == 1, unused symbols are always removed regardless  */

// [spec:foma:def:sigma.sigma-cleanup-fn]
// [spec:foma:sem:sigma.sigma-cleanup-fn]
// [spec:foma:def:fomalibconf.sigma-cleanup-fn]
// [spec:foma:sem:fomalibconf.sigma-cleanup-fn]
pub fn sigma_cleanup(net: &mut Fsm, force: i32) {
    if force == 0 {
        if sigma_find_number(IDENTITY, net.sigma.as_deref()) != -1 {
            return;
        }
        if sigma_find_number(UNKNOWN, net.sigma.as_deref()) != -1 {
            return;
        }
    }

    let maxsigma = sigma_max(net.sigma.as_deref());
    if maxsigma < 0 {
        return;
    }
    /* C: malloc(sizeof(int)*(maxsigma+1)) followed by an explicit zeroing loop */
    let mut attested: Vec<i32> = vec![0; maxsigma as usize + 1];
    let fsm = &mut net.states;
    let mut i = 0usize;
    while fsm[i].state_no != -1 {
        if fsm[i].r#in >= 0 {
            attested[fsm[i].r#in as usize] = 1;
        }
        if fsm[i].out >= 0 {
            attested[fsm[i].out as usize] = 1;
        }
        i += 1;
    }
    let mut j: i32 = 3;
    let mut i: i32 = 3;
    while i <= maxsigma {
        if attested[i as usize] != 0 {
            attested[i as usize] = j;
            j += 1;
        }
        i += 1;
    }
    let mut i = 0usize;
    while fsm[i].state_no != -1 {
        if fsm[i].r#in > 2 {
            fsm[i].r#in = attested[fsm[i].r#in as usize] as i16;
        }
        if fsm[i].out > 2 {
            fsm[i].out = attested[fsm[i].out as usize] as i16;
        }
        i += 1;
    }
    /* C walks with sig_prev, unlinking unattested nodes (updating net->sigma
    when the head is removed); safe-Rust equivalent: a cursor over the owning
    Option links, so head and interior removals are the same operation */
    let mut sig: &mut Option<Box<Sigma>> = &mut net.sigma;
    loop {
        let remove = match sig.as_deref() {
            Some(node) if node.number != -1 => attested[node.number as usize] == 0,
            _ => break,
        };
        if remove {
            /* free(sig->symbol); free(sig); relink (drop) */
            let removed = sig.take().unwrap();
            *sig = removed.next;
        } else {
            match sig {
                Some(node) => {
                    node.number = if node.number >= 3 {
                        attested[node.number as usize]
                    } else {
                        node.number
                    };
                    sig = &mut node.next;
                }
                None => break,
            }
        }
    }
}

// [spec:foma:def:sigma.sigma-max-fn]
// [spec:foma:sem:sigma.sigma-max-fn]
// [spec:foma:def:fomalibconf.sigma-max-fn]
// [spec:foma:sem:fomalibconf.sigma-max-fn]
pub fn sigma_max(sigma: Option<&Sigma>) -> i32 {
    if sigma.is_none() {
        return -1;
    }
    /* visits every node (no -1 sentinel stop), accumulator starts at -1 */
    let mut i = -1;
    let mut sigma = sigma;
    while let Some(s) = sigma {
        i = if s.number > i { s.number } else { i };
        sigma = s.next.as_deref();
    }
    i
}

// [spec:foma:def:sigma.sigma-size-fn]
// [spec:foma:sem:sigma.sigma-size-fn]
// [spec:foma:def:fomalibconf.sigma-size-fn]
// [spec:foma:sem:fomalibconf.sigma-size-fn]
pub fn sigma_size(sigma: Option<&Sigma>) -> i32 {
    /* raw node count — the empty sentinel counts as a node */
    let mut i = 0;
    let mut sigma = sigma;
    while let Some(s) = sigma {
        i += 1;
        sigma = s.next.as_deref();
    }
    i
}

// [spec:foma:def:sigma.sigma-to-list-fn]
// [spec:foma:sem:sigma.sigma-to-list-fn]
// [spec:foma:def:fomalibconf.sigma-to-list-fn]
// [spec:foma:sem:fomalibconf.sigma-to-list-fn]
pub fn sigma_to_list(sigma: Option<&Sigma>) -> Vec<FsmSigmaList> {
    /* calloc(sigma_max(sigma)+1, ...) — zero entries for an empty sigma */
    let mut sl: Vec<FsmSigmaList> =
        vec![FsmSigmaList { symbol: None }; (sigma_max(sigma) + 1) as usize];
    let mut s = sigma;
    while let Some(node) = s {
        if node.number == -1 {
            break;
        }
        // DEVIATION from C (symbol pointers alias the sigma's strings; owned copies here)
        sl[node.number as usize].symbol = node.symbol.clone();
        s = node.next.as_deref();
    }
    sl
}

// [spec:foma:def:sigma.sigma-add-number-fn]
// [spec:foma:sem:sigma.sigma-add-number-fn]
// [spec:foma:def:fomalibconf.sigma-add-number-fn]
// [spec:foma:sem:fomalibconf.sigma-add-number-fn]
pub fn sigma_add_number(sigma: &mut Sigma, symbol: &str, number: i32) -> i32 {
    if sigma.number == -1 {
        sigma.symbol = Some(symbol.to_string());
        sigma.number = number;
        sigma.next = None;
        return 1;
    }
    /* C walks newsigma to NULL keeping prev_sigma — i.e. prev_sigma ends at
    the tail node */
    let mut prev_sigma: &mut Sigma = sigma;
    while prev_sigma.next.is_some() {
        prev_sigma = prev_sigma.next.as_deref_mut().unwrap();
    }
    let newsigma = Box::new(Sigma {
        symbol: Some(symbol.to_string()),
        number,
        next: None,
    });
    prev_sigma.next = Some(newsigma);
    1
}

// [spec:foma:def:sigma.sigma-find-number-fn]
// [spec:foma:sem:sigma.sigma-find-number-fn]
// [spec:foma:def:fomalibconf.sigma-find-number-fn]
// [spec:foma:sem:fomalibconf.sigma-find-number-fn]
pub fn sigma_find_number(number: i32, sigma: Option<&Sigma>) -> i32 {
    let mut sigma = match sigma {
        None => return -1,
        Some(s) => {
            if s.number == -1 {
                return -1;
            }
            Some(s)
        }
    };
    /* for (;(sigma != NULL) && (sigma->number <= number); sigma = sigma->next) { */
    while let Some(s) = sigma {
        if s.number == -1 {
            break;
        }
        if number == s.number {
            return s.number;
        }
        sigma = s.next.as_deref();
    }
    -1
}

// [spec:foma:def:sigma.sigma-string-fn]
// [spec:foma:sem:sigma.sigma-string-fn]
// [spec:foma:def:fomalibconf.sigma-string-fn]
// [spec:foma:sem:fomalibconf.sigma-string-fn]
pub fn sigma_string(number: i32, sigma: Option<&Sigma>) -> Option<&str> {
    let mut sigma = match sigma {
        None => return None,
        Some(s) => {
            if s.number == -1 {
                return None;
            }
            Some(s)
        }
    };
    while let Some(s) = sigma {
        if s.number == -1 {
            break;
        }
        if number == s.number {
            /* aliased pointer in C (caller must not free) — borrowed &str here */
            return s.symbol.as_deref();
        }
        sigma = s.next.as_deref();
    }
    None
}

/* Substitutes string symbol for sub in sigma */
/* no check for duplicates                    */
// [spec:foma:def:sigma.sigma-substitute-fn]
// [spec:foma:sem:sigma.sigma-substitute-fn]
// [spec:foma:def:fomalibconf.sigma-substitute-fn]
// [spec:foma:sem:fomalibconf.sigma-substitute-fn]
pub fn sigma_substitute(symbol: &str, sub: &str, sigma: &mut Sigma) -> i32 {
    if sigma.number == -1 {
        return -1;
    }
    let mut sigma: Option<&mut Sigma> = Some(sigma);
    while let Some(s) = sigma {
        if s.number == -1 {
            break;
        }
        if s.symbol.as_deref() == Some(symbol) {
            /* free(sigma->symbol); sigma->symbol = strdup(sub) */
            s.symbol = Some(sub.to_string());
            return s.number;
        }
        sigma = s.next.as_deref_mut();
    }
    -1
}

// [spec:foma:def:sigma.sigma-find-fn]
// [spec:foma:sem:sigma.sigma-find-fn]
// [spec:foma:def:fomalibconf.sigma-find-fn]
// [spec:foma:sem:fomalibconf.sigma-find-fn]
pub fn sigma_find(symbol: &str, sigma: Option<&Sigma>) -> i32 {
    let mut sigma = match sigma {
        None => return -1,
        Some(s) => {
            if s.number == -1 {
                return -1;
            }
            Some(s)
        }
    };
    while let Some(s) = sigma {
        if s.number == -1 {
            break;
        }
        if s.symbol.as_deref() == Some(symbol) {
            return s.number;
        }
        sigma = s.next.as_deref();
    }
    -1
}

// [spec:foma:def:sigma.ssort]
/* C: { char *symbol; int number; } — symbol aliases a sigma node's string;
here the String is moved through the sort scratch array and moved back
(same permutation, observably equivalent) */
#[derive(Debug, Clone)]
pub struct Ssort {
    pub symbol: Option<String>,
    pub number: i32,
}

// [spec:foma:def:sigma.ssortcmp-fn]
// [spec:foma:sem:sigma.ssortcmp-fn]
pub fn ssortcmp(a: &Ssort, b: &Ssort) -> i32 {
    /* return(strcmp(a->symbol, b->symbol)) — bytewise order, sign only */
    match a
        .symbol
        .as_deref()
        .unwrap()
        .as_bytes()
        .cmp(b.symbol.as_deref().unwrap().as_bytes())
    {
        core::cmp::Ordering::Less => -1,
        core::cmp::Ordering::Equal => 0,
        core::cmp::Ordering::Greater => 1,
    }
}

// [spec:foma:def:sigma.sigma-copy-fn]
// [spec:foma:sem:sigma.sigma-copy-fn]
// [spec:foma:def:fomalib.sigma-copy-fn]
// [spec:foma:sem:fomalib.sigma-copy-fn]
pub fn sigma_copy(sigma: Option<&Sigma>) -> Option<Box<Sigma>> {
    let mut f = 0;

    if sigma.is_none() {
        return None;
    }
    /* C mallocs the head node uninitialized; every field is assigned on the
    first loop pass (sigma is non-NULL, so the loop runs at least once) */
    let mut copy_sigma_s = Box::new(Sigma {
        number: 0,
        symbol: None,
        next: None,
    });

    let mut copy_sigma: &mut Sigma = &mut copy_sigma_s;
    let mut sigma = sigma;
    while let Some(s) = sigma {
        if f == 1 {
            copy_sigma.next = Some(Box::new(Sigma {
                number: 0,
                symbol: None,
                next: None,
            }));
            copy_sigma = copy_sigma.next.as_deref_mut().unwrap();
        }
        copy_sigma.number = s.number;
        if s.symbol.is_some() {
            copy_sigma.symbol = s.symbol.clone();
        } else {
            copy_sigma.symbol = None;
        }
        copy_sigma.next = None;
        f = 1;
        sigma = s.next.as_deref();
    }
    Some(copy_sigma_s)
}

/* Assigns a consecutive numbering to symbols in sigma > IDENTITY */
/* and sorts the sigma based on the symbol string contents        */

// [spec:foma:def:sigma.sigma-sort-fn]
// [spec:foma:sem:sigma.sigma-sort-fn+1]
// [spec:foma:def:fomalibconf.sigma-sort-fn]
// [spec:foma:sem:fomalibconf.sigma-sort-fn+1]
pub fn sigma_sort(net: &mut Fsm) -> i32 {
    let size = sigma_max(net.sigma.as_deref());
    if size < 0 {
        return 1;
    }
    /* C mallocs `size` ssort entries and fills the first `max` */
    let mut ssort: Vec<Ssort> = Vec::with_capacity(size as usize);

    let mut i: i32 = 0;
    {
        let mut sigma = net.sigma.as_deref_mut();
        while let Some(s) = sigma {
            if s.number > IDENTITY {
                /* C aliases the symbol pointer; the String is moved out here
                and moved back in the "Replace sigma" pass below */
                ssort.push(Ssort {
                    symbol: s.symbol.take(),
                    number: s.number,
                });
                i += 1;
            }
            sigma = s.next.as_deref_mut();
        }
    }
    let max = i;
    /* qsort(ssort, max, sizeof(struct ssort), comp) with comp = ssortcmp */
    ssort.sort_by(|a, b| ssortcmp(a, b).cmp(&0));
    // Wave 4 fix (sem +1): the C read replacearray slots for numbers absent
    // from sigma while they were still uninitialized (garbage in C, collapsed
    // to 0 by the Wave-2 port), silently corrupting any arc carrying such a
    // label. Seed the table with the identity map so a label with no sigma
    // entry is left unchanged rather than corrupted; present numbers are then
    // overwritten with their sorted position.
    let mut replacearray: Vec<i32> = (0..(size + 3)).collect();
    for i in 0..max {
        replacearray[ssort[i as usize].number as usize] = i + 3;
    }

    /* Replace arcs */
    let fsm_state = &mut net.states;
    let mut i = 0usize;
    while fsm_state[i].state_no != -1 {
        if (fsm_state[i].r#in as i32) > IDENTITY {
            fsm_state[i].r#in = replacearray[fsm_state[i].r#in as usize] as i16;
        }
        if (fsm_state[i].out as i32) > IDENTITY {
            fsm_state[i].out = replacearray[fsm_state[i].out as usize] as i16;
        }
        i += 1;
    }
    /* Replace sigma */
    let mut i: i32 = 0;
    let mut sigma = net.sigma.as_deref_mut();
    while let Some(s) = sigma {
        if s.number > IDENTITY {
            s.number = i + 3;
            s.symbol = ssort[i as usize].symbol.take();
            i += 1;
        }
        sigma = s.next.as_deref_mut();
    }
    1
}

// [spec:foma:def:sigma.sigma-create-fn]
// [spec:foma:sem:sigma.sigma-create-fn]
// [spec:foma:def:fomalibconf.sigma-create-fn]
// [spec:foma:sem:fomalibconf.sigma-create-fn]
pub fn sigma_create() -> Box<Sigma> {
    Box::new(Sigma {
        number: -1, /*Empty sigma*/
        next: None,
        symbol: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structures::fsm_create;
    use crate::types::FsmState;

    /* ---- scaffolding (no facets) ---- */

    fn node(number: i32, symbol: Option<&str>, next: Option<Box<Sigma>>) -> Box<Sigma> {
        Box::new(Sigma {
            number,
            symbol: symbol.map(|s| s.to_string()),
            next,
        })
    }

    /// Walk the whole list (sentinels included) into (number, symbol) pairs.
    fn syms(sigma: Option<&Sigma>) -> Vec<(i32, Option<String>)> {
        let mut out = Vec::new();
        let mut s = sigma;
        while let Some(n) = s {
            out.push((n.number, n.symbol.clone()));
            s = n.next.as_deref();
        }
        out
    }

    fn pair(number: i32, symbol: &str) -> (i32, Option<String>) {
        (number, Some(symbol.to_string()))
    }

    fn line(state_no: i32, r#in: i16, out: i16, target: i32) -> FsmState {
        FsmState {
            state_no,
            r#in,
            out,
            target,
            final_state: 1,
            start_state: 1,
        }
    }

    fn sentinel_line() -> FsmState {
        FsmState {
            state_no: -1,
            r#in: -1,
            out: -1,
            target: -1,
            final_state: -1,
            start_state: -1,
        }
    }

    /* ---- sigma_create ---- */

    // [spec:foma:sem:sigma.sigma-create-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-create-fn/test]
    #[test]
    fn sigma_create_returns_single_empty_sentinel() {
        let s = sigma_create();
        assert_eq!(s.number, -1);
        assert_eq!(s.symbol, None);
        assert!(s.next.is_none());
    }

    /* ---- sigma_add ---- */

    // [spec:foma:sem:sigma.sigma-add-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-add-fn/test]
    #[test]
    fn sigma_add_nonspecial_overwrites_empty_sentinel_with_3() {
        let mut s = sigma_create();
        assert_eq!(sigma_add("a", &mut s), 3);
        assert_eq!(syms(Some(&s)), vec![pair(3, "a")]);
    }

    // [spec:foma:sem:sigma.sigma-add-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-add-fn/test]
    #[test]
    fn sigma_add_nonspecial_appends_tail_number_plus_one() {
        let mut s = sigma_create();
        assert_eq!(sigma_add("a", &mut s), 3);
        assert_eq!(sigma_add("b", &mut s), 4);
        assert_eq!(sigma_add("c", &mut s), 5);
        assert_eq!(
            syms(Some(&s)),
            vec![pair(3, "a"), pair(4, "b"), pair(5, "c")]
        );
    }

    // [spec:foma:sem:sigma.sigma-add-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-add-fn/test]
    #[test]
    fn sigma_add_nonspecial_clamps_number_up_to_3() {
        /* tail is EPSILON (0): 0+1 < 3 forces the new number to 3 */
        let mut s = sigma_create();
        assert_eq!(sigma_add("@_EPSILON_SYMBOL_@", &mut s), EPSILON);
        assert_eq!(sigma_add("x", &mut s), 3);
        assert_eq!(
            syms(Some(&s)),
            vec![pair(0, "@_EPSILON_SYMBOL_@"), pair(3, "x")]
        );
    }

    // [spec:foma:sem:sigma.sigma-add-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-add-fn/test]
    #[test]
    fn sigma_add_nonspecial_numbers_from_tail_not_list_maximum() {
        /* unsorted list: head number 10, tail number 5 → new number is 6, not 11 */
        let mut s = sigma_create();
        sigma_add_number(&mut s, "hi", 10);
        sigma_add_number(&mut s, "lo", 5);
        assert_eq!(sigma_add("x", &mut s), 6);
        assert_eq!(
            syms(Some(&s)),
            vec![pair(10, "hi"), pair(5, "lo"), pair(6, "x")]
        );
    }

    // [spec:foma:sem:sigma.sigma-add-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-add-fn/test]
    #[test]
    fn sigma_add_has_no_duplicate_check() {
        let mut s = sigma_create();
        assert_eq!(sigma_add("a", &mut s), 3);
        assert_eq!(sigma_add("a", &mut s), 4);
        assert_eq!(syms(Some(&s)), vec![pair(3, "a"), pair(4, "a")]);
    }

    // [spec:foma:sem:sigma.sigma-add-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-add-fn/test]
    #[test]
    fn sigma_add_special_names_map_to_reserved_codes_on_empty() {
        let mut s = sigma_create();
        assert_eq!(sigma_add("@_UNKNOWN_SYMBOL_@", &mut s), UNKNOWN);
        assert_eq!(syms(Some(&s)), vec![pair(1, "@_UNKNOWN_SYMBOL_@")]);

        let mut s = sigma_create();
        assert_eq!(sigma_add("@_IDENTITY_SYMBOL_@", &mut s), IDENTITY);
        assert_eq!(syms(Some(&s)), vec![pair(2, "@_IDENTITY_SYMBOL_@")]);
    }

    // [spec:foma:sem:sigma.sigma-add-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-add-fn/test]
    #[test]
    fn sigma_add_special_name_head_insert_overwrites_head_in_place() {
        let mut s = sigma_create();
        sigma_add("a", &mut s);
        assert_eq!(sigma_add("@_EPSILON_SYMBOL_@", &mut s), EPSILON);
        assert_eq!(
            syms(Some(&s)),
            vec![pair(0, "@_EPSILON_SYMBOL_@"), pair(3, "a")]
        );
    }

    // [spec:foma:sem:sigma.sigma-add-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-add-fn/test]
    #[test]
    fn sigma_add_special_splices_sorted_between_nodes() {
        let mut s = sigma_create();
        sigma_add("@_EPSILON_SYMBOL_@", &mut s);
        sigma_add("a", &mut s);
        assert_eq!(sigma_add("@_IDENTITY_SYMBOL_@", &mut s), IDENTITY);
        assert_eq!(
            syms(Some(&s)),
            vec![
                pair(0, "@_EPSILON_SYMBOL_@"),
                pair(2, "@_IDENTITY_SYMBOL_@"),
                pair(3, "a"),
            ]
        );
    }

    // [spec:foma:sem:sigma.sigma-add-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-add-fn/test]
    #[test]
    fn sigma_add_special_duplicate_inserted_before_equal_numbered_node() {
        let mut s = sigma_create();
        sigma_add("@_EPSILON_SYMBOL_@", &mut s);
        assert_eq!(sigma_add("@_EPSILON_SYMBOL_@", &mut s), EPSILON);
        assert_eq!(
            syms(Some(&s)),
            vec![pair(0, "@_EPSILON_SYMBOL_@"), pair(0, "@_EPSILON_SYMBOL_@")]
        );
    }

    /* ---- sigma_add_number ---- */

    // [spec:foma:sem:sigma.sigma-add-number-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-add-number-fn/test]
    #[test]
    fn sigma_add_number_fills_empty_sentinel_in_place() {
        let mut s = sigma_create();
        assert_eq!(sigma_add_number(&mut s, "a", 7), 1);
        assert_eq!(syms(Some(&s)), vec![pair(7, "a")]);
    }

    // [spec:foma:sem:sigma.sigma-add-number-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-add-number-fn/test]
    #[test]
    fn sigma_add_number_appends_unsorted_no_dedup() {
        let mut s = sigma_create();
        assert_eq!(sigma_add_number(&mut s, "a", 9), 1);
        assert_eq!(sigma_add_number(&mut s, "b", 4), 1);
        assert_eq!(sigma_add_number(&mut s, "a", 9), 1);
        assert_eq!(
            syms(Some(&s)),
            vec![pair(9, "a"), pair(4, "b"), pair(9, "a")]
        );
    }

    /* ---- sigma_add_special ---- */

    // [spec:foma:sem:sigma.sigma-add-special-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-add-special-fn/test]
    #[test]
    fn sigma_add_special_maps_codes_to_canonical_strings_on_empty() {
        let mut s = sigma_create();
        assert_eq!(sigma_add_special(EPSILON, &mut s), 0);
        assert_eq!(syms(Some(&s)), vec![pair(0, "@_EPSILON_SYMBOL_@")]);

        let mut s = sigma_create();
        assert_eq!(sigma_add_special(UNKNOWN, &mut s), 1);
        assert_eq!(syms(Some(&s)), vec![pair(1, "@_UNKNOWN_SYMBOL_@")]);

        let mut s = sigma_create();
        assert_eq!(sigma_add_special(IDENTITY, &mut s), 2);
        assert_eq!(syms(Some(&s)), vec![pair(2, "@_IDENTITY_SYMBOL_@")]);
    }

    // [spec:foma:sem:sigma.sigma-add-special-fn+1/test]
    // [spec:foma:sem:fomalibconf.sigma-add-special-fn+1/test]
    #[test]
    fn sigma_add_special_nonreserved_code_inserts_placeholder() {
        /* a non-reserved code gets a well-formed placeholder, not the symbol-less
        (NULL) node C created (which later crashed when the sigma was read) */
        let mut s = sigma_create();
        sigma_add("a", &mut s); /* number 3 */
        assert_eq!(sigma_add_special(5, &mut s), 5);
        assert_eq!(syms(Some(&s)), vec![pair(3, "a"), pair(5, "@_SPECIAL_5_@")]);

        /* empty-sentinel head fill also gets a placeholder */
        let mut s = sigma_create();
        assert_eq!(sigma_add_special(9, &mut s), 9);
        assert_eq!(syms(Some(&s)), vec![pair(9, "@_SPECIAL_9_@")]);
    }

    // [spec:foma:sem:sigma.sigma-add-special-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-add-special-fn/test]
    #[test]
    fn sigma_add_special_head_insert_overwrites_head_in_place() {
        let mut s = sigma_create();
        sigma_add_special(IDENTITY, &mut s);
        assert_eq!(sigma_add_special(EPSILON, &mut s), 0);
        assert_eq!(
            syms(Some(&s)),
            vec![
                pair(0, "@_EPSILON_SYMBOL_@"),
                pair(2, "@_IDENTITY_SYMBOL_@")
            ]
        );
    }

    // [spec:foma:sem:sigma.sigma-add-special-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-add-special-fn/test]
    #[test]
    fn sigma_add_special_splices_pre_sorted_between_nodes() {
        let mut s = sigma_create();
        sigma_add_special(EPSILON, &mut s);
        sigma_add_special(IDENTITY, &mut s);
        assert_eq!(sigma_add_special(UNKNOWN, &mut s), 1);
        assert_eq!(
            syms(Some(&s)),
            vec![
                pair(0, "@_EPSILON_SYMBOL_@"),
                pair(1, "@_UNKNOWN_SYMBOL_@"),
                pair(2, "@_IDENTITY_SYMBOL_@"),
            ]
        );
    }

    // [spec:foma:sem:sigma.sigma-add-special-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-add-special-fn/test]
    #[test]
    fn sigma_add_special_duplicate_code_inserted_before_existing() {
        let mut s = sigma_create();
        sigma_add_special(EPSILON, &mut s);
        sigma_add_special(IDENTITY, &mut s);
        assert_eq!(sigma_add_special(IDENTITY, &mut s), 2);
        assert_eq!(
            syms(Some(&s)),
            vec![
                pair(0, "@_EPSILON_SYMBOL_@"),
                pair(2, "@_IDENTITY_SYMBOL_@"),
                pair(2, "@_IDENTITY_SYMBOL_@"),
            ]
        );
    }

    /* ---- sigma_find ---- */

    // [spec:foma:sem:sigma.sigma-find-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-find-fn/test]
    #[test]
    fn sigma_find_returns_number_of_first_match_or_minus_one() {
        let mut s = sigma_create();
        sigma_add("a", &mut s);
        sigma_add("b", &mut s);
        sigma_add("a", &mut s); /* duplicate: first wins */
        assert_eq!(sigma_find("a", Some(&s)), 3);
        assert_eq!(sigma_find("b", Some(&s)), 4);
        assert_eq!(sigma_find("z", Some(&s)), -1);
    }

    // [spec:foma:sem:sigma.sigma-find-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-find-fn/test]
    #[test]
    fn sigma_find_null_empty_and_sentinel_stop() {
        assert_eq!(sigma_find("a", None), -1);
        let s = sigma_create();
        assert_eq!(sigma_find("a", Some(&s)), -1);
        /* scanning stops at an interior number==-1 node */
        let s = node(
            3,
            Some("a"),
            Some(node(-1, None, Some(node(4, Some("b"), None)))),
        );
        assert_eq!(sigma_find("b", Some(&s)), -1);
    }

    /* ---- sigma_find_number ---- */

    // [spec:foma:sem:sigma.sigma-find-number-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-find-number-fn/test]
    #[test]
    fn sigma_find_number_returns_number_if_present_else_minus_one() {
        let mut s = sigma_create();
        sigma_add("@_EPSILON_SYMBOL_@", &mut s);
        sigma_add("a", &mut s);
        sigma_add("b", &mut s);
        assert_eq!(sigma_find_number(0, Some(&s)), 0);
        assert_eq!(sigma_find_number(4, Some(&s)), 4);
        assert_eq!(sigma_find_number(1, Some(&s)), -1);
        assert_eq!(sigma_find_number(9, Some(&s)), -1);
    }

    // [spec:foma:sem:sigma.sigma-find-number-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-find-number-fn/test]
    #[test]
    fn sigma_find_number_null_and_empty_sentinel_give_minus_one() {
        assert_eq!(sigma_find_number(3, None), -1);
        let s = sigma_create();
        assert_eq!(sigma_find_number(3, Some(&s)), -1);
        /* the sentinel's own -1 is never findable */
        assert_eq!(sigma_find_number(-1, Some(&s)), -1);
    }

    /* ---- sigma_string ---- */

    // [spec:foma:sem:sigma.sigma-string-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-string-fn/test]
    #[test]
    fn sigma_string_returns_symbol_of_first_matching_number() {
        let mut s = sigma_create();
        sigma_add("@_EPSILON_SYMBOL_@", &mut s);
        sigma_add("a", &mut s);
        assert_eq!(sigma_string(0, Some(&s)), Some("@_EPSILON_SYMBOL_@"));
        assert_eq!(sigma_string(3, Some(&s)), Some("a"));
        assert_eq!(sigma_string(9, Some(&s)), None);
    }

    // [spec:foma:sem:sigma.sigma-string-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-string-fn/test]
    #[test]
    fn sigma_string_null_and_empty_give_none() {
        assert_eq!(sigma_string(0, None), None);
        let s = sigma_create();
        assert_eq!(sigma_string(0, Some(&s)), None);
    }

    /* ---- sigma_substitute ---- */

    // [spec:foma:sem:sigma.sigma-substitute-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-substitute-fn/test]
    #[test]
    fn sigma_substitute_renames_first_match_and_returns_number() {
        let mut s = sigma_create();
        sigma_add("a", &mut s);
        sigma_add("a", &mut s);
        assert_eq!(sigma_substitute("a", "z", &mut s), 3);
        /* only the first duplicate is renamed */
        assert_eq!(syms(Some(&s)), vec![pair(3, "z"), pair(4, "a")]);
    }

    // [spec:foma:sem:sigma.sigma-substitute-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-substitute-fn/test]
    #[test]
    fn sigma_substitute_no_duplicate_check_can_create_same_string_twice() {
        let mut s = sigma_create();
        sigma_add("a", &mut s);
        sigma_add("b", &mut s);
        assert_eq!(sigma_substitute("a", "b", &mut s), 3);
        assert_eq!(syms(Some(&s)), vec![pair(3, "b"), pair(4, "b")]);
    }

    // [spec:foma:sem:sigma.sigma-substitute-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-substitute-fn/test]
    #[test]
    fn sigma_substitute_empty_sentinel_or_miss_returns_minus_one() {
        let mut s = sigma_create();
        assert_eq!(sigma_substitute("a", "b", &mut s), -1);
        sigma_add("a", &mut s);
        assert_eq!(sigma_substitute("z", "b", &mut s), -1);
        assert_eq!(syms(Some(&s)), vec![pair(3, "a")]);
    }

    /* ---- sigma_remove ---- */

    // [spec:foma:sem:sigma.sigma-remove-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-remove-fn/test]
    #[test]
    fn sigma_remove_head_returns_next_as_new_head() {
        let mut s = sigma_create();
        sigma_add("a", &mut s);
        sigma_add("b", &mut s);
        let s = sigma_remove("a", Some(s));
        assert_eq!(syms(s.as_deref()), vec![pair(4, "b")]);
    }

    // [spec:foma:sem:sigma.sigma-remove-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-remove-fn/test]
    #[test]
    fn sigma_remove_interior_unlinks_only_first_match() {
        let mut s = sigma_create();
        sigma_add("a", &mut s);
        sigma_add("b", &mut s);
        sigma_add("b", &mut s);
        sigma_add("c", &mut s);
        let s = sigma_remove("b", Some(s));
        assert_eq!(
            syms(s.as_deref()),
            vec![pair(3, "a"), pair(5, "b"), pair(6, "c")]
        );
    }

    // [spec:foma:sem:sigma.sigma-remove-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-remove-fn/test]
    #[test]
    fn sigma_remove_miss_null_and_empty_sentinel_unchanged() {
        assert!(sigma_remove("a", None).is_none());
        let s = sigma_remove("a", Some(sigma_create()));
        assert_eq!(syms(s.as_deref()), vec![(-1, None)]);
        let mut s = sigma_create();
        sigma_add("a", &mut s);
        let s = sigma_remove("z", Some(s));
        assert_eq!(syms(s.as_deref()), vec![pair(3, "a")]);
    }

    /* ---- sigma_remove_num ---- */

    // [spec:foma:sem:sigma.sigma-remove-num-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-remove-num-fn/test]
    #[test]
    fn sigma_remove_num_head_and_interior() {
        let mut s = sigma_create();
        sigma_add("a", &mut s);
        sigma_add("b", &mut s);
        sigma_add("c", &mut s);
        let s = sigma_remove_num(3, Some(s));
        assert_eq!(syms(s.as_deref()), vec![pair(4, "b"), pair(5, "c")]);
        let s = sigma_remove_num(5, s);
        assert_eq!(syms(s.as_deref()), vec![pair(4, "b")]);
    }

    // [spec:foma:sem:sigma.sigma-remove-num-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-remove-num-fn/test]
    #[test]
    fn sigma_remove_num_miss_null_and_sentinel_never_removed() {
        assert!(sigma_remove_num(3, None).is_none());
        /* the empty sentinel is not removable, not even by its own -1 */
        let s = sigma_remove_num(-1, Some(sigma_create()));
        assert_eq!(syms(s.as_deref()), vec![(-1, None)]);
        let mut s = sigma_create();
        sigma_add("a", &mut s);
        let s = sigma_remove_num(9, Some(s));
        assert_eq!(syms(s.as_deref()), vec![pair(3, "a")]);
    }

    /* ---- sigma_size ---- */

    // [spec:foma:sem:sigma.sigma-size-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-size-fn/test]
    #[test]
    fn sigma_size_counts_raw_nodes_including_sentinel() {
        assert_eq!(sigma_size(None), 0);
        /* fresh sigma_create: the sentinel itself counts → 1 */
        let s = sigma_create();
        assert_eq!(sigma_size(Some(&s)), 1);
        let mut s = sigma_create();
        sigma_add("a", &mut s);
        sigma_add("a", &mut s); /* duplicates both count */
        assert_eq!(sigma_size(Some(&s)), 2);
    }

    /* ---- sigma_max ---- */

    // [spec:foma:sem:sigma.sigma-max-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-max-fn/test]
    #[test]
    fn sigma_max_null_and_empty_give_minus_one() {
        assert_eq!(sigma_max(None), -1);
        let s = sigma_create();
        assert_eq!(sigma_max(Some(&s)), -1);
    }

    // [spec:foma:sem:sigma.sigma-max-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-max-fn/test]
    #[test]
    fn sigma_max_visits_every_node_including_sentinels() {
        let mut s = sigma_create();
        sigma_add_number(&mut s, "a", 3);
        sigma_add_number(&mut s, "b", 7);
        sigma_add_number(&mut s, "c", 5);
        assert_eq!(sigma_max(Some(&s)), 7);
        /* no stop at a -1 sentinel: nodes after it are still visited */
        let s = node(-1, None, Some(node(7, Some("x"), None)));
        assert_eq!(sigma_max(Some(&s)), 7);
    }

    /* ---- sigma_to_list ---- */

    // [spec:foma:sem:sigma.sigma-to-list-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-to-list-fn/test]
    #[test]
    fn sigma_to_list_empty_sigma_gives_zero_length_table() {
        let s = sigma_create();
        assert_eq!(sigma_to_list(Some(&s)).len(), 0);
    }

    // [spec:foma:sem:sigma.sigma-to-list-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-to-list-fn/test]
    #[test]
    fn sigma_to_list_indexes_symbols_by_number_with_none_gaps() {
        let mut s = sigma_create();
        sigma_add_number(&mut s, "@_EPSILON_SYMBOL_@", 0);
        sigma_add_number(&mut s, "c", 5);
        sigma_add_number(&mut s, "a", 3);
        let sl = sigma_to_list(Some(&s));
        assert_eq!(sl.len(), 6); /* sigma_max + 1 */
        assert_eq!(sl[0].symbol.as_deref(), Some("@_EPSILON_SYMBOL_@"));
        assert_eq!(sl[1].symbol, None);
        assert_eq!(sl[2].symbol, None);
        assert_eq!(sl[3].symbol.as_deref(), Some("a"));
        assert_eq!(sl[4].symbol, None);
        assert_eq!(sl[5].symbol.as_deref(), Some("c"));
    }

    // [spec:foma:sem:sigma.sigma-to-list-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-to-list-fn/test]
    #[test]
    fn sigma_to_list_stops_filling_at_interior_sentinel() {
        /* the fill loop breaks at number==-1; sigma_max still sizes the table */
        let s = node(
            3,
            Some("a"),
            Some(node(-1, None, Some(node(4, Some("b"), None)))),
        );
        let sl = sigma_to_list(Some(&s));
        assert_eq!(sl.len(), 5);
        assert_eq!(sl[3].symbol.as_deref(), Some("a"));
        assert_eq!(sl[4].symbol, None);
    }

    /* ---- sigma_copy ---- */

    // [spec:foma:sem:sigma.sigma-copy-fn/test]
    // [spec:foma:sem:fomalib.sigma-copy-fn/test]
    #[test]
    fn sigma_copy_null_gives_null_and_sentinel_is_copied() {
        assert!(sigma_copy(None).is_none());
        let src = sigma_create();
        let copy = sigma_copy(Some(&src));
        assert_eq!(syms(copy.as_deref()), vec![(-1, None)]);
    }

    // [spec:foma:sem:sigma.sigma-copy-fn/test]
    // [spec:foma:sem:fomalib.sigma-copy-fn/test]
    #[test]
    fn sigma_copy_deep_copies_order_numbers_symbols_source_untouched() {
        let mut src = sigma_create();
        sigma_add("@_EPSILON_SYMBOL_@", &mut src);
        sigma_add("a", &mut src);
        sigma_add("b", &mut src);
        let mut copy = sigma_copy(Some(&src)).unwrap();
        assert_eq!(syms(Some(&copy)), syms(Some(&src)));
        /* deep: mutating the copy leaves the source intact */
        copy.next.as_deref_mut().unwrap().symbol = Some("zzz".to_string());
        assert_eq!(
            syms(Some(&src)),
            vec![pair(0, "@_EPSILON_SYMBOL_@"), pair(3, "a"), pair(4, "b")]
        );
    }

    // [spec:foma:sem:sigma.sigma-copy-fn/test]
    // [spec:foma:sem:fomalib.sigma-copy-fn/test]
    #[test]
    fn sigma_copy_preserves_null_symbols_and_trailing_sentinel() {
        let src = node(
            3,
            Some("a"),
            Some(node(5, None, Some(node(-1, None, None)))),
        );
        let copy = sigma_copy(Some(&src));
        assert_eq!(
            syms(copy.as_deref()),
            vec![pair(3, "a"), (5, None), (-1, None)]
        );
    }

    /* ---- ssortcmp ---- */

    // [spec:foma:sem:sigma.ssortcmp-fn/test]
    #[test]
    fn ssortcmp_orders_bytewise_on_symbol_ignoring_number() {
        let a = Ssort {
            symbol: Some("a".to_string()),
            number: 99,
        };
        let b = Ssort {
            symbol: Some("b".to_string()),
            number: 1,
        };
        assert_eq!(ssortcmp(&a, &b), -1);
        assert_eq!(ssortcmp(&b, &a), 1);
        /* equal strings → 0 even with different numbers */
        let a2 = Ssort {
            symbol: Some("a".to_string()),
            number: 5,
        };
        assert_eq!(ssortcmp(&a, &a2), 0);
        /* byte order, not case-insensitive: 'B' (0x42) < 'a' (0x61) */
        let upper = Ssort {
            symbol: Some("B".to_string()),
            number: 0,
        };
        assert_eq!(ssortcmp(&upper, &a), -1);
    }

    /* ---- sigma_sort ---- */

    // [spec:foma:sem:sigma.sigma-sort-fn+1/test]
    // [spec:foma:sem:fomalibconf.sigma-sort-fn+1/test]
    #[test]
    fn sigma_sort_empty_sigma_returns_1_without_touching_states() {
        let mut net = fsm_create("t");
        assert_eq!(sigma_sort(&mut net), 1);
        assert!(net.states.is_empty());
    }

    // [spec:foma:sem:sigma.sigma-sort-fn+1/test]
    // [spec:foma:sem:fomalibconf.sigma-sort-fn+1/test]
    #[test]
    fn sigma_sort_permutes_symbols_renumbers_from_3_and_rewrites_arcs() {
        let mut net = fsm_create("t");
        {
            let sig = net.sigma.as_deref_mut().unwrap();
            sigma_add_number(sig, "@_EPSILON_SYMBOL_@", 0);
            sigma_add_number(sig, "c", 3);
            sigma_add_number(sig, "a", 4);
            sigma_add_number(sig, "b", 5);
        }
        net.states = vec![line(0, 3, 4, 0), line(0, 5, 0, 0), sentinel_line()];
        assert_eq!(sigma_sort(&mut net), 1);
        /* specials keep number/position; k-th non-special node gets the
        k-th smallest symbol and number k+3 */
        assert_eq!(
            syms(net.sigma.as_deref()),
            vec![
                pair(0, "@_EPSILON_SYMBOL_@"),
                pair(3, "a"),
                pair(4, "b"),
                pair(5, "c"),
            ]
        );
        /* replacearray: c:3→5, a:4→3, b:5→4; labels <= IDENTITY untouched */
        assert_eq!((net.states[0].r#in, net.states[0].out), (5, 3));
        assert_eq!((net.states[1].r#in, net.states[1].out), (4, 0));
    }

    // [spec:foma:sem:sigma.sigma-sort-fn+1/test]
    // [spec:foma:sem:fomalibconf.sigma-sort-fn+1/test]
    #[test]
    fn sigma_sort_arc_label_absent_from_sigma_kept_unchanged() {
        /* Wave 4 fix: a label with no sigma entry is left unchanged (identity
        map) instead of collapsing to a garbage/zero value. */
        let mut net = fsm_create("t");
        {
            let sig = net.sigma.as_deref_mut().unwrap();
            sigma_add_number(sig, "b", 3);
            sigma_add_number(sig, "a", 5);
        }
        net.states = vec![line(0, 4, 3, 0), sentinel_line()];
        assert_eq!(sigma_sort(&mut net), 1);
        assert_eq!(syms(net.sigma.as_deref()), vec![pair(3, "a"), pair(4, "b")]);
        /* in=4 is absent from sigma → replacearray[4] == 4 (identity, kept);
        out: b 3→4 */
        assert_eq!((net.states[0].r#in, net.states[0].out), (4, 4));
    }

    /* ---- sigma_cleanup ---- */

    // [spec:foma:sem:sigma.sigma-cleanup-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-cleanup-fn/test]
    #[test]
    fn sigma_cleanup_force_0_noop_when_identity_or_unknown_present() {
        let mut net = fsm_create("t");
        {
            let sig = net.sigma.as_deref_mut().unwrap();
            sigma_add("@_IDENTITY_SYMBOL_@", sig);
            sigma_add("a", sig);
            sigma_add("b", sig); /* unused, but kept: IDENTITY blocks cleanup */
        }
        net.states = vec![line(0, 3, 3, 0), sentinel_line()];
        sigma_cleanup(&mut net, 0);
        assert_eq!(
            syms(net.sigma.as_deref()),
            vec![pair(2, "@_IDENTITY_SYMBOL_@"), pair(3, "a"), pair(4, "b")]
        );

        let mut net = fsm_create("t");
        {
            let sig = net.sigma.as_deref_mut().unwrap();
            sigma_add("@_UNKNOWN_SYMBOL_@", sig);
            sigma_add("a", sig);
        }
        net.states = vec![line(0, 3, 3, 0), sentinel_line()];
        sigma_cleanup(&mut net, 0);
        assert_eq!(
            syms(net.sigma.as_deref()),
            vec![pair(1, "@_UNKNOWN_SYMBOL_@"), pair(3, "a")]
        );
    }

    // [spec:foma:sem:sigma.sigma-cleanup-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-cleanup-fn/test]
    #[test]
    fn sigma_cleanup_force_0_proceeds_without_identity_unknown() {
        let mut net = fsm_create("t");
        {
            let sig = net.sigma.as_deref_mut().unwrap();
            sigma_add("a", sig);
            sigma_add("b", sig);
        }
        net.states = vec![line(0, 3, 3, 0), sentinel_line()];
        sigma_cleanup(&mut net, 0);
        assert_eq!(syms(net.sigma.as_deref()), vec![pair(3, "a")]);
    }

    // [spec:foma:sem:sigma.sigma-cleanup-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-cleanup-fn/test]
    #[test]
    fn sigma_cleanup_force_1_removes_unattested_incl_reserved_and_renumbers() {
        let mut net = fsm_create("t");
        {
            let sig = net.sigma.as_deref_mut().unwrap();
            sigma_add("@_EPSILON_SYMBOL_@", sig); /* 0, unattested head */
            sigma_add("@_UNKNOWN_SYMBOL_@", sig); /* 1, unattested */
            sigma_add("@_IDENTITY_SYMBOL_@", sig); /* 2, unattested */
            sigma_add("a", sig); /* 3, used */
            sigma_add("b", sig); /* 4, unused */
            sigma_add("c", sig); /* 5, used → renumbered 4 */
        }
        net.states = vec![line(0, 3, 5, 0), sentinel_line()];
        sigma_cleanup(&mut net, 1);
        /* unattested reserved 0–2 entries are removed too (head included) */
        assert_eq!(syms(net.sigma.as_deref()), vec![pair(3, "a"), pair(4, "c")]);
        /* arcs rewritten through the renumbering table */
        assert_eq!((net.states[0].r#in, net.states[0].out), (3, 4));
    }

    // [spec:foma:sem:sigma.sigma-cleanup-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-cleanup-fn/test]
    #[test]
    fn sigma_cleanup_keeps_attested_reserved_numbers_unrenumbered() {
        let mut net = fsm_create("t");
        {
            let sig = net.sigma.as_deref_mut().unwrap();
            sigma_add("@_EPSILON_SYMBOL_@", sig); /* 0, used on an arc */
            sigma_add("a", sig); /* 3, used */
        }
        net.states = vec![line(0, 0, 3, 0), sentinel_line()];
        sigma_cleanup(&mut net, 1);
        assert_eq!(
            syms(net.sigma.as_deref()),
            vec![pair(0, "@_EPSILON_SYMBOL_@"), pair(3, "a")]
        );
        assert_eq!((net.states[0].r#in, net.states[0].out), (0, 3));
    }

    // [spec:foma:sem:sigma.sigma-cleanup-fn/test]
    // [spec:foma:sem:fomalibconf.sigma-cleanup-fn/test]
    #[test]
    fn sigma_cleanup_empty_sigma_returns_without_touching_states() {
        let mut net = fsm_create("t");
        /* sigma_max < 0: early return, states never dereferenced */
        sigma_cleanup(&mut net, 1);
        assert_eq!(syms(net.sigma.as_deref()), vec![(-1, None)]);
    }
}

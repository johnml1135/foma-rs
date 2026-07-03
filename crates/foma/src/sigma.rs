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
// [spec:foma:sem:sigma.sigma-add-special-fn]
// [spec:foma:def:fomalibconf.sigma-add-special-fn]
// [spec:foma:sem:fomalibconf.sigma-add-special-fn]
pub fn sigma_add_special(symbol: i32, sigma: &mut Sigma) -> i32 {
    let mut str: Option<String> = None;
    if symbol == EPSILON {
        str = Some("@_EPSILON_SYMBOL_@".to_string());
    }
    if symbol == IDENTITY {
        str = Some("@_IDENTITY_SYMBOL_@".to_string());
    }
    if symbol == UNKNOWN {
        str = Some("@_UNKNOWN_SYMBOL_@".to_string());
    }
    /* any other code leaves str NULL (latent bug in C, kept: node gets NULL symbol) */

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
// [spec:foma:sem:sigma.sigma-sort-fn]
// [spec:foma:def:fomalibconf.sigma-sort-fn]
// [spec:foma:sem:fomalibconf.sigma-sort-fn]
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
    // DEVIATION from C (entries for numbers absent from sigma stay uninitialized
    // and are read if an arc carries such a label; Rust initializes them to 0)
    let mut replacearray: Vec<i32> = vec![0; (size + 3) as usize];
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

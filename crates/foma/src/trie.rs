//! foma/trie.c — literal (bug-for-bug) Wave-2 port.
//!
//! Word-list-to-trie construction. The transition hash table is a Vec of
//! THASH_TABLESIZE in-array bucket heads chained on collision (C: calloc'd
//! array of struct trie_hash), and the state table is a Vec of statesize
//! trie_states grown on demand, exactly as in C.

use crate::dynarray::{
    fsm_construct_add_arc, fsm_construct_done, fsm_construct_init, fsm_construct_set_final,
    fsm_construct_set_initial,
};
use crate::mem::next_power_of_two;
use crate::stringhash::{sh_done, sh_find_add_string, sh_init};
use crate::types::{Fsm, FsmTrieHandle, TrieHash, TrieStates};
use crate::utf8::utf8skip;

/* C: #define THASH_TABLESIZE 1048573 */
pub const THASH_TABLESIZE: u32 = 1048573;
/* C: #define TRIE_STATESIZE 32768 */
pub const TRIE_STATESIZE: u32 = 32768;

// [spec:foma:def:trie.fsm-trie-init-fn]
// [spec:foma:sem:trie.fsm-trie-init-fn]
// [spec:foma:def:fomalib.fsm-trie-init-fn]
// [spec:foma:sem:fomalib.fsm-trie-init-fn]
pub fn fsm_trie_init() -> Box<FsmTrieHandle> {
    let th = Box::new(FsmTrieHandle {
        /* calloc(THASH_TABLESIZE, sizeof(struct trie_hash)) — zeroed heads */
        trie_hash: vec![
            TrieHash {
                insym: None,
                outsym: None,
                sourcestate: 0,
                targetstate: 0,
                next: None,
            };
            THASH_TABLESIZE as usize
        ],
        /* calloc(TRIE_STATESIZE, sizeof(struct trie_states)) — all non-final */
        trie_states: vec![TrieStates { is_final: false }; TRIE_STATESIZE as usize],
        statesize: TRIE_STATESIZE,
        trie_cursor: 0,
        /* calloc(1, ...) zeroes the rest of the handle */
        used_states: 0,
        sh_hash: Some(sh_init()),
    });
    th
}

// [spec:foma:def:trie.fsm-trie-done-fn]
// [spec:foma:sem:trie.fsm-trie-done-fn]
// [spec:foma:def:fomalib.fsm-trie-done-fn]
// [spec:foma:sem:fomalib.fsm-trie-done-fn]
pub fn fsm_trie_done(th: Box<FsmTrieHandle>) -> Box<Fsm> {
    let mut th = th;
    let mut newh = fsm_construct_init("name");
    for i in 0..THASH_TABLESIZE as usize {
        let mut thash: Option<&TrieHash> = Some(&th.trie_hash[i]);
        while let Some(t) = thash {
            if t.insym.is_some() {
                fsm_construct_add_arc(
                    &mut newh,
                    t.sourcestate as i32,
                    t.targetstate as i32,
                    t.insym.as_deref().unwrap(),
                    t.outsym.as_deref().unwrap(),
                );
            } else {
                /* only possible for an unused in-array bucket head */
                break;
            }
            thash = t.next.as_deref();
        }
    }
    for i in 0..=th.used_states {
        if th.trie_states[i as usize].is_final {
            fsm_construct_set_final(&mut newh, i as i32);
        }
    }
    fsm_construct_set_initial(&mut newh, 0);
    let newnet = fsm_construct_done(newh);
    /* Free all mem: chained overflow nodes and the bucket/state arrays are
    dropped with the handle; sh_done consumes the string-intern hash */
    sh_done(th.sh_hash.take().unwrap());
    newnet
}

// [spec:foma:def:trie.fsm-trie-add-word-fn]
// [spec:foma:sem:trie.fsm-trie-add-word-fn]
// [spec:foma:def:fomalib.fsm-trie-add-word-fn]
// [spec:foma:sem:fomalib.fsm-trie-add-word-fn]
pub fn fsm_trie_add_word(th: &mut FsmTrieHandle, word: &str) {
    /* C: wcopy = strdup(word) is a scratch buffer holding one UTF-8
    character at a time; here each character is sliced out of `word`
    directly (observably equivalent). len = strlen(wcopy). */
    let len = word.len() as i32;
    /* C advances the `word` pointer; here a byte offset into it */
    let mut pos = 0usize;
    let mut i: i32 = 0;
    while pos < word.len() && i < len {
        let skip = utf8skip(&word.as_bytes()[pos..]);
        /* strncpy(wcopy, word, utf8skip(word)+1); *(wcopy+utf8skip(word)+1) = '\0';
        for an invalid lead byte utf8skip returns -1, so wcopy becomes ""
        and word does not advance (the i < len bound terminates the loop) */
        let end = (pos + (skip + 1) as usize).min(word.len());
        let wcopy = &word[pos..end];
        fsm_trie_symbol(th, wcopy, wcopy);
        pos += (skip + 1) as usize;
        i += 1;
    }
    fsm_trie_end_word(th);
}

// [spec:foma:def:trie.fsm-trie-end-word-fn]
// [spec:foma:sem:trie.fsm-trie-end-word-fn]
// [spec:foma:def:fomalib.fsm-trie-end-word-fn]
// [spec:foma:sem:fomalib.fsm-trie-end-word-fn]
pub fn fsm_trie_end_word(th: &mut FsmTrieHandle) {
    th.trie_states[th.trie_cursor as usize].is_final = true;
    th.trie_cursor = 0;
}

// [spec:foma:def:trie.fsm-trie-symbol-fn]
// [spec:foma:sem:trie.fsm-trie-symbol-fn]
// [spec:foma:def:fomalib.fsm-trie-symbol-fn]
// [spec:foma:sem:fomalib.fsm-trie-symbol-fn]
pub fn fsm_trie_symbol(th: &mut FsmTrieHandle, insym: &str, outsym: &str) {
    let h = trie_hashf(th.trie_cursor, insym, outsym);
    if th.trie_hash[h as usize].insym.is_some() {
        let mut thash: Option<&TrieHash> = Some(&th.trie_hash[h as usize]);
        while let Some(t) = thash {
            if t.insym.as_deref() == Some(insym)
                && t.outsym.as_deref() == Some(outsym)
                && t.sourcestate == th.trie_cursor
            {
                /* Exists, move cursor */
                th.trie_cursor = t.targetstate;
                return;
            }
            thash = t.next.as_deref();
        }
    }
    /* Doesn't exist */

    /* Insert trans, move counter and cursor */
    th.used_states += 1;
    // DEVIATION from C (insym/outsym are interned aliases into the sh_hash;
    // owned copies here, per the TrieHash type in types.rs)
    let thash = &mut th.trie_hash[h as usize];
    if thash.insym.is_none() {
        thash.insym = Some(sh_find_add_string(
            th.sh_hash.as_deref_mut().unwrap(),
            insym,
            1,
        ));
        thash.outsym = Some(sh_find_add_string(
            th.sh_hash.as_deref_mut().unwrap(),
            outsym,
            1,
        ));
        thash.sourcestate = th.trie_cursor;
        thash.targetstate = th.used_states;
    } else {
        let newthash = Box::new(TrieHash {
            /* calloc'd node spliced in right after the head */
            next: thash.next.take(),
            insym: Some(sh_find_add_string(
                th.sh_hash.as_deref_mut().unwrap(),
                insym,
                1,
            )),
            outsym: Some(sh_find_add_string(
                th.sh_hash.as_deref_mut().unwrap(),
                outsym,
                1,
            )),
            sourcestate: th.trie_cursor,
            targetstate: th.used_states,
        });
        th.trie_hash[h as usize].next = Some(newthash);
    }
    th.trie_cursor = th.used_states;

    /* Realloc */
    if th.used_states >= th.statesize {
        th.statesize = next_power_of_two(th.statesize as i32) as u32;
        /* realloc leaves the grown region uninitialized in C; every new state
        is initialized below before it is ever read */
        th.trie_states
            .resize(th.statesize as usize, TrieStates { is_final: false });
    }
    th.trie_states[th.used_states as usize].is_final = false;
}

// [spec:foma:def:trie.trie-hashf-fn]
// [spec:foma:sem:trie.trie-hashf-fn]
pub fn trie_hashf(source: u32, insym: &str, outsym: &str) -> u32 {
    /* Hash based on insym, outsym, and sourcestate */
    let mut hash: u32 = 0;

    /* bytes go through plain (signed) char in C: sign-extended before the
    unsigned add, hence `as i8 as i32 as u32` */
    for &b in insym.as_bytes() {
        hash = hash.wrapping_mul(101).wrapping_add(b as i8 as i32 as u32);
    }
    for &b in outsym.as_bytes() {
        hash = hash.wrapping_mul(101).wrapping_add(b as i8 as i32 as u32);
    }
    hash = hash.wrapping_mul(101).wrapping_add(source);
    hash % THASH_TABLESIZE
}

//! Literal port of foma/stringhash.c (Wave 2, bug-for-bug).
//!
//! The hash table owns its interned Strings (the C table owns strdup'd
//! copies, freed by sh_done). C returns pointers to the interned copies;
//! safe Rust cannot hand out aliases into the table, so the find/add
//! functions return owned clones of the interned copy — observably the
//! same bytes at every call site (trie.c stores owned copies per the
//! types.rs TrieHash deviation; spelling.c only NULL-checks the result).

use crate::types::{ShHandle, ShHashtable};

/// C: `#define STRING_HASH_SIZE 8191`
const STRING_HASH_SIZE: usize = 8191;

// [spec:foma:def:stringhash.sh-init-fn]
// [spec:foma:sem:stringhash.sh-init-fn]
// [spec:foma:def:fomalib.sh-init-fn]
// [spec:foma:sem:fomalib.sh-init-fn]
pub fn sh_init() -> Box<ShHandle> {
    let sh: Box<ShHandle> = Box::new(ShHandle {
        /* C: calloc(STRING_HASH_SIZE, sizeof(struct sh_hashtable)) —
        zero-initialized bucket heads stored inline in the array */
        hash: vec![
            ShHashtable {
                string: None,
                value: 0,
                next: None,
            };
            STRING_HASH_SIZE
        ],
        /* C: malloc leaves lastvalue uninitialized; 0 here (safe Rust
        cannot reproduce an uninitialized read) */
        lastvalue: 0,
    });
    sh
}

// [spec:foma:def:stringhash.sh-done-fn]
// [spec:foma:sem:stringhash.sh-done-fn]
// [spec:foma:def:fomalib.sh-done-fn]
// [spec:foma:sem:fomalib.sh-done-fn]
// Consumes the handle (C frees every chain node, every interned string,
// the bucket array, and the handle itself — all handled by drop here).
pub fn sh_done(sh: Box<ShHandle>) {
    drop(sh);
}

// [spec:foma:def:stringhash.sh-get-value-fn]
// [spec:foma:sem:stringhash.sh-get-value-fn]
// [spec:foma:def:fomalib.sh-get-value-fn]
// [spec:foma:sem:fomalib.sh-get-value-fn]
pub fn sh_get_value(sh: &ShHandle) -> i32 {
    sh.lastvalue
}

// [spec:foma:def:stringhash.sh-find-string-fn]
// [spec:foma:sem:stringhash.sh-find-string-fn]
// [spec:foma:def:fomalib.sh-find-string-fn]
// [spec:foma:sem:fomalib.sh-find-string-fn]
// C returns the interned pointer owned by the table (NULL on a miss);
// here a clone of the interned copy is returned (see module doc).
pub fn sh_find_string(sh: &mut ShHandle, string: &str) -> Option<String> {
    let mut found: Option<(String, i32)> = None;
    {
        let mut hash: Option<&ShHashtable> = Some(&sh.hash[sh_hashf(string) as usize]);
        while let Some(h) = hash {
            if h.string.is_none() {
                /* An empty head slot means the bucket has never been used */
                return None;
            }
            if h.string.as_deref() == Some(string) {
                /* C: strcmp(hash->string, string) == 0 */
                found = Some((h.string.clone().unwrap(), h.value));
                break;
            }
            hash = h.next.as_deref();
        }
    }
    match found {
        Some((s, value)) => {
            sh.lastvalue = value;
            Some(s)
        }
        None => None, /* chain exhausted: lastvalue left unchanged */
    }
}

// [spec:foma:def:stringhash.sh-find-add-string-fn]
// [spec:foma:sem:stringhash.sh-find-add-string-fn]
// [spec:foma:def:fomalib.sh-find-add-string-fn]
// [spec:foma:sem:fomalib.sh-find-add-string-fn]
// C never returns NULL here; the return is a clone of the interned copy
// (see module doc).
pub fn sh_find_add_string(sh: &mut ShHandle, string: &str, value: i32) -> String {
    let s: Option<String> = sh_find_string(sh, string);
    if s.is_none() {
        sh_add_string(sh, string, value)
    } else {
        s.unwrap()
    }
}

// [spec:foma:def:stringhash.sh-add-string-fn]
// [spec:foma:sem:stringhash.sh-add-string-fn]
// [spec:foma:def:fomalib.sh-add-string-fn]
// [spec:foma:sem:fomalib.sh-add-string-fn]
// Unconditional insert (no duplicate check). The table owns the interned
// copy (C: strdup); the return is a clone of it (see module doc).
// sh->lastvalue is not touched.
pub fn sh_add_string(sh: &mut ShHandle, string: &str, value: i32) -> String {
    let hash: &mut ShHashtable = &mut sh.hash[sh_hashf(string) as usize];
    if hash.string.is_none() {
        hash.string = Some(string.to_string()); /* C: strdup(string) */
        hash.value = value;
        hash.string.clone().unwrap()
    } else {
        let newhash: Box<ShHashtable> = Box::new(ShHashtable {
            string: Some(string.to_string()), /* C: strdup(string) */
            value,
            next: hash.next.take(), /* C: newhash->next = hash->next */
        });
        let ret: String = newhash.string.clone().unwrap();
        hash.next = Some(newhash); /* C: hash->next = newhash */
        ret
    }
}

// [spec:foma:def:stringhash.sh-hashf-fn]
// [spec:foma:sem:stringhash.sh-hashf-fn]
pub fn sh_hashf(string: &str) -> u32 {
    let mut hash: u32;
    hash = 0;

    for &c in string.as_bytes() {
        /* C: hash = hash * 101 + *string++ — *string is a (signed) char,
        so bytes >= 0x80 contribute a sign-extended negative value; all
        arithmetic wraps modulo 2^32 */
        hash = hash.wrapping_mul(101).wrapping_add((c as i8 as i32) as u32);
    }
    hash % (STRING_HASH_SIZE as u32)
}

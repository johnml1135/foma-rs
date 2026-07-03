//! Literal port of foma/utf8.c (Wave 2, bug-for-bug).
//!
//! These functions do byte-level C-string work; they operate on byte
//! buffers (`&[u8]` / `&mut Vec<u8>`) so that the C's byte semantics —
//! including UTF-8-corrupting reversal and CESU-8-like surrogate output —
//! are reproducible. Writing `'\0'` at position i in C corresponds to
//! `truncate(i)` on the buffer; reading the terminating NUL corresponds
//! to reading an implicit 0 at index `len`.

/* Removes trailing character c, as well as spaces and tabs */
// [spec:foma:def:utf8.remove-trailing-fn]
// [spec:foma:sem:utf8.remove-trailing-fn]
// [spec:foma:def:fomalibconf.remove-trailing-fn]
// [spec:foma:sem:fomalibconf.remove-trailing-fn]
// C returns `s` (the same pointer) for chaining; here the buffer is
// mutated in place.
pub fn remove_trailing(s: &mut Vec<u8>, c: u8) {
    let len: i32 = s.len() as i32 - 1;
    let mut i: i32 = len;
    while i >= 0 {
        if s[i as usize] != c && s[i as usize] != b' ' && s[i as usize] != b'\t' {
            break;
        }
        s.truncate(i as usize); /* C: *(s+i) = '\0' */
        i -= 1;
    }
}

/* Remove trailing space and \t */
// [spec:foma:def:utf8.trim-fn]
// [spec:foma:sem:utf8.trim-fn]
// [spec:foma:def:fomalibconf.trim-fn]
// [spec:foma:sem:fomalibconf.trim-fn]
// C returns `string` (NULL for NULL input); here the buffer is mutated
// in place, with the NULL branch kept as Option.
pub fn trim(string: Option<&mut Vec<u8>>) {
    let string = match string {
        None => return, /* C: if (string == NULL) return(string); */
        Some(s) => s,
    };
    let mut i: i32 = string.len() as i32 - 1;
    while i >= 0 {
        if string[i as usize] != b' ' && string[i as usize] != b'\t' {
            break;
        }
        string.truncate(i as usize); /* C: *(string+i) = '\0' */
        i -= 1;
    }
}

/* Reverses string in-place */
// [spec:foma:def:utf8.xstrrev-fn]
// [spec:foma:sem:utf8.xstrrev-fn]
// [spec:foma:def:fomalibconf.xstrrev-fn]
// [spec:foma:sem:fomalibconf.xstrrev-fn]
// Byte-wise reversal: multi-byte UTF-8 characters are corrupted, exactly
// as in C (see the utf8.xstrrev-fn sem rule). C returns `str`.
pub fn xstrrev(str: Option<&mut Vec<u8>>) {
    let str = match str {
        None => return, /* C: if (! str ...) return str; */
        Some(s) => s,
    };
    if str.is_empty() {
        return; /* C: ... || ! *str */
    }
    let mut p1: usize = 0;
    let mut p2: usize = str.len() - 1;
    while p2 > p1 {
        str[p1] ^= str[p2];
        str[p2] ^= str[p1];
        str[p1] ^= str[p2];
        p1 += 1;
        p2 -= 1;
    }
}

// [spec:foma:def:utf8.escape-string-fn]
// [spec:foma:sem:utf8.escape-string-fn]
// [spec:foma:def:fomalibconf.escape-string-fn]
// [spec:foma:sem:fomalibconf.escape-string-fn]
// DEVIATION from C (no unterminated-buffer hazard in Rust): the C callocs
// strlen(string)+j bytes — one byte short of room for the NUL terminator,
// relying on calloc zeroing plus an exact fill. Building the same visible
// bytes into a Vec yields the identical string content.
// DEVIATION from C (C returns the caller's own pointer when there is
// nothing to escape; here an owned copy is returned in that branch).
pub fn escape_string(string: &[u8], chr: u8) -> Vec<u8> {
    let mut i: usize;
    let mut j: usize = 0;
    i = 0;
    while i < string.len() {
        if string[i] == chr {
            j += 1;
        }
        i += 1;
    }
    if j > 0 {
        /* C: newstring = calloc((strlen(string)+j),sizeof(char)); */
        let mut newstring: Vec<u8> = vec![0; string.len() + j];
        i = 0;
        j = 0;
        while i < string.len() {
            if string[i] == chr {
                newstring[j] = b'\\';
                j += 1;
                newstring[j] = chr;
            } else {
                newstring[j] = string[i];
            }
            i += 1;
            j += 1;
        }
        newstring
    } else {
        string.to_vec()
    }
}

/* Substitute first \n for \0 */
// [spec:foma:def:utf8.strip-newline-fn]
// [spec:foma:sem:utf8.strip-newline-fn]
// [spec:foma:def:fomalibconf.strip-newline-fn]
// [spec:foma:sem:fomalibconf.strip-newline-fn]
pub fn strip_newline(s: &mut Vec<u8>) {
    let len: i32 = s.len() as i32;
    /* remove the null terminator */
    let mut i: i32 = 0;
    while i < len {
        if s[i as usize] == b'\n' {
            s.truncate(i as usize); /* C: s[i] = '\0' */
            return;
        }
        i += 1;
    }
}

/* Removes initial and final quote, and decodes the string if it contains special chars */
// [spec:foma:def:utf8.dequote-string-fn]
// [spec:foma:sem:utf8.dequote-string-fn]
// [spec:foma:def:fomalibconf.dequote-string-fn]
// [spec:foma:sem:fomalibconf.dequote-string-fn]
pub fn dequote_string(s: &mut Vec<u8>) {
    let len: i32 = s.len() as i32;
    /* C: *s reads the NUL terminator of an empty string (test fails) */
    if len > 0 && s[0] == 0x22 && s[(len - 1) as usize] == 0x22 {
        let mut i: i32 = 1;
        let mut j: i32 = 0;
        while i < len - 1 {
            s[j as usize] = s[i as usize];
            i += 1;
            j += 1;
        }
        s.truncate(j as usize); /* C: *(s+j) = '\0' */
        decode_quoted(s);
    }
}

/* Decode quoted strings. This includes: */
/* Changing \uXXXX sequences to their unicode equivalents */

// [spec:foma:def:utf8.decode-quoted-fn]
// [spec:foma:sem:utf8.decode-quoted-fn]
// [spec:foma:def:fomalibconf.decode-quoted-fn]
// [spec:foma:sem:fomalibconf.decode-quoted-fn]
// Latent bug reproduced literally (see the utf8.decode-quoted-fn sem rule):
// if utf8skip returns -1 on an invalid lead byte, skip == 0 and neither
// cursor advances — infinite loop on malformed input.
pub fn decode_quoted(s: &mut Vec<u8>) {
    let len: i32 = s.len() as i32;
    let mut i: i32 = 0;
    let mut j: i32 = 0;
    while i < len {
        if s[i as usize] == 0x5c
            && len - i > 5
            && s[(i + 1) as usize] == 0x75
            && ishexstr(&s[(i + 2) as usize..]) != 0
        {
            /* Cannot be None: the codepoint parsed from 4 hex digits is <= 0xFFFF */
            let unistr: Vec<u8> = utf8code16tostr(&s[(i + 2) as usize..]).unwrap();
            /* C: for (unistr=...; *unistr; j++, unistr++) — copy up to the
            NUL. The \u0000 escape yields the single byte 0, which the
            *unistr test rejects immediately, so the escape is deleted
            from the output (per the sem rule). */
            let mut u: usize = 0;
            while u < unistr.len() && unistr[u] != 0 {
                s[j as usize] = unistr[u];
                j += 1;
                u += 1;
            }
            i += 6;
        } else {
            let mut skip: i32 = utf8skip(&s[i as usize..]) + 1;
            while skip > 0 {
                s[j as usize] = s[i as usize];
                i += 1;
                j += 1;
                skip -= 1;
            }
        }
    }
    s.truncate(j as usize); /* C: *(s+j) = *(s+i) copies the terminating NUL */
}

/* Replace equal length substrings in s */
// [spec:foma:def:utf8.streqrep-fn]
// [spec:foma:sem:utf8.streqrep-fn]
// [spec:foma:def:fomalibconf.streqrep-fn]
// [spec:foma:sem:fomalibconf.streqrep-fn]
// C returns `s`; here the buffer is mutated in place.
// Latent bug reproduced literally (see the utf8.streqrep-fn sem rule):
// the search restarts from the beginning after every replacement, so if
// newstring still matches oldstring at the same position (e.g. old == new,
// or oldstring is empty — strstr matches at offset 0) the loop never
// terminates.
// DEVIATION from C (if newstring is shorter than oldstring the C memcpy
// reads past newstring's end; here the slice index panics instead).
pub fn streqrep(s: &mut Vec<u8>, oldstring: &[u8], newstring: &[u8]) {
    let len: usize = oldstring.len();

    loop {
        /* C: ptr = strstr(s, oldstring) */
        let mut ptr: Option<usize> = None;
        let mut start: usize = 0;
        while start + len <= s.len() {
            if &s[start..start + len] == oldstring {
                ptr = Some(start);
                break;
            }
            start += 1;
        }
        match ptr {
            Some(p) => {
                /* C: memcpy(ptr, newstring, len) */
                s[p..p + len].copy_from_slice(&newstring[..len]);
            }
            None => break,
        }
    }
}

// [spec:foma:def:utf8.ishexstr-fn]
// [spec:foma:sem:utf8.ishexstr-fn]
// [spec:foma:def:fomalibconf.ishexstr-fn]
// [spec:foma:sem:fomalibconf.ishexstr-fn]
pub fn ishexstr(str: &[u8]) -> i32 {
    let mut i: i32 = 0;
    while i < 4 {
        /* C compares (signed) char, so bytes >= 0x80 are negative and fail
        every range; index len stands in for the NUL terminator (itself
        failing, so C never reads past an early NUL either). */
        let c: i32 = if (i as usize) < str.len() {
            str[i as usize] as i8 as i32
        } else {
            0
        };
        if (c > 0x2f && c < 0x3a) || (c > 0x40 && c < 0x47) || (c > 0x60 && c < 0x67) {
            i += 1;
            continue;
        }
        return 0;
    }
    1
}

// [spec:foma:def:utf8.utf8strlen-fn]
// [spec:foma:sem:utf8.utf8strlen-fn]
// [spec:foma:def:fomalibconf.utf8strlen-fn]
// [spec:foma:sem:fomalibconf.utf8strlen-fn]
// Latent bug reproduced literally (see the utf8.utf8strlen-fn sem rule):
// on an invalid lead byte utf8skip returns -1, the advance is 0, and the
// loop never terminates.
pub fn utf8strlen(str: &[u8]) -> i32 {
    let len: i32 = str.len() as i32;
    let mut i: i32 = 0;
    let mut j: i32 = 0;
    /* C: *(str+i) != '\0' && i < len — index >= len stands in for the NUL
    (or, past it, for the out-of-bounds read whose value cannot matter
    because the i < len guard fails anyway). */
    while (if (i as usize) < str.len() { str[i as usize] } else { 0 }) != 0 && i < len {
        i = i + utf8skip(&str[i as usize..]) + 1;
        j += 1;
    }
    j
}

/* Checks if the next character in the string is a combining character     */
/* according to Unicode 7.0                                                */
/* i.e. codepoints 0300-036F  Combining Diacritical Marks                  */
/*                 1AB0-1ABE  Combining Diacritical Marks Extended         */
/*                 1DC0-1DFF  Combining Diacritical Marks Supplement       */
/*                 20D0-20F0  Combining Diacritical Marks for Symbols      */
/*                 FE20-FE2D  Combining Half Marks                         */
/* Returns number of bytes of char. representation, or 0 if not combining  */

// [spec:foma:def:utf8.utf8iscombining-fn]
// [spec:foma:sem:utf8.utf8iscombining-fn]
// [spec:foma:def:fomalibconf.utf8iscombining-fn]
// [spec:foma:sem:fomalibconf.utf8iscombining-fn]
pub fn utf8iscombining(s: &[u8]) -> i32 {
    /* Index >= len stands in for the C string's NUL terminator */
    let s0: u8 = if !s.is_empty() { s[0] } else { 0 };
    let s1: u8 = if s.len() > 1 { s[1] } else { 0 };
    if s0 == 0 || s1 == 0 {
        return 0;
    }
    if !(s0 == 0xcc || s0 == 0xcd || s0 == 0xe1 || s0 == 0xe2 || s0 == 0xef) {
        return 0;
    }
    /* 0300-036F */
    if s0 == 0xcc && s1 >= 0x80 && s1 <= 0xbf {
        return 2;
    }
    if s0 == 0xcd && s1 >= 0x80 && s1 <= 0xaf {
        return 2;
    }
    let s2: u8 = if s.len() > 2 { s[2] } else { 0 };
    if s2 == 0 {
        return 0;
    }
    /* 1AB0-1ABE */
    if s0 == 0xe1 && s1 == 0xaa && s2 >= 0xb0 && s2 <= 0xbe {
        return 3;
    }
    /* 1DC0-1DFF */
    if s0 == 0xe1 && s1 == 0xb7 && s2 >= 0x80 && s2 <= 0xbf {
        return 3;
    }
    /* 20D0-20F0 */
    if s0 == 0xe2 && s1 == 0x83 && s2 >= 0x90 && s2 <= 0xb0 {
        return 3;
    }
    /* FE20-FE2D */
    if s0 == 0xef && s1 == 0xb8 && s2 >= 0xa0 && s2 <= 0xad {
        return 3;
    }
    0
}

// [spec:foma:def:utf8.utf8skip-fn]
// [spec:foma:sem:utf8.utf8skip-fn]
// [spec:foma:def:fomalibconf.utf8skip-fn]
// [spec:foma:sem:fomalibconf.utf8skip-fn]
pub fn utf8skip(str: &[u8]) -> i32 {
    let s: u8;

    /* C: s = (unsigned char)(unsigned int) (*str) — an empty slice stands
    in for pointing at the NUL terminator (byte 0, classified as ASCII). */
    s = if !str.is_empty() { str[0] } else { 0 };
    if s < 0x80 {
        return 0;
    }
    if (s & 0xe0) == 0xc0 {
        return 1;
    }
    if (s & 0xf0) == 0xe0 {
        return 2;
    }
    if (s & 0xf8) == 0xf0 {
        return 3;
    }
    -1
}

// [spec:foma:def:utf8.utf8code16tostr-fn]
// [spec:foma:sem:utf8.utf8code16tostr-fn]
// [spec:foma:def:fomalibconf.utf8code16tostr-fn]
// [spec:foma:sem:fomalibconf.utf8code16tostr-fn]
pub fn utf8code16tostr(str: &[u8]) -> Option<Vec<u8>> {
    let codepoint: i32;
    codepoint = (hexstrtoint(str) << 8) + hexstrtoint(&str[2..]);
    int2utf8str(codepoint)
}

// [spec:foma:def:utf8.int2utf8str-fn]
// [spec:foma:sem:utf8.int2utf8str-fn]
// C returns a fresh NUL-terminated buffer, or NULL for codepoints >=
// 0x10000 (leaking the already-malloc'd 5-byte buffer — a leak-only bug,
// nothing to reproduce in Rust). NULL → None; the Vec holds the encoded
// bytes without the trailing NUL (codepoint 0 yields the single byte 0,
// which consumers treating the result as a C string see as empty).
pub fn int2utf8str(codepoint: i32) -> Option<Vec<u8>> {
    let mut value: Vec<u8> = Vec::with_capacity(5); /* C: malloc(5) */

    if codepoint < 0x80 {
        /* Negative codepoints fall in here and produce a garbage
        (truncated) byte, as in C */
        value.push(codepoint as u8);
        Some(value)
    } else if codepoint < 0x800 {
        value.push(0xc0 | ((codepoint >> 6) as u8));
        value.push(0x80 | ((codepoint & 0x3f) as u8));
        Some(value)
    } else if codepoint < 0x10000 {
        value.push(0xe0 | ((codepoint >> 12) as u8));
        value.push(0x80 | (((codepoint >> 6) & 0x3f) as u8));
        value.push(0x80 | ((codepoint & 0x3f) as u8));
        Some(value)
    } else {
        None
    }
}

// [spec:foma:def:utf8.hexstrtoint-fn]
// [spec:foma:sem:utf8.hexstrtoint-fn]
// C: file-static. No validation — callers must pre-check with ishexstr;
// garbage in gives garbage out.
pub(crate) fn hexstrtoint(str: &[u8]) -> i32 {
    let mut hex: i32;

    /* C reads (signed) char values; reproduce with `as i8 as i32` */
    let c0: i32 = str[0] as i8 as i32;
    let c1: i32 = str[1] as i8 as i32;
    if c0 > 0x60 {
        hex = (c0 - 0x57) << 4;
    } else if c0 > 0x40 {
        hex = (c0 - 0x37) << 4;
    } else {
        hex = (c0 - 0x30) << 4;
    }
    if c1 > 0x60 {
        hex += c1 - 0x57;
    } else if c1 > 0x40 {
        hex += c1 - 0x37;
    } else {
        hex += c1 - 0x30;
    }
    hex
}

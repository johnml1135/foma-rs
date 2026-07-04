//! foma/io.c — literal (bug-for-bug) Wave-2 port per
//! docs/port/rust-conventions.md. Sem rules: docs/spec/port/foma/io.md
//! (per-file `io.*` ids) plus the fomalib.h prototype ids for the exported
//! readers/writers.
//!
//! The gzip layer that C reaches through zlib's gzopen/gzread/gzprintf is
//! provided by flate2. zlib's gzopen transparently reads *uncompressed* files
//! too (gzdirect), whereas flate2's GzDecoder errors on non-gzip input, so the
//! readers sniff the 1f 8b magic and fall back to a plain read to reproduce the
//! C behavior. Writers wrap the output file in a GzEncoder.
//!
//! In-memory buffer walking: C's io_buf_handle threads a `char *io_buf` and an
//! interior `char *io_buf_ptr` cursor; per the conventions the cursor becomes a
//! byte index into the buffer Vec. The spaced-text tokenizers likewise take a
//! `(buffer, cursor)` pair instead of the C `char **` (the buffer is implicit
//! in the C pointer).

use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;

use crate::constructions::fsm_count;
use crate::define::add_defined;
use crate::dynarray::{
    fsm_construct_add_arc, fsm_construct_add_symbol, fsm_construct_check_symbol,
    fsm_construct_done, fsm_construct_init, fsm_construct_set_final, fsm_construct_set_initial,
};
use crate::mem::G_ATT_EPSILON;
use crate::sigma::{sigma_add_number, sigma_max, sigma_string, sigma_to_list};
use crate::structures::{fsm_create, fsm_destroy};
use crate::topsort::fsm_topsort;
use crate::trie::{
    fsm_trie_add_word, fsm_trie_done, fsm_trie_end_word, fsm_trie_init, fsm_trie_symbol,
};
use crate::types::{
    DefinedNetworks, FSM_NAME_LEN, Fsm, FsmConstructHandle, FsmReadBinaryHandle, FsmState, IDENTITY,
    UNKNOWN,
};

/* C: #define READ_BUF_SIZE 4096 (the io_gets/io_net_read line buffer size).
Rust uses growable Strings for line buffers, so no fixed-size overrun. */
pub const READ_BUF_SIZE: usize = 4096;

/* ------------------------------------------------------------------ */
/* Types declared inside io.c                                          */
/* ------------------------------------------------------------------ */

// [spec:foma:def:io.binaryline]
// C: struct binaryline { int type; int state; int in; int target; int out;
// int symbol; char *name; char *value; } — declared in io.c but never read by
// any function; kept for id coverage.
#[derive(Debug, Clone)]
pub struct Binaryline {
    pub r#type: i32,
    pub state: i32,
    pub r#in: i32,
    pub target: i32,
    pub out: i32,
    pub symbol: i32,
    pub name: Option<String>,
    pub value: Option<String>,
}

// [spec:foma:def:io.io-buf-handle]
// C: struct io_buf_handle { char *io_buf; char *io_buf_ptr; }.
// io_buf is the whole (decompressed) file image; io_buf_ptr is an interior read
// cursor into it, represented as a byte index per the conventions.
#[derive(Debug)]
pub struct IoBufHandle {
    /// C: char *io_buf — None ↔ NULL (before io_gz_file_to_mem loads it).
    pub io_buf: Option<Vec<u8>>,
    /// C: char *io_buf_ptr — interior cursor into io_buf → byte index.
    pub io_buf_ptr: usize,
}

// [spec:foma:def:io.bom]
// C: typedef struct BOM { char code[4]; int len; char *name; } BOM;
pub(crate) struct Bom {
    code: [u8; 4],
    len: i32,
    name: Option<&'static str>,
}

/* ------------------------------------------------------------------ */
/* C library twins (no spec ids — these are libc, not io.c functions)  */
/* ------------------------------------------------------------------ */

/* C `atoll`: skip leading whitespace, optional sign, base-10 digits. Overflow
is UB in C; reproduced here with wrapping arithmetic. */
fn atoll(s: &str) -> i64 {
    let b = s.as_bytes();
    let mut i = 0usize;
    while i < b.len() && matches!(b[i], b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r') {
        i += 1;
    }
    let mut sign: i64 = 1;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        if b[i] == b'-' {
            sign = -1;
        }
        i += 1;
    }
    let mut n: i64 = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        n = n.wrapping_mul(10).wrapping_add((b[i] - b'0') as i64);
        i += 1;
    }
    sign.wrapping_mul(n)
}

/* C `atoi`: like atoll truncated to int. */
fn atoi(s: &str) -> i32 {
    atoll(s) as i32
}

/* C `strncmp` twin — stops at n bytes OR at a mutual '\0' (this NUL behavior is
what makes check_BOM's false matches happen). Bytes past the end of `b` read as
0, since check_BOM is called on a buffer that is not yet NUL-terminated. */
fn strncmp(a: &[u8], b: &[u8], n: usize) -> i32 {
    for i in 0..n {
        let ca = a.get(i).copied().unwrap_or(0);
        let cb = b.get(i).copied().unwrap_or(0);
        if ca != cb {
            return ca as i32 - cb as i32;
        }
        if ca == 0 {
            return 0;
        }
    }
    0
}

/* strncpy(dst, src, FSM_NAME_LEN): at most 40 bytes are copied, with no NUL
terminator when the source is >= 40 bytes — reproduced as truncation to 40
bytes per the conventions.
DEVIATION from C (a cut inside a UTF-8 codepoint is lossy-decoded; C keeps the
raw byte prefix). */
fn truncate_name(name: &str) -> String {
    if name.as_bytes().len() > FSM_NAME_LEN {
        String::from_utf8_lossy(&name.as_bytes()[..FSM_NAME_LEN]).into_owned()
    } else {
        name.to_string()
    }
}

/* strlen from a byte index into a NUL-terminated buffer image. */
fn cstrlen(buf: &[u8], idx: usize) -> usize {
    let mut n = 0usize;
    while idx + n < buf.len() && buf[idx + n] != b'\0' {
        n += 1;
    }
    n
}

/* Extract the NUL-terminated C string starting at byte index `idx` as an owned
String (owned so the borrow of `buf` ends before the next destructive token
call).
DEVIATION from C (lossy decode of non-UTF-8; C keeps the raw bytes). */
fn cstr_at(buf: &[u8], idx: usize) -> String {
    let len = cstrlen(buf, idx);
    String::from_utf8_lossy(&buf[idx..idx + len]).into_owned()
}

/* ------------------------------------------------------------------ */
/* Functions                                                           */
/* ------------------------------------------------------------------ */

// [spec:foma:def:io.escape-print-fn]
// [spec:foma:sem:io.escape-print-fn]
pub fn escape_print(stream: &mut dyn Write, string: &str) {
    if string.contains('"') {
        /* strchr(string, '"') != NULL: byte-by-byte, emitting \" for each " */
        for &c in string.as_bytes() {
            if c == b'"' {
                let _ = stream.write_all(b"\\\"");
            } else {
                let _ = stream.write_all(&[c]);
            }
        }
    } else {
        /* fprintf(stream, "%s", string) */
        let _ = stream.write_all(string.as_bytes());
    }
}

// [spec:foma:def:io.foma-write-prolog-fn]
// [spec:foma:sem:io.foma-write-prolog-fn]
// [spec:foma:def:fomalib.foma-write-prolog-fn]
// [spec:foma:sem:fomalib.foma-write-prolog-fn]
pub fn foma_write_prolog(net: &mut Fsm, filename: Option<&str>) -> i32 {
    let mut out: Box<dyn Write>;
    match filename {
        None => {
            out = Box::new(std::io::stdout());
        }
        Some(fname) => {
            match File::create(fname) {
                Ok(f) => {
                    out = Box::new(f);
                }
                Err(_) => {
                    print!("Error writing to file '{}'. Using stdout.\n", fname);
                    out = Box::new(std::io::stdout());
                }
            }
            /* printed whenever filename != NULL, even after the stdout fallback */
            print!("Writing prolog to file '{}'.\n", fname);
        }
    }
    fsm_count(net);
    let maxsigma = sigma_max(net.sigma.as_deref());
    /* calloc(maxsigma+1, sizeof(int)) */
    let mut used_symbols: Vec<i32> = vec![0; (maxsigma + 1) as usize];
    /* malloc(sizeof(int)*statecount) — indexed by state_no below.
    DEVIATION from C (state_no >= statecount OOB-writes in C; Rust panics) */
    let mut finals: Vec<i32> = vec![0; net.statecount as usize];
    /* identifier[100]; strcpy(identifier, net->name) — net->name is capped at
    40 bytes here (the raw over-read on a non-NUL-terminated 40-byte name is not
    reproduced) */
    let identifier = net.name.clone();

    /* Print identifier: fprintf(out, "%s%s%s", "network(", identifier, ").\n") */
    let _ = write!(out, "network({}).\n", identifier);

    let mut i = 0usize;
    while net.states[i].state_no != -1 {
        let state_no = net.states[i].state_no;
        let in_ = net.states[i].r#in;
        let out_ = net.states[i].out;
        let final_state = net.states[i].final_state;
        if final_state == 1 {
            finals[state_no as usize] = 1;
        } else {
            finals[state_no as usize] = 0;
        }
        if in_ != -1 {
            used_symbols[in_ as usize] = 1;
        }
        if out_ != -1 {
            used_symbols[out_ as usize] = 1;
        }
        i += 1;
    }

    for k in 3..=maxsigma {
        if used_symbols[k as usize] == 0 {
            /* C derefs sigma_string unconditionally (NULL for a numbering gap) */
            let mut instring = sigma_string(k, net.sigma.as_deref()).unwrap();
            if instring == "0" {
                instring = "%0";
            }
            let _ = write!(out, "symbol({}, \"", identifier);
            escape_print(&mut *out, instring);
            let _ = write!(out, "\").\n");
        }
    }

    let mut i = 0usize;
    while net.states[i].state_no != -1 {
        let state_no = net.states[i].state_no;
        let target = net.states[i].target;
        let in_ = net.states[i].r#in as i32;
        let out_ = net.states[i].out as i32;
        if target == -1 {
            i += 1;
            continue;
        }
        let _ = write!(out, "arc({}, {}, {}, ", identifier, state_no, target);
        let mut instring: &str = if in_ == 0 {
            "0"
        } else if in_ == 1 {
            "?"
        } else if in_ == 2 {
            "?"
        } else {
            sigma_string(in_, net.sigma.as_deref()).unwrap()
        };
        let mut outstring: &str = if out_ == 0 {
            "0"
        } else if out_ == 1 {
            "?"
        } else if out_ == 2 {
            "?"
        } else {
            sigma_string(out_, net.sigma.as_deref()).unwrap()
        };
        if instring == "0" && in_ != 0 {
            instring = "%0";
        }
        if outstring == "0" && out_ != 0 {
            outstring = "%0";
        }
        if instring == "?" && in_ > 2 {
            instring = "%?";
        }
        /* BUG (kept): the out-side "?" escape tests stateptr->in > 2 instead of
        stateptr->out > 2, so a literal "?" out-symbol is only escaped when the
        in-symbol number is > 2 */
        if outstring == "?" && in_ > 2 {
            outstring = "%?";
        }

        if net.arity == 2 && in_ == IDENTITY && out_ == IDENTITY {
            let _ = write!(out, "\"?\").\n");
        } else if net.arity == 2 && in_ == out_ && in_ != UNKNOWN {
            let _ = write!(out, "\"");
            escape_print(&mut *out, instring);
            let _ = write!(out, "\").\n");
        } else if net.arity == 2 {
            let _ = write!(out, "\"");
            escape_print(&mut *out, instring);
            let _ = write!(out, "\":\"");
            escape_print(&mut *out, outstring);
            let _ = write!(out, "\").\n");
        } else if net.arity == 1 {
            let _ = write!(out, "\"");
            escape_print(&mut *out, instring);
            let _ = write!(out, "\").\n");
        }
        i += 1;
    }

    for k in 0..net.statecount {
        if finals[k as usize] != 0 {
            let _ = write!(out, "final({}, {}).\n", identifier, k);
        }
    }
    /* if (filename != NULL) fclose(out); — the File is dropped here either way;
    stdout is not closed. free(finals)/free(used_symbols) — dropped. */
    1
}

// [spec:foma:def:io.read-att-fn]
// [spec:foma:sem:io.read-att-fn]
// [spec:foma:def:fomalib.read-att-fn]
// [spec:foma:sem:fomalib.read-att-fn]
pub fn read_att(filename: &str) -> Option<Box<Fsm>> {
    let infile = match File::open(filename) {
        Ok(f) => f,
        Err(_) => return None,
    };
    let mut reader = BufReader::new(infile);
    let mut h = fsm_construct_init(filename);
    let mut inword = String::new();
    /* fgets loop (the 1023-byte line split is not reproduced; read_line reads a
    whole line). read_line requires valid UTF-8 — a decode error is treated as
    EOF (DEVIATION: C reads raw bytes). */
    loop {
        inword.clear();
        match reader.read_line(&mut inword) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => break,
        }
        /* strip a single trailing '\n' (a '\r' is left in place, as in C) */
        if inword.ends_with('\n') {
            inword.pop();
        }
        /* strtok on "\t": non-empty tokens only, capped at 6 */
        let tokens: Vec<&str> = inword.split('\t').filter(|s| !s.is_empty()).take(6).collect();
        let i = tokens.len();
        if i == 0 {
            continue;
        }
        if i >= 4 {
            let ge = G_ATT_EPSILON.with(|e| e.borrow().clone());
            let t2 = if tokens[2] == ge.as_str() {
                "@_EPSILON_SYMBOL_@"
            } else {
                tokens[2]
            };
            let t3 = if tokens[3] == ge.as_str() {
                "@_EPSILON_SYMBOL_@"
            } else {
                tokens[3]
            };
            fsm_construct_add_arc(&mut h, atoi(tokens[0]), atoi(tokens[1]), t2, t3);
        } else {
            /* i in 1..=3 */
            fsm_construct_set_final(&mut h, atoi(tokens[0]));
        }
    }
    fsm_construct_set_initial(&mut h, 0);
    /* fclose (drop reader) */
    let mut net = fsm_construct_done(h);
    fsm_count(&mut net);
    let net = fsm_topsort(net);
    Some(net)
}

// [spec:foma:def:io.fsm-read-prolog-fn]
// [spec:foma:sem:io.fsm-read-prolog-fn]
// [spec:foma:def:fomalib.fsm-read-prolog-fn]
// [spec:foma:sem:fomalib.fsm-read-prolog-fn]
pub fn fsm_read_prolog(filename: &str) -> Option<Box<Fsm>> {
    /* Many strstr lookups below are unchecked in C (NULL-deref crash on a
    malformed line); reproduced as .unwrap() (panic). The fixed C buffers
    temp[1024]/in[128]/out[128] can overflow on long fields — here the extracted
    Strings grow (DEVIATION, memory-safe). */
    let mut has_net = 0i32;
    let prolog_file = match File::open(filename) {
        Ok(f) => f,
        Err(_) => return None,
    };
    let mut reader = BufReader::new(prolog_file);
    let mut outh: Option<Box<FsmConstructHandle>> = None;
    let mut buf = String::new();
    loop {
        buf.clear();
        match reader.read_line(&mut buf) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => break,
        }
        if buf.starts_with("network(") {
            /* Extract network name */
            if has_net == 1 {
                /* C: perror(...) — appends the errno string, not reproduced */
                eprint!(
                    "WARNING: prolog file contains multiple nets. Only returning the first one.\n"
                );
                break;
            } else {
                has_net = 1;
            }
            let temp_ptr = buf.find("network(").unwrap() + 8;
            let temp_ptr2 = buf.find(").").unwrap();
            let temp = &buf[temp_ptr..temp_ptr2];
            outh = Some(fsm_construct_init(temp));
        }
        if buf.starts_with("final(") {
            let temp_ptr = buf.find(' ').unwrap() + 1;
            let temp_ptr2 = buf[temp_ptr..].find(").").unwrap() + temp_ptr;
            let temp = &buf[temp_ptr..temp_ptr2];
            fsm_construct_set_final(outh.as_deref_mut().unwrap(), atoi(temp));
        }
        if buf.starts_with("symbol(") {
            let temp_ptr = buf.find(", \"").unwrap() + 3;
            let temp_ptr2 = buf[temp_ptr..].find("\").").unwrap() + temp_ptr;
            let mut temp = buf[temp_ptr..temp_ptr2].to_string();
            if temp == "%0" {
                temp = "0".to_string();
            }
            let oh = outh.as_deref_mut().unwrap();
            if fsm_construct_check_symbol(oh, &temp) == -1 {
                fsm_construct_add_symbol(oh, &temp);
            }
            continue;
        }
        if buf.starts_with("arc(") {
            let mut in_ = String::new();
            let mut out_ = String::new();

            let arity = if buf.find("\":\"").is_none() || buf.find(", \":\").").is_some() {
                1
            } else {
                2
            };

            /* Get source */
            let mut temp_ptr = buf.find(' ').unwrap() + 1;
            let mut temp_ptr2 = buf[temp_ptr..].find(',').unwrap() + temp_ptr;
            let source = atoi(&buf[temp_ptr..temp_ptr2]);

            /* Get target */
            temp_ptr = buf[temp_ptr2..].find(' ').unwrap() + temp_ptr2 + 1;
            temp_ptr2 = buf[temp_ptr..].find(',').unwrap() + temp_ptr;
            let target = atoi(&buf[temp_ptr..temp_ptr2]);

            temp_ptr = buf[temp_ptr2..].find('"').unwrap() + temp_ptr2 + 1;
            if arity == 2 {
                temp_ptr2 = buf[temp_ptr..].find("\":").unwrap() + temp_ptr;
            } else {
                temp_ptr2 = buf[temp_ptr..].find("\").").unwrap() + temp_ptr;
            }
            in_ = buf[temp_ptr..temp_ptr2].to_string();

            if arity == 2 {
                temp_ptr = buf[temp_ptr2..].find(":\"").unwrap() + temp_ptr2 + 2;
                temp_ptr2 = buf[temp_ptr..].find("\").").unwrap() + temp_ptr;
                out_ = buf[temp_ptr..temp_ptr2].to_string();
            }
            if arity == 1 && in_ == "?" {
                in_ = "@_IDENTITY_SYMBOL_@".to_string();
            }
            if arity == 2 && in_ == "?" {
                in_ = "@_UNKNOWN_SYMBOL_@".to_string();
            }
            if arity == 2 && out_ == "?" {
                out_ = "@_UNKNOWN_SYMBOL_@".to_string();
            }
            if in_ == "0" {
                in_ = "@_EPSILON_SYMBOL_@".to_string();
            }
            if out_ == "0" {
                out_ = "@_EPSILON_SYMBOL_@".to_string();
            }
            if in_ == "%0" {
                in_ = "0".to_string();
            }
            if out_ == "%0" {
                out_ = "0".to_string();
            }
            if in_ == "%?" {
                in_ = "?".to_string();
            }
            if out_ == "%?" {
                out_ = "?".to_string();
            }

            let oh = outh.as_deref_mut().unwrap();
            if arity == 1 {
                fsm_construct_add_arc(oh, source, target, &in_, &in_);
            } else {
                fsm_construct_add_arc(oh, source, target, &in_, &out_);
            }
        }
    }
    /* fclose (drop reader) */
    if has_net == 1 {
        fsm_construct_set_initial(outh.as_deref_mut().unwrap(), 0);
        let mut outnet = fsm_construct_done(outh.take().unwrap());
        /* C: fsm_topsort(outnet) with the return value ignored (relies on
        in-place update). DEVIATION from C: fsm_topsort consumes/returns the
        Box, so we rebind — observably identical, topsort returns the net. */
        outnet = fsm_topsort(outnet);
        Some(outnet)
    } else {
        None
    }
}

// [spec:foma:def:io.io-init-fn]
// [spec:foma:sem:io.io-init-fn]
pub fn io_init() -> Box<IoBufHandle> {
    Box::new(IoBufHandle {
        io_buf: None,
        io_buf_ptr: 0,
    })
}

// [spec:foma:def:io.io-free-fn]
// [spec:foma:sem:io.io-free-fn]
pub fn io_free(mut iobh: Box<IoBufHandle>) {
    if iobh.io_buf.is_some() {
        /* free(io_buf); io_buf = NULL */
        iobh.io_buf = None;
    }
    /* free(iobh) — dropped */
}

// [spec:foma:def:io.spacedtext-get-next-line-fn]
// [spec:foma:sem:io.spacedtext-get-next-line-fn]
// C threads the buffer through a `char **text` cursor; here the buffer and the
// cursor index are separate arguments and a matched line is a start index.
pub fn spacedtext_get_next_line(text: &mut [u8], cursor: &mut usize) -> Option<usize> {
    let ret = *cursor;
    if text[*cursor] == b'\0' {
        return None;
    }
    let mut t = *cursor;
    while text[t] != b'\0' && text[t] != b'\n' {
        t += 1;
    }
    if text[t] == b'\0' {
        *cursor = t;
    } else {
        *cursor = t + 1;
    }
    text[t] = b'\0';
    Some(ret)
}

// [spec:foma:def:io.spacedtext-get-next-token-fn]
// [spec:foma:sem:io.spacedtext-get-next-token-fn]
pub fn spacedtext_get_next_token(text: &mut [u8], cursor: &mut usize) -> Option<usize> {
    if text[*cursor] == b'\0' || text[*cursor] == b'\n' {
        return None;
    }
    while text[*cursor] == b' ' {
        *cursor += 1;
    }
    let ret = *cursor;
    let mut t = *cursor;
    while text[t] != b'\0' && text[t] != b'\n' && text[t] != b' ' {
        t += 1;
    }
    if text[t] == b'\0' || text[t] == b'\n' {
        *cursor = t;
    } else {
        *cursor = t + 1;
    }
    text[t] = b'\0';
    Some(ret)
}

// [spec:foma:def:io.fsm-read-spaced-text-file-fn]
// [spec:foma:sem:io.fsm-read-spaced-text-file-fn]
// [spec:foma:def:fomalib.fsm-read-spaced-text-file-fn]
// [spec:foma:sem:fomalib.fsm-read-spaced-text-file-fn]
pub fn fsm_read_spaced_text_file(filename: &str) -> Option<Box<Fsm>> {
    let mut text = match file_to_mem(filename) {
        None => return None,
        Some(t) => t,
    };
    let mut th = fsm_trie_init();
    let mut cursor = 0usize;
    loop {
        /* skip consecutive '\n' */
        while text[cursor] != b'\0' && text[cursor] == b'\n' {
            cursor += 1;
        }
        let t1 = match spacedtext_get_next_line(&mut text, &mut cursor) {
            None => break,
            Some(idx) => idx,
        };
        if cstrlen(&text, t1) == 0 {
            continue;
        }
        let t2 = spacedtext_get_next_line(&mut text, &mut cursor);
        let t2_empty = match t2 {
            None => true,
            Some(idx) => cstrlen(&text, idx) == 0,
        };
        if t2_empty {
            let mut l1 = t1;
            loop {
                let insym_i = match spacedtext_get_next_token(&mut text, &mut l1) {
                    None => break,
                    Some(idx) => idx,
                };
                let insym = cstr_at(&text, insym_i);
                if insym == "0" {
                    fsm_trie_symbol(&mut th, "@_EPSILON_SYMBOL_@", "@_EPSILON_SYMBOL_@");
                } else if insym == "%0" {
                    fsm_trie_symbol(&mut th, "0", "0");
                } else {
                    fsm_trie_symbol(&mut th, &insym, &insym);
                }
            }
            fsm_trie_end_word(&mut th);
        } else {
            let t2 = t2.unwrap();
            let mut l1 = t1;
            let mut l2 = t2;
            loop {
                let insym_i = spacedtext_get_next_token(&mut text, &mut l1);
                let outsym_i = spacedtext_get_next_token(&mut text, &mut l2);
                if insym_i.is_none() && outsym_i.is_none() {
                    break;
                }
                let insym: String = match insym_i {
                    None => "@_EPSILON_SYMBOL_@".to_string(),
                    Some(idx) => {
                        let s = cstr_at(&text, idx);
                        if s == "0" {
                            "@_EPSILON_SYMBOL_@".to_string()
                        } else if s == "%0" {
                            "0".to_string()
                        } else {
                            s
                        }
                    }
                };
                let outsym: String = match outsym_i {
                    None => "@_EPSILON_SYMBOL_@".to_string(),
                    Some(idx) => {
                        let s = cstr_at(&text, idx);
                        if s == "0" {
                            "@_EPSILON_SYMBOL_@".to_string()
                        } else if s == "%0" {
                            "0".to_string()
                        } else {
                            s
                        }
                    }
                };
                fsm_trie_symbol(&mut th, &insym, &outsym);
            }
            fsm_trie_end_word(&mut th);
        }
    }
    /* free(textorig) — dropped */
    Some(fsm_trie_done(th))
}

// [spec:foma:def:io.fsm-read-text-file-fn]
// [spec:foma:sem:io.fsm-read-text-file-fn]
// [spec:foma:def:fomalib.fsm-read-text-file-fn]
// [spec:foma:sem:fomalib.fsm-read-text-file-fn]
pub fn fsm_read_text_file(filename: &str) -> Option<Box<Fsm>> {
    let mut text = match file_to_mem(filename) {
        None => return None,
        Some(t) => t,
    };
    let mut textp1 = 0usize;
    let mut th = fsm_trie_init();
    let mut lastword = 0i32;
    while lastword == 0 {
        let mut textp2 = textp1;
        while text[textp2] != b'\n' && text[textp2] != b'\0' {
            textp2 += 1;
        }
        if text[textp2] == b'\0' {
            lastword = 1;
            if textp2 == textp1 {
                break;
            }
        }
        text[textp2] = b'\0';
        if cstrlen(&text, textp1) > 0 {
            let word = cstr_at(&text, textp1);
            fsm_trie_add_word(&mut th, &word);
        }
        textp1 = textp2 + 1;
    }
    /* free(text) — dropped */
    Some(fsm_trie_done(th))
}

// [spec:foma:def:io.fsm-write-binary-file-fn]
// [spec:foma:sem:io.fsm-write-binary-file-fn]
// [spec:foma:def:fomalib.fsm-write-binary-file-fn]
// [spec:foma:sem:fomalib.fsm-write-binary-file-fn]
pub fn fsm_write_binary_file(net: &Fsm, filename: &str) -> i32 {
    /* gzopen(filename, "wb") — a GzEncoder over the output File */
    let file = match File::create(filename) {
        Ok(f) => f,
        Err(_) => return 1,
    };
    let mut outfile = GzEncoder::new(file, Compression::default());
    foma_net_print(net, &mut outfile);
    /* gzclose(outfile) — finish the gzip stream */
    let _ = outfile.finish();
    0
}

// [spec:foma:def:io.fsm-read-binary-file-multiple-fn]
// [spec:foma:sem:io.fsm-read-binary-file-multiple-fn]
// [spec:foma:def:fomalib.fsm-read-binary-file-multiple-fn]
// [spec:foma:sem:fomalib.fsm-read-binary-file-multiple-fn]
// The opaque handle is reused across calls and freed on the NULL return, so the
// caller passes it as `&mut Option<...>`; on the NULL path the handle is dropped
// (io_free) and the caller's Option becomes None ("must not be used again").
pub fn fsm_read_binary_file_multiple(
    fsrh: &mut Option<Box<FsmReadBinaryHandle>>,
) -> Option<Box<Fsm>> {
    /* iobh = (struct io_buf_handle *) fsrh (must be non-NULL) */
    let result = {
        let handle = fsrh.as_mut().unwrap();
        io_net_read(&mut handle.iobh)
    };
    match result {
        None => {
            /* io_free(iobh) — drop the whole handle */
            *fsrh = None;
            None
        }
        Some((net, _net_name)) => {
            /* free(net_name) — dropped */
            Some(net)
        }
    }
}

// [spec:foma:def:io.fsm-read-binary-file-multiple-init-fn]
// [spec:foma:sem:io.fsm-read-binary-file-multiple-init-fn]
pub fn fsm_read_binary_file_multiple_init(filename: &str) -> Option<Box<FsmReadBinaryHandle>> {
    let mut iobh = io_init();
    if io_gz_file_to_mem(&mut iobh, filename) == 0 {
        io_free(iobh);
        return None;
    }
    /* (void *) iobh */
    Some(Box::new(FsmReadBinaryHandle { iobh }))
}

// [spec:foma:def:io.fsm-read-binary-file-fn]
// [spec:foma:sem:io.fsm-read-binary-file-fn]
// [spec:foma:def:fomalib.fsm-read-binary-file-fn]
// [spec:foma:sem:fomalib.fsm-read-binary-file-fn]
pub fn fsm_read_binary_file(filename: &str) -> Option<Box<Fsm>> {
    let mut iobh = io_init();
    if io_gz_file_to_mem(&mut iobh, filename) == 0 {
        io_free(iobh);
        return None;
    }
    /* *net_name is strdup'd and never freed in C (leak); here it is dropped */
    let net = io_net_read(&mut iobh).map(|(n, _net_name)| n);
    io_free(iobh);
    net
}

// [spec:foma:def:io.save-defined-fn]
// [spec:foma:sem:io.save-defined-fn]
// [spec:foma:def:fomalib.save-defined-fn]
// [spec:foma:sem:fomalib.save-defined-fn]
pub fn save_defined(def: &mut DefinedNetworks, filename: &str) -> i32 {
    /* C: def == NULL → "No defined networks.\n" (stderr) and return 0. A &mut
    reference is never NULL, so that NULL check stays at the call site. */
    let file = match File::create(filename) {
        Ok(f) => f,
        Err(_) => {
            print!("Error opening file {} for writing.\n", filename);
            return -1;
        }
    };
    print!("Writing definitions to file {}.\n", filename);
    let mut outfile = GzEncoder::new(file, Compression::default());
    let mut d = Some(&mut *def);
    while let Some(node) = d {
        if node.net.is_none() {
            print!("Skipping definition without network.\n");
            d = node.next.as_deref_mut();
            continue;
        }
        /* strncpy(d->net->name, d->name, FSM_NAME_LEN) */
        let name = node.name.as_deref().unwrap_or("");
        node.net.as_mut().unwrap().name = truncate_name(name);
        foma_net_print(node.net.as_deref().unwrap(), &mut outfile);
        d = node.next.as_deref_mut();
    }
    /* gzclose(outfile) */
    let _ = outfile.finish();
    1
}

// [spec:foma:def:io.load-defined-fn]
// [spec:foma:sem:io.load-defined-fn]
// [spec:foma:def:fomalib.load-defined-fn]
// [spec:foma:sem:fomalib.load-defined-fn]
pub fn load_defined(def: &mut DefinedNetworks, filename: &str) -> i32 {
    let mut iobh = io_init();
    print!("Loading definitions from {}.\n", filename);
    if io_gz_file_to_mem(&mut iobh, filename) == 0 {
        eprint!("File error.\n");
        io_free(iobh);
        return 0;
    }
    loop {
        match io_net_read(&mut iobh) {
            None => break,
            Some((net, net_name)) => {
                /* the stored net name is the definition name; add_defined copies
                it, so the strdup'd net_name is leaked in C (dropped here) */
                add_defined(def, Some(net), &net_name);
            }
        }
    }
    io_free(iobh);
    1
}

// [spec:foma:def:io.explode-line-fn]
// [spec:foma:sem:io.explode-line-fn]
// DEVIATION from C: C writes fields into a fixed int[5] that a >5-field line
// overruns (documented latent bug); here `values` is a growable Vec, so the
// overrun becomes a longer Vec and io_net_read's switch default reports the
// format error instead of corrupting the stack.
pub(crate) fn explode_line(buf: &str, values: &mut Vec<i32>) -> i32 {
    values.clear();
    let bytes = buf.as_bytes();
    let mut j = 0usize;
    let mut items = 0i32;
    loop {
        let i = j;
        while j < bytes.len() && bytes[j] != b' ' {
            j += 1;
        }
        if j >= bytes.len() {
            /* buf[j] == '\0' */
            values.push(atoi(&buf[i..j]));
            items += 1;
            break;
        } else {
            /* buf[j] == ' ' */
            values.push(atoi(&buf[i..j]));
            items += 1;
            j += 1;
        }
    }
    items
}

/* The file format we use is an extremely simple text format */
/* which is gzip compressed through libz and consists of the following sections: */
/* ##foma-net VERSION## / ##props## / PROPERTIES LINE / ##sigma## / ...SIGMA... */
/* / ##states## / ...TRANSITIONS... / ##end## (see foma/io.c for the full note) */

// [spec:foma:def:io.io-net-read-fn]
// [spec:foma:sem:io.io-net-read-fn]
// C signature: struct fsm *io_net_read(io_buf_handle *iobh, char **net_name).
// Here the net and its name are returned together; None ↔ NULL return.
pub fn io_net_read(iobh: &mut IoBufHandle) -> Option<(Box<Fsm>, String)> {
    let mut buf = String::new();
    let net_name: String;
    let mut lineint: Vec<i32> = Vec::new();
    /* char last_final = '1' (49) — only consumed if the first states line has 2
    or 3 fields, which well-formed files never produce */
    let mut last_final: i8 = b'1' as i8;

    if io_gets(iobh, &mut buf) == 0 {
        return None;
    }

    let mut net = fsm_create("");

    if buf != "##foma-net 1.0##" {
        fsm_destroy(net);
        /* C: perror("File format error foma!\n") */
        eprint!("File format error foma!\n");
        return None;
    }
    io_gets(iobh, &mut buf);
    if buf != "##props##" {
        eprint!("File format error props!\n");
        fsm_destroy(net);
        return None;
    }
    /* Properties */
    io_gets(iobh, &mut buf);
    let mut extras: i32 = 0;
    {
        /* sscanf(buf, "%i %i %i %i %i %lld %i %i %i %i %i %i %s", ...) — each %i
        stops assigning on a missing field (a truncated props line leaves the
        remaining net fields at their fsm_create defaults) */
        let toks: Vec<&str> = buf.split_whitespace().collect();
        if !toks.is_empty() {
            net.arity = atoi(toks[0]);
        }
        if toks.len() > 1 {
            net.arccount = atoi(toks[1]);
        }
        if toks.len() > 2 {
            net.statecount = atoi(toks[2]);
        }
        if toks.len() > 3 {
            net.linecount = atoi(toks[3]);
        }
        if toks.len() > 4 {
            net.finalcount = atoi(toks[4]);
        }
        if toks.len() > 5 {
            net.pathcount = atoll(toks[5]);
        }
        if toks.len() > 6 {
            net.is_deterministic = atoi(toks[6]);
        }
        if toks.len() > 7 {
            net.is_pruned = atoi(toks[7]);
        }
        if toks.len() > 8 {
            net.is_minimized = atoi(toks[8]);
        }
        if toks.len() > 9 {
            net.is_epsilon_free = atoi(toks[9]);
        }
        if toks.len() > 10 {
            net.is_loop_free = atoi(toks[10]);
        }
        if toks.len() > 11 {
            extras = atoi(toks[11]);
        }
        /* %s reads the name into buf (C aliases the input); a missing name field
        leaves buf as the whole props line — that line then becomes the net name
        (latent quirk, reproduced) */
        let name = if toks.len() > 12 {
            toks[12].to_string()
        } else {
            buf.clone()
        };
        net.name = truncate_name(&name);
        net_name = name;
    }
    io_gets(iobh, &mut buf);

    net.is_completed = extras & 3;
    net.arcs_sorted_in = (extras & 12) >> 2;
    net.arcs_sorted_out = (extras & 48) >> 4;

    /* Sigma header: skip anything until ##sigma## */
    while buf != "##sigma##" {
        if buf.is_empty() {
            print!("File format error at sigma definition!\n");
            fsm_destroy(net);
            return None;
        }
        io_gets(iobh, &mut buf);
    }

    /* Sigma lines */
    loop {
        io_gets(iobh, &mut buf);
        if buf.as_bytes().first() == Some(&b'#') {
            break;
        }
        if buf.is_empty() {
            /* NOTE (kept): at end-of-buffer io_gets keeps returning empty lines,
            so a file truncated inside the sigma section loops forever, exactly
            as in C (memory-safe, so ported literally) */
            continue;
        }
        /* new_symbol = strstr(buf, " ") — a spaceless line NULL-derefs in C */
        let p = buf.find(' ').unwrap();
        let number_str = &buf[..p];
        let new_symbol = &buf[p + 1..];
        let n = atoi(number_str);
        if new_symbol.is_empty() {
            /* a literal-newline symbol survives the line-oriented format */
            sigma_add_number(net.sigma.as_deref_mut().unwrap(), "\n", n);
        } else {
            sigma_add_number(net.sigma.as_deref_mut().unwrap(), new_symbol, n);
        }
    }

    /* States */
    if buf != "##states##" {
        print!("File format error!\n");
        /* C leaks net here */
        return None;
    }
    /* malloc(linecount * sizeof(struct fsm_state)).
    DEVIATION from C (more lines than linecount OOB-write in C; Rust panics on
    the index; a negative/zero linecount likewise mis-sizes the buffer) */
    net.states = vec![
        FsmState {
            state_no: 0,
            r#in: 0,
            out: 0,
            target: 0,
            final_state: 0,
            start_state: 0,
        };
        net.linecount as usize
    ];
    let mut laststate: i32 = -1;
    let mut i = 0usize;
    loop {
        io_gets(iobh, &mut buf);
        if buf.as_bytes().first() == Some(&b'#') {
            break;
        }
        let items = explode_line(&buf, &mut lineint);
        match items {
            2 => {
                net.states[i].state_no = laststate;
                net.states[i].r#in = lineint[0] as i16;
                net.states[i].out = lineint[0] as i16;
                net.states[i].target = lineint[1];
                net.states[i].final_state = last_final;
            }
            3 => {
                net.states[i].state_no = laststate;
                net.states[i].r#in = lineint[0] as i16;
                net.states[i].out = lineint[1] as i16;
                net.states[i].target = lineint[2];
                net.states[i].final_state = last_final;
            }
            4 => {
                net.states[i].state_no = lineint[0];
                net.states[i].r#in = lineint[1] as i16;
                net.states[i].out = lineint[1] as i16;
                net.states[i].target = lineint[2];
                net.states[i].final_state = lineint[3] as i8;
                laststate = lineint[0];
                last_final = lineint[3] as i8;
            }
            5 => {
                net.states[i].state_no = lineint[0];
                net.states[i].r#in = lineint[1] as i16;
                net.states[i].out = lineint[2] as i16;
                net.states[i].target = lineint[3];
                net.states[i].final_state = lineint[4] as i8;
                laststate = lineint[0];
                last_final = lineint[4] as i8;
            }
            _ => {
                print!("File format error\n");
                /* C leaks net here */
                return None;
            }
        }
        if laststate > 0 {
            net.states[i].start_state = 0;
        } else if laststate == -1 {
            net.states[i].start_state = -1;
        } else {
            net.states[i].start_state = 1;
        }
        i += 1;
    }

    if buf == "##cmatrix##" {
        crate::spelling::cmatrix_init(&mut net);
        let mut cm = 0usize;
        loop {
            io_gets(iobh, buf);
            if buf.starts_with('#') {
                break;
            }
            let val: i32 = buf.trim().parse().unwrap_or(0);
            /* DEVIATION from C (no bounds check on cm; a matrix overrun writes
            OOB — Rust panics on the index instead) */
            net.medlookup.as_mut().unwrap().confusion_matrix[cm] = val;
            cm += 1;
        }
    }
    if buf != "##end##" {
        print!("File format error!\n");
        /* C leaks net here */
        return None;
    }
    Some((net, net_name))
}

// [spec:foma:def:io.io-gets-fn]
// [spec:foma:sem:io.io-gets-fn]
pub(crate) fn io_gets(iobh: &mut IoBufHandle, target: &mut String) -> i32 {
    /* NULL-derefs in C when io_buf == NULL; io_gets is only ever called after a
    successful load */
    let buf = iobh.io_buf.as_ref().unwrap();
    let base = iobh.io_buf_ptr;
    let mut i = 0usize;
    let mut bytes: Vec<u8> = Vec::new();
    /* copy bytes up to but excluding the next '\n' or '\0' (or end of buffer).
    DEVIATION from C (no bounds check on target — a long line overruns the C
    READ_BUF_SIZE buffer; here target is a growable String) */
    while base + i < buf.len() && buf[base + i] != b'\n' && buf[base + i] != b'\0' {
        bytes.push(buf[base + i]);
        i += 1;
    }
    /* advance past the '\n', or onto the '\0'/end (sticky at end-of-buffer) */
    let new_ptr = if base + i >= buf.len() || buf[base + i] == b'\0' {
        base + i
    } else {
        base + i + 1
    };
    /* NUL-terminate target (the String length replaces the C NUL).
    DEVIATION from C (lossy UTF-8 decode; C keeps the raw bytes) */
    *target = String::from_utf8_lossy(&bytes).into_owned();
    iobh.io_buf_ptr = new_ptr;
    i as i32
}

// [spec:foma:def:io.foma-net-print-fn]
// [spec:foma:sem:io.foma-net-print-fn]
// [spec:foma:def:fomalib.foma-net-print-fn]
// [spec:foma:sem:fomalib.foma-net-print-fn]
// C signature: int foma_net_print(struct fsm *net, gzFile outfile). Here the
// gzip layer is the GzEncoder the caller passes as `&mut dyn Write`.
pub fn foma_net_print(net: &Fsm, outfile: &mut dyn Write) -> i32 {
    /* Header */
    let _ = outfile.write_all(b"##foma-net 1.0##\n");
    /* Properties */
    let _ = outfile.write_all(b"##props##\n");

    let extras = net.is_completed | (net.arcs_sorted_in << 2) | (net.arcs_sorted_out << 4);

    let _ = write!(
        outfile,
        "{} {} {} {} {} {} {} {} {} {} {} {} {}\n",
        net.arity,
        net.arccount,
        net.statecount,
        net.linecount,
        net.finalcount,
        net.pathcount,
        net.is_deterministic,
        net.is_pruned,
        net.is_minimized,
        net.is_epsilon_free,
        net.is_loop_free,
        extras,
        net.name
    );

    /* Sigma */
    let _ = outfile.write_all(b"##sigma##\n");
    let mut sigma = net.sigma.as_deref();
    while let Some(s) = sigma {
        if s.number == -1 {
            break;
        }
        /* gzprintf("%i %s\n", ...) — glibc %s prints "(null)" for a NULL symbol */
        let _ = write!(
            outfile,
            "{} {}\n",
            s.number,
            s.symbol.as_deref().unwrap_or("(null)")
        );
        sigma = s.next.as_deref();
    }

    /* State array */
    let mut laststate: i32 = -1;
    let _ = outfile.write_all(b"##states##\n");
    let mut i = 0usize;
    while net.states[i].state_no != -1 {
        let fsm = &net.states[i];
        if fsm.state_no != laststate {
            if fsm.r#in != fsm.out {
                let _ = write!(
                    outfile,
                    "{} {} {} {} {}\n",
                    fsm.state_no, fsm.r#in, fsm.out, fsm.target, fsm.final_state
                );
            } else {
                let _ = write!(
                    outfile,
                    "{} {} {} {}\n",
                    fsm.state_no, fsm.r#in, fsm.target, fsm.final_state
                );
            }
        } else if fsm.r#in != fsm.out {
            let _ = write!(outfile, "{} {} {}\n", fsm.r#in, fsm.out, fsm.target);
        } else {
            let _ = write!(outfile, "{} {}\n", fsm.r#in, fsm.target);
        }
        laststate = fsm.state_no;
        i += 1;
    }
    /* Sentinel for states */
    let _ = outfile.write_all(b"-1 -1 -1 -1 -1\n");

    /* Store confusion matrix */
    if let Some(ml) = net.medlookup.as_deref() {
        /* C: net->medlookup->confusion_matrix != NULL — an empty Vec ↔ NULL */
        if !ml.confusion_matrix.is_empty() {
            let _ = outfile.write_all(b"##cmatrix##\n");
            let maxsigma = sigma_max(net.sigma.as_deref()) + 1;
            for k in 0..(maxsigma * maxsigma) {
                let _ = write!(outfile, "{}\n", ml.confusion_matrix[k as usize]);
            }
        }
    }

    /* End */
    let _ = outfile.write_all(b"##end##\n");
    1
}

// [spec:foma:def:io.net-print-att-fn]
// [spec:foma:sem:io.net-print-att-fn]
// [spec:foma:def:fomalib.net-print-att-fn]
// [spec:foma:sem:fomalib.net-print-att-fn]
pub fn net_print_att(net: &Fsm, outfile: &mut dyn Write) -> i32 {
    let mut sl = sigma_to_list(net.sigma.as_deref());
    if sigma_max(net.sigma.as_deref()) >= 0 {
        /* (sl+0)->symbol = g_att_epsilon */
        sl[0].symbol = Some(G_ATT_EPSILON.with(|e| e.borrow().clone()));
    }
    let mut i = 0usize;
    while net.states[i].state_no != -1 {
        let fsm = &net.states[i];
        if fsm.target != -1 {
            let _ = write!(
                outfile,
                "{}\t{}\t{}\t{}\n",
                fsm.state_no,
                fsm.target,
                sl[fsm.r#in as usize].symbol.as_deref().unwrap_or("(null)"),
                sl[fsm.out as usize].symbol.as_deref().unwrap_or("(null)")
            );
        }
        i += 1;
    }
    let mut prev: i32 = -1;
    let mut i = 0usize;
    while net.states[i].state_no != -1 {
        let fsm = &net.states[i];
        if fsm.state_no != prev && fsm.final_state == 1 {
            let _ = write!(outfile, "{}\n", fsm.state_no);
        }
        prev = fsm.state_no;
        i += 1;
    }
    /* free(sl) — dropped */
    1
}

// [spec:foma:def:io.io-get-gz-file-size-fn]
// [spec:foma:sem:io.io-get-gz-file-size-fn]
pub(crate) fn io_get_gz_file_size(filename: &str) -> usize {
    /* The last four bytes in a .gz file are the uncompressed size (ISIZE),
    little-endian. C leaves fopen unchecked; here a failed open/seek/read
    returns 0 (DEVIATION: C NULL-derefs / reads garbage). */
    let mut infile = match File::open(filename) {
        Ok(f) => f,
        Err(_) => return 0,
    };
    let mut bytes = [0u8; 4];
    if infile.seek(SeekFrom::End(-4)).is_err() {
        return 0;
    }
    if infile.read_exact(&mut bytes).is_err() {
        return 0;
    }
    ((bytes[0] as u32)
        | ((bytes[1] as u32) << 8)
        | ((bytes[2] as u32) << 16)
        | ((bytes[3] as u32) << 24)) as usize
}

// [spec:foma:def:io.io-get-regular-file-size-fn]
// [spec:foma:sem:io.io-get-regular-file-size-fn]
pub(crate) fn io_get_regular_file_size(filename: &str) -> usize {
    /* C: fopen, fseek END, ftell, fclose. fopen unchecked (DEVIATION: 0 on
    failure here). */
    std::fs::metadata(filename)
        .map(|m| m.len() as usize)
        .unwrap_or(0)
}

// [spec:foma:def:io.io-get-file-size-fn]
// [spec:foma:sem:io.io-get-file-size-fn]
pub(crate) fn io_get_file_size(filename: &str) -> usize {
    /* C: gzopen(filename, "r"); if NULL return 0. gzdirect() == 1 (file is not
    gzip data, read raw) → regular on-disk size; else → gzip trailer size.
    flate2 has no gzdirect, so sniff the 1f 8b magic (what gzdirect keys on). */
    let mut file = match File::open(filename) {
        Ok(f) => f,
        Err(_) => return 0,
    };
    let mut magic = [0u8; 2];
    let is_gzip = file.read_exact(&mut magic).is_ok() && magic == [0x1f, 0x8b];
    if !is_gzip {
        io_get_regular_file_size(filename)
    } else {
        io_get_gz_file_size(filename)
    }
}

// [spec:foma:def:io.io-gz-file-to-mem-fn]
// [spec:foma:sem:io.io-gz-file-to-mem-fn]
pub fn io_gz_file_to_mem(iobh: &mut IoBufHandle, filename: &str) -> usize {
    let size = io_get_file_size(filename);
    if size == 0 {
        return 0;
    }
    /* C: malloc(size+1); gzopen "rb"; gzread(size); gzclose; buf[size]='\0'.
    gzopen transparently decompresses gzip AND passes plain files through;
    flate2's GzDecoder errors on non-gzip input, so sniff the magic and fall
    back to a plain read. */
    let mut file = match File::open(filename) {
        Ok(f) => f,
        Err(_) => return 0,
    };
    let mut magic = [0u8; 2];
    let is_gzip = file.read_exact(&mut magic).is_ok() && magic == [0x1f, 0x8b];
    let _ = file.seek(SeekFrom::Start(0));
    let mut content: Vec<u8> = Vec::new();
    if is_gzip {
        /* gzread's return is unchecked in C: a corrupt body leaves the tail
        uninitialized. read_to_end reads the whole (single-member) stream, which
        equals `size` for well-formed foma files. */
        let mut dec = GzDecoder::new(file);
        let _ = dec.read_to_end(&mut content);
    } else {
        let _ = file.read_to_end(&mut content);
    }
    /* buf[size] = '\0' */
    content.push(0);
    iobh.io_buf = Some(content);
    iobh.io_buf_ptr = 0;
    size
}

// [spec:foma:def:io.check-bom-fn]
// [spec:foma:sem:io.check-bom-fn]
#[allow(non_snake_case)]
pub(crate) fn check_BOM(buffer: &[u8]) -> Option<&'static Bom> {
    /* for (bom = BOM_codes; bom->len; bom++) — see strncmp for the NUL-based
    quirks (any leading '\0' → UTF-32BE; FF FE 00 → UTF-32LE) */
    for bom in BOM_CODES.iter() {
        if bom.len == 0 {
            break;
        }
        if strncmp(&bom.code, buffer, bom.len as usize) == 0 {
            return Some(bom);
        }
    }
    None
}

/* C: static BOM BOM_codes[] — trailing initializers of `code` default to 0 */
static BOM_CODES: [Bom; 6] = [
    Bom {
        code: [0xEF, 0xBB, 0xBF, 0x00],
        len: 3,
        name: Some("UTF-8"),
    },
    Bom {
        code: [0xFF, 0xFE, 0x00, 0x00],
        len: 4,
        name: Some("UTF-32LE"),
    },
    Bom {
        code: [0x00, 0x00, 0xFE, 0xFF],
        len: 4,
        name: Some("UTF-32BE"),
    },
    Bom {
        code: [0xFF, 0xFE, 0x00, 0x00],
        len: 2,
        name: Some("UTF16-LE"),
    },
    Bom {
        code: [0xFE, 0xFF, 0x00, 0x00],
        len: 2,
        name: Some("UTF16-BE"),
    },
    Bom {
        code: [0x00, 0x00, 0x00, 0x00],
        len: 0,
        name: None,
    },
];

// [spec:foma:def:io.file-to-mem-fn]
// [spec:foma:sem:io.file-to-mem-fn]
// [spec:foma:def:fomalib.file-to-mem-fn]
// [spec:foma:sem:fomalib.file-to-mem-fn]
pub fn file_to_mem(name: &str) -> Option<Vec<u8>> {
    let mut infile = match File::open(name) {
        Ok(f) => f,
        Err(_) => {
            print!("Error opening file '{}'\n", name);
            return None;
        }
    };
    /* fseek END + ftell → on-disk size */
    let numbytes = infile.metadata().map(|m| m.len() as usize).unwrap_or(0);
    /* malloc(numbytes+1) — never NULL in Rust; fread numbytes */
    let mut content = vec![0u8; numbytes];
    if infile.read_exact(&mut content).is_err() {
        print!("Error reading file '{}'\n", name);
        return None;
    }
    /* check_BOM runs on the buffer BEFORE the '\0' terminator is written, as in
    C. DEVIATION from C (for empty/short files C reads uninitialized bytes; here
    bytes past the file end read as 0, so an empty file is reported UTF-32BE). */
    if let Some(bom) = check_BOM(&content) {
        print!(
            "{} BOM mark is detected in file '{}'.\n",
            bom.name.unwrap(),
            name
        );
        return None;
    }
    /* fclose (drop infile); buffer[numbytes] = '\0' */
    content.push(0);
    Some(content)
}

//! Apply/lookup-path microbenchmark + arc-memory footprint.
//!
//!   cargo run --release --example apply_bench
//!
//! The build_bench deliberately skips apply; this measures the runtime-hot
//! path (running words through a compiled transducer) plus the in-memory line
//! table's byte cost — the two things a CSR representation would move.

#![allow(
    clippy::disallowed_methods,
    reason = "native-only benchmark; wall-clock timing is the point"
)]

use std::time::{Duration, Instant};

use foma::apply::{apply_down, apply_init};
use foma::options::FomaOptions;
use foma::regex::fsm_parse_regex;
use foma::types::Fsm;

fn opts() -> FomaOptions {
    FomaOptions::default()
}

fn compile(rx: &str) -> Fsm {
    fsm_parse_regex(&opts(), rx, None, None)
        .unwrap_or_else(|| panic!("regex failed to compile: {rx:?}"))
}

/// A dictionary FST: N distinct base-B words unioned. A wide base + long words
/// keeps the minimized DAWG large (a realistic lexicon shape), so the line
/// table's byte cost is actually exercised.
const BASE: usize = 20;
const WLEN: usize = 8;

fn word_union(n: usize) -> String {
    let mut parts = Vec::with_capacity(n);
    for i in 0..n {
        let mut w = String::new();
        let mut k = i;
        for _ in 0..WLEN {
            w.push((b'a' + (k % BASE) as u8) as char);
            w.push(' ');
            k /= BASE;
        }
        parts.push(w.trim_end().to_string());
    }
    parts.join(" | ")
}

/// The words that the above FST accepts, as apply inputs (space-joined symbols
/// -> the concatenated string the transducer reads).
fn words(n: usize) -> Vec<String> {
    (0..n)
        .map(|i| {
            let mut w = String::new();
            let mut k = i;
            for _ in 0..WLEN {
                w.push((b'a' + (k % BASE) as u8) as char);
                k /= BASE;
            }
            w
        })
        .collect()
}

fn main() {
    // A transducer that lowercases: [a:A | b:B | ...]* is overkill; use the
    // dictionary and apply each accepted word (exercises the arc-walk + sigma
    // match on every input symbol).
    for &n in &[4000usize, 40000] {
        let net = compile(&word_union(n));
        let inputs = words(n);

        let line_bytes = net.states.len() * std::mem::size_of::<foma::types::FsmState>();
        // A packed CSR arc is (in:i16, out:i16, target:i32) = 8 bytes + a
        // per-state (offset:u32, final/start bits) header — model it to show
        // the footprint delta the rewrite would deliver.
        let arcs = net.arccount.max(0) as usize;
        let states = net.statecount.max(0) as usize;
        let csr_bytes = arcs * 8 + states * 8;

        let mut hits;
        let (elapsed, iters) = {
            for _ in 0..3 {
                let mut h = apply_init(&net);
                for w in &inputs {
                    let _ = apply_down(&mut h, Some(w));
                }
            }
            hits = 0u64;
            let start = Instant::now();
            let mut iters = 0u64;
            while start.elapsed() < Duration::from_millis(600) || iters < 4 {
                let mut h = apply_init(&net);
                for w in &inputs {
                    if apply_down(&mut h, Some(w)).is_some() {
                        hits += 1;
                    }
                }
                iters += 1;
            }
            (start.elapsed(), iters)
        };
        let per_word = elapsed / (iters as u32 * inputs.len() as u32);

        println!(
            "n={n:<5} states={states:<6} arcs={arcs:<7} linecount={:<7} \
             line_table={line_bytes:>8}B  csr_model={csr_bytes:>8}B ({:.0}% of line) \
             apply={per_word:?}/word hits={}",
            net.linecount,
            100.0 * csr_bytes as f64 / line_bytes as f64,
            hits / iters,
        );
    }
}

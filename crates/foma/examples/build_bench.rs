//! Build-path microbenchmarks: what does it cost to *compile* FSTs?
//!
//!   cargo run --release --example build_bench            # timing table
//!   cargo run --release --example build_bench profile    # tight loop for samply
//!
//! We time whole `fsm_parse_regex` compiles (parse → construct → minimize) at
//! a range of shapes, plus a few isolated ops (determinize/minimize/compose)
//! rebuilt fresh each iteration. Lookup/apply is deliberately out of scope.

#![allow(
    clippy::disallowed_methods,
    reason = "native-only benchmark; wall-clock timing is the point"
)]

use std::time::{Duration, Instant};

use foma::constructions::{fsm_compose, fsm_union};
use foma::determinize::fsm_determinize;
use foma::minimize::fsm_minimize;
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

/// Run `f` for a warmup then a measured window; return (per-iter, iters).
fn measure(mut f: impl FnMut()) -> (Duration, u64) {
    for _ in 0..3 {
        f();
    }
    let budget = Duration::from_millis(400);
    let start = Instant::now();
    let mut iters = 0u64;
    while start.elapsed() < budget || iters < 8 {
        f();
        iters += 1;
    }
    (start.elapsed() / iters as u32, iters)
}

struct Row {
    name: &'static str,
    per_iter: Duration,
    states: i32,
    arcs: i32,
    iters: u64,
}

/// Time a full compile of `rx`; the resulting machine's size is reported to
/// give context for the cost.
fn bench_compile(rows: &mut Vec<Row>, name: &'static str, rx: &str) {
    let net = compile(rx);
    let (states, arcs) = (net.statecount, net.arccount);
    let (per_iter, iters) = measure(|| {
        std::hint::black_box(compile(rx));
    });
    rows.push(Row {
        name,
        per_iter,
        states,
        arcs,
        iters,
    });
}

/// Time an isolated op that consumes its input, rebuilding the input fresh each
/// iteration. Subtract the build cost so the row reflects the op alone.
fn bench_op(
    rows: &mut Vec<Row>,
    name: &'static str,
    build: impl Fn() -> Fsm,
    op: impl Fn(Fsm) -> Fsm,
) {
    let sample = op(build());
    let (states, arcs) = (sample.statecount, sample.arccount);
    let (build_only, _) = measure(|| {
        std::hint::black_box(build());
    });
    let (build_plus_op, iters) = measure(|| {
        std::hint::black_box(op(build()));
    });
    let per_iter = build_plus_op.saturating_sub(build_only);
    rows.push(Row {
        name,
        per_iter,
        states,
        arcs,
        iters,
    });
}

// ---- workload generators ------------------------------------------------

/// N distinct words (space-separated symbols) unioned — a dictionary FST.
fn word_union(n: usize) -> String {
    let mut parts = Vec::with_capacity(n);
    for i in 0..n {
        // spell i in base-5 letters so words share prefixes/suffixes (realistic)
        let mut w = String::new();
        let mut k = i;
        for _ in 0..6 {
            w.push((b'a' + (k % 5) as u8) as char);
            w.push(' ');
            k /= 5;
        }
        parts.push(w.trim_end().to_string());
    }
    parts.join(" | ")
}

fn deep_concat(n: usize) -> String {
    (0..n)
        .map(|i| ((b'a' + (i % 20) as u8) as char).to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

/// `?* a ? ? ... ?` — the classic subset-construction blowup: k trailing anys.
fn exp_det(k: usize) -> String {
    let anys = std::iter::repeat_n("?", k).collect::<Vec<_>>().join(" ");
    format!("?* a {anys}")
}

fn main() {
    let profile = std::env::args().nth(1).as_deref() == Some("profile");

    if profile {
        // Tight loop over the heaviest builds for a sampling profiler.
        let wu = word_union(400);
        let ed = exp_det(6);
        let start = Instant::now();
        let mut n = 0u64;
        while start.elapsed() < Duration::from_secs(8) {
            std::hint::black_box(compile(&wu));
            std::hint::black_box(compile(&ed));
            std::hint::black_box(compile("[a|b|c|d|e]* c a t"));
            std::hint::black_box(compile("a -> b || c _ d"));
            n += 1;
        }
        eprintln!("profile: {n} rounds in {:?}", start.elapsed());
        return;
    }

    let mut rows: Vec<Row> = Vec::new();

    // Full compiles (parse → construct → minimize)
    bench_compile(&mut rows, "compile: a b c d e", "a b c d e");
    bench_compile(&mut rows, "compile: [a|b|c|d|e]*", "[a|b|c|d|e]*");
    bench_compile(&mut rows, "compile: deep concat (40)", &deep_concat(40));
    bench_compile(&mut rows, "compile: word union (100)", &word_union(100));
    bench_compile(&mut rows, "compile: word union (400)", &word_union(400));
    bench_compile(&mut rows, "compile: word union (1000)", &word_union(1000));
    bench_compile(&mut rows, "compile: star+concat", "[a|b|c|d|e]* c a t");
    bench_compile(&mut rows, "compile: intersect", "[?* a ?*] & [?* b ?*]");
    bench_compile(&mut rows, "compile: complement ~[a b c]", "~[a b c]");
    bench_compile(&mut rows, "compile: replace a->b", "a -> b");
    bench_compile(&mut rows, "compile: ctx replace", "a -> b || c _ d");
    bench_compile(&mut rows, "compile: compose .o.", "[a:b]* .o. [b:c]*");
    bench_compile(&mut rows, "compile: det blowup ?* a ?^4", &exp_det(4));
    bench_compile(&mut rows, "compile: det blowup ?* a ?^6", &exp_det(6));

    // Isolated ops (rebuild input each iter, subtract build cost)
    let o = &opts();
    bench_op(
        &mut rows,
        "op: determinize ?* a ?^6",
        {
            let rx = exp_det(6);
            // build the NON-minimized NFA: parse only would need internals; use a
            // union of two epsilon-heavy branches then determinize.
            move || compile_nfa(&rx)
        },
        fsm_determinize,
    );
    bench_op(
        &mut rows,
        "op: minimize (word union 400)",
        {
            let rx = word_union(400);
            move || fsm_determinize(compile_nfa(&rx))
        },
        move |n| fsm_minimize(o, n),
    );
    bench_op(
        &mut rows,
        "op: union 200 words (incremental)",
        || compile("a"),
        move |mut acc| {
            for i in 0..200 {
                let w = compile(&format!("{} b c", (b'a' + (i % 20) as u8) as char));
                acc = fsm_union(o, acc, w);
            }
            acc
        },
    );
    bench_op(
        &mut rows,
        "op: compose [a:b]* .o. [b:c]*",
        || compile("[a:b]*"),
        move |n| fsm_compose(o, n, compile("[b:c]*")),
    );

    // ---- report ----
    rows.sort_by_key(|r| std::cmp::Reverse(r.per_iter));
    println!(
        "\n{:<34} {:>12} {:>10} {:>9} {:>7}",
        "workload", "per-iter", "states", "arcs", "iters"
    );
    println!("{}", "-".repeat(76));
    for r in &rows {
        println!(
            "{:<34} {:>12} {:>10} {:>9} {:>7}",
            r.name,
            format!("{:?}", r.per_iter),
            r.states,
            r.arcs,
            r.iters
        );
    }
}

/// A regex compile that skips the final minimize, to feed determinize/minimize
/// benches a non-minimal machine. `fsm_parse_regex` always minimizes, so we
/// approximate by compiling then un-minimizing via a self-union (cheap, keeps
/// the language, leaves a non-minimal NFA-ish net for the op to chew on).
fn compile_nfa(rx: &str) -> Fsm {
    let o = opts();
    let a = compile(rx);
    let b = compile(rx);
    fsm_union(&o, a, b)
}

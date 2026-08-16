//! nom vs chumsky vs combine vs syan(char) vs syan(token), one grammar, three input shapes plus the
//! error path.
//!
//! Fairness rules, because they are what make or break a parser comparison:
//!
//! * every backend starts from `&str` and produces the same `ast::Expr` (gated by `tests/agree.rs`);
//! * every backend requires full input consumption, so nobody wins by stopping early;
//! * `syan-token` is reported twice — `lex+parse` (comparable) and `parse only` (not comparable,
//!   shown to separate proc-macro2's lexer from syan's parser), plus `lex only` as the baseline.

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use syan_bench::{chumsky_impl, combine_impl, input, nom_impl, syan_char, syan_token};

fn bench_shape(c: &mut Criterion, name: &str, cases: Vec<(String, String)>) {
    let mut g = c.benchmark_group(name);
    for (label, src) in &cases {
        g.throughput(Throughput::Bytes(src.len() as u64));

        g.bench_with_input(BenchmarkId::new("nom", label), src, |b, s| {
            b.iter(|| nom_impl::parse(s).unwrap())
        });
        g.bench_with_input(BenchmarkId::new("chumsky", label), src, |b, s| {
            b.iter(|| chumsky_impl::parse(s).unwrap())
        });
        g.bench_with_input(BenchmarkId::new("combine", label), src, |b, s| {
            b.iter(|| combine_impl::parse(s).unwrap())
        });
        for (eng, cf, tf, pf) in [
            (
                "ranked",
                syan_char::ranked::parse as fn(&str) -> Result<_, _>,
                syan_token::ranked::lex_then_parse as fn(&str) -> Result<_, _>,
                syan_token::ranked::parse_pretokenised
                    as fn(proc_macro2::TokenStream) -> Result<_, _>,
            ),
            (
                "structural",
                syan_char::structural::parse as fn(&str) -> Result<_, _>,
                syan_token::structural::lex_then_parse as fn(&str) -> Result<_, _>,
                syan_token::structural::parse_pretokenised
                    as fn(proc_macro2::TokenStream) -> Result<_, _>,
            ),
        ] {
            g.bench_with_input(
                BenchmarkId::new(format!("syan-char/{eng}"), label),
                src,
                |b, s| b.iter(|| cf(s).unwrap()),
            );
            g.bench_with_input(
                BenchmarkId::new(format!("syan-token/{eng}/lex+parse"), label),
                src,
                |b, s| b.iter(|| tf(s).unwrap()),
            );
            // Not comparable with the &str backends — isolates syan's parser from the lexer.
            g.bench_with_input(
                BenchmarkId::new(format!("syan-token/{eng}/parse-only"), label),
                src,
                |b, s| {
                    // `ts.clone()` goes in SETUP, not in the measured routine: proc-macro2 deep-copies
                    // a `TokenStream`, and the `&str` backends are handed a borrow. Charging syan for
                    // that copy was worth ~4% of its allocations and is not like-for-like.
                    let ts = syan_token::tokenise(s);
                    b.iter_batched(|| ts.clone(), |ts| pf(ts).unwrap(), BatchSize::SmallInput)
                },
            );
        }
        g.bench_with_input(
            BenchmarkId::new("syan-token/lex-only", label),
            src,
            |b, s| b.iter(|| syan_token::tokenise(s)),
        );
    }
    g.finish();
}

/// Flat, iterative: `1 + 22 * 333 - …`. No recursion, so this isolates per-atom and per-node cost.
fn flat(c: &mut Criterion) {
    let cases = [4usize, 16, 64, 256]
        .iter()
        .map(|n| (format!("{n}ops"), input::flat(*n)))
        .collect();
    bench_shape(c, "flat", cases);
}

/// Deeply nested parens. This is the shape `#[recurse]` exists for, and the shape where the
/// token source's "a group is one atom" advantage should show up.
fn nested(c: &mut Criterion) {
    let cases = [1usize, 8, 32, 128]
        .iter()
        .map(|d| (format!("depth{d}"), input::nested(*d)))
        .collect();
    bench_shape(c, "nested", cases);
}

/// Balanced binary tree, `2^depth` leaves — recursion and width together.
fn tree(c: &mut Criterion) {
    let cases = [2usize, 4, 6, 8]
        .iter()
        .map(|d| (format!("depth{d}"), input::tree(*d)))
        .collect();
    bench_shape(c, "tree", cases);
}

/// The FAILURE path: input that is valid up to the last atom. Error construction is on the hot
/// path of any backtracking parser, and `error-design-vs-chumsky.md` measures ~19% of syan's
/// allocations there — this is where that shows up as wall time.
fn errors(c: &mut Criterion) {
    let mut g = c.benchmark_group("error");
    for n in [4usize, 64] {
        let src = input::bad_at(n);
        let label = format!("{n}ops");
        g.throughput(Throughput::Bytes(src.len() as u64));
        g.bench_with_input(BenchmarkId::new("nom", &label), &src, |b, s| {
            b.iter(|| nom_impl::parse(s).unwrap_err())
        });
        g.bench_with_input(BenchmarkId::new("chumsky", &label), &src, |b, s| {
            b.iter(|| chumsky_impl::parse(s).unwrap_err())
        });
        g.bench_with_input(BenchmarkId::new("combine", &label), &src, |b, s| {
            b.iter(|| combine_impl::parse(s).unwrap_err())
        });
        // token = parse only; tokenisation is a separate stage and is excluded.
        let ts = syan_token::tokenise(&src);
        for (eng, cf, pf) in [
            (
                "ranked",
                syan_char::ranked::parse as fn(&str) -> Result<_, _>,
                syan_token::ranked::parse_pretokenised
                    as fn(proc_macro2::TokenStream) -> Result<_, _>,
            ),
            (
                "structural",
                syan_char::structural::parse as fn(&str) -> Result<_, _>,
                syan_token::structural::parse_pretokenised
                    as fn(proc_macro2::TokenStream) -> Result<_, _>,
            ),
        ] {
            g.bench_with_input(
                BenchmarkId::new(format!("syan-char/{eng}"), &label),
                &src,
                |b, s| b.iter(|| cf(s).unwrap_err()),
            );
            g.bench_with_input(
                BenchmarkId::new(format!("syan-token/{eng}/parse-only"), &label),
                &ts,
                |b, ts| {
                    b.iter_batched(
                        || ts.clone(),
                        |ts| pf(ts).unwrap_err(),
                        BatchSize::SmallInput,
                    )
                },
            );
        }
        g.bench_with_input(
            BenchmarkId::new("syan-token/lex-only", &label),
            &src,
            |b, s| b.iter(|| syan_token::tokenise(s)),
        );
    }
    g.finish();
}

criterion_group!(benches, flat, nested, tree, errors);
criterion_main!(benches);

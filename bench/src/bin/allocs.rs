//! Allocations and bytes per parse, per backend. Run with `cargo run -p syan-bench --release --bin allocs`.
//!
//! Criterion measures time; time is noisy and machine-dependent. Allocation counts are exact and
//! reproducible, and for these five backends they explain most of the time difference — so this is
//! the table to quote in a design discussion.

use syan_bench::alloc::{measure, Counting};
use syan_bench::{chumsky_impl, combine_impl, input, nom_impl, syan_char, syan_token};

#[global_allocator]
static A: Counting = Counting;

struct Row {
    backend: &'static str,
    allocs: usize,
    bytes: usize,
}

fn row(backend: &'static str, f: impl FnOnce()) -> Row {
    // warm up first so one-time lazy init is not attributed to the measured call
    let (_, allocs, bytes) = measure(f);
    Row {
        backend,
        allocs,
        bytes,
    }
}

fn report(shape: &str, src: &str, nodes: usize) {
    println!("\n## {shape}  ({} bytes, {nodes} AST nodes)", src.len());
    println!(
        "{:<24} {:>10} {:>12} {:>12} {:>12}",
        "backend", "allocs", "allocs/node", "bytes", "bytes/node"
    );

    // warm-up pass (outside measurement)
    let _ = nom_impl::parse(src);
    let _ = chumsky_impl::parse(src);
    let _ = combine_impl::parse(src);
    let _ = syan_char::ranked::parse(src);
    let _ = syan_token::ranked::lex_then_parse(src);

    let ts = syan_token::tokenise(src);
    let rows = vec![
        row("nom", || {
            nom_impl::parse(src).unwrap();
        }),
        row("chumsky", || {
            chumsky_impl::parse(src).unwrap();
        }),
        row("combine", || {
            combine_impl::parse(src).unwrap();
        }),
        row("syan-char ranked", || {
            syan_char::ranked::parse(src).unwrap();
        }),
        row("syan-token rk lex+parse", || {
            syan_token::ranked::lex_then_parse(src).unwrap();
        }),
        row("syan-token rk parse-only", || {
            syan_token::ranked::parse_pretokenised(ts.clone()).unwrap();
        }),
        row("syan-char structural", || {
            syan_char::structural::parse(src).unwrap();
        }),
        row("syan-token st parse-only", || {
            syan_token::structural::parse_pretokenised(ts.clone()).unwrap();
        }),
        row("syan-token lex-only", || {
            let _ = syan_token::tokenise(src);
        }),
    ];

    for r in rows {
        println!(
            "{:<24} {:>10} {:>12.2} {:>12} {:>12.1}",
            r.backend,
            r.allocs,
            r.allocs as f64 / nodes as f64,
            r.bytes,
            r.bytes as f64 / nodes as f64,
        );
    }
}

fn nodes_of(src: &str) -> usize {
    nom_impl::parse(src).expect("valid input").nodes()
}

fn main() {
    println!("# Allocations per parse — nom vs chumsky vs combine vs syan(char) vs syan(token)");
    println!(
        "\nSame grammar, same input, same output AST (gated by `tests/agree.rs`). \
         `allocs/node` is the number to compare: it is independent of input size and \
         of how many atoms a source model happens to use."
    );

    for n in [4usize, 64, 256] {
        let src = input::flat(n);
        report(&format!("flat/{n}ops"), &src, nodes_of(&src));
    }
    for d in [8usize, 32, 128] {
        let src = input::nested(d);
        report(&format!("nested/depth{d}"), &src, nodes_of(&src));
    }
    for d in [4usize, 8] {
        let src = input::tree(d);
        report(&format!("tree/depth{d}"), &src, nodes_of(&src));
    }

    // Failure path: no AST, so normalise per operator instead.
    println!("\n\n# Failure path (input valid until the last atom)");
    for n in [4usize, 64] {
        let src = input::bad_at(n);
        println!("\n## error/{n}ops  ({} bytes)", src.len());
        println!("{:<24} {:>10} {:>12}", "backend", "allocs", "bytes");
        let _ = nom_impl::parse(&src);
        let _ = chumsky_impl::parse(&src);
        let _ = combine_impl::parse(&src);
        let _ = syan_char::ranked::parse(&src);
        let _ = syan_token::ranked::lex_then_parse(&src);
        for r in [
            row("nom", || {
                nom_impl::parse(&src).unwrap_err();
            }),
            row("chumsky", || {
                chumsky_impl::parse(&src).unwrap_err();
            }),
            row("combine", || {
                combine_impl::parse(&src).unwrap_err();
            }),
            row("syan-char ranked", || {
                syan_char::ranked::parse(&src).unwrap_err();
            }),
            row("syan-token rk lex+parse", || {
                syan_token::ranked::lex_then_parse(&src).unwrap_err();
            }),
            row("syan-char structural", || {
                syan_char::structural::parse(&src).unwrap_err();
            }),
            row("syan-token st lex+parse", || {
                syan_token::structural::lex_then_parse(&src).unwrap_err();
            }),
        ] {
            println!("{:<24} {:>10} {:>12}", r.backend, r.allocs, r.bytes);
        }
    }
}

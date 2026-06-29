//! SPIKE (a) for docs/recurse-unbounded-plan.md §9.1 — the unbounded-`#[recurse]` re-entry mechanism
//! against syan's REAL `ParseStream`/`Dup` (not the toy of `spike_unbounded_recurse.rs`).
//!
//! Validates that erasing the stream to `&mut dyn ParseStream<Atom = TokenTree, Error = Infallible>` at a
//! terminator boundary:
//!   (a) compiles — finite type recursion with the actual `Dup<…>` (the compile IS the proof: if
//!       `term_parse` recursed directly into `expr_parse` instead of through the erased fn ptr, the
//!       `Dup<&mut Dup<&mut …>>` monomorphization would be infinite → E0275),
//!   (b) parses arbitrarily deep input (far past the old type `limit`),
//!   (c) backtracks correctly THROUGH the erased re-entry.
//!
//! Grammar over flat tokens (so nesting is flat puncts, not proc-macro `Group`s):
//!   Expr = "<" Expr ">"  |  Int
#![allow(clippy::missing_transmute_annotations)]

use proc_macro2::TokenTree;
use std::any::type_name;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use syan::parse::{IntoParseStream, ParseStream};
use template_quote::quote;

type Atom = TokenTree;
type Inf = core::convert::Infallible;
// The erased stream — usable only because the §6 object-safety split made `ParseStream` object-safe. The
// `+ '_` (vs the implicit `+ 'static`) is essential: a `Dup<&mut …>` borrows, so the erased dyn must
// carry the stream's (non-'static) lifetime. As a fn-ptr type this is `for<'r,'s> fn(&'r mut (dyn …+'s))`.
type ParseFn = fn(&mut (dyn ParseStream<Atom = Atom, Error = Inf> + '_)) -> Result<Expr, ()>;

#[derive(Debug, PartialEq)]
enum Expr {
    Angle(Box<Expr>),
    Int(i64),
}

// ── registry: type_name-pointer key (ICF-safe, §4), fn-ptr-as-usize ──────────────────────────────
static REG: OnceLock<Mutex<HashMap<usize, usize>>> = OnceLock::new();
fn key<T: ?Sized>() -> usize {
    type_name::<T>().as_ptr() as *const core::ffi::c_void as usize
}
fn register<T: ?Sized>(f: ParseFn) {
    let m = REG.get_or_init(|| Mutex::new(HashMap::new()));
    m.lock().unwrap().entry(key::<T>()).or_insert(f as usize);
}
fn lookup<T: ?Sized>() -> ParseFn {
    let m = REG.get_or_init(|| Mutex::new(HashMap::new()));
    let raw = *m.lock().unwrap().get(&key::<T>()).expect("not registered");
    unsafe { core::mem::transmute::<usize, ParseFn>(raw) }
}
struct ExprKey;

// ── token helpers over the REAL stream ───────────────────────────────────────────────────────────
fn punct<S: ParseStream<Atom = Atom>>(s: &mut S, c: char) -> Result<(), ()> {
    match s.peek() {
        Some(TokenTree::Punct(p)) if p.as_char() == c => {
            s.next();
            Ok(())
        }
        _ => Err(()),
    }
}
fn int<S: ParseStream<Atom = Atom>>(s: &mut S) -> Result<i64, ()> {
    let n = match s.peek() {
        Some(TokenTree::Literal(l)) => l.to_string().parse::<i64>().map_err(|_| ())?,
        _ => return Err(()),
    };
    s.next();
    Ok(n)
}

// ── the engine (depth-1) + Term re-entry, using the real `dup` ───────────────────────────────────
// `term_parse` is the recursive-child re-entry: ERASE the stream to `&mut dyn ParseStream` and call the
// registered top-level fn. It does NOT recurse generically into `expr_parse`, so only finitely many
// monomorphizations are instantiated even though the real `Dup<…>` would otherwise grow per level.
fn term_parse<S: ParseStream<Atom = Atom, Error = Inf>>(s: &mut S) -> Result<Expr, ()> {
    let f = lookup::<ExprKey>();
    // unsize coercion `&'b mut S → &'b mut (dyn … + 'b)` — the type-growth cut. WF gives `S: 'b` from the
    // `&'b mut S` borrow, so the inferred `'_` is sound without an explicit `S: 'static` bound.
    let dyns: &mut (dyn ParseStream<Atom = Atom, Error = Inf> + '_) = s;
    f(dyns)
}

// Expr = "<" Expr ">" | Int.  Each variant tried with the real `ParseStream::dup` (so `Dup<&mut S>`
// nests exactly as a derived recursive `Parse` would). LL(1): the wrong variant fails on token 1.
fn expr_parse<S: ParseStream<Atom = Atom, Error = Inf>>(s: &mut S) -> Result<Expr, ()> {
    if let Ok(v) = s.dup(|d| {
        punct(d, '<')?;
        let inner = term_parse(d)?;
        punct(d, '>')?;
        Ok::<_, ()>(Expr::Angle(Box::new(inner)))
    }) {
        return Ok(v);
    }
    s.dup(|d| int(d).map(Expr::Int))
}

// Top-level backtrack THROUGH the boundary:  Top = Expr "!" | Expr.
fn top_bt<S: ParseStream<Atom = Atom, Error = Inf>>(s: &mut S) -> Result<Expr, ()> {
    if let Ok(v) = s.dup(|d| {
        let e = expr_parse(d)?;
        punct(d, '!')?;
        Ok::<_, ()>(e)
    }) {
        return Ok(v);
    }
    expr_parse(s)
}

// The registered erased entry: the top-level parse monomorphized at the erased stream type, so re-entry
// always restarts from `Dup<&mut dyn ParseStream>` — a fixed type that never grows.
fn expr_parse_dyn(mut s: &mut (dyn ParseStream<Atom = Atom, Error = Inf> + '_)) -> Result<Expr, ()> {
    expr_parse(&mut s)
}

fn stream_of(ts: proc_macro2::TokenStream) -> impl ParseStream<Atom = Atom, Error = Inf> {
    ts.into_parse_stream()
}

fn parse(ts: proc_macro2::TokenStream) -> Result<Expr, ()> {
    register::<ExprKey>(expr_parse_dyn);
    let mut s = stream_of(ts);
    expr_parse(&mut s)
}
fn parse_bt(ts: proc_macro2::TokenStream) -> Result<Expr, ()> {
    register::<ExprKey>(expr_parse_dyn);
    let mut s = stream_of(ts);
    top_bt(&mut s)
}

// Build `< < … < N > … > >` with `depth` angle layers as a flat token stream.
fn nested(depth: usize, n: i64) -> proc_macro2::TokenStream {
    let mut out = proc_macro2::TokenStream::new();
    for _ in 0..depth {
        out.extend(quote!(<));
    }
    // unsuffixed so `int()`'s `to_string().parse::<i64>()` sees `7`, not `7i64`.
    out.extend(std::iter::once(TokenTree::Literal(proc_macro2::Literal::i64_unsuffixed(n))));
    for _ in 0..depth {
        out.extend(quote!(>));
    }
    out
}

fn angle_depth(mut e: &Expr) -> usize {
    let mut d = 0;
    while let Expr::Angle(inner) = e {
        d += 1;
        e = inner;
    }
    d
}

#[test]
fn b_unbounded_depth() {
    // far past the old type `limit` (~4–12); ceiling is only the OS call stack.
    for depth in [0usize, 1, 5, 50, 500] {
        let e = parse(nested(depth, depth as i64)).expect("deep parse");
        assert_eq!(angle_depth(&e), depth, "depth {depth}");
    }
}

#[test]
fn c_backtracks_through_terminators() {
    // `< < 1 > >` (no `!`): the `Expr "!"` alternative parses the whole deep Expr through the erased
    // re-entries, fails on the missing `!`, and rewinds every boundary before the plain `Expr` succeeds.
    let e = parse_bt(nested(2, 1)).expect("<<1>>");
    assert_eq!(angle_depth(&e), 2);

    // `< < 1 > > !`: the `Expr "!"` alternative succeeds after the deep descent.
    let mut banged = nested(2, 1);
    banged.extend(quote!(!));
    let e = parse_bt(banged).expect("<<1>>!");
    assert_eq!(angle_depth(&e), 2);

    // deep backtrack: D=200 boundaries rewound in one go.
    let e = parse_bt(nested(200, 7)).expect("deep");
    assert_eq!(angle_depth(&e), 200);
    let mut cur = &e;
    while let Expr::Angle(i) = cur {
        cur = i;
    }
    assert_eq!(*cur, Expr::Int(7));
}

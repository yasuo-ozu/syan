// Parses `proc_macro2` tokens throughout; the whole suite is skipped without the optional
// dependency that provides them.
#![cfg(feature = "proc_macro2")]

//! Regression tests for three `#[derive(Parse/Unparse/Spanned)]` audit fixes:
//!  - #1: a `where`-clause on a Parse derive used to PANIC the macro
//!    (`assert!(generics.where_clause.is_none())`). It now expands and compiles.
//!  - #4: an Unparse / Spanned derive with a `where`-clause used to DROP the user's bounds from the
//!    generated impl, so the Self type failed WF (cryptic E0277). The bounds are now threaded in.
//!  - #5: a Spanned node with composite/bounded span fields (`WithSpan<_, S>`) used to leave the
//!    invented `__Syan_Span` unconstrained (E0207) and the migrate fold mismatched (E0308). Each
//!    folded field now gets a `FieldTy: Spanned<Span = __Syan_Span>` predicate, pinning the span.
#![allow(dead_code)]

use syan::parse::{Parse, Unparse};
use syan::span::{Spanned, WithSpan};

// #1 — a where-clause on a Parse derive must expand and compile (was a macro panic).
#[derive(Parse)]
struct W1<S, T>
where
    T: Clone,
{
    a: syan::source::proc_macro2::literal::Integer,
    _p: core::marker::PhantomData<(S, T)>,
}

// #4 — an Unparse derive with a where-clause (the user's bounds reach the generated impl).
#[derive(Unparse)]
struct U4<S, T>
where
    T: Clone,
{
    a: WithSpan<u32, S>,
    b: T,
}

// #4 — a Spanned derive with a where-clause likewise compiles.
#[derive(Spanned)]
struct S4<S: syan::span::Span>
where
    S: Clone,
{
    a: WithSpan<u32, S>,
    b: WithSpan<u64, S>,
}

// #5 — a Spanned node whose fields have a concrete `Span = S` (via WithSpan) compiles. Before the
// fix the invented `__Syan_Span` was unconstrained (E0207) for this composite-field shape.
#[derive(Spanned)]
struct N5<S: syan::span::Span> {
    a: WithSpan<u32, S>,
    b: WithSpan<u64, S>,
}

#[test]
fn where_clause_and_composite_span_derives_compile() {
    fn assert_spanned<T: Spanned>() {}
    assert_spanned::<N5<()>>();
    assert_spanned::<S4<()>>();
    let _: Option<W1<(), ()>> = None;
    let _: Option<U4<(), u8>> = None;
}

//! A **group-ful** `#[recurse]` cycle fully supports `Unparse` and `Spanned` on the natural type, and they
//! are now **unbounded** (any tree depth): they delegate through a DEPTH-1 *borrow* engine whose terminator
//! re-enters the top-level impl at runtime (`core::parse::vtable`) — borrowing the natural remainder, so
//! only leaves are cloned and there is no `Root: Clone` requirement. The two former library-level leaf gaps
//! are also closed:
//!   - `Group` unparses to a single `proc_macro2::TokenTree::Group` (delimiter + slot stream), not three
//!     separate tokens — so a brace group round-trips to a `TokenTree` atom;
//!   - `Group`'s span comes from its delimiters, so an empty `Group<(), …>` slot needs no `(): Spanned`.
#![allow(dead_code)]

use syan::parse::{recurse, Parse, Unparse};
use template_quote::quote;

// ── Unparse round-trip (parsing a group fixes the span type to the atom's, so `S` is inferred) ──────
#[recurse]
mod up {
    use syan::nested::group::GroupBrace;
    use syan::parse::{Parse, Unparse};
    use syan::source::proc_macro2::literal::Integer;

    // A brace-delimited list of integer literals, recursive in `inner`.
    #[derive(Parse, Unparse)]
    pub enum Expr<S> {
        Lit(Integer),
        Block {
            brace: GroupBrace<(), S>,
            #[group(self.brace)]
            inner: Vec<Expr<S>>,
        },
    }
}

#[test]
fn group_ful_unparse_round_trips_to_token_group() {
    // `{ 1 2 }` parses into the natural `Expr`, then the delegated `Unparse` emits it back as ONE
    // `TokenTree::Group` token (the brace group).
    let e: up::Expr<_> = Parse::parse(quote! { { 1 2 } }).unwrap();
    let mut out = Vec::<proc_macro2::TokenTree>::new();
    e.unparse(&mut (&mut out)).unwrap();
    assert_eq!(out.len(), 1, "the whole expression is a single brace `TokenTree::Group`");
    assert!(matches!(out[0], proc_macro2::TokenTree::Group(_)));
    assert_eq!(out[0].to_string(), "{ 1 2 }");
}

#[test]
fn group_ful_unparse_is_unbounded() {
    // A 60-deep `{ { … 1 … } }` — FAR past the fixed engine depth (4). Both `Parse` (terminator re-entry)
    // and `Unparse` (depth-1 borrow engine + re-entry) are unbounded, so it round-trips in full.
    let mut src = quote! { 1 };
    for _ in 0..60 {
        src = quote! { { #src } };
    }
    let e: up::Expr<_> = Parse::parse(src.clone()).unwrap();
    let mut out = Vec::<proc_macro2::TokenTree>::new();
    e.unparse(&mut (&mut out)).unwrap();
    assert_eq!(
        out.into_iter().collect::<proc_macro2::TokenStream>().to_string(),
        src.to_string(),
        "deep group-ful tree round-trips (Unparse past the old depth limit)",
    );
}

// ── Spanned over a group-ful cycle (constructed by hand, so the span type can be `()`) ──────────────
#[recurse]
mod sp {
    use syan::nested::group::GroupBrace;
    use syan::span::{Span, Spanned, WithSpan};
    use syan::visit::Ast;

    #[derive(Ast, Spanned)]
    #[subast()]
    pub enum Expr<S: Span> {
        Atom(WithSpan<u32, S>),
        Block {
            brace: GroupBrace<(), S>,
            #[group(self.brace)]
            inner: Vec<Expr<S>>,
        },
    }
}

#[test]
fn group_ful_spanned_folds_delimiters() {
    use sp::Expr;
    use syan::nested::group::Group;
    use syan::span::{Spanned, WithSpan};
    // Build `{ 7 }` by hand (the `Group`/`WithSpan` delimiters and the empty `()` slot are all
    // `Default`). `.span()` folds the group's delimiter spans + the leaf span — the empty slot needs no
    // `Spanned` impl. The point is that the group-ful natural `Spanned` compiles and is callable.
    let brace = Group { open: Default::default(), slot: (), close: Default::default() };
    let tree: Expr<()> = Expr::Block {
        brace,
        inner: vec![Expr::Atom(WithSpan { slot: 7, span: () })],
    };
    let _s: () = tree.span();

    // Unbounded: a depth-2000 hand-built tree — far past the engine depth — still folds its span (the
    // depth-1 borrow engine re-enters per level; no `Root: Clone`).
    let mut deep: Expr<()> = Expr::Atom(WithSpan { slot: 7, span: () });
    for _ in 0..2000 {
        let brace = Group { open: Default::default(), slot: (), close: Default::default() };
        deep = Expr::Block { brace, inner: vec![deep] };
    }
    let _s: () = deep.span();
}

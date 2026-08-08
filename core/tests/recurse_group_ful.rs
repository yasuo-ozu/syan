//! A **group-ful** `#[recurse]` cycle now fully supports `Unparse` and `Spanned` on the natural type
//! (delegated through the engine like any cycle). The two former library-level leaf gaps are closed:
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
    // `TokenTree::Group` token (the brace group), within the depth limit.
    let e: up::Expr<_> = Parse::parse(quote! { { 1 2 } }).unwrap();
    let mut out = Vec::<proc_macro2::TokenTree>::new();
    e.unparse(&mut (&mut out)).unwrap();
    assert_eq!(out.len(), 1, "the whole expression is a single brace `TokenTree::Group`");
    assert!(matches!(out[0], proc_macro2::TokenTree::Group(_)));
    assert_eq!(out[0].to_string(), "{ 1 2 }");
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
}

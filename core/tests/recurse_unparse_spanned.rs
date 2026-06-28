//! `#[derive(Unparse)]` / `#[derive(Spanned)]` with `#[ignore_bounds]` on a `#[recurse]` cycle that
//! carries **type parameters**. Answer to "does it still work?": **yes**, for a *single self-recursive*
//! group-free cycle. There `#[recurse]` keeps `Unparse`/`Spanned` on the **natural** public type (only
//! `Parse` is routed to the depth-limited engine), injecting `#[ignore_bounds]` on each recursive-child
//! field so the leaf-only-bounded impl compiles — the body's recursive `.unparse()`/`.span()` call
//! resolves against the *same* impl, with no E0275 where-bound cycle. (A multi-type or group-ful cycle
//! keeps `Unparse`/`Spanned` on the engine — its members' leaf bounds can't be unioned per-type; see
//! CLAUDE.md.)
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::parse::{recurse, Parse, Unparse};
use template_quote::quote;

// ── Parse + Unparse round-trip over a self-recursive recurse type with a type param `S` ────────────
#[recurse]
mod pu {
    use core::marker::PhantomData;
    use syan::parse::{Parse, Unparse};
    use syan::source::proc_macro2::literal::Integer;

    // A list `1 2 3` → Cons(1, Cons(2, Cons(3, Nil))). Group-free, self-recursive, all-`Integer` leaves
    // (so the round-trip unparses cleanly). `#[recurse]` auto-injects `#[ignore_bounds]` on `tail`; we
    // also write one by hand to confirm it is honored, not doubled.
    #[derive(Parse, Unparse)]
    pub enum Expr<S> {
        Cons {
            head: Integer,
            #[ignore_bounds]
            tail: Box<Expr<S>>,
        },
        Nil(PhantomData<S>),
    }
}

#[test]
fn unparse_roundtrip_with_type_param() {
    // `1 2 3` parses (through the engine, depth-limited) into the NATURAL `Expr<S>`, then the NATURAL
    // `Unparse` (leaf-only bounds via `#[ignore_bounds]`) emits it back.
    let toks = quote! { 1 2 3 };
    // `S` is phantom here (used only in `Nil`), so annotate it; the atom is `TokenTree` (from `Integer`).
    let e: pu::Expr<()> = Parse::parse(toks).unwrap();
    let mut out = Vec::<proc_macro2::TokenTree>::new();
    e.unparse(&mut (&mut out)).unwrap();
    assert_eq!(out.len(), 3, "the three integer literals, round-tripped");
}

#[test]
fn unparse_constructed_deep_tree() {
    use pu::Expr;
    use syan::source::proc_macro2::literal::Integer;
    // Build a tree by hand and unparse it (no parse) — exercises the NATURAL recursive `Unparse`
    // directly. The natural type accepts ANY depth (only `Parse` is engine-bounded), so this depth-11
    // tree — past the default recursion limit — still unparses.
    let mut e: Expr<()> = Expr::Nil(PhantomData);
    for _ in 0..11 {
        e = Expr::Cons {
            head: Integer { value: "1".into(), suffix: None },
            tail: Box::new(e),
        };
    }
    let mut out = Vec::<proc_macro2::TokenTree>::new();
    e.unparse(&mut (&mut out)).unwrap();
    assert_eq!(out.len(), 11, "eleven `1`s — depth 11, far past the engine limit");
}

// ── Spanned over a self-recursive recurse type with a type param `S: Span` ─────────────────────────
#[recurse]
mod sp {
    use syan::span::{Spanned, WithSpan};
    use syan::visit::Ast;

    // All non-recursive leaves are `WithSpan<_, S>` (Spanned, `Span = S`); the recursive `child` is
    // `#[ignore_bounds]` (auto-injected) and so excluded from the span fold.
    #[derive(Ast, Spanned)]
    #[subast()]
    pub enum Expr<S: syan::span::Span> {
        Node {
            head: WithSpan<u32, S>,
            #[ignore_bounds]
            child: Box<Expr<S>>,
        },
        Leaf(WithSpan<u64, S>),
    }
}

#[test]
fn spanned_with_type_param() {
    use sp::Expr;
    use syan::span::{Spanned, WithSpan};
    // `()` is a `Span`. `.span()` folds the (non-ignored) `WithSpan` leaves; the recursive child is
    // ignored. The point is that the NATURAL recursive `Spanned` impl compiles (via `#[ignore_bounds]`)
    // and `.span()` is callable on a type with a parameter.
    let tree: Expr<()> = Expr::Node {
        head: WithSpan { slot: 0, span: () },
        child: Box::new(Expr::Node {
            head: WithSpan { slot: 0, span: () },
            child: Box::new(Expr::Leaf(WithSpan { slot: 0, span: () })),
        }),
    };
    let _s: () = tree.span();
}

// ── MULTI-TYPE cycle: Unparse via natural→engine `from_nat` delegation ─────────────────────────────
// Direct natural `Unparse` can't work here (the members' leaf bounds differ — `Stmt` has no `Integer`),
// so `#[recurse]` routes `Unparse` to the engine and emits a delegated `impl Unparse for Expr/Stmt`
// that converts the (borrowed) natural value to the depth-default engine value (cloning leaves) and
// calls the engine's `Unparse`. Depth-limited (panics past `limit`).
#[recurse]
mod mt {
    use core::marker::PhantomData;
    use syan::parse::{Parse, Unparse};
    use syan::source::proc_macro2::literal::Integer;

    #[derive(Parse, Unparse)]
    pub enum Expr<S> {
        Wrap(Box<Stmt<S>>),
        Lit(Integer, PhantomData<S>), // `Integer` leaf — asymmetric vs `Stmt`
    }

    #[derive(Parse, Unparse)]
    pub enum Stmt<S> {
        Wrap(Box<Expr<S>>),
        Nil(PhantomData<S>), // no `Integer` leaf
    }
}

#[test]
fn multi_type_unparse_via_delegation() {
    use core::marker::PhantomData;
    use mt::{Expr, Stmt};
    use syan::source::proc_macro2::literal::Integer;
    // Expr → Stmt → Expr(Lit 7); within the depth limit. Delegated `Unparse` converts to the engine and
    // emits the single `7` literal.
    let tree: Expr<()> = Expr::Wrap(Box::new(Stmt::Wrap(Box::new(Expr::Lit(
        Integer { value: "7".into(), suffix: None },
        PhantomData,
    )))));
    let mut out = Vec::<proc_macro2::TokenTree>::new();
    tree.unparse(&mut (&mut out)).unwrap();
    assert_eq!(out.len(), 1, "the `7` literal (Stmt::Wrap/Nil emit nothing)");
    let _n: Stmt<()> = Stmt::Nil(PhantomData); // Stmt is also `Unparse` (delegated)
}

// ── MULTI-TYPE cycle: Spanned via delegation (`S: Span`; threaded through the conversion impls) ─────
#[recurse]
mod mts {
    use syan::span::{Span, Spanned, WithSpan};
    use syan::visit::Ast;

    #[derive(Ast, Spanned)]
    #[subast(crate::mts::Stmt)]
    pub enum Expr<S: Span> {
        Wrap(Box<Stmt<S>>),
        Leaf(WithSpan<u32, S>),
    }

    #[derive(Ast, Spanned)]
    #[subast(crate::mts::Expr)]
    pub enum Stmt<S: Span> {
        Wrap(Box<Expr<S>>),
        Tag(WithSpan<u8, S>),
    }
}

#[test]
fn multi_type_spanned_via_delegation() {
    use mts::{Expr, Stmt};
    use syan::span::{Spanned, WithSpan};
    // Depth-3 tree, within the limit; delegated `Spanned` converts to the engine and folds the spans.
    let tree: Expr<()> = Expr::Wrap(Box::new(Stmt::Wrap(Box::new(Expr::Leaf(WithSpan {
        slot: 0,
        span: (),
    })))));
    let _s: () = tree.span();
}

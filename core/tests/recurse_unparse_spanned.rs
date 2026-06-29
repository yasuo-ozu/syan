//! `#[derive(Unparse)]` / `#[derive(Spanned)]` on a `#[recurse]` cycle that carries **type parameters**.
//! `Unparse`/`Spanned` are derived **directly on the natural type** (not delegated through the engine):
//! `#[ignore_bounds]` on recursive-child fields drops the per-field bound (no E0275 where-cycle), and an
//! injected `#[predicate_unparse/spanned(<cycle leaf union>)]` supplies the bounds a member's body needs
//! to call its siblings'. So they are **unbounded** (any depth) — only `Parse` still goes through the
//! fixed-depth engine. This holds for every **group-free** cycle, single or multi-type (a multi-type
//! cycle works because the injected predicate is the *union* of all members' leaf-field bounds, so each
//! member can unparse its siblings). A **group-ful** cycle keeps `Unparse`/`Spanned` engine-delegated
//! (bounded) — see `recurse_group_ful.rs`. All cycles in this file are group-free. See CLAUDE.md.
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
    // (so the round-trip unparses cleanly). `Unparse` is derived DIRECTLY on the natural type (unbounded).
    #[derive(Parse, Unparse)]
    pub enum Expr<S> {
        Cons {
            head: Integer,
            tail: Box<Expr<S>>,
        },
        Nil(PhantomData<S>),
    }
}

#[test]
fn unparse_roundtrip_with_type_param() {
    // `1 2 3` parses (through the engine) into the NATURAL `Expr<S>`, then the DIRECT natural `Unparse`
    // emits it back.
    let toks = quote! { 1 2 3 };
    // `S` is phantom here (used only in `Nil`), so annotate it; the atom is `TokenTree` (from `Integer`).
    let e: pu::Expr<()> = Parse::parse(toks).unwrap();
    let mut out = Vec::<proc_macro2::TokenTree>::new();
    e.unparse(&mut (&mut out)).unwrap();
    assert_eq!(out.len(), 3, "the three integer literals, round-tripped");
}

#[test]
fn unparse_unbounded_depth() {
    use pu::Expr;
    use syan::source::proc_macro2::literal::Integer;
    // `Unparse` is now DIRECT on the natural type (not delegated through the depth-limited engine), so it
    // is **unbounded**: a depth-5000 list — far past any engine `limit` — unparses fine.
    let mut e: Expr<()> = Expr::Nil(PhantomData);
    for _ in 0..5000 {
        e = Expr::Cons {
            head: Integer { value: "1".into(), suffix: None },
            tail: Box::new(e),
        };
    }
    let mut out = Vec::<proc_macro2::TokenTree>::new();
    e.unparse(&mut (&mut out)).unwrap();
    assert_eq!(out.len(), 5000, "five thousand `1`s — depth far past the old limit");
}

// ── Spanned over a self-recursive recurse type with a type param `S: Span` ─────────────────────────
#[recurse]
mod sp {
    use syan::span::{Spanned, WithSpan};
    use syan::visit::Ast;

    // All non-recursive leaves are `WithSpan<_, S>` (Spanned, `Span = S`). `Spanned` is derived DIRECTLY
    // on the natural type (group-free → unbounded), like `Unparse`.
    #[derive(Ast, Spanned)]
    #[subast()]
    pub enum Expr<S: syan::span::Span> {
        Node {
            head: WithSpan<u32, S>,
            child: Box<Expr<S>>,
        },
        Leaf(WithSpan<u64, S>),
    }
}

#[test]
fn spanned_with_type_param() {
    use sp::Expr;
    use syan::span::{Spanned, WithSpan};
    // `()` is a `Span`. `.span()` folds the `WithSpan` leaf spans directly on the natural type. The
    // point is that the direct `Spanned` impl compiles and `.span()` is callable on a type with a
    // parameter (this is a depth-3 tree; being direct it is unbounded).
    let tree: Expr<()> = Expr::Node {
        head: WithSpan { slot: 0, span: () },
        child: Box::new(Expr::Node {
            head: WithSpan { slot: 0, span: () },
            child: Box::new(Expr::Leaf(WithSpan { slot: 0, span: () })),
        }),
    };
    let _s: () = tree.span();
}

// ── MULTI-TYPE cycle: DIRECT natural Unparse via the leaf-bound UNION ───────────────────────────────
// The members' leaf bounds differ (`Expr` has an `Integer` leaf, `Stmt` does not), so each member alone
// couldn't unparse the other. `#[recurse]` injects the *union* of all members' leaf-field bounds as
// `#[predicate_unparse(…)]` on every member, so `Stmt`'s impl also carries `Integer: Unparse` and can
// build/unparse `Expr`. Both impls are therefore DIRECT on the natural type — unbounded (no engine).
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
fn multi_type_unparse_direct_unbounded() {
    use core::marker::PhantomData;
    use mt::{Expr, Stmt};
    use syan::source::proc_macro2::literal::Integer;
    // Expr → Stmt → Expr(Lit 7). Direct natural `Unparse` (no engine) emits the single `7` literal.
    let tree: Expr<()> = Expr::Wrap(Box::new(Stmt::Wrap(Box::new(Expr::Lit(
        Integer { value: "7".into(), suffix: None },
        PhantomData,
    )))));
    let mut out = Vec::<proc_macro2::TokenTree>::new();
    tree.unparse(&mut (&mut out)).unwrap();
    assert_eq!(out.len(), 1, "the `7` literal (Stmt::Wrap/Nil emit nothing)");
    let _n: Stmt<()> = Stmt::Nil(PhantomData); // Stmt is also `Unparse` (direct)

    // Being DIRECT (not engine-delegated), it is unbounded: a depth-2000 alternating tree unparses —
    // far past any engine depth. This is what the leaf-bound *union* buys for a multi-type cycle.
    let mut e: Expr<()> = Expr::Lit(Integer { value: "1".into(), suffix: None }, PhantomData);
    for _ in 0..2000 {
        e = Expr::Wrap(Box::new(Stmt::Wrap(Box::new(e))));
    }
    let mut deep = Vec::<proc_macro2::TokenTree>::new();
    e.unparse(&mut (&mut deep)).unwrap();
    assert_eq!(deep.len(), 1, "deep multi-type tree round-trips (direct → unbounded)");
}

// ── MULTI-TYPE cycle: DIRECT Spanned via the leaf-bound union (`S: Span`) ───────────────────────────
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
fn multi_type_spanned_direct_unbounded() {
    use mts::{Expr, Stmt};
    use syan::span::{Spanned, WithSpan};
    // Depth-3 tree; direct natural `Spanned` (no engine, unbounded) folds the spans.
    let tree: Expr<()> = Expr::Wrap(Box::new(Stmt::Wrap(Box::new(Expr::Leaf(WithSpan {
        slot: 0,
        span: (),
    })))));
    let _s: () = tree.span();
}

//! `#[derive(Unparse)]` / `#[derive(Spanned)]` on a `#[recurse]` cycle that carries **type parameters**.
//! Answer to "does it still work?": **yes** — and now via ONE uniform mechanism. `#[recurse]` routes
//! `Parse`/`Unparse`/`Spanned` to the depth-limited engine and re-supplies them on the natural public
//! type by **delegation** (`emit_delegated_impl`): `Parse` parses the engine then `__ToNat`-converts;
//! `Unparse`/`Spanned` `__FromNat`-convert the natural value to the engine then call the engine's impl.
//! This holds for every cycle — single/multi-type, group-free/group-ful — so the code path is the same.
//! Consequence: delegated `Unparse`/`Spanned` are **depth-limited** like `Parse` (a tree deeper than
//! `limit` panics at the terminator); group-ful `Spanned` additionally relies on `(): Spanned` for the
//! empty group slot. See CLAUDE.md.
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
    // (so the round-trip unparses cleanly). `Unparse` is delegated through the engine like `Parse`.
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
    // `1 2 3` parses (through the engine) into the NATURAL `Expr<S>`, then the delegated `Unparse`
    // (`__FromNat` → engine) emits it back. Within the depth limit.
    let toks = quote! { 1 2 3 };
    // `S` is phantom here (used only in `Nil`), so annotate it; the atom is `TokenTree` (from `Integer`).
    let e: pu::Expr<()> = Parse::parse(toks).unwrap();
    let mut out = Vec::<proc_macro2::TokenTree>::new();
    e.unparse(&mut (&mut out)).unwrap();
    assert_eq!(out.len(), 3, "the three integer literals, round-tripped");
}

#[test]
fn unparse_constructed_within_limit() {
    use pu::Expr;
    use syan::source::proc_macro2::literal::Integer;
    // Build a within-limit tree by hand and unparse it (no parse) — exercises the delegated `Unparse`.
    // The delegated path is depth-limited (default limit 4), so build a depth-3 list.
    let mut e: Expr<()> = Expr::Nil(PhantomData);
    for _ in 0..3 {
        e = Expr::Cons {
            head: Integer { value: "1".into(), suffix: None },
            tail: Box::new(e),
        };
    }
    let mut out = Vec::<proc_macro2::TokenTree>::new();
    e.unparse(&mut (&mut out)).unwrap();
    assert_eq!(out.len(), 3, "three `1`s — depth 3, within the engine limit");
}

#[test]
#[should_panic(expected = "recursion")]
fn unparse_past_limit_panics() {
    use pu::Expr;
    use syan::source::proc_macro2::literal::Integer;
    // The delegated `Unparse` is depth-limited (it converts the natural value to the depth-default engine
    // value): a tree deeper than `limit` reaches the terminator's `__from_nat`/`Unparse` and panics. This
    // is the documented trade-off of the uniform delegated path (vs. the old direct path's arbitrary
    // depth). Default limit is 4, so a depth-11 list overflows.
    let mut e: Expr<()> = Expr::Nil(PhantomData);
    for _ in 0..11 {
        e = Expr::Cons {
            head: Integer { value: "1".into(), suffix: None },
            tail: Box::new(e),
        };
    }
    let mut out = Vec::<proc_macro2::TokenTree>::new();
    let _ = e.unparse(&mut (&mut out)); // panics at the depth-limit terminator
}

// ── Spanned over a self-recursive recurse type with a type param `S: Span` ─────────────────────────
#[recurse]
mod sp {
    use syan::span::{Spanned, WithSpan};
    use syan::visit::Ast;

    // All non-recursive leaves are `WithSpan<_, S>` (Spanned, `Span = S`). `Spanned` is delegated through
    // the engine (`__FromNat` → engine's `Spanned`), uniformly with every other cycle.
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
    // `()` is a `Span`. `.span()` delegates through the engine and folds the `WithSpan` leaf spans. The
    // point is that the delegated `Spanned` impl compiles and `.span()` is callable on a type with a
    // parameter (within the depth limit — this is a depth-3 tree).
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

// ── GROUP-FUL cycle: Spanned via delegation (unlocked by `impl Spanned for ()`) ────────────────────
// The `brace: GroupBrace<(), S>` field makes this group-ful, so `Spanned` is engine-routed and reaches
// the natural type via the `__FromNat` delegation. The engine's group `Spanned` folds the brace's `()`
// slot, which needs `(): Spanned` — now provided (`Span = ()`), so `.span()` works for `S = ()` (the way
// span is exercised in these tests).
#[recurse]
mod grpsp {
    use syan::nested::group::GroupBrace;
    use syan::span::{Span, Spanned, WithSpan};
    use syan::visit::Ast;

    #[derive(Ast, Spanned)]
    #[subast()]
    pub enum Expr<S: Span> {
        Leaf(WithSpan<u32, S>),
        Block {
            brace: GroupBrace<(), S>,
            #[group(self.brace)]
            inner: Vec<Expr<S>>,
        },
    }
}

#[test]
fn group_ful_spanned_via_delegation() {
    use grpsp::Expr;
    use syan::nested::group::Group;
    use syan::span::{Spanned, WithSpan};
    // A group-ful tree with a nested `Block`, within the depth limit; delegated `Spanned` converts to the
    // engine and folds the delimiter + leaf spans (the empty `()` group slot is span-neutral). `Group`
    // has no `Default`, so build the brace explicitly (its delimiter `WithSpan`s and the `()` slot are).
    let brace = Group { open: Default::default(), slot: (), close: Default::default() };
    let tree: Expr<()> = Expr::Block {
        brace,
        inner: vec![Expr::Leaf(WithSpan { slot: 1, span: () })],
    };
    let _s: () = tree.span();
}

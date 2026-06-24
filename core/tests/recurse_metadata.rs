//! Phase 0 of splitting `#[recurse]` from `visitor!()`: `#[recurse]` additively emits, under each
//! cycle type's *original* name, a `visitor!()`-consumable metadata muncher macro. It mirrors
//! `#[derive(Ast)]`'s `X! { @ast $cb { $pre } }` shape (append `@ast { <ORIGINAL def> } @subast { .. }`,
//! then re-invoke `$cb`) **plus** a `@recurse { @node .. @roots .. @depth .. @terms .. @cycle .. }`
//! section the future `visitor!()` consumer keys on.
//!
//! These tests don't build a visitor (the consumer support is a later phase). They invoke the
//! metadata macro with a trivial stringifying `$cb` and assert the emitted sections — proving the
//! macro resolves under the original name and carries the contract's `@recurse` rows. The renamed
//! `__XRec` keeps its own `#[derive(Ast)]` metadata macro; this is purely additive.
#![allow(dead_code)]

use syan::parse::recurse;

// ── single-root cycle: `Expr` is directly self-referential (the `Bin` back-edges) ─────────────────
#[recurse]
mod single {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum Expr<S> {
        Bin(Box<Expr<S>>, Box<Expr<S>>),
        Lit(PhantomData<S>),
    }
}

// ── single-root cycle carrying a non-empty `#[subast]` (a cross-edge to a sibling AST type) ───────
#[recurse]
mod withsub {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    // A leaf AST type the cycle's `Expr` points at; named in `Expr`'s `#[subast]`.
    #[derive(Ast)]
    #[subast()]
    pub struct Ty<S>(pub PhantomData<S>);

    #[derive(Ast)]
    #[subast(crate::withsub::Ty)]
    pub enum Expr<S> {
        Bin(Box<Expr<S>>, Box<Expr<S>>),
        Typed(Ty<S>),
        Lit(PhantomData<S>),
    }
}

// ── multi-root cycle: `A` and `B` each self-reference (both `Box<Self>`) and cross-reference ──────
#[recurse]
mod multi {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum A<S> {
        SelfRef(Box<A<S>>),
        Cross(Box<B<S>>),
        Lit(PhantomData<S>),
    }

    #[derive(Ast)]
    #[subast()]
    pub enum B<S> {
        SelfRef(Box<B<S>>),
        Cross(Box<A<S>>),
        Lit(PhantomData<S>),
    }
}

/// A stringifying callback in the same muncher shape the metadata macro re-invokes: it receives the
/// `$pre` prefix (here a single ident naming the module of consts to emit), then the appended
/// `@ast`/`@subast`/`@recurse` sections, and emits seven `&str` consts (one per section) so a test
/// can read them. Invoked at item position — the same position `visitor!()` fetches a type from.
/// Defined before its uses (textual `macro_rules!` scoping).
macro_rules! capture_consts {
    (
        $name:ident
        @ast { $($ast:tt)* }
        @subast { $($subast:tt)* }
        @recurse {
            @node { $($node:tt)* }
            @roots { $($roots:tt)* }
            @depth { $($depth:tt)* }
            @terms { $($terms:tt)* }
            @cycle { $($cycle:tt)* }
        }
    ) => {
        mod $name {
            pub const DEF: &str = stringify!($($ast)*);
            pub const SUBAST: &str = stringify!($($subast)*);
            pub const NODE: &str = stringify!($($node)*);
            pub const ROOTS: &str = stringify!($($roots)*);
            pub const DEPTH: &str = stringify!($($depth)*);
            pub const TERMS: &str = stringify!($($terms)*);
            pub const CYCLE: &str = stringify!($($cycle)*);
        }
    };
}

// Drive the metadata macros at item position; `$pre` = the target module ident for the consts.
single::Expr! { @ast capture_consts { expr_meta } }
withsub::Expr! { @ast capture_consts { withsub_meta } }
multi::A! { @ast capture_consts { a_meta } }
multi::B! { @ast capture_consts { b_meta } }

#[test]
fn single_root_metadata_has_recurse_section() {
    // ORIGINAL def (NOT the renamed `__ExprRec` form): the back-edge field stays `Box < Expr < S > >`.
    assert!(expr_meta::DEF.contains("enum Expr"), "def = {}", expr_meta::DEF);
    assert!(
        expr_meta::DEF.contains("Box < Expr < S > >"),
        "def keeps original field types: {}",
        expr_meta::DEF
    );
    assert!(
        !expr_meta::DEF.contains("__ExprRec"),
        "def must be the original, not the renamed type: {}",
        expr_meta::DEF
    );
    assert!(
        !expr_meta::DEF.contains("__Rec"),
        "def must not carry the depth param: {}",
        expr_meta::DEF
    );

    // `#[subast()]` is empty here.
    assert_eq!(expr_meta::SUBAST, "");

    // `@recurse` rows for a single root.
    assert_eq!(expr_meta::NODE, "$crate :: single :: __ExprRec", "node = {}", expr_meta::NODE);
    assert_eq!(expr_meta::ROOTS, "Expr", "roots = {}", expr_meta::ROOTS);
    assert_eq!(expr_meta::DEPTH, "__Rec", "depth = {}", expr_meta::DEPTH);
    assert_eq!(expr_meta::TERMS, "$crate :: single :: ExprTerm", "terms = {}", expr_meta::TERMS);
    assert_eq!(expr_meta::CYCLE, "Expr", "cycle = {}", expr_meta::CYCLE);
}

#[test]
fn subast_entries_are_crate_rooted_and_keyed() {
    // A cross-edge listed in `#[subast]` is carried as `<$crate-rooted path> as <matchkey>`, exactly
    // like `#[derive(Ast)]`. `Ty` is a non-cycle (acyclic) sibling, so it gets normal `#[derive(Ast)]`
    // metadata and no `@recurse` of its own — but it appears here as `Expr`'s followed child.
    assert_eq!(withsub_meta::SUBAST, "$crate :: withsub :: Ty as Ty");
    // The recurse section is still emitted (single root `Expr`); `Ty` is NOT in the cycle.
    assert_eq!(withsub_meta::NODE, "$crate :: withsub :: __ExprRec");
    assert_eq!(withsub_meta::ROOTS, "Expr");
    assert_eq!(withsub_meta::DEPTH, "__Rec");
    assert_eq!(withsub_meta::TERMS, "$crate :: withsub :: ExprTerm");
    assert_eq!(withsub_meta::CYCLE, "Expr");
}

#[test]
fn multi_root_metadata_is_parallel_and_scc_wide() {
    // `A`'s metadata: per-type `@node`, but SCC-level `@roots`/`@depth`/`@terms`/`@cycle`.
    assert!(a_meta::DEF.contains("enum A"), "def_a = {}", a_meta::DEF);
    assert_eq!(a_meta::NODE, "$crate :: multi :: __ARec");
    // roots/depth/terms are PARALLEL (sorted-root order A, B); cycle lists every SCC type.
    assert_eq!(a_meta::ROOTS, "A B");
    assert_eq!(a_meta::DEPTH, "__RecA __RecB");
    assert_eq!(a_meta::TERMS, "$crate :: multi :: ATerm $crate :: multi :: BTerm");
    assert_eq!(a_meta::CYCLE, "A B");

    // `B`'s metadata: only `@node` differs; the SCC-level rows are identical.
    assert!(b_meta::DEF.contains("enum B"), "def_b = {}", b_meta::DEF);
    assert_eq!(b_meta::NODE, "$crate :: multi :: __BRec");
    assert_eq!(b_meta::ROOTS, a_meta::ROOTS);
    assert_eq!(b_meta::DEPTH, a_meta::DEPTH);
    assert_eq!(b_meta::TERMS, a_meta::TERMS);
    assert_eq!(b_meta::CYCLE, a_meta::CYCLE);
}

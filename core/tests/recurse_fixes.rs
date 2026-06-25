//! Regression tests for two `#[recurse]` audit fixes:
//!  - #6: `#[recurse(limit = 1)]` on a *generic* cycle type used to fail E0091 (the depth-default
//!    alias bottomed out at the bare non-generic terminator, leaving the param unused). The
//!    terminator is now generic over the root's params (carrying a `PhantomData`) when the cycle is
//!    generic, so the alias binds them. The non-generic path is unchanged.
//!  - #7: a foreign field whose LAST path segment equals a cycle type name (`super::other::Stmt`)
//!    used to be misdispatched as a cycle reference (E0308). Cycle membership now keys on the FIRST
//!    path segment (via `Peeled::head_lead`), so it is correctly treated as a leaf.
#![allow(dead_code)]
// `visitor!()` fetching the `#[recurse]` `ast` module's metadata (with its foreign-typed `Foreign`
// leaf) makes rustc misattribute a spurious "unused import" to the `#[recurse]` line — the
// `use syan::visit::Ast;` is in fact used by `#[derive(Ast)]`. Cosmetic span-mapping artifact.
#![allow(unused_imports)]

use syan::parse::recurse;

// ── #6: generic cycle at limit = 1 (the previously-failing case) ─────────────────────────────────
#[recurse(limit = 1)]
mod generic_limit1 {
    use syan::nested::group::GroupBrace;
    use syan::parse::{Parse, Unparse};
    use syan::source::proc_macro2::literal::Integer;

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

// Non-generic cycle at limit = 1: the terminator stays the unit struct; must still compile.
#[recurse(limit = 1)]
mod nongeneric_limit1 {
    use syan::parse::{Parse, Unparse};
    use syan::source::proc_macro2::literal::Integer;

    #[derive(Parse, Unparse)]
    pub enum E {
        Lit(Integer),
        Nest(Box<E>),
    }
}

#[test]
fn bug6_generic_limit1_compiles() {
    use syan::source::proc_macro2::literal::Integer;
    // Naming the instantiated aliases is the regression check (they failed to *compile* before).
    let _e: generic_limit1::Expr<()> =
        generic_limit1::Expr::Lit(Integer { value: "1".to_string(), suffix: None });
    let _n: nongeneric_limit1::E =
        nongeneric_limit1::E::Lit(Integer { value: "2".to_string(), suffix: None });
}

// ── #7: a foreign field whose last segment collides with a cycle type name is a leaf ─────────────
mod other {
    pub struct Stmt;
}

#[recurse]
mod ast {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum Expr<S> {
        ToStmt(Box<Stmt<S>>),
        Foreign(super::other::Stmt), // unrelated leaf; last segment == cycle type name `Stmt`
        Lit(PhantomData<S>),
    }

    #[derive(Ast)]
    #[subast()]
    pub enum Stmt<S> {
        Back(Box<Expr<S>>),
        Nop(PhantomData<S>),
    }
}

mod v_ast {
    syan::visit::visitor!(crate::ast::Expr, crate::ast::Stmt);
}

#[test]
fn bug7_foreign_field_sharing_cycle_last_segment_is_a_leaf() {
    // Compilation of the `visitor!()` over the `#[recurse]` module above is the regression check (the
    // generated visitor used to mis-call `visit_stmt` on the foreign `super::other::Stmt`). The empty
    // impl relies on the trait's default method bodies.
    struct V;
    impl v_ast::Visit<()> for V {}
    let _ = V;
}

//! GAP (documented, not yet supported): a `visitor!(..)` directly over a `#[recurse]` cyclic type.
//!
//! `#[recurse]` now emits, under each cycle type's *original* name, a metadata macro carrying the
//! type's def + `#[subast]` **plus** a `@recurse { .. }` section (Phase 0 of bridging recurse into
//! `visitor!()`). So `crate::ast::Expr! { .. }` (the fetch the visitor emits) *does* resolve — but the
//! `visitor!()` consumer (`__visitor_build`) does not yet understand `@recurse`, so it rejects it with
//! `unknown section @recurse`. Consuming the recurse metadata (the depth-generic `visit_*<R>` + shared
//! `VisitRec`) is a later phase; until then, building a `visitor!()` over the cycle is still
//! unsupported (use `#[recurse(visit)]`).

use syan::parse::recurse;

#[recurse]
mod ast {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    pub enum Expr<S> {
        Stmt(Box<Stmt<S>>),
        Lit(PhantomData<S>),
    }

    #[derive(Ast)]
    pub enum Stmt<S> {
        Expr(Box<Expr<S>>),
        Nop(PhantomData<S>),
    }
}

pub mod visit {
    syan::visit::visitor!(crate::ast::Expr, crate::ast::Stmt);
}

fn main() {}

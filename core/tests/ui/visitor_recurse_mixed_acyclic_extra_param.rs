// `visitor!()` over a `#[recurse]` cycle, MIXED with an acyclic type (`Program`) carrying a generic
// param (`T`) that no cycle root has. The depth-generic `VisitRec` impls are keyed on the roots'
// params only, so `T` would be an unconstrained impl param (E0207) — rejected with a clear message.
//
// Pure-recurse heterogeneity (a *cycle* type with extra params, e.g. `Stmt<S, T>` alongside the root
// `Expr<S>`) IS supported — see `visitor_recurse_heterogeneous.rs`. Only an acyclic type mixed into
// the same `visitor!()` with an extra param is walled.

use core::marker::PhantomData;
use syan::parse::recurse;

#[recurse]
mod ast {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum Expr<S> {
        Nest(Box<Expr<S>>),
        Lit(PhantomData<S>),
    }
}

// Acyclic outer type with an EXTRA param `T` beyond the cycle root's `S`.
#[derive(syan::visit::Ast)]
#[subast(crate::ast::Expr)]
pub struct Program<S, T> {
    pub body: ast::Expr<S>,
    pub tag: PhantomData<T>,
}

mod v {
    syan::visit::visitor!(crate::Program, crate::ast::Expr);
}

fn main() {}

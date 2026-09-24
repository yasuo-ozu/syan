// A `#[subast(..)]` path must be fully qualified — rooted at `crate`, an external crate, or a leading
// `::` — whatever it names.
//
// The path has two jobs. As a *match key* only its last segment is compared, so the root is irrelevant
// there. As a *fetch target*, for a type `visitor!(..)` does not list, it is invoked as a macro to pull
// that type's definition — in the visitor's scope, where a bare or `self::`/`super::`-relative path
// means something else.
//
// The rule is unconditional rather than applied only to the entries that are fetched: which ones those
// are depends on the `visitor!(..)` list, so a conditional rule would accept an attribute today and
// reject it when someone stops listing that type, and would report at the `visitor!` invocation instead
// of at the attribute.
use core::marker::PhantomData;
use syan::visit::Ast;

#[derive(Ast)]
pub enum Stmt<S> {
    Nop(PhantomData<S>),
}

#[derive(Ast)]
#[subast(Stmt)] // not fully qualified — should be `crate::Stmt`
pub struct Expr<S> {
    pub stmt: Stmt<S>,
}

fn main() {}

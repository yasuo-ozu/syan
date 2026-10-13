// A `#[visiting(..)]` path must be fully qualified — rooted at `crate`, an external crate, or a leading
// `::` — whatever it names.
//
// The path has two jobs. As a *match key* only its last segment is compared, so the root is irrelevant
// there. As a *fetch target*, for a type `define_visit!(..)` does not list, it is invoked as a macro to pull
// that type's definition — in the visitor's scope, where a bare or `self::`/`super::`-relative path
// means something else.
//
// The rule is unconditional rather than applied only to the entries that are fetched: which ones those
// are depends on the `define_visit!(..)` list, so a conditional rule would accept an attribute today and
// reject it when someone stops listing that type, and would report at the `define_visit!` invocation instead
// of at the attribute.
use core::marker::PhantomData;
use syan::visit::Visiting;

#[derive(Visiting)]
pub enum Stmt<S> {
    Nop(PhantomData<S>),
}

#[derive(Visiting)]
#[visiting(Stmt)] // not fully qualified — should be `crate::Stmt`
pub struct Expr<S> {
    pub stmt: Stmt<S>,
}

fn main() {}

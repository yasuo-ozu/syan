// A `#[subast(..)]` path must be fully qualified (rooted at `crate`, an external crate, or a leading
// `::`). A bare single-segment ident is relative — it would resolve in the consumer's scope when the
// metadata macro is expanded — so it is rejected at the derive with a clear, actionable message instead
// of surfacing far away as a cryptic "cannot find macro/type" error. (`self::`/`super::`-relative paths
// are rejected the same way.)
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

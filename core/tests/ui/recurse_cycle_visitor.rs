//! GAP (documented, not yet supported): a `visitor!(..)` directly over a `#[recurse]` cyclic type.
//!
//! `#[recurse]` renames the cyclic `Expr`/`Stmt` to internal `__ExprRec`/`__StmtRec` and exposes
//! `Expr`/`Stmt` only as *type aliases*. `#[derive(Ast)]`'s metadata macro is therefore re-exported
//! under the internal name, not the alias — so `crate::ast::Expr! { .. }` (the fetch the visitor
//! emits) finds no macro. Even if that were bridged, the cycle's back-edges are rewritten to the
//! generic `__Rec` param and so are not name-resolvable traversal edges. Building a visitor over the
//! cycle is future work.

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

//! `#[derive(Ast)]` coexists with `#[recurse]` on the same (mutually recursive) types.
//!
//! `#[recurse]` (a module attribute) expands first: it renames the cyclic types (e.g. `Expr` ->
//! `__ExprRec`), threads a depth parameter, and emits depth-limited public aliases. The
//! `#[derive(Ast)]` on each type then applies to the *renamed* type, so the marker `Ast` impl is in
//! effect for the public alias too.
#![allow(dead_code)]

use syan::parse::recurse;
use syan::visit::Ast;

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

use ast::{Expr, Stmt};

fn assert_is_ast<T: Ast>() {}

#[test]
fn ast_marker_holds_for_recurse_aliases() {
    // `Expr<()>` is the depth-limited alias of the renamed internal type; the `Ast` impl carries
    // through to it.
    assert_is_ast::<Expr<()>>();
    assert_is_ast::<Stmt<()>>();
}

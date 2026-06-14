//! Cross-crate: the AST types, their `#[derive(Ast)]`, and the visitor all live in the `syan_rust`
//! library. A downstream crate (this test) just calls the inherent `.visit()` — no trait import,
//! no `#[visitor]` here.

use syan_rust::ast::{Expr, Stmt};

#[test]
fn inherent_visit_is_callable_downstream() {
    let ast: Expr<()> = Expr::Stmt(Box::new(Stmt::Expr(Box::new(Expr::Lit(
        core::marker::PhantomData,
    )))));

    let mut exprs = 0usize;
    let mut stmts = 0usize;
    ast.visit((
        |_e: &Expr<()>| exprs += 1,
        |_s: &Stmt<()>| stmts += 1,
    ));
    assert_eq!(exprs, 2);
    assert_eq!(stmts, 1);
}

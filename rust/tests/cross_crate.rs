//! Stage 11 + portability: a visitor generated in *this* crate over AST types whose
//! `#[derive(Ast)]` lives in the `syan_rust` library crate.
//!
//! The AST types are intentionally **not** imported at module scope — the generated `visit` module
//! must name them by the full path given to `#[visitor(...)]`, with no `use` needed.

use syan::visit::visitor;

#[visitor(syan_rust::ast::Expr, syan_rust::ast::Stmt)]
pub mod visit {}

use visit::Visitable;

#[test]
fn visitor_works_across_crates() {
    // Imported locally (only to build the value); the generated module above does not see this.
    use syan_rust::ast::{Expr, Stmt};

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

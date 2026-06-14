//! Stage 11: a visitor generated in *this* crate over AST types whose `#[derive(Ast)]` (and their
//! metadata macros) live in the `syan_rust` library crate — exercising the cross-crate path.

use syan::visit::visitor;
// Bring the AST types into scope so the generated module (`use super::*`) can name them.
use syan_rust::ast::{Expr, Stmt};

// The metadata macros are reached by their full cross-crate path.
#[visitor(syan_rust::ast::Expr, syan_rust::ast::Stmt)]
pub mod visit {}

use visit::Visitable;

#[test]
fn visitor_works_across_crates() {
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

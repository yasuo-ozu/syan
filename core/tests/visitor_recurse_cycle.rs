//! `#[recurse(visit)]` generates a depth-generic visitor over a recurse'd cycle.
//!
//! `#[recurse]` rewrites the cycle's back-edges to a generic `__Rec` param and each nesting level
//! into a distinct type, so the generated visitor is *depth-generic*: its `visit_*` methods take a
//! depth parameter `R`, and a `VisitRec` dispatch trait (implemented by the root's depth chain and
//! the terminator) turns the depth recursion into trait calls. This is a trait-based visitor (no
//! closures — a closure can't be generic over the depth).
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::parse::recurse;

#[recurse(visit)]
mod ast {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum Expr<S> {
        Stmt(Box<Stmt<S>>),
        Lit(PhantomData<S>),
    }

    #[derive(Ast)]
    #[subast()]
    pub enum Stmt<S> {
        Expr(Box<Expr<S>>),
        Nop(PhantomData<S>),
    }
}

#[derive(Default)]
struct Counter {
    exprs: usize,
    stmts: usize,
}

impl<S> ast::Visit<S> for Counter {
    fn visit_expr<R: ast::VisitRec<S, Self>>(&mut self, i: &ast::ExprNode<S, R>) {
        self.exprs += 1;
        ast::visit_expr(self, i);
    }
    fn visit_stmt<R: ast::VisitRec<S, Self>>(&mut self, i: &ast::StmtNode<S, R>) {
        self.stmts += 1;
        ast::visit_stmt(self, i);
    }
}

#[test]
fn visits_root_only() {
    let e: ast::Expr<()> = ast::Expr::Lit(PhantomData);
    let mut c = Counter::default();
    ast::Visit::visit_expr(&mut c, &e);
    assert_eq!((c.exprs, c.stmts), (1, 0));
}

#[test]
fn visits_cross_edge() {
    // Expr -> Stmt is a cross-edge (no depth decrement), so this is built directly.
    let e: ast::Expr<()> = ast::Expr::Stmt(Box::new(ast::Stmt::Nop(PhantomData)));
    let mut c = Counter::default();
    ast::Visit::visit_expr(&mut c, &e);
    assert_eq!((c.exprs, c.stmts), (1, 1), "Expr drilled into its Stmt child");
}

#[test]
fn visits_back_edge_through_depth() {
    // Expr -> Stmt -> Expr: the Stmt -> Expr edge is the back-edge to the root (a `__Rec` field),
    // dispatched via VisitRec. The inner Expr lives one depth level shallower.
    let e: ast::Expr<()> =
        ast::Expr::Stmt(Box::new(ast::Stmt::Expr(Box::new(ast::ExprNode::Lit(PhantomData)))));
    let mut c = Counter::default();
    ast::Visit::visit_expr(&mut c, &e);
    assert_eq!(
        (c.exprs, c.stmts),
        (2, 1),
        "outer Expr + Stmt + inner Expr (reached via the VisitRec back-edge)"
    );
}

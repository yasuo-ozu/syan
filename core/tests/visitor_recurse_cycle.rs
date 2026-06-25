//! A depth-generic visitor over a recurse'd cycle: the cycle uses `#[recurse]` and the visitor is
//! built by a sibling `visitor!()`.
//!
//! `#[recurse]` rewrites the cycle's back-edges to a generic `__Rec` param and each nesting level
//! into a distinct type, so the generated visitor is *depth-generic*: its `visit_*` methods take a
//! depth parameter `R`, and a `VisitRec` dispatch trait (implemented by the root's depth chain and
//! the terminator) turns the depth recursion into trait calls. This is a trait-based visitor (no
//! closures — a closure can't be generic over the depth).
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::parse::recurse;

#[recurse]
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

mod v_ast {
    syan::visit::visitor!(crate::ast::Expr, crate::ast::Stmt);
}

#[derive(Default)]
struct Counter {
    exprs: usize,
    stmts: usize,
}

impl<S> v_ast::Visit<S> for Counter {
    fn visit_expr<R: v_ast::VisitRec<S, Self>>(&mut self, i: &v_ast::ExprNode<S, R>) {
        self.exprs += 1;
        v_ast::visit_expr(self, i);
    }
    fn visit_stmt<R: v_ast::VisitRec<S, Self>>(&mut self, i: &v_ast::StmtNode<S, R>) {
        self.stmts += 1;
        v_ast::visit_stmt(self, i);
    }
}

#[test]
fn visits_root_only() {
    let e: ast::Expr<()> = ast::Expr::Lit(PhantomData);
    let mut c = Counter::default();
    v_ast::Visit::visit_expr(&mut c, &e);
    assert_eq!((c.exprs, c.stmts), (1, 0));
}

#[test]
fn visits_cross_edge() {
    // Expr -> Stmt is a cross-edge (no depth decrement), so this is built directly.
    let e: ast::Expr<()> = ast::Expr::Stmt(Box::new(ast::Stmt::Nop(PhantomData)));
    let mut c = Counter::default();
    v_ast::Visit::visit_expr(&mut c, &e);
    assert_eq!((c.exprs, c.stmts), (1, 1), "Expr drilled into its Stmt child");
}

#[test]
fn visits_back_edge_through_depth() {
    // Expr -> Stmt -> Expr: the Stmt -> Expr edge is the back-edge to the root (a `__Rec` field),
    // dispatched via VisitRec. The inner Expr lives one depth level shallower.
    let e: ast::Expr<()> =
        ast::Expr::Stmt(Box::new(ast::Stmt::Expr(Box::new(v_ast::ExprNode::Lit(PhantomData)))));
    let mut c = Counter::default();
    v_ast::Visit::visit_expr(&mut c, &e);
    assert_eq!(
        (c.exprs, c.stmts),
        (2, 1),
        "outer Expr + Stmt + inner Expr (reached via the VisitRec back-edge)"
    );
}

// A single self-recursive root (no other cycle type): both `Add` operands are root back-edges.
#[recurse]
mod tree {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum Expr<S> {
        Add(Box<Expr<S>>, Box<Expr<S>>),
        Lit(PhantomData<S>),
    }
}

mod v_tree {
    syan::visit::visitor!(crate::tree::Expr);
}

#[derive(Default)]
struct Nodes(usize);

impl<S> v_tree::Visit<S> for Nodes {
    fn visit_expr<R: v_tree::VisitRec<S, Self>>(&mut self, i: &v_tree::ExprNode<S, R>) {
        self.0 += 1;
        v_tree::visit_expr(self, i);
    }
}

#[test]
fn visits_self_recursive_root() {
    let e: tree::Expr<()> = tree::Expr::Add(
        Box::new(v_tree::ExprNode::Lit(PhantomData)),
        Box::new(v_tree::ExprNode::Lit(PhantomData)),
    );
    let mut n = Nodes::default();
    v_tree::Visit::visit_expr(&mut n, &e);
    assert_eq!(n.0, 3, "the Add node + its two operands (both back-edges to the root)");
}

//! Stage 3: `#[visitor(..)]` generates a working visitor over visited types, recursing through
//! `Box`. Struct visitors plug in via the `IntoVisitor<_, ()>` identity impl.

use core::marker::PhantomData;
use syan::visit::{visitor, Ast};

#[derive(Debug, Ast)]
pub enum Expr<S> {
    Stmt(Box<Stmt<S>>),
    Other(PhantomData<S>),
}

#[derive(Debug, Ast)]
pub enum Stmt<S> {
    Expr(Box<Expr<S>>),
    Other(PhantomData<S>),
}

#[visitor(Expr, Stmt)]
pub mod visit {}

fn sample() -> Expr<()> {
    Expr::Stmt(Box::new(Stmt::Expr(Box::new(Expr::Other(PhantomData)))))
}

#[derive(Default)]
struct Counter {
    exprs: usize,
    stmts: usize,
}

impl<S> visit::Visit<S> for Counter {
    fn visit_expr(&mut self, i: &Expr<S>) {
        self.exprs += 1;
        visit::visit_expr(self, i);
    }
    fn visit_stmt(&mut self, i: &Stmt<S>) {
        self.stmts += 1;
        visit::visit_stmt(self, i);
    }
}

#[test]
fn struct_visitor_counts_nodes() {
    let ast = sample();
    let mut counter = Counter::default();
    // `&mut Counter: Visit` via the forwarding impl, so we can read results after the traversal.
    ast.visit(&mut counter);
    assert_eq!(counter.exprs, 2, "outer Expr::Stmt + inner Expr::Other");
    assert_eq!(counter.stmts, 1, "the single Stmt::Expr");
}

#[test]
fn single_closure_visitor() {
    let mut exprs = 0usize;
    sample().visit(|_e: &Expr<()>| exprs += 1);
    assert_eq!(exprs, 2);

    let mut stmts = 0usize;
    sample().visit(|_s: &Stmt<()>| stmts += 1);
    assert_eq!(stmts, 1);
}

#[test]
fn tuple_of_closures_single_traversal() {
    let mut exprs = 0usize;
    let mut stmts = 0usize;
    // Both closures fire in ONE traversal, each at its own node type, any order.
    sample().visit((
        |_s: &Stmt<()>| stmts += 1,
        |_e: &Expr<()>| exprs += 1,
    ));
    assert_eq!(exprs, 2);
    assert_eq!(stmts, 1);
}

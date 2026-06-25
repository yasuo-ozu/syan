//! The previously-rejected gap, now unblocked: `visitor!()` directly over a `#[recurse]` cycle.
//! `#[recurse]` emits `@recurse` metadata (it no longer needs `visit`); `visitor!()` consumes it and
//! generates the depth-generic visitor (`Visit`/`VisitRec`/`visit_*<R>` keyed on ITS own trait), so a
//! `Visit` impl + `Visit::visit_*` walks the cycle. `visit_<X>` exists only for listed `X`.
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::parse::recurse;

#[recurse]
mod ast {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast(crate::ast::Stmt)]
    pub enum Expr<S> {
        Stmt(Box<Stmt<S>>), // cross-edge to the (listed) Stmt
        Lit(PhantomData<S>),
    }

    #[derive(Ast)]
    #[subast(crate::ast::Expr)]
    pub enum Stmt<S> {
        Expr(Box<Expr<S>>), // back-edge to the root Expr → drives via the depth param
        Nop(PhantomData<S>),
    }
}

mod v {
    syan::visit::visitor!(crate::ast::Expr, crate::ast::Stmt);
}

#[derive(Default)]
struct Counter {
    e: usize,
    s: usize,
}

impl<S> v::Visit<S> for Counter {
    fn visit_expr<R: v::VisitRec<S, Self>>(&mut self, i: &v::ExprNode<S, R>) {
        self.e += 1;
        v::visit_expr(self, i);
    }
    fn visit_stmt<R: v::VisitRec<S, Self>>(&mut self, i: &v::StmtNode<S, R>) {
        self.s += 1;
        v::visit_stmt(self, i);
    }
}

#[test]
fn walks_the_cycle() {
    // Expr -> Stmt (cross-edge) -> Expr (back-edge) -> Lit.
    let e: ast::Expr<()> = ast::Expr::Stmt(Box::new(v::StmtNode::Expr(Box::new(
        v::ExprNode::Lit(PhantomData),
    ))));
    let mut c = Counter::default();
    v::Visit::visit_expr(&mut c, &e);
    assert_eq!(
        (c.e, c.s),
        (2, 1),
        "outer Expr + inner Expr (via back-edge) = 2; one Stmt"
    );
}

#[test]
fn leaf_only() {
    let e: ast::Expr<()> = ast::Expr::Lit(PhantomData);
    let mut c = Counter::default();
    v::Visit::visit_expr(&mut c, &e);
    assert_eq!((c.e, c.s), (1, 0));
}

// The visitor also generates the mutable mirror: VisitMut / VisitRecMut / visit_*_mut + inherent
// .visit_mut(), all depth-generic over the recurse cycle.
impl<S> v::VisitMut<S> for Counter {
    fn visit_expr_mut<R: v::VisitRecMut<S, Self>>(&mut self, i: &mut v::ExprNode<S, R>) {
        self.e += 1;
        v::visit_expr_mut(self, i);
    }
    fn visit_stmt_mut<R: v::VisitRecMut<S, Self>>(&mut self, i: &mut v::StmtNode<S, R>) {
        self.s += 1;
        v::visit_stmt_mut(self, i);
    }
}

#[test]
fn walks_the_cycle_mut() {
    let mut e: ast::Expr<()> = ast::Expr::Stmt(Box::new(v::StmtNode::Expr(Box::new(
        v::ExprNode::Lit(PhantomData),
    ))));
    let mut c = Counter::default();
    e.visit_mut(&mut c); // inherent .visit_mut(), depth-generic mutable traversal
    assert_eq!((c.e, c.s), (2, 1), "same shape as the shared walk, via &mut");
}

//! `visitor!()` directly over a `#[recurse]` cycle. `#[recurse]` exposes the cycle as *natural*
//! recursive types (a single `Expr<S>` at every depth), so `visitor!()` generates an **ordinary
//! acyclic visitor** — `visit_*` methods take the natural type (no depth parameter), and a `Visit`
//! impl + `Visit::visit_*` walks the cycle. `visit_<X>` exists only for listed `X`.
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
        Expr(Box<Expr<S>>), // back-reference to the root Expr
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
    fn visit_expr(&mut self, i: &ast::Expr<S>) {
        self.e += 1;
        v::visit_expr(self, i);
    }
    fn visit_stmt(&mut self, i: &ast::Stmt<S>) {
        self.s += 1;
        v::visit_stmt(self, i);
    }
}

#[test]
fn walks_the_cycle() {
    // Expr -> Stmt (cross-edge) -> Expr (back-edge) -> Lit.
    let e: ast::Expr<()> = ast::Expr::Stmt(Box::new(ast::Stmt::Expr(Box::new(
        ast::Expr::Lit(PhantomData),
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

// The visitor also generates the mutable mirror: VisitMut / visit_*_mut + inherent .visit_mut(),
// over the natural acyclic types.
impl<S> v::VisitMut<S> for Counter {
    fn visit_expr_mut(&mut self, i: &mut ast::Expr<S>) {
        self.e += 1;
        v::visit_expr_mut(self, i);
    }
    fn visit_stmt_mut(&mut self, i: &mut ast::Stmt<S>) {
        self.s += 1;
        v::visit_stmt_mut(self, i);
    }
}

#[test]
fn walks_the_cycle_mut() {
    let mut e: ast::Expr<()> = ast::Expr::Stmt(Box::new(ast::Stmt::Expr(Box::new(
        ast::Expr::Lit(PhantomData),
    ))));
    let mut c = Counter::default();
    e.visit_mut(&mut c); // inherent .visit_mut(), natural mutable traversal
    assert_eq!((c.e, c.s), (2, 1), "same shape as the shared walk, via &mut");
}

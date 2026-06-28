//! A visitor over a `#[recurse]`'d cycle. With the natural-type design, `#[recurse]` exposes the cycle
//! as *natural* recursive types (a single `Expr<S>` at every depth, backed by an internal depth-limited
//! engine for `Parse`), so the visitor is an **ordinary acyclic visitor** — `visit_*` methods take the
//! natural type (no depth parameter), and **closures work** (the long-deferred gap is closed).
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
        Stmt(Box<Stmt<S>>),
        Lit(PhantomData<S>),
    }

    #[derive(Ast)]
    #[subast(crate::ast::Expr)]
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
    fn visit_expr(&mut self, i: &ast::Expr<S>) {
        self.exprs += 1;
        v_ast::visit_expr(self, i);
    }
    fn visit_stmt(&mut self, i: &ast::Stmt<S>) {
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
    let e: ast::Expr<()> = ast::Expr::Stmt(Box::new(ast::Stmt::Nop(PhantomData)));
    let mut c = Counter::default();
    v_ast::Visit::visit_expr(&mut c, &e);
    assert_eq!((c.exprs, c.stmts), (1, 1), "Expr drilled into its Stmt child");
}

#[test]
fn visits_back_edge_arbitrary_depth() {
    // Expr -> Stmt -> Expr: a *natural* tree, no depth limit on traversal.
    let e: ast::Expr<()> =
        ast::Expr::Stmt(Box::new(ast::Stmt::Expr(Box::new(ast::Expr::Lit(PhantomData)))));
    let mut c = Counter::default();
    v_ast::Visit::visit_expr(&mut c, &e);
    assert_eq!((c.exprs, c.stmts), (2, 1), "outer Expr + Stmt + inner Expr");
}

#[test]
fn closure_over_recurse_cycle() {
    // The payoff: a *closure* visitor over a former-`#[recurse]` cycle (impossible under the old
    // depth-generic design). The inherent `.visit(closure)` counts every Expr node.
    let e: ast::Expr<()> =
        ast::Expr::Stmt(Box::new(ast::Stmt::Expr(Box::new(ast::Expr::Lit(PhantomData)))));
    let mut exprs = 0usize;
    e.visit(|_e: &ast::Expr<()>| exprs += 1);
    assert_eq!(exprs, 2, "both Expr nodes seen by the closure");
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
    fn visit_expr(&mut self, i: &tree::Expr<S>) {
        self.0 += 1;
        v_tree::visit_expr(self, i);
    }
}

#[test]
fn visits_self_recursive_root() {
    let e: tree::Expr<()> = tree::Expr::Add(
        Box::new(tree::Expr::Lit(PhantomData)),
        Box::new(tree::Expr::Lit(PhantomData)),
    );
    let mut n = Nodes::default();
    v_tree::Visit::visit_expr(&mut n, &e);
    assert_eq!(n.0, 3, "the Add node + its two operands");
}

#[test]
fn closure_over_self_recursive_root() {
    let e: tree::Expr<()> = tree::Expr::Add(
        Box::new(tree::Expr::Lit(PhantomData)),
        Box::new(tree::Expr::Lit(PhantomData)),
    );
    let mut n = 0usize;
    e.visit(|_e: &tree::Expr<()>| n += 1);
    assert_eq!(n, 3);
}

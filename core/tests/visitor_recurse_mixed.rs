//! Phase 1b: ONE `visitor!()` spanning an acyclic outer type (`Program`) AND a `#[recurse]` cycle
//! (`Expr`/`Stmt`). A single `Visit` impl with a fixed `visit_program` and depth-generic
//! `visit_expr`/`visit_stmt`, and one `.visit()` that crosses the boundary automatically — replacing
//! the manual two-trait hand-off of `visitor_mixed_recurse.rs`.
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::parse::recurse;
use syan::visit::Ast;

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

// Acyclic outer type with fields into the recurse cycle (a Vec and a bare one).
#[derive(Ast)]
#[subast(crate::ast::Expr)]
pub struct Program<S> {
    pub body: Vec<ast::Expr<S>>,
    pub tail: ast::Expr<S>,
}

mod v {
    syan::visit::visitor!(crate::Program, crate::ast::Expr, crate::ast::Stmt);
}

#[derive(Default)]
struct Counter {
    p: usize,
    e: usize,
    s: usize,
}

impl<S> v::Visit<S> for Counter {
    fn visit_program(&mut self, i: &Program<S>) {
        self.p += 1;
        v::visit_program(self, i); // drills body/tail → crosses into the Expr cycle
    }
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
fn one_visit_spans_outer_and_inner() {
    let prog: Program<()> = Program {
        // Expr -> Stmt -> Expr(back-edge) -> Lit
        body: vec![ast::Expr::Stmt(Box::new(v::StmtNode::Expr(Box::new(
            v::ExprNode::Lit(PhantomData),
        ))))],
        tail: ast::Expr::Lit(PhantomData),
    };
    let mut c = Counter::default();
    prog.visit(&mut c); // single unified entry point, crosses the boundary automatically
    assert_eq!(c.p, 1, "the one Program");
    assert_eq!(c.e, 3, "body Expr + its inner Expr (back-edge) + tail Expr");
    assert_eq!(c.s, 1, "the one Stmt");
}

//! A complex tree spanning both visitor worlds: acyclic AST types **outside** a `#[recurse]` module
//! (walked by the normal `visitor!()` with drill-in) whose tree contains a node from a cyclic AST
//! **inside** a `#[recurse(visit)]` module (walked by the depth-generic recurse visitor). One
//! `Walker` implements both visitor traits and crosses the boundary, so a single `.visit(&mut w)`
//! walks the whole tree: outside `Func -> Param -> Type` (drilled) plus the inside `Expr/Stmt` cycle
//! hanging off `Func::body`.
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::parse::recurse;
use syan::visit::Ast;

// ── inside: a recurse'd cycle, visited depth-generically ─────────────────────────────────────────
#[recurse(visit)]
mod rec {
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

// ── outside: acyclic types, visited by the normal visitor with drill-in ──────────────────────────
#[derive(Ast)]
pub enum Type<S> {
    Int(PhantomData<S>),
    Bool(PhantomData<S>),
}

// Drilled-through intermediate (unlisted): Param -> Type.
#[derive(Ast)]
#[subast(crate::Type)]
pub struct Param<S> {
    pub ty: Type<S>,
}

#[derive(Ast)]
#[subast(crate::Param, crate::Type)]
pub struct Func<S> {
    pub params: Vec<Param<S>>,
    pub ret: Type<S>,
    // A node from the inside-recurse cycle. The normal visitor can't follow it (it's not in
    // `#[subast]`), so it is a leaf here; the `Walker` descends into it via the recurse visitor.
    pub body: rec::Expr<S>,
}

pub mod nv {
    syan::visit::visitor!(crate::Func, crate::Type);
}

#[derive(Default)]
struct Walker {
    types: usize,
    exprs: usize,
    stmts: usize,
}

// Outside side: drill Func -> Param -> Type / ret; at Func, cross into the recurse visitor for body.
impl<S> nv::Visit<S> for Walker {
    fn visit_func(&mut self, i: &Func<S>) {
        rec::Visit::visit_expr(self, &i.body); // boundary crossing into the recurse cycle
        nv::visit_func(self, i); // drill into params / ret (Types)
    }
    fn visit_type(&mut self, i: &Type<S>) {
        self.types += 1;
        nv::visit_type(self, i);
    }
}

// Inside side: depth-generic visitor over the recurse cycle.
impl<S> rec::Visit<S> for Walker {
    fn visit_expr<R: rec::VisitRec<S, Self>>(&mut self, i: &rec::ExprNode<S, R>) {
        self.exprs += 1;
        rec::visit_expr(self, i);
    }
    fn visit_stmt<R: rec::VisitRec<S, Self>>(&mut self, i: &rec::StmtNode<S, R>) {
        self.stmts += 1;
        rec::visit_stmt(self, i);
    }
}

fn sample() -> Func<()> {
    Func {
        params: vec![
            Param { ty: Type::Int(PhantomData) },
            Param { ty: Type::Bool(PhantomData) },
        ],
        ret: Type::Int(PhantomData),
        // Expr -> Stmt -> Expr: exercises a cross-edge and the recurse back-edge.
        body: rec::Expr::Stmt(Box::new(rec::Stmt::Expr(Box::new(rec::ExprNode::Lit(PhantomData))))),
    }
}

#[test]
fn one_walk_spans_outside_and_inside_recurse() {
    let func = sample();
    let mut w = Walker::default();
    func.visit(&mut w);
    assert_eq!(w.types, 3, "two param Types (drilled through Param) + the ret Type");
    assert_eq!(w.exprs, 2, "body Expr + the inner Expr reached via the recurse back-edge");
    assert_eq!(w.stmts, 1, "the single Stmt in body");
}

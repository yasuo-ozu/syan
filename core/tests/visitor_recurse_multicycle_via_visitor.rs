//! Phase 2a: ONE `visitor!()` listing recurse types from TWO independent cycles (`Expr` and `Type`,
//! disjoint self-referential cycles in the same `#[recurse]` module). Each cycle keeps its own depth
//! dimension; the unified `Visit` trait carries a depth-generic `visit_*` for each, and a single
//! `VisitRec` dispatch serves both cycles' nodes/terminators.
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
        Nest(Box<Expr<S>>),
        Lit(PhantomData<S>),
    }

    #[derive(Ast)]
    #[subast()]
    pub enum Type<S> {
        Arrow(Box<Type<S>>),
        Unit(PhantomData<S>),
    }
}

mod v {
    syan::visit::visitor!(crate::ast::Expr, crate::ast::Type);
}

#[derive(Default)]
struct C {
    e: usize,
    t: usize,
}

impl<S> v::Visit<S> for C {
    fn visit_expr<R: v::VisitRec<S, Self>>(&mut self, i: &v::ExprNode<S, R>) {
        self.e += 1;
        v::visit_expr(self, i);
    }
    fn visit_type<R: v::VisitRec<S, Self>>(&mut self, i: &v::TypeNode<S, R>) {
        self.t += 1;
        v::visit_type(self, i);
    }
}

#[test]
fn two_independent_cycles_one_visitor() {
    let e: ast::Expr<()> = ast::Expr::Nest(Box::new(v::ExprNode::Lit(PhantomData)));
    let t: ast::Type<()> = ast::Type::Arrow(Box::new(v::TypeNode::Unit(PhantomData)));
    let mut c = C::default();
    e.visit(&mut c);
    t.visit(&mut c);
    assert_eq!((c.e, c.t), (2, 2), "each cycle traversed to depth 2, independently");
}

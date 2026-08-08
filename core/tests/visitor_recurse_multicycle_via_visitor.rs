//! Phase 2a: ONE `visitor!()` listing recurse types from TWO independent cycles (`Expr` and `Type`,
//! disjoint self-referential cycles in the same `#[recurse]` module). With natural types it is a
//! unified ordinary acyclic `Visit` trait carrying a `visit_*` for each, traversing both cycles
//! unbounded.
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
    fn visit_expr(&mut self, i: &ast::Expr<S>) {
        self.e += 1;
        v::visit_expr(self, i);
    }
    fn visit_type(&mut self, i: &ast::Type<S>) {
        self.t += 1;
        v::visit_type(self, i);
    }
}

#[test]
fn two_independent_cycles_one_visitor() {
    let e: ast::Expr<()> = ast::Expr::Nest(Box::new(ast::Expr::Lit(PhantomData)));
    let t: ast::Type<()> = ast::Type::Arrow(Box::new(ast::Type::Unit(PhantomData)));
    let mut c = C::default();
    e.visit(&mut c);
    t.visit(&mut c);
    assert_eq!((c.e, c.t), (2, 2), "each cycle traversed (2 nodes each), independently");
}

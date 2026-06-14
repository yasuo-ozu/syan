//! Stage 10: `#[visitor(base => New)]` inherits a base visitor's methods (supertrait) and adds
//! methods for the new types. Works for a one-directional reference DAG (new -> base).

use core::marker::PhantomData;
use syan::visit::Ast;

#[derive(Ast)]
pub enum Type<S> {
    Unit(PhantomData<S>),
}

#[derive(Ast)]
pub enum Expr<S> {
    Typed(Box<Type<S>>),
    Lit(PhantomData<S>),
}

#[derive(Ast)]
pub enum Stmt<S> {
    E(Box<Expr<S>>),
    Empty(PhantomData<S>),
}

pub mod base {
    syan::visit::visitor!(super::Type, super::Expr);
}

pub mod ext {
    syan::visit::visitor!(super::base => super::Stmt);
}

#[derive(Default)]
struct Counter {
    types: u32,
    exprs: u32,
    stmts: u32,
}

impl<S> base::Visit<S> for Counter {
    fn visit_type(&mut self, i: &Type<S>) {
        self.types += 1;
        base::visit_type(self, i);
    }
    fn visit_expr(&mut self, i: &Expr<S>) {
        self.exprs += 1;
        base::visit_expr(self, i);
    }
}

impl<S> ext::Visit<S> for Counter {
    fn visit_stmt(&mut self, i: &Stmt<S>) {
        self.stmts += 1;
        ext::visit_stmt(self, i);
    }
}

#[test]
fn inheriting_visitor_descends_into_base_types() {
    let ast: Stmt<()> = Stmt::E(Box::new(Expr::Typed(Box::new(Type::Unit(PhantomData)))));
    let mut counter = Counter::default();
    ast.visit(&mut counter);
    assert_eq!(counter.stmts, 1);
    assert_eq!(counter.exprs, 1, "reached the inherited Expr method");
    assert_eq!(counter.types, 1, "reached the inherited Type method");
}

#[test]
fn inheriting_closure_targets_new_type() {
    let ast: Stmt<()> = Stmt::E(Box::new(Expr::Typed(Box::new(Type::Unit(PhantomData)))));
    let mut stmts = 0;
    ast.visit(|_s: &Stmt<()>| stmts += 1);
    assert_eq!(stmts, 1);
}

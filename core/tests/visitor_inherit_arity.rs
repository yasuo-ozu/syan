//! Inheritance where the extending visitor's generic union is *wider* than the base's: base is
//! `Visit<S>` (over `Type<S>`/`Expr<S>`), the extension adds `Stmt<S, T>` so its union is `<S, T>`.
//! The new trait must reference the base supertrait with the *base's* arity (`base::Visit<S>`), not
//! the new union (`base::Visit<S, T>` would be `E0107`). See the base-generics protocol in
//! `__syan_visited` / `@bg`.
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::visit::Ast;

#[derive(Ast)]
pub enum Type<S> {
    Unit(PhantomData<S>),
}

#[derive(Ast)]
#[subast(crate::Type)]
pub enum Expr<S> {
    Typed(Box<Type<S>>),
    Lit(PhantomData<S>),
}

#[derive(Ast)]
#[subast(crate::Expr)]
pub enum Stmt<S, T> {
    E(Box<Expr<S>>),
    Tagged(PhantomData<T>),
}

pub mod base {
    syan::visit::visitor!(crate::Type, crate::Expr);
}

pub mod ext {
    syan::visit::visitor!(crate::base => crate::Stmt);
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

impl<S, T> ext::Visit<S, T> for Counter {
    fn visit_stmt(&mut self, i: &Stmt<S, T>) {
        self.stmts += 1;
        ext::visit_stmt(self, i);
    }
}

#[test]
fn inheriting_visitor_with_extra_generic_param() {
    let ast: Stmt<(), ()> = Stmt::E(Box::new(Expr::Typed(Box::new(Type::Unit(PhantomData)))));
    let mut counter = Counter::default();
    ast.visit(&mut counter);
    assert_eq!(counter.stmts, 1);
    assert_eq!(counter.exprs, 1, "reached the inherited Expr method");
    assert_eq!(counter.types, 1, "reached the inherited Type method");
}

#[test]
fn inheriting_closure_over_wider_arity() {
    let ast: Stmt<(), ()> = Stmt::Tagged(PhantomData);
    let mut stmts = 0usize;
    ast.visit(|_s: &Stmt<(), ()>| stmts += 1);
    assert_eq!(stmts, 1);
}

//! Multi-level inheritance: `base => mid => top`. Because `mid::Visit: base::Visit` and
//! `top::Visit: mid::Visit`, `top`'s generated `Driver` must satisfy *every* transitive supertrait
//! (`base::Visit` too), not just the direct parent. The ancestor chain is carried through
//! `__syan_visited` (`@an`) so `top` emits an empty `Driver` impl + `use` for each ancestor.
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
pub enum Stmt<S> {
    E(Box<Expr<S>>),
    Empty(PhantomData<S>),
}

#[derive(Ast)]
#[subast(crate::Stmt)]
pub enum Item<S> {
    S(Box<Stmt<S>>),
    Nil(PhantomData<S>),
}

pub mod base {
    syan::visit::visitor!(crate::Type, crate::Expr);
}
pub mod mid {
    syan::visit::visitor!(crate::base => crate::Stmt);
}
pub mod top {
    syan::visit::visitor!(crate::mid => crate::Item);
}

fn sample() -> Item<()> {
    Item::S(Box::new(Stmt::E(Box::new(Expr::Typed(Box::new(Type::Unit(PhantomData)))))))
}

#[derive(Default)]
struct Counter {
    types: u32,
    exprs: u32,
    stmts: u32,
    items: u32,
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
impl<S> mid::Visit<S> for Counter {
    fn visit_stmt(&mut self, i: &Stmt<S>) {
        self.stmts += 1;
        mid::visit_stmt(self, i);
    }
}
impl<S> top::Visit<S> for Counter {
    fn visit_item(&mut self, i: &Item<S>) {
        self.items += 1;
        top::visit_item(self, i);
    }
}

#[test]
fn three_level_struct_visitor_descends_through_all_ancestors() {
    let mut c = Counter::default();
    sample().visit(&mut c);
    assert_eq!(c.items, 1);
    assert_eq!(c.stmts, 1, "reached the direct parent (mid) method");
    assert_eq!(c.exprs, 1, "reached the grandparent (base) method");
    assert_eq!(c.types, 1, "reached the grandparent (base) method");
}

#[test]
fn three_level_closure_uses_transitive_driver() {
    // The closure path uses top::Driver, which must impl mid::Visit AND base::Visit.
    let mut items = 0usize;
    sample().visit(|_i: &Item<()>| items += 1);
    assert_eq!(items, 1);
}

// 3-level chain AND arity widening at the leaf: top2's union is <S, T> while mid/base are <S>. Each
// transitive ancestor impl must be quantified over only its own param (S), leaving T out.
#[derive(Ast)]
#[subast(crate::Stmt)]
pub enum Item2<S, T> {
    S(Box<Stmt<S>>),
    Tag(PhantomData<T>),
}

pub mod top2 {
    syan::visit::visitor!(crate::mid => crate::Item2);
}

impl<S, T> top2::Visit<S, T> for Counter {
    fn visit_item2(&mut self, i: &Item2<S, T>) {
        self.items += 1;
        top2::visit_item2(self, i);
    }
}

#[test]
fn three_level_with_arity_widening() {
    let ast: Item2<(), ()> =
        Item2::S(Box::new(Stmt::E(Box::new(Expr::Typed(Box::new(Type::Unit(PhantomData)))))));
    let mut c = Counter::default();
    ast.visit(&mut c);
    assert_eq!((c.items, c.stmts, c.exprs, c.types), (1, 1, 1, 1));
}

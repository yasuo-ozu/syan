//! Reaching a visitor's ancestors: the `__syan_base` relay, and `impl_chain!` for the empty
//! supertrait impls an extending visitor would otherwise spell out by hand.
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
    T(Box<Type<S>>),
}

#[derive(Ast)]
#[subast(crate::Expr)]
pub enum Stmt<S> {
    E(Box<Expr<S>>),
}

#[derive(Ast)]
#[subast(crate::Stmt)]
pub enum Item<S> {
    S(Box<Stmt<S>>),
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
    Item::S(Box::new(Stmt::E(Box::new(Expr::T(Box::new(Type::Unit(
        PhantomData,
    )))))))
}

// `impl_chain!` writes every ancestor impl, so a visitor that only overrides its own types needs
// no mention of the chain at all.
#[derive(Default)]
struct OnlyItems(u32);
top::impl_chain!(top; OnlyItems);
impl<S> top::Visit<S> for OnlyItems {
    fn visit_item(&mut self, i: &Item<S>) {
        self.0 += 1;
        top::visit_item(self, i);
    }
}

#[test]
fn impl_chain_fills_every_ancestor() {
    let mut p = OnlyItems::default();
    sample().visit(&mut p);
    assert_eq!(p.0, 1);
}

// Acting on an inherited node: hand the method to the macro and it lands in the impl of whichever
// ancestor declares it. Nothing names that ancestor, and the other impls are still written.
#[derive(Default)]
struct Mixed {
    types: u32,
}
top::impl_chain! { top; Mixed;
    // The argument type is left off: the macro knows it.
    fn visit_type(&mut self, i) {
        self.types += 1;
        base::visit_type(self, i);
    }
}
impl<S> top::Visit<S> for Mixed {}

// The same, written out in full — including a return type, which is accepted and passed through.
#[derive(Default)]
struct Spelled {
    types: u32,
}
top::impl_chain! { top; Spelled;
    fn visit_type(&mut self, i: &Type<S>) -> () {
        self.types += 1;
        base::visit_type(self, i);
    }
}
impl<S> top::Visit<S> for Spelled {}

#[test]
fn impl_chain_accepts_a_full_signature() {
    let mut s = Spelled::default();
    sample().visit(&mut s);
    assert_eq!(s.types, 1);
}

// `fn <name>;` hands that ancestor back, so its impls can be ordinary Rust.
#[derive(Default)]
struct HandBack {
    types: u32,
}
top::impl_chain! { top; HandBack; fn visit_type; }
impl<S> base::Visit<S> for HandBack {
    fn visit_type(&mut self, i: &Type<S>) {
        self.types += 1;
        base::visit_type(self, i);
    }
}
impl<S> base::VisitMut<S> for HandBack {}
impl<S> top::Visit<S> for HandBack {}

#[test]
fn impl_chain_hands_an_ancestor_back() {
    let mut h = HandBack::default();
    sample().visit(&mut h);
    assert_eq!(h.types, 1);
}

#[test]
fn impl_chain_routes_a_method_to_its_ancestor() {
    let mut m = Mixed::default();
    sample().visit(&mut m);
    assert_eq!(m.types, 1);
}

// An ancestor two links up, reached through the relay rather than by its own path. This is the
// spelling that keeps working when the ancestor's module cannot be named — see the cross-crate
// case in `visitor_ancestor_access_private`.
#[derive(Default)]
struct ViaRelay {
    types: u32,
}
impl<S> top::__syan_base::__syan_base::Visit<S> for ViaRelay {
    fn visit_type(&mut self, _i: &Type<S>) {
        self.types += 1;
    }
}
impl<S> top::__syan_base::__syan_base::VisitMut<S> for ViaRelay {}
top::impl_chain! { top; ViaRelay; fn visit_type; }
impl<S> top::Visit<S> for ViaRelay {}

#[test]
fn relay_names_a_transitive_ancestor() {
    let mut v = ViaRelay::default();
    sample().visit(&mut v);
    assert_eq!(v.types, 1);
}

// `impl_chain!` on a visitor with no ancestors expands to nothing, so generated code can call it
// without first asking whether the visitor inherits.
#[derive(Default)]
struct Root(u32);
base::impl_chain!(base; Root);
impl<S> base::Visit<S> for Root {
    fn visit_type(&mut self, _i: &Type<S>) {
        self.0 += 1;
    }
}

#[test]
fn impl_chain_on_a_root_visitor_is_a_no_op() {
    let mut r = Root::default();
    Expr::T(Box::new(Type::<()>::Unit(PhantomData))).visit(&mut r);
    assert_eq!(r.0, 1);
}

// A closure over an *inherited* type: the thing that needed a hand-written adapter before.
#[test]
fn closure_over_an_inherited_type() {
    let mut types = 0;
    sample().visit(|_: &Type<()>| types += 1);
    assert_eq!(types, 1);
}

#[test]
fn closure_over_a_type_two_levels_up() {
    let mut exprs = 0;
    sample().visit(|_: &Expr<()>| exprs += 1);
    assert_eq!(exprs, 1);
}

#[test]
fn tuple_of_closures_mixing_own_and_inherited() {
    let (mut items, mut types) = (0, 0);
    sample().visit((|_: &Item<()>| items += 1, |_: &Type<()>| types += 1));
    assert_eq!((items, types), (1, 1));
}

#[test]
fn closure_mut_over_an_inherited_type() {
    let mut types = 0;
    sample().visit_mut(|_: &mut Type<()>| types += 1);
    assert_eq!(types, 1);
}

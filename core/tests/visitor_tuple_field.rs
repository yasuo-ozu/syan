//! Regression for audit #3: a tuple-typed field is traversed, not silently skipped.
//! `visitor!()`'s `lower_field` peeled containers but had no tuple arm, so a `(Ty<S>, Ty<S>)` field
//! bound `_` and was never visited (no diagnostic). It now destructures the tuple and lowers each
//! element (mirroring the `#[recurse]` path).
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::visit::Ast;

#[derive(Ast)]
pub enum Ty<S> {
    Unit(PhantomData<S>),
}

#[derive(Ast)]
#[subast(crate::Ty)]
pub enum Expr<S> {
    Pair((Ty<S>, Ty<S>)),
    // A nested tuple with a leaf element exercises recursion + `_` binding for non-followed members.
    Triple((Ty<S>, (PhantomData<S>, Ty<S>))),
    Lit(PhantomData<S>),
}

pub mod v {
    syan::visit::visitor!(crate::Expr, crate::Ty);
}

#[test]
fn tuple_field_visits_each_element() {
    let e = Expr::Pair((Ty::Unit(PhantomData), Ty::Unit(PhantomData)));
    let mut n = 0usize;
    e.visit(|_: &Ty<()>| n += 1);
    assert_eq!(n, 2, "both tuple elements should be visited");
}

#[test]
fn nested_tuple_with_leaf_element() {
    let e = Expr::Triple((Ty::Unit(PhantomData), (PhantomData, Ty::Unit(PhantomData))));
    let mut n = 0usize;
    e.visit(|_: &Ty<()>| n += 1);
    assert_eq!(n, 2, "the two Ty elements (skipping the PhantomData leaf) should be visited");
}

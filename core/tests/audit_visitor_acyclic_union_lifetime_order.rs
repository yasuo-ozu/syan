//! AUDIT C' (regression): when a `visitor!()`'s union of visited-type params orders a lifetime AFTER
//! a type param (here `Outer<S>` listed before `Inner<'a>` → union `[S, 'a]`), the generated trait /
//! free-fn generic lists must be normalized lifetime-first, else "lifetime parameters must be declared
//! prior to type and const parameters". This file simply compiling is the check.
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::visit::Ast;

#[derive(Ast)]
#[subast()]
pub struct Outer<S> {
    pub _p: PhantomData<S>,
}

#[derive(Ast)]
#[subast()]
pub struct Inner<'a> {
    pub _p: PhantomData<&'a ()>,
}

mod v {
    // Outer (type param `S`) listed BEFORE Inner (lifetime `'a`) → union order is `[S, 'a]`.
    syan::visit::visitor!(crate::Outer, crate::Inner);
}

#[test]
fn union_orders_lifetime_first() {
    let o = Outer::<()> { _p: PhantomData };
    o.visit(|_x: &Inner<'_>| {});
}

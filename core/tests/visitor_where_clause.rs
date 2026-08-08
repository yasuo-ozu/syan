//! Regression for audit #8: a visited type's `where`-clause is threaded onto every generated item
//! that names the type. Before the fix `enum Expr<S> where S: Bound { .. }` produced ~24 `E0277`s
//! (the generated trait / free fns / Driver / closure impls / inherent methods all named `Expr<S>`
//! without `where S: Bound`). This is a compile-success test.
//!
//! The bound is written `crate::Bound` (a resolvable path) because the generated items live in
//! `mod v`; a bare `S: Bound` would not resolve there — the same canonical-path requirement the
//! `#[subast]` paths already have. Fully-qualified bounds like `S: ::core::clone::Clone` also work.
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::visit::Ast;

pub trait Bound {}
impl Bound for () {}

#[derive(Ast)]
#[subast()]
pub enum Expr<S>
where
    S: crate::Bound,
{
    Nest(Box<Expr<S>>),
    Leaf(PhantomData<S>),
}

pub mod v {
    syan::visit::visitor!(crate::Expr);
}

#[test]
fn where_bounded_visitor_compiles_and_runs() {
    let e: Expr<()> = Expr::Nest(Box::new(Expr::Leaf(PhantomData)));
    let mut n = 0usize;
    e.visit(|_: &Expr<()>| n += 1);
    assert_eq!(n, 2, "outer Nest + inner Leaf");
}

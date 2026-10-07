//! Extending a visitor whose visited type carries a `where` clause.
//!
//! A node's predicates ride into its visitor's trait, so `base::Visit<S>` can require `S: Bound`.
//! Every reference to that trait has to state the bound, including the empty impls the extender's
//! `Driver` writes to satisfy its supertraits.
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::visit::Ast;

pub trait Bound {}
impl Bound for () {}

#[derive(Ast)]
pub struct Leaf<S>
where
    S: crate::Bound,
{
    pub p: PhantomData<S>,
}

#[derive(Ast)]
#[subast(crate::Leaf)]
pub struct Node<S>
where
    S: crate::Bound,
{
    pub a: Leaf<S>,
}

#[derive(Ast)]
#[subast(crate::Node)]
pub struct Root<S>
where
    S: crate::Bound,
{
    pub n: Node<S>,
}

pub mod base {
    syan::visit::visitor!(crate::Leaf);
}
pub mod mid {
    syan::visit::visitor!(crate::base => crate::Node);
}
// Two levels deep, so the `Driver` writes an impl for an ancestor that is not its direct base.
pub mod top {
    syan::visit::visitor!(crate::mid => crate::Root);
}

fn sample() -> Root<()> {
    Root {
        n: Node {
            a: Leaf { p: PhantomData },
        },
    }
}

#[derive(Default)]
struct Counter {
    leaves: u32,
    nodes: u32,
    roots: u32,
}

impl<S: crate::Bound> base::Visit<S> for Counter {
    fn visit_leaf(&mut self, i: &Leaf<S>) {
        self.leaves += 1;
        base::visit_leaf(self, i);
    }
}
impl<S: crate::Bound> base::VisitMut<S> for Counter {}
impl<S: crate::Bound> mid::Visit<S> for Counter {
    fn visit_node(&mut self, i: &Node<S>) {
        self.nodes += 1;
        mid::visit_node(self, i);
    }
}
impl<S: crate::Bound> mid::VisitMut<S> for Counter {}
impl<S: crate::Bound> top::Visit<S> for Counter {
    fn visit_root(&mut self, i: &Root<S>) {
        self.roots += 1;
        top::visit_root(self, i);
    }
}

#[test]
fn a_bounded_param_survives_two_levels_of_inheritance() {
    let mut c = Counter::default();
    sample().visit(&mut c);
    assert_eq!((c.leaves, c.nodes, c.roots), (1, 1, 1));
}

#[test]
fn a_closure_still_works_over_the_bounded_grammar() {
    let mut n = 0;
    sample().visit(|_: &Root<()>| n += 1);
    assert_eq!(n, 1);
}

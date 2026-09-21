//! Transparent single-slot wrappers descend as a [`Slot`](syan::visit::Slot).
//!
//! `Box` and `Attempt` ship with impls; a consumer's own wrapper joins with one `Slot`/`SlotMut` pair.
//! No reference type is a `Slot`, deliberately — a blanket over `Deref` would catch `&T` and turn a
//! missing view impl into a type mismatch instead of "no method named `view_iter`".
#![allow(dead_code)]

mod ast {
    use syan::visit::Ast;

    /// A consumer wrapper that joins the walk with one `Slot`/`SlotMut` pair.
    #[derive(Debug, Clone)]
    pub struct MyPtr<T>(pub Box<T>);

    impl<T> syan::visit::Slot for MyPtr<T> {
        type Target = T;
        fn get(&self) -> &T {
            &self.0
        }
    }
    impl<T> syan::visit::SlotMut for MyPtr<T> {
        fn get_mut(&mut self) -> &mut T {
            &mut self.0
        }
    }

    #[derive(Debug, Clone, Ast)]
    pub enum Node {
        Leaf(i64),
    }

    #[derive(Debug, Clone, Ast)]
    #[subast(crate::ast::Node)]
    pub struct Holder {
        pub boxed: Box<Node>,
        pub mine: MyPtr<Node>,
        pub attempt: syan::nested::Attempt<Node>,
        pub nested: MyPtr<Vec<Node>>,
    }

    pub mod v {
        syan::visit::visitor!(crate::ast::Node, crate::ast::Holder);
    }
}

use ast::{Holder, MyPtr, Node};

fn sample() -> Holder {
    Holder {
        boxed: Box::new(Node::Leaf(1)),
        mine: MyPtr(Box::new(Node::Leaf(2))),
        attempt: syan::nested::Attempt(Node::Leaf(3)),
        nested: MyPtr(Box::new(vec![Node::Leaf(4), Node::Leaf(5)])),
    }
}

fn seen(h: &Holder) -> Vec<i64> {
    let mut v = Vec::new();
    h.visit(|n: &Node| {
        let Node::Leaf(x) = n;
        v.push(*x);
    });
    v.sort_unstable();
    v
}

#[test]
fn every_deref_wrapper_descends() {
    assert_eq!(seen(&sample()), vec![1, 2, 3, 4, 5]);
}

/// The point of the blanket: `MyPtr` implements no view trait, only `Deref`.
#[test]
fn a_consumer_wrapper_needs_only_deref() {
    let mut hit = 0;
    sample().visit(|n: &Node| {
        if matches!(n, Node::Leaf(2)) {
            hit += 1;
        }
    });
    assert_eq!(hit, 1, "the node inside MyPtr was reached");
}

#[test]
fn visit_mut_reaches_through_every_wrapper() {
    let mut h = sample();
    h.visit_mut(|n: &mut Node| {
        let Node::Leaf(x) = n;
        *x *= 10;
    });
    assert_eq!(seen(&h), vec![10, 20, 30, 40, 50]);
}

/// A wrapper over a container is two layers, not one: the slot yields the `Vec`, the `Vec` the nodes.
#[test]
fn layers_nest() {
    let mut h = sample();
    h.nested.0.push(Node::Leaf(6));
    assert_eq!(seen(&h), vec![1, 2, 3, 4, 5, 6]);
}

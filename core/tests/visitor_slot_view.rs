//! Transparent single-slot wrappers descend through the `SlotView` blanket over `Deref`/`DerefMut`.
//!
//! `Box` and `Attempt` are covered, and so is a consumer's own wrapper — one `Deref` impl is the
//! whole cost of entry, with no view trait to implement. `Rc`/`Arc` are deliberately absent: they
//! have no `DerefMut`, so a shared slot descends on neither side.
#![allow(dead_code)]

mod ast {
    use syan::visit::Ast;

    /// A consumer wrapper with nothing but `Deref`/`DerefMut` — no view impl of its own.
    #[derive(Debug, Clone)]
    pub struct MyPtr<T>(pub Box<T>);

    impl<T> std::ops::Deref for MyPtr<T> {
        type Target = T;
        fn deref(&self) -> &T {
            &self.0
        }
    }
    impl<T> std::ops::DerefMut for MyPtr<T> {
        fn deref_mut(&mut self) -> &mut T {
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
    h.nested.push(Node::Leaf(6));
    assert_eq!(seen(&h), vec![1, 2, 3, 4, 5, 6]);
}

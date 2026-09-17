//! `Rc`/`Arc` descend like `Box`, with copy-on-write on the `visit_mut` side.

#![allow(dead_code)]

use std::rc::Rc;
use std::sync::Arc;

mod ast {
    use syan::visit::Ast;

    #[derive(Debug, Clone, Ast)]
    #[subast()]
    pub enum Node {
        Leaf(i64),
    }

    #[derive(Debug, Clone, Ast)]
    #[subast(crate::ast::Node)]
    pub struct Holder {
        pub boxed: Box<Node>,
        pub rc: std::rc::Rc<Node>,
        pub arc: std::sync::Arc<Node>,
        pub many: Vec<std::rc::Rc<Node>>,
        pub maybe: Option<std::rc::Rc<Node>>,
    }

    pub mod v {
        syan::visit::visitor!(super::Node, super::Holder);
    }
}

use ast::{Holder, Node};

fn sample() -> Holder {
    Holder {
        boxed: Box::new(Node::Leaf(1)),
        rc: Rc::new(Node::Leaf(2)),
        arc: Arc::new(Node::Leaf(3)),
        many: vec![Rc::new(Node::Leaf(4)), Rc::new(Node::Leaf(5))],
        maybe: Some(Rc::new(Node::Leaf(6))),
    }
}

#[test]
fn shared_slots_are_descended() {
    let h = sample();
    let mut seen = Vec::new();
    h.visit(|n: &Node| {
        let Node::Leaf(v) = n;
        seen.push(*v);
    });
    seen.sort_unstable();
    assert_eq!(
        seen,
        vec![1, 2, 3, 4, 5, 6],
        "every slot descends, Rc/Arc included"
    );
}

#[test]
fn visit_mut_reaches_through_rc_and_arc() {
    let mut h = sample();
    h.visit_mut(|n: &mut Node| {
        let Node::Leaf(v) = n;
        *v *= 10;
    });
    let mut seen = Vec::new();
    h.visit(|n: &Node| {
        let Node::Leaf(v) = n;
        seen.push(*v);
    });
    seen.sort_unstable();
    assert_eq!(seen, vec![10, 20, 30, 40, 50, 60]);
}

/// The documented consequence of `make_mut`: editing through a *shared* `Rc` unshares it, so the
/// other holder keeps the original. This is the price of a complete, deterministic traversal.
#[test]
fn mutation_through_a_shared_rc_is_copy_on_write() {
    let shared = Rc::new(Node::Leaf(7));
    let mut h = sample();
    h.rc = Rc::clone(&shared);
    assert_eq!(Rc::strong_count(&shared), 2);

    h.visit_mut(|n: &mut Node| {
        let Node::Leaf(v) = n;
        *v *= 10;
    });

    let Node::Leaf(theirs) = &*shared;
    assert_eq!(*theirs, 7, "the other holder is untouched");
    let Node::Leaf(ours) = &*h.rc;
    assert_eq!(*ours, 70, "our copy was edited");
    assert_eq!(Rc::strong_count(&shared), 1, "make_mut unshared our handle");
}

/// A uniquely-owned `Rc` is edited in place, with no clone.
#[test]
fn a_unique_rc_is_edited_without_cloning() {
    let mut h = sample();
    let addr = Rc::as_ptr(&h.rc);
    h.visit_mut(|n: &mut Node| {
        let Node::Leaf(v) = n;
        *v += 1;
    });
    assert_eq!(Rc::as_ptr(&h.rc), addr, "unique Rc is not reallocated");
}

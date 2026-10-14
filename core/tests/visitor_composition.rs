//! Several passes over one tree, and a visitor reached through a wrapper.
//!
//! **A tuple is a visitor only when every element is a closure.** Those fold into one hook, and so
//! into one walk. A tuple of *structs* is not a visitor: the extracted `visiting` crate dropped
//! those impls deliberately, so two struct passes are two walks, written as two calls. This file
//! pins that, because it is the one way the visitor's API changed when it moved out of this crate.
//!
//! A `Box` around a visitor is still a visitor, so `node.visit(Box::new(pass))` works without
//! unwrapping, and a consumer's own wrapper writes the same one-line forwarding impl.
#![allow(dead_code)]

mod ast {
    use syan::visit::Visiting;

    #[derive(Debug, Visiting)]
    pub enum Node {
        Leaf(i64),
    }

    #[derive(Debug, Visiting)]
    #[visiting(crate::ast::Node)]
    pub struct Tree {
        pub nodes: Vec<Node>,
    }

    pub mod v {
        syan::visit::define_visit!(crate::ast::Node, crate::ast::Tree);
    }
}

use ast::{Node, Tree};

fn sample() -> Tree {
    Tree {
        nodes: vec![Node::Leaf(1), Node::Leaf(2), Node::Leaf(3)],
    }
}

#[derive(Default)]
struct Count(usize);
impl ast::v::Visit for Count {
    fn visit_node(&mut self, _: &Node) {
        self.0 += 1;
    }
}

#[derive(Default)]
struct Sum(i64);
impl ast::v::Visit for Sum {
    fn visit_node(&mut self, n: &Node) {
        let Node::Leaf(x) = n;
        self.0 += x;
    }
}

#[test]
fn two_struct_passes_are_two_walks() {
    let (mut count, mut sum) = (Count::default(), Sum::default());
    let t = sample();
    t.visit(&mut count);
    t.visit(&mut sum);
    assert_eq!((count.0, sum.0), (3, 6));
}

#[test]
fn a_tuple_of_closures_interleaves_in_one_walk() {
    let (mut nodes, mut trees) = (0, 0);
    sample().visit((|_: &Node| nodes += 1, |_: &Tree| trees += 1));
    assert_eq!((nodes, trees), (3, 1), "one traversal, both closures");
}

#[test]
fn a_boxed_visitor_is_a_visitor() {
    let mut boxed = Box::new(Count::default());
    sample().visit(&mut *boxed);
    assert_eq!(boxed.0, 3);
}

#[test]
fn a_visitor_passed_by_reference_is_a_visitor() {
    let mut count = Count::default();
    sample().visit(&mut count);
    assert_eq!(count.0, 3);
}

// The generated module exports its public API and nothing else: the hook/driver machinery lives one
// module down, so a glob cannot reach it.
mod glob {
    use super::ast::v::*;
    use super::{Node, Tree};

    #[derive(Default)]
    struct P(usize);
    impl Visit for P {
        fn visit_node(&mut self, i: &Node) {
            self.0 += 1;
            visit_node(self, i);
        }
    }
    impl VisitMut for P {}

    #[test]
    fn a_glob_import_brings_in_the_public_api_only() {
        let mut p = P::default();
        let t = Tree {
            nodes: vec![Node::Leaf(1), Node::Leaf(2)],
        };
        Visit::visit_tree(&mut p, &t);
        assert_eq!(p.0, 2);
    }
}

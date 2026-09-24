//! Several visitors in one traversal, and a visitor reached through a wrapper.
//!
//! A tuple of visitors implements the generated `Visit`/`VisitMut` directly, so it composes and can be
//! passed wherever one visitor can. So does a `Box` around a visitor, so `node.visit(Box::new(pass))`
//! works without unwrapping; a consumer's own wrapper writes the same one-line forwarding impl.
#![allow(dead_code)]

mod ast {
    use syan::visit::Ast;

    #[derive(Debug, Ast)]
    pub enum Node {
        Leaf(i64),
    }

    #[derive(Debug, Ast)]
    #[subast(crate::ast::Node)]
    pub struct Tree {
        pub nodes: Vec<Node>,
    }

    pub mod v {
        syan::visit::visitor!(crate::ast::Node, crate::ast::Tree);
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
fn a_tuple_of_visitors_is_a_visitor() {
    let mut both = (Count::default(), Sum::default());
    sample().visit(&mut both);
    assert_eq!(
        (both.0 .0, both.1 .0),
        (3, 6),
        "one traversal, both visitors"
    );
}

#[test]
fn tuples_nest_and_reach_higher_arities() {
    let mut three = (Count::default(), Sum::default(), Count::default());
    sample().visit(&mut three);
    assert_eq!((three.0 .0, three.1 .0, three.2 .0), (3, 6, 3));
}

/// `visit` takes the visitor by value, so a wrapper is moved in — the tally comes back through a
/// borrow the visitor holds.
struct Tally<'a>(&'a mut usize);
impl ast::v::Visit for Tally<'_> {
    fn visit_node(&mut self, _: &Node) {
        *self.0 += 1;
    }
}

#[test]
fn a_boxed_visitor_is_a_visitor() {
    let mut n = 0usize;
    sample().visit(Box::new(Tally(&mut n)));
    assert_eq!(n, 3);
}

#[test]
fn a_consumer_wrapper_forwarding_to_a_visitor_works() {
    struct Holder<V>(V);
    impl<V: ast::v::Visit> ast::v::Visit for Holder<V> {
        fn visit_node(&mut self, i: &Node) {
            self.0.visit_node(i);
        }
    }

    let mut n = 0usize;
    sample().visit(Holder(Tally(&mut n)));
    assert_eq!(n, 3);
}

#[test]
fn closures_and_tuples_of_closures_still_work() {
    let (mut n, mut total) = (0usize, 0i64);
    sample().visit((
        |_: &Node| n += 1,
        |x: &Node| {
            let Node::Leaf(v) = x;
            total += v;
        },
    ));
    assert_eq!((n, total), (3, 6));
}

#[test]
fn a_tuple_of_visitors_edits_in_place() {
    struct Double;
    impl ast::v::VisitMut for Double {
        fn visit_node_mut(&mut self, n: &mut Node) {
            let Node::Leaf(x) = n;
            *x *= 2;
        }
    }
    struct Bump;
    impl ast::v::VisitMut for Bump {
        fn visit_node_mut(&mut self, n: &mut Node) {
            let Node::Leaf(x) = n;
            *x += 1;
        }
    }

    let mut t = sample();
    t.visit_mut(&mut (Double, Bump));
    let mut got = Vec::new();
    t.visit(|n: &Node| {
        let Node::Leaf(x) = n;
        got.push(*x);
    });
    assert_eq!(got, vec![3, 5, 7], "Double then Bump, per node");
}

// A `use path::to::visit::*;` must bring in the public API and nothing else. The closure adapters,
// the driver and the module's walk tag are private to the generated module, so a glob cannot reach
// them and cannot shadow a name of the caller's own. (`#[doc(hidden)]` alone would not do this — it
// hides an item from rustdoc while leaving it importable.)
mod glob_import {
    use syan::visit::Ast;

    #[derive(Ast)]
    pub enum Node {
        Leaf(i64),
    }

    pub mod v {
        syan::visit::visitor!(super::Node);
    }

    use v::*;

    // Same names as the generated machinery. These compile only because the glob does not import it.
    pub struct Driver;
    pub struct DriverMut;
    pub struct NodeHook;
    pub struct NodeHookMut;
    pub struct __SyanWalkTag;
    pub trait Hook {}
    pub trait HookMut {}
    pub trait IntoHook {}
    pub trait IntoHookMut {}

    #[test]
    fn a_glob_import_brings_in_the_public_api_only() {
        // `Visit` and the free fns did come through the glob, and still work.
        struct Count(usize);
        impl Visit for Count {
            fn visit_node(&mut self, i: &Node) {
                self.0 += 1;
                visit_node(self, i);
            }
        }
        let mut c = Count(0);
        Node::Leaf(1).visit(&mut c);
        assert_eq!(c.0, 1);
    }
}

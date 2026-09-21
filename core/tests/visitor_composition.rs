//! Several visitors in one traversal, and a visitor reached through a wrapper.
//!
//! A tuple of visitors implements the generated `Visit`/`VisitMut` directly, so it composes and can be
//! passed wherever one visitor can. A visitor inside a [`Slot`] — `Box`, `Attempt`, a consumer's own
//! wrapper — arrives through `IntoVisitor`, so `node.visit(wrapped)` works without unwrapping.
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
fn an_attempt_wrapped_visitor_works() {
    let mut n = 0usize;
    sample().visit(syan::nested::Attempt(Tally(&mut n)));
    assert_eq!(n, 3);
}

#[test]
fn a_consumer_wrapper_holding_a_visitor_works() {
    struct Holder<V>(V);
    impl<V> syan::visit::Slot for Holder<V> {
        type Target = V;
        fn get(&self) -> &V {
            &self.0
        }
    }
    impl<V> syan::visit::SlotMut for Holder<V> {
        fn get_mut(&mut self) -> &mut V {
            &mut self.0
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

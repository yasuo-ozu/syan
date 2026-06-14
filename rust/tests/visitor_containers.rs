//! Stage 6: `Vec<X>` / `Option<X>` fields traverse through the `visit_*_seq` / `visit_*_opt` hooks.

use core::marker::PhantomData;
use syan::visit::{visitor, Ast};

#[derive(Ast)]
pub struct Leaf<S>(PhantomData<S>);

#[derive(Ast)]
pub struct Node<S> {
    pub children: Vec<Leaf<S>>,
    pub maybe: Option<Leaf<S>>,
}

#[visitor(Node, Leaf)]
pub mod visit {}

use visit::Visitable;

fn sample() -> Node<()> {
    Node {
        children: vec![Leaf(PhantomData), Leaf(PhantomData)],
        maybe: Some(Leaf(PhantomData)),
    }
}

#[test]
fn seq_and_opt_are_traversed() {
    let mut leaves = 0usize;
    sample().visit(|_l: &Leaf<()>| leaves += 1);
    assert_eq!(leaves, 3, "2 in the Vec + 1 in the Option");
}

#[test]
fn overriding_seq_hook_short_circuits() {
    // A struct visitor overriding `visit_leaf_seq` to skip the Vec entirely; the Option still fires.
    #[derive(Default)]
    struct OnlyOption {
        seen: usize,
    }
    impl<S> visit::Visit<S> for OnlyOption {
        fn visit_leaf(&mut self, _i: &Leaf<S>) {
            self.seen += 1;
        }
        fn visit_leaf_seq(&mut self, _seq: &[Leaf<S>]) {
            // intentionally do not descend into the Vec
        }
    }
    let mut v = OnlyOption::default();
    sample().visit(&mut v);
    assert_eq!(v.seen, 1, "only the Option leaf, Vec skipped by the override");
}

//! `visitor!(base => More)`: a visitor that extends another.
//!
//! An inherited node's `Walk` impl cannot come from the base module — that one is written against the
//! base's tag. So the extending module emits its own, against its own tag, reaching the method through
//! the supertrait. This covers the shapes that took most getting right: an inherited head behind a
//! container, behind a wrapper, inside a tuple, and three levels deep.
#![allow(dead_code)]

mod ast {
    use syan::visit::Ast;

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct Tag(pub i64);

    #[derive(Debug, Ast)]
    pub enum Leaf {
        N(i64),
    }

    /// Reaches `Leaf` four ways at once — plain, in a `Vec`, through a `Box`, and inside a tuple.
    #[derive(Debug, Ast)]
    #[subast(crate::ast::Leaf)]
    pub struct Mid {
        pub direct: Leaf,
        pub many: Vec<Leaf>,
        pub boxed: Box<Leaf>,
        pub tagged: Vec<(Tag, Leaf)>,
    }

    #[derive(Debug, Ast)]
    #[subast(crate::ast::Mid)]
    pub struct Top {
        pub mids: Vec<Mid>,
    }
}

use ast::{Leaf, Mid, Tag, Top};

mod base {
    syan::visit::visitor!(crate::ast::Leaf);
}
mod mid {
    syan::visit::visitor!(super::base => crate::ast::Mid);
}
mod top {
    syan::visit::visitor!(super::mid => crate::ast::Top);
}

fn a_mid() -> Mid {
    Mid {
        direct: Leaf::N(1),
        many: vec![Leaf::N(2), Leaf::N(3)],
        boxed: Box::new(Leaf::N(4)),
        tagged: vec![(Tag(9), Leaf::N(5))],
    }
}

#[derive(Default)]
struct Sum(i64);
impl base::Visit for Sum {
    fn visit_leaf(&mut self, i: &Leaf) {
        let Leaf::N(x) = i;
        self.0 += x;
        base::visit_leaf(self, i);
    }
}
impl mid::Visit for Sum {}
impl top::Visit for Sum {}

/// The extending visitor reaches its base's type through every container shape.
#[test]
fn an_inherited_head_is_reached_behind_every_wrapper() {
    let mut s = Sum::default();
    mid::Visit::visit_mid(&mut s, &a_mid());
    assert_eq!(
        s.0,
        1 + 2 + 3 + 4 + 5,
        "direct, Vec, Box and tuple all descended"
    );
}

/// Three levels: `top` inherits `Mid` from `mid`, which inherits `Leaf` from `base`.
#[test]
fn inheritance_chains_three_deep() {
    let t = Top {
        mids: vec![a_mid(), a_mid()],
    };
    let mut s = Sum::default();
    top::Visit::visit_top(&mut s, &t);
    assert_eq!(s.0, 2 * 15);
}

/// The base's own entry point still works, and sees only what the base knows.
#[test]
fn the_base_still_walks_on_its_own() {
    let mut s = Sum::default();
    base::Visit::visit_leaf(&mut s, &Leaf::N(7));
    assert_eq!(s.0, 7);
}

/// Overriding at the extending level and delegating downwards.
#[test]
fn an_override_at_the_outer_level_still_descends() {
    #[derive(Default)]
    struct Count {
        mids: usize,
        leaves: usize,
    }
    impl base::Visit for Count {
        fn visit_leaf(&mut self, i: &Leaf) {
            self.leaves += 1;
            base::visit_leaf(self, i);
        }
    }
    impl mid::Visit for Count {
        fn visit_mid(&mut self, i: &Mid) {
            self.mids += 1;
            mid::visit_mid(self, i);
        }
    }
    impl top::Visit for Count {}

    let t = Top {
        mids: vec![a_mid(), a_mid()],
    };
    let mut c = Count::default();
    top::Visit::visit_top(&mut c, &t);
    assert_eq!((c.mids, c.leaves), (2, 10));
}

/// The `&mut` side of the same chain.
#[test]
fn the_mut_chain_edits_through_inherited_heads() {
    struct Bump;
    impl base::VisitMut for Bump {
        fn visit_leaf_mut(&mut self, i: &mut Leaf) {
            let Leaf::N(x) = i;
            *x *= 10;
            base::visit_leaf_mut(self, i);
        }
    }
    impl mid::VisitMut for Bump {}
    impl top::VisitMut for Bump {}

    let mut t = Top {
        mids: vec![a_mid()],
    };
    top::VisitMut::visit_top_mut(&mut Bump, &mut t);

    let mut s = Sum::default();
    top::Visit::visit_top(&mut s, &t);
    assert_eq!(s.0, 150, "every leaf multiplied, through all four shapes");
    assert_eq!(t.mids[0].tagged[0].0, Tag(9), "the tuple's leaf untouched");
}

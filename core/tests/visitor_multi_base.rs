//! `visitor!(a, b => T)` — a visitor extending more than one other visitor.
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::visit::Ast;

#[derive(Ast)]
pub enum Ty<S> {
    Unit(PhantomData<S>),
}

#[derive(Ast)]
pub enum Lit<S> {
    Num(PhantomData<S>),
}

#[derive(Ast)]
#[subast(crate::Ty, crate::Lit)]
pub struct Expr<S> {
    pub t: Ty<S>,
    pub l: Lit<S>,
}

// Two independent visitors, neither extending the other.
pub mod types {
    syan::visit::visitor!(crate::Ty);
}
pub mod lits {
    syan::visit::visitor!(crate::Lit);
}

// One visitor extending both.
pub mod both {
    syan::visit::visitor!(crate::types, crate::lits => crate::Expr);
}

#[derive(Default)]
struct Counter {
    tys: u32,
    lits: u32,
    exprs: u32,
}

impl<S> types::Visit<S> for Counter {
    fn visit_ty(&mut self, i: &Ty<S>) {
        self.tys += 1;
        types::visit_ty(self, i);
    }
}
impl<S> types::VisitMut<S> for Counter {}
impl<S> lits::Visit<S> for Counter {
    fn visit_lit(&mut self, i: &Lit<S>) {
        self.lits += 1;
        lits::visit_lit(self, i);
    }
}
impl<S> lits::VisitMut<S> for Counter {}
impl<S> both::Visit<S> for Counter {
    fn visit_expr(&mut self, i: &Expr<S>) {
        self.exprs += 1;
        both::visit_expr(self, i);
    }
}

fn sample() -> Expr<()> {
    Expr {
        t: Ty::Unit(PhantomData),
        l: Lit::Num(PhantomData),
    }
}

#[test]
fn both_bases_are_reached() {
    let mut c = Counter::default();
    sample().visit(&mut c);
    assert_eq!((c.tys, c.lits, c.exprs), (1, 1, 1));
}

// A closure still covers this visitor's own types (an inherited one needs the hook machinery that
// only generates for a visitor's own list — the same rule as with one base).
#[test]
fn a_closure_covers_its_own_type() {
    let mut n = 0;
    sample().visit(|_: &Expr<()>| n += 1);
    assert_eq!(n, 1);
}

// Each base's own entry points keep working on a value that implements the whole set.
#[test]
fn each_base_can_be_driven_directly() {
    let mut c = Counter::default();
    let (t, l): (Ty<()>, Lit<()>) = (Ty::Unit(PhantomData), Lit::Num(PhantomData));
    types::Visit::visit_ty(&mut c, &t);
    lits::Visit::visit_lit(&mut c, &l);
    assert_eq!((c.tys, c.lits), (1, 1));
}

// Passing by `&mut`, boxed, and as a tuple still resolves with several supertraits in play.
#[test]
fn wrappers_still_satisfy_every_supertrait() {
    let mut c = Counter::default();
    sample().visit(&mut c);
    assert_eq!(c.exprs, 1);

    let mut b: Box<Counter> = Box::default();
    both::Visit::visit_expr(&mut *b, &sample());
    assert_eq!(b.exprs, 1);

    let mut pair = (Counter::default(), Counter::default());
    sample().visit(&mut pair);
    assert_eq!((pair.0.tys, pair.1.lits), (1, 1));
}

// A diamond: both bases extend one shared visitor, so its trait is a supertrait twice over and the
// ancestor list must name it once.
mod diamond {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    pub enum Leaf<S> {
        L(PhantomData<S>),
    }
    #[derive(Ast)]
    #[subast(crate::diamond::Leaf)]
    pub struct Left<S> {
        pub a: Leaf<S>,
    }
    #[derive(Ast)]
    #[subast(crate::diamond::Leaf)]
    pub struct Right<S> {
        pub a: Leaf<S>,
    }
    #[derive(Ast)]
    #[subast(crate::diamond::Left, crate::diamond::Right)]
    pub struct Top<S> {
        pub l: Left<S>,
        pub r: Right<S>,
    }

    pub mod shared {
        syan::visit::visitor!(crate::diamond::Leaf);
    }
    pub mod l {
        syan::visit::visitor!(crate::diamond::shared => crate::diamond::Left);
    }
    pub mod r {
        syan::visit::visitor!(crate::diamond::shared => crate::diamond::Right);
    }
    pub mod top {
        syan::visit::visitor!(crate::diamond::l, crate::diamond::r => crate::diamond::Top);
    }

    #[derive(Default)]
    struct C {
        leaves: u32,
        tops: u32,
    }
    impl<S> shared::Visit<S> for C {
        fn visit_leaf(&mut self, i: &Leaf<S>) {
            self.leaves += 1;
            shared::visit_leaf(self, i);
        }
    }
    impl<S> shared::VisitMut<S> for C {}
    impl<S> l::Visit<S> for C {}
    impl<S> l::VisitMut<S> for C {}
    impl<S> r::Visit<S> for C {}
    impl<S> r::VisitMut<S> for C {}
    impl<S> top::Visit<S> for C {
        fn visit_top(&mut self, i: &Top<S>) {
            self.tops += 1;
            top::visit_top(self, i);
        }
    }

    #[test]
    fn a_shared_ancestor_is_named_once() {
        let t: Top<()> = Top {
            l: Left {
                a: Leaf::L(PhantomData),
            },
            r: Right {
                a: Leaf::L(PhantomData),
            },
        };
        let mut c = C::default();
        t.visit(&mut c);
        assert_eq!((c.leaves, c.tops), (2, 1));
    }
}

// Bases of different arity, and a further visitor extending the multi-base one.
mod widened {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    pub enum A<S> {
        X(PhantomData<S>),
    }
    #[derive(Ast)]
    pub enum B<S, T> {
        Y(PhantomData<(S, T)>),
    }
    #[derive(Ast)]
    #[subast(crate::widened::A, crate::widened::B)]
    pub struct Both<S, T> {
        pub a: A<S>,
        pub b: B<S, T>,
    }
    #[derive(Ast)]
    #[subast(crate::widened::Both)]
    pub struct Outer<S, T> {
        pub inner: Both<S, T>,
    }

    pub mod va {
        syan::visit::visitor!(crate::widened::A);
    }
    pub mod vb {
        syan::visit::visitor!(crate::widened::B);
    }
    pub mod vboth {
        syan::visit::visitor!(crate::widened::va, crate::widened::vb => crate::widened::Both);
    }
    pub mod vouter {
        syan::visit::visitor!(crate::widened::vboth => crate::widened::Outer);
    }

    #[derive(Default)]
    struct C {
        a: u32,
        b: u32,
        outer: u32,
    }
    impl<S> va::Visit<S> for C {
        fn visit_a(&mut self, i: &A<S>) {
            self.a += 1;
            va::visit_a(self, i);
        }
    }
    impl<S> va::VisitMut<S> for C {}
    impl<S, T> vb::Visit<S, T> for C {
        fn visit_b(&mut self, i: &B<S, T>) {
            self.b += 1;
            vb::visit_b(self, i);
        }
    }
    impl<S, T> vb::VisitMut<S, T> for C {}
    impl<S, T> vboth::Visit<S, T> for C {}
    impl<S, T> vboth::VisitMut<S, T> for C {}
    impl<S, T> vouter::Visit<S, T> for C {
        fn visit_outer(&mut self, i: &Outer<S, T>) {
            self.outer += 1;
            vouter::visit_outer(self, i);
        }
    }

    #[test]
    fn bases_of_different_arity_and_a_further_extender() {
        let o: Outer<(), ()> = Outer {
            inner: Both {
                a: A::X(PhantomData),
                b: B::Y(PhantomData),
            },
        };
        let mut c = C::default();
        o.visit(&mut c);
        assert_eq!((c.a, c.b, c.outer), (1, 1, 1));
    }
}

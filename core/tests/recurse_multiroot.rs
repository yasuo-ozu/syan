//! Step 2 of multi-root support: several *self-referential* roots **within one** strongly-connected
//! cycle (e.g. `A` and `B` that each self-reference AND reference each other). Each root keeps its own
//! depth dimension — every cycle type carries one depth param per root (`__R0`, `__R1`, …), a back-
//! edge to root `i` drives via `__Ri`, and the per-root depth chains are unrolled mutually. The
//! visitor exposes a `visit_*` method per type, each generic over *all* roots' remaining depth.
//!
//! Previously this was a hard `abort!` ("does not support multi-root cycles").
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::parse::recurse;

#[recurse]
mod ast {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum A<S> {
        Me(Box<A<S>>),
        ToB(Box<B<S>>),
        Lit(PhantomData<S>),
    }

    #[derive(Ast)]
    #[subast()]
    pub enum B<S> {
        Me(Box<B<S>>),
        ToA(Box<A<S>>),
        Lit(PhantomData<S>),
    }
}

mod v_ast {
    syan::visit::visitor!(crate::ast::A, crate::ast::B);
}

#[derive(Default)]
struct Counter {
    a: usize,
    b: usize,
}

impl<S> v_ast::Visit<S> for Counter {
    fn visit_a<R0: v_ast::VisitRec<S, Self>, R1: v_ast::VisitRec<S, Self>>(
        &mut self,
        i: &v_ast::ANode<S, R0, R1>,
    ) {
        self.a += 1;
        v_ast::visit_a(self, i);
    }
    fn visit_b<R0: v_ast::VisitRec<S, Self>, R1: v_ast::VisitRec<S, Self>>(
        &mut self,
        i: &v_ast::BNode<S, R0, R1>,
    ) {
        self.b += 1;
        v_ast::visit_b(self, i);
    }
}

#[test]
fn each_root_keeps_its_own_depth() {
    // A(outer) -> ToB(B) -> ToA(A) -> Lit. Each back-edge crosses between the A and B depth
    // dimensions; the visitor descends both and counts each type's nodes independently.
    let v: ast::A<()> = ast::A::ToB(Box::new(v_ast::BNode::ToA(Box::new(v_ast::ANode::Lit(
        PhantomData,
    )))));
    let mut c = Counter::default();
    v_ast::Visit::visit_a(&mut c, &v);
    assert_eq!((c.a, c.b), (2, 1), "two A nodes (outer + inner) and one B node");
}

#[test]
fn visit_from_either_root() {
    // Pure-B nesting B -> Me(B) -> Lit, entered through visit_b.
    let v: ast::B<()> = ast::B::Me(Box::new(v_ast::BNode::Lit(PhantomData)));
    let mut c = Counter::default();
    v_ast::Visit::visit_b(&mut c, &v);
    assert_eq!((c.a, c.b), (0, 2), "two B nodes, no A");
}

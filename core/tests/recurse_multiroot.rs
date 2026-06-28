//! A strongly-connected cycle with several *self-referential* types (`A` and `B` each self-reference
//! AND reference each other). With the natural-type design these are ordinary mutually-recursive enums
//! (the depth-limited engine + per-root depth dimensions are an internal `Parse` detail); the
//! `visitor!()` over them is an ordinary acyclic visitor with a `visit_*` per type.
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::parse::recurse;

#[recurse]
mod ast {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast(crate::ast::B)]
    pub enum A<S> {
        Me(Box<A<S>>),
        ToB(Box<B<S>>),
        Lit(PhantomData<S>),
    }

    #[derive(Ast)]
    #[subast(crate::ast::A)]
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
    fn visit_a(&mut self, i: &ast::A<S>) {
        self.a += 1;
        v_ast::visit_a(self, i);
    }
    fn visit_b(&mut self, i: &ast::B<S>) {
        self.b += 1;
        v_ast::visit_b(self, i);
    }
}

#[test]
fn each_root_keeps_its_own_depth() {
    // A(outer) -> ToB(B) -> ToA(A) -> Lit. The visitor descends both types and counts independently.
    let v: ast::A<()> =
        ast::A::ToB(Box::new(ast::B::ToA(Box::new(ast::A::Lit(PhantomData)))));
    let mut c = Counter::default();
    v_ast::Visit::visit_a(&mut c, &v);
    assert_eq!((c.a, c.b), (2, 1), "two A nodes (outer + inner) and one B node");
}

#[test]
fn visit_from_either_root() {
    // Pure-B nesting B -> Me(B) -> Lit, entered through visit_b.
    let v: ast::B<()> = ast::B::Me(Box::new(ast::B::Lit(PhantomData)));
    let mut c = Counter::default();
    v_ast::Visit::visit_b(&mut c, &v);
    assert_eq!((c.a, c.b), (0, 2), "two B nodes, no A");
}

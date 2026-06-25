//! Validates the CLAUDE.md multi-root example: visitor!() over a #[recurse] cycle with TWO roots
//! (A and B both self-referential). Each visit_* is generic over both roots' depth.
#![allow(dead_code)]
use core::marker::PhantomData;
use syan::parse::recurse;

#[recurse]
mod ast {
    use core::marker::PhantomData;
    use syan::visit::Ast;
    #[derive(Ast)] #[subast()]
    pub enum A<S> { SelfA(Box<A<S>>), ToB(Box<B<S>>), Lit(PhantomData<S>) }
    #[derive(Ast)] #[subast()]
    pub enum B<S> { ToA(Box<A<S>>), SelfB(Box<B<S>>), Lit(PhantomData<S>) }
}

mod v { syan::visit::visitor!(crate::ast::A, crate::ast::B); }

#[derive(Default)]
struct C { a: usize, b: usize }
impl<S> v::Visit<S> for C {
    fn visit_a<R0: v::VisitRec<S, Self>, R1: v::VisitRec<S, Self>>(&mut self, i: &v::ANode<S, R0, R1>) {
        self.a += 1; v::visit_a(self, i);
    }
    fn visit_b<R0: v::VisitRec<S, Self>, R1: v::VisitRec<S, Self>>(&mut self, i: &v::BNode<S, R0, R1>) {
        self.b += 1; v::visit_b(self, i);
    }
}

#[test]
fn multiroot_via_visitor() {
    // A -> ToB(B) -> ToA(A) -> SelfA(A) -> Lit
    let x: ast::A<()> = ast::A::ToB(Box::new(v::BNode::ToA(Box::new(
        v::ANode::SelfA(Box::new(v::ANode::Lit(PhantomData))),
    ))));
    let mut c = C::default();
    v::Visit::visit_a(&mut c, &x);
    // A(outer) + A(via ToA) + A(via SelfA) = 3; B(via ToB) = 1
    assert_eq!((c.a, c.b), (3, 1));
}

//! visitor!() over a former-#[recurse] cycle with two mutually-referential, self-referential types
//! (A and B). With natural types this is an ordinary acyclic visitor (one `visit_*` per type).
#![allow(dead_code)]
use core::marker::PhantomData;
use syan::parse::recurse;

#[recurse]
mod ast {
    use core::marker::PhantomData;
    use syan::visit::Ast;
    #[derive(Ast)] #[subast(crate::ast::B)]
    #[allow(clippy::enum_variant_names)] // `SelfA`/`ToB` deliberately name the self/cross edges
    pub enum A<S> { SelfA(Box<A<S>>), ToB(Box<B<S>>), Lit(PhantomData<S>) }
    #[derive(Ast)] #[subast(crate::ast::A)]
    #[allow(clippy::enum_variant_names)]
    pub enum B<S> { ToA(Box<A<S>>), SelfB(Box<B<S>>), Lit(PhantomData<S>) }
}

mod v { syan::visit::visitor!(crate::ast::A, crate::ast::B); }

#[derive(Default)]
struct C { a: usize, b: usize }
impl<S> v::Visit<S> for C {
    fn visit_a(&mut self, i: &ast::A<S>) {
        self.a += 1; v::visit_a(self, i);
    }
    fn visit_b(&mut self, i: &ast::B<S>) {
        self.b += 1; v::visit_b(self, i);
    }
}

#[test]
fn multiroot_via_visitor() {
    // A -> ToB(B) -> ToA(A) -> SelfA(A) -> Lit
    let x: ast::A<()> = ast::A::ToB(Box::new(ast::B::ToA(Box::new(
        ast::A::SelfA(Box::new(ast::A::Lit(PhantomData))),
    ))));
    let mut c = C::default();
    v::Visit::visit_a(&mut c, &x);
    // A(outer) + A(via ToA) + A(via SelfA) = 3; B(via ToB) = 1
    assert_eq!((c.a, c.b), (3, 1));
}

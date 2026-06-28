//! Cross-crate: a `visitor!()` here (downstream) over an UPSTREAM former-`#[recurse]` cycle
//! (`syan_rust::recursed`). The upstream crate emits natural recursive types + their `#[derive(Ast)]`
//! metadata (no in-crate visitor); this crate fetches that metadata and generates an acyclic visitor.
//! The visited types are foreign, so there is no inherent `.visit()` — use `Visit::visit_*` (an
//! inherent impl on a foreign type would be E0116).
#![allow(dead_code)]

use core::marker::PhantomData;

mod v {
    syan::visit::visitor!(syan_rust::recursed::Expr, syan_rust::recursed::Stmt);
}

#[derive(Default)]
struct C {
    e: usize,
    s: usize,
}

impl<S> v::Visit<S> for C {
    fn visit_expr(&mut self, i: &syan_rust::recursed::Expr<S>) {
        self.e += 1;
        v::visit_expr(self, i);
    }
    fn visit_stmt(&mut self, i: &syan_rust::recursed::Stmt<S>) {
        self.s += 1;
        v::visit_stmt(self, i);
    }
}

#[test]
fn downstream_visitor_over_upstream_recurse() {
    let e: syan_rust::recursed::Expr<()> = syan_rust::recursed::Expr::Stmt(Box::new(
        syan_rust::recursed::Stmt::Expr(Box::new(syan_rust::recursed::Expr::Lit(PhantomData))),
    ));
    let mut c = C::default();
    v::Visit::visit_expr(&mut c, &e);
    assert_eq!((c.e, c.s), (2, 1));
}

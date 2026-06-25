//! Cross-crate: a `visitor!()` here (downstream) over an UPSTREAM `#[recurse]` cycle
//! (`syan_rust::recursed`). The upstream crate emits only the `#[recurse]` types + `@recurse`
//! metadata (no in-crate visitor); this crate fetches that metadata and generates the depth-generic
//! visitor. Validates the `$crate`-rooted `@node`/`@terms` paths resolving back to `syan_rust`.
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
    fn visit_expr<R: v::VisitRec<S, Self>>(&mut self, i: &v::ExprNode<S, R>) {
        self.e += 1;
        v::visit_expr(self, i);
    }
    fn visit_stmt<R: v::VisitRec<S, Self>>(&mut self, i: &v::StmtNode<S, R>) {
        self.s += 1;
        v::visit_stmt(self, i);
    }
}

#[test]
fn downstream_visitor_over_upstream_recurse() {
    let e: syan_rust::recursed::Expr<()> = syan_rust::recursed::Expr::Stmt(Box::new(
        v::StmtNode::Expr(Box::new(v::ExprNode::Lit(PhantomData))),
    ));
    let mut c = C::default();
    v::Visit::visit_expr(&mut c, &e);
    assert_eq!((c.e, c.s), (2, 1));
}

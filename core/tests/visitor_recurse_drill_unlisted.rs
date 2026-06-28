//! Feature: `visitor!()` over a `#[recurse]` cycle DRILLS through an *unlisted* cross-edge cycle type
//! (it gets no `visit_*`), reaching the listed types nested inside it — mirroring the acyclic drill-in.
//! With the natural-type design the cycle is acyclic and the visitor is ordinary.
//!
//! Here `Expr` is the root (self-referential via `Bin`) and the only listed type; `Cast` is an
//! unlisted cross-edge. `visit_expr` drills through `Cast` to reach the inner `Expr`.
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::parse::recurse;

#[recurse]
mod ast {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast(crate::ast::Cast)]
    pub enum Expr<S> {
        Bin(Box<Expr<S>>),  // self-reference → Expr is the root
        Cast(Box<Cast<S>>), // cross-edge to the UNLISTED Cast
        Lit(PhantomData<S>),
    }

    #[derive(Ast)]
    #[subast(crate::ast::Expr)]
    pub enum Cast<S> {
        Inner(Box<Expr<S>>), // ref to the root Expr — reached by drilling through the unlisted Cast
        Nope(PhantomData<S>),
    }
}

mod v {
    // Cast is NOT listed → it must be drilled through.
    syan::visit::visitor!(crate::ast::Expr);
}

#[derive(Default)]
struct Counter(usize);

impl<S> v::Visit<S> for Counter {
    fn visit_expr(&mut self, i: &ast::Expr<S>) {
        self.0 += 1;
        v::visit_expr(self, i);
    }
}

#[test]
fn drills_through_unlisted_cast() {
    // Expr::Cast( Cast::Inner( Expr::Lit ) ) → outer Expr + (drill Cast, no visit_cast) + inner Expr.
    let e: ast::Expr<()> =
        ast::Expr::Cast(Box::new(ast::Cast::Inner(Box::new(ast::Expr::Lit(PhantomData)))));
    let mut c = Counter::default();
    v::Visit::visit_expr(&mut c, &e);
    assert_eq!(c.0, 2, "outer Expr + inner Expr reached by drilling through the unlisted Cast");
}

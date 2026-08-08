//! `visitor!()` over a **heterogeneous** `#[recurse]` cycle: the cycle types carry DIFFERENT generic
//! params. `Expr<S>` is the root; `Stmt<S, T>` carries an extra `T` (filled concretely by the
//! cross-edge `Box<Stmt<S, u8>>`). The generated visitor is keyed on the ROOT's params (`v::Visit<S>`),
//! and `Stmt`'s extra `T` becomes a generic on `visit_stmt` (`visit_stmt<T, R>`). (The `het` module in
//! `recurse_generics.rs` exercises the same heterogeneous shape within the generics-focused suite.)
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::parse::recurse;

#[recurse]
mod ast {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast(crate::ast::Stmt)]
    pub enum Expr<S> {
        Stmt(Box<Stmt<S, u8>>), // cross-edge to Stmt, filling its extra param T = u8
        Lit(PhantomData<S>),
    }

    #[derive(Ast)]
    #[subast(crate::ast::Expr)]
    pub enum Stmt<S, T> {
        Back(Box<Expr<S>>), // back-edge to the root Expr → drives via the depth param
        Tag(PhantomData<(S, T)>),
    }
}

mod v {
    syan::visit::visitor!(crate::ast::Expr, crate::ast::Stmt);
}

#[derive(Default)]
struct Counter(usize);

// `v::Visit` is keyed on the ROOT's `S` only; `Stmt`'s extra `T` is a generic on `visit_stmt`.
impl<S> v::Visit<S> for Counter {
    fn visit_expr<R: v::VisitRec<S, Self>>(&mut self, i: &v::ExprNode<S, R>) {
        self.0 += 1;
        v::visit_expr(self, i);
    }
    fn visit_stmt<T, R: v::VisitRec<S, Self>>(&mut self, i: &v::StmtNode<S, T, R>) {
        self.0 += 1;
        v::visit_stmt(self, i);
    }
}

#[test]
fn heterogeneous_cycle_via_visitor() {
    // Expr -> Stmt<_, u8> (cross) -> Expr (back-edge) -> Lit.
    let e: ast::Expr<()> = ast::Expr::Stmt(Box::new(v::StmtNode::Back(Box::new(v::ExprNode::Lit(
        PhantomData,
    )))));
    let mut c = Counter::default();
    v::Visit::visit_expr(&mut c, &e);
    assert_eq!(c.0, 3, "Expr + Stmt (extra param T=u8) + inner Expr");
}

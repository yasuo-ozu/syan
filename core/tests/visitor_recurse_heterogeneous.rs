//! `visitor!()` over a **heterogeneous** former-`#[recurse]` cycle: the cycle types carry DIFFERENT
//! generic params. `Expr<S>` is the root; `Stmt<S, T>` carries an extra `T`, concrete-filled by the
//! cross-edge `Box<Stmt<S, u8>>`. With natural types the visitor is acyclic; because `T` is a
//! *non-shared* param that is *concrete-filled* in a cross-edge, the generated visitor keys its trait
//! on the shared `S` and makes `T` a **method generic** on `visit_stmt` (`visit_stmt<T>`). That mode is
//! struct-only (a closure can't be `for<T>` generic).
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
        Back(Box<Expr<S>>), // back-edge to the root Expr
        Tag(PhantomData<(S, T)>),
    }
}

mod v {
    syan::visit::visitor!(crate::ast::Expr, crate::ast::Stmt);
}

#[derive(Default)]
struct Counter(usize);

// `v::Visit` is keyed on the shared `S`; `Stmt`'s extra `T` is a generic on `visit_stmt`.
impl<S> v::Visit<S> for Counter {
    fn visit_expr(&mut self, i: &ast::Expr<S>) {
        self.0 += 1;
        v::visit_expr(self, i);
    }
    fn visit_stmt<T>(&mut self, i: &ast::Stmt<S, T>) {
        self.0 += 1;
        v::visit_stmt(self, i);
    }
}

#[test]
fn heterogeneous_cycle_via_visitor() {
    // Expr -> Stmt<_, u8> (cross) -> Expr (back-edge) -> Lit.
    let e: ast::Expr<()> =
        ast::Expr::Stmt(Box::new(ast::Stmt::Back(Box::new(ast::Expr::Lit(PhantomData)))));
    let mut c = Counter::default();
    v::Visit::visit_expr(&mut c, &e);
    assert_eq!(c.0, 3, "Expr + Stmt (extra param T=u8) + inner Expr");
}

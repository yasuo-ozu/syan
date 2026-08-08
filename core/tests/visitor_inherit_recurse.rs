//! Inheritance over a former-`#[recurse]` cycle. With natural types the base visitor is an ordinary
//! acyclic visitor, so this is plain supertrait inheritance.
//! (a) An acyclic `New` visitor `visitor!(base => …)` extends the base, adding an outer type whose
//!     field drills into the cycle via the inherited `visit_*`.
//! (b) A second natural cycle extends the base (independent cycles), inheriting `base`'s `visit_*`.
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::parse::recurse;
use syan::visit::Ast;

#[recurse]
mod ast {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum Expr<S> {
        Bin(Box<Expr<S>>),
        Lit(PhantomData<S>),
    }
}

mod base {
    syan::visit::visitor!(crate::ast::Expr);
}

// ── (a) acyclic New extends recurse base ─────────────────────────────────────────
#[derive(Ast)]
#[subast(crate::ast::Expr)]
pub struct Program<S> {
    pub body: ast::Expr<S>,
}

mod nv {
    syan::visit::visitor!(crate::base => crate::Program);
}

#[derive(Default)]
struct Walker {
    p: usize,
    e: usize,
}

impl<S> nv::Visit<S> for Walker {
    fn visit_program(&mut self, i: &Program<S>) {
        self.p += 1;
        nv::visit_program(self, i); // drills body → crosses into the inherited recurse visit_expr
    }
}

impl<S> base::Visit<S> for Walker {
    fn visit_expr(&mut self, i: &ast::Expr<S>) {
        self.e += 1;
        base::visit_expr(self, i);
    }
}

#[test]
fn acyclic_extends_recurse() {
    let prog: Program<()> =
        Program { body: ast::Expr::Bin(Box::new(ast::Expr::Lit(PhantomData))) };
    let mut w = Walker::default();
    prog.visit(&mut w);
    assert_eq!((w.p, w.e), (1, 2), "Program + 2 Exprs (Bin + inner Lit)");
}

// ── (b) recurse New extends recurse base ─────────────────────────────────────────
#[recurse]
mod new_ast {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum Stmt<S> {
        Seq(Box<Stmt<S>>),
        Nop(PhantomData<S>),
    }
}

mod nv2 {
    // a recurse New cycle (Stmt) extending the recurse base (Expr) — its `Visit` is a `base::Visit`
    syan::visit::visitor!(crate::base => crate::new_ast::Stmt);
}

#[derive(Default)]
struct Both {
    e: usize,
    s: usize,
}

impl<S> nv2::Visit<S> for Both {
    fn visit_stmt(&mut self, i: &new_ast::Stmt<S>) {
        self.s += 1;
        nv2::visit_stmt(self, i);
    }
}
impl<S> base::Visit<S> for Both {
    fn visit_expr(&mut self, i: &ast::Expr<S>) {
        self.e += 1;
        base::visit_expr(self, i);
    }
}

#[test]
fn recurse_extends_recurse() {
    // One `Both` (a `nv2::Visit`, hence a `base::Visit`) walks both independent cycles.
    let s: new_ast::Stmt<()> = new_ast::Stmt::Seq(Box::new(new_ast::Stmt::Nop(PhantomData)));
    let e: ast::Expr<()> = ast::Expr::Bin(Box::new(ast::Expr::Lit(PhantomData)));
    let mut b = Both::default();
    nv2::Visit::visit_stmt(&mut b, &s);
    base::Visit::visit_expr(&mut b, &e);
    assert_eq!((b.s, b.e), (2, 2), "2 Stmts + 2 Exprs, one visitor over both cycles");
}

//! Inheritance over a recurse base through an ACYCLIC intermediate:
//! `recurse-base (Expr cycle) => acyclic mid (Program) => acyclic new (Module)`.
//!
//! AUDIT: the `@recbase {}` marker (which forces struct-only inheritance because a recurse base's
//! `visit_*` methods carry `where Self: Sized`) was dropped when an *acyclic* intermediate re-exported
//! its `__syan_visited` macro — `generate_module` hardcoded `recbase = false`. So `new`, consuming
//! `mid`, took the non-struct-only path and emitted the `&mut V` blanket impl + `?Sized` free fn,
//! neither of which can satisfy the transitive recurse supertrait. Fixed by propagating
//! `st.base_is_recurse`.
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

#[derive(Ast)]
#[subast(crate::ast::Expr)]
pub struct Program<S> {
    pub body: ast::Expr<S>,
}

mod mid {
    syan::visit::visitor!(crate::base => crate::Program);
}

#[derive(Ast)]
#[subast(crate::Program)]
pub struct Module<S> {
    pub prog: Program<S>,
}

mod nv {
    // The acyclic link in the chain — must still carry `@recbase` downstream.
    syan::visit::visitor!(crate::mid => crate::Module);
}

#[derive(Default)]
struct Walker {
    m: usize,
    p: usize,
    e: usize,
}

impl<S> nv::Visit<S> for Walker {
    fn visit_module(&mut self, i: &Module<S>) {
        self.m += 1;
        nv::visit_module(self, i);
    }
}
impl<S> mid::Visit<S> for Walker {
    fn visit_program(&mut self, i: &Program<S>) {
        self.p += 1;
        mid::visit_program(self, i);
    }
}
impl<S> base::Visit<S> for Walker {
    fn visit_expr<R: base::VisitRec<S, Self>>(&mut self, i: &base::ExprNode<S, R>) {
        self.e += 1;
        base::visit_expr(self, i);
    }
}

#[test]
fn three_level_over_recurse_base() {
    let m: Module<()> = Module {
        prog: Program { body: ast::Expr::Bin(Box::new(base::ExprNode::Lit(PhantomData))) },
    };
    let mut w = Walker::default();
    m.visit(&mut w);
    assert_eq!((w.m, w.p, w.e), (1, 1, 2), "Module + Program + 2 Exprs (Bin + inner Lit)");
}

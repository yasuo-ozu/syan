//! Multi-level inheritance over a former-`#[recurse]` cycle through an acyclic intermediate:
//! `base (Expr cycle) => mid (Program) => new (Module)`. With natural types every link is an ordinary
//! acyclic visitor, so this is plain three-level supertrait inheritance.
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
    fn visit_expr(&mut self, i: &ast::Expr<S>) {
        self.e += 1;
        base::visit_expr(self, i);
    }
}

#[test]
fn three_level_over_recurse_base() {
    let m: Module<()> = Module {
        prog: Program { body: ast::Expr::Bin(Box::new(ast::Expr::Lit(PhantomData))) },
    };
    let mut w = Walker::default();
    m.visit(&mut w);
    assert_eq!((w.m, w.p, w.e), (1, 1, 2), "Module + Program + 2 Exprs (Bin + inner Lit)");
}

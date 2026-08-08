//! Formerly a diagnostic wall (`ui/visitor_recurse_mixed_acyclic_extra_param.rs`): a `visitor!()` over
//! a former-`#[recurse]` cycle mixed with an acyclic outer type carrying an extra param the cycle
//! doesn't have. Under the old depth-generic design this was rejected (the `VisitRec` impls were keyed
//! on the cycle roots' params, leaving the extra param unconstrained — E0207). With natural types it's
//! an ordinary union-param acyclic visitor (`Visit<S, T>`), exactly like `visitor_generics.rs`.
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::parse::recurse;

#[recurse]
mod ast {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum Expr<S> {
        Nest(Box<Expr<S>>),
        Lit(PhantomData<S>),
    }
}

// Acyclic outer type with an EXTRA param `T` beyond the cycle's `S`.
#[derive(syan::visit::Ast)]
#[subast(crate::ast::Expr)]
pub struct Program<S, T> {
    pub body: ast::Expr<S>,
    pub tag: PhantomData<T>,
}

mod v {
    syan::visit::visitor!(crate::Program, crate::ast::Expr);
}

#[test]
fn mixed_recurse_with_extra_acyclic_param() {
    let prog: Program<(), u32> = Program {
        body: ast::Expr::Nest(Box::new(ast::Expr::Lit(PhantomData))),
        tag: PhantomData,
    };
    // Closures over the mixed visitor: the tuple infers `T = u32` from `prog`. (Closures now work over
    // a former-recurse cycle — the inherent `.visit()` drills `Program.body` into the cycle's
    // `visit_expr`.)
    let mut p = 0usize;
    let mut e = 0usize;
    prog.visit((
        |_: &Program<(), u32>| p += 1,
        |_: &ast::Expr<()>| e += 1,
    ));
    assert_eq!((p, e), (1, 2), "Program + 2 Exprs (Nest + inner Lit)");
}

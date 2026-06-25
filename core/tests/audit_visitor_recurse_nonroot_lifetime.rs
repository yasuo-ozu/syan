//! AUDIT (param ordering, `visitor!()`-over-`#[recurse]`) — BULK compile test, RED until fixed.
//!
//! `generate_module_mixed` lowers a non-root cycle type's beyond-roots params to `visit_*` method
//! generics (`extra_decl`) and emits them AFTER the root params (`#g_params`), so for root `Expr<S>` +
//! non-root `Stmt<'a, S>` it produces `fn visit_stmt<S, 'a, __V, __R0>(…)` → "lifetime parameters must
//! be declared prior to type and const parameters". An extra *type* or *const* param works (it may
//! legally follow `S`); only an extra *lifetime* trips the ordering rule.
//!
//! This file FAILS TO BUILD today — that ordering error *is* the audit finding. It builds once
//! `extra_decl` lifetimes are emitted lifetime-first (before the root's type params).
#![allow(dead_code)]

use syan::parse::recurse;

#[recurse]
mod m {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum Expr<S> {
        Stmt(Box<Stmt<'static, S>>),
        Lit(PhantomData<S>),
    }

    #[derive(Ast)]
    #[subast()]
    pub enum Stmt<'a, S> {
        Back(Box<Expr<S>>),
        Tag(PhantomData<(&'a (), S)>),
    }
}

mod v {
    syan::visit::visitor!(crate::m::Expr, crate::m::Stmt);
}

#[test]
fn nonroot_extra_lifetime_threads_through() {
    // Reaching this body means `visit_stmt`'s generic list put the extra lifetime before `S`.
}

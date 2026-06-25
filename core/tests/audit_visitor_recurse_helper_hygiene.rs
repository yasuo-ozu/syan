//! AUDIT (hygiene, `visitor!()`-over-`#[recurse]`) — BULK compile test, RED until fixed.
//!
//! The depth-generic visitor (`generate_module_mixed`) emits its helper params (`__V`, `__W`,
//! `__R0`/`__R1`) as literal idents and never fresh-names them, so a cycle/target type that declares a
//! generic param named `__V` (or `__R0` / `__W`) collides → E0403. The acyclic-only path already mints
//! collision-free helpers (`fresh_ident`, see `visitor_hygiene.rs`); the recurse/mixed path doesn't.
//!
//! This file FAILS TO BUILD today — that compile error *is* the audit finding. It builds (and the
//! trivial test below runs) once the recurse/mixed path fresh-names `__V`/`__W`/`__R{i}` against the
//! visited types' param names.
#![allow(dead_code)]

use syan::parse::recurse;

#[recurse]
mod m {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum Expr<S, __V> {
        Nest(Box<Expr<S, __V>>),
        Lit(PhantomData<(S, __V)>),
    }
}

mod v {
    syan::visit::visitor!(crate::m::Expr);
}

#[test]
fn helper_idents_do_not_collide_with_a_cycle_param() {
    // Reaching this body means `visitor!()` expanded without its helper idents colliding on `__V`.
}

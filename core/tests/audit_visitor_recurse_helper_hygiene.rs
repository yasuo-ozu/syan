//! AUDIT A (hygiene, `visitor!()`-over-`#[recurse]`) — fixed → BULK regression test (compiling is the check).
//!
//! The depth-generic visitor (`generate_module_mixed`) used to emit its helper params (`__V`, `__W`,
//! `__R0`/`__R1`) as literal idents, so a cycle/target type declaring a generic param named `__V` (or
//! `__R0` / `__W`) collided → E0403. The acyclic-only path already minted collision-free helpers
//! (`fresh_ident`, see `visitor_hygiene.rs`).
//!
//! Fixed: the recurse/mixed path now fresh-names `__V`/`__W`/`__R{i}` against the visited types' param
//! names, so this (whose cycle type declares a `__V` param) compiles. This test guards the fix.
#![allow(dead_code)]
// `visitor!()` fetching the `#[recurse]` module's metadata misattributes a spurious "unused import"
// to the `use ... recurse;` line; the import is in fact used by `#[recurse]`. Cosmetic span artifact.
#![allow(unused_imports)]

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

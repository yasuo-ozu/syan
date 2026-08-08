//! AUDIT C (param ordering, `visitor!()`-over-`#[recurse]`) — fixed → BULK regression test.
//!
//! `generate_module_mixed` lowers a non-root cycle type's beyond-roots params to `visit_*` method
//! generics and used to emit them AFTER the root params, so for root `Expr<S>` + non-root `Stmt<'a, S>`
//! it produced `fn visit_stmt<S, 'a, __V, __R0>(…)` → "lifetime parameters must be declared prior to
//! type and const parameters". (An extra *type* or *const* param was fine; only an extra *lifetime*
//! tripped the rule.)
//!
//! Fixed: the free fn now emits extra lifetimes before the root's type params (lifetimes-first), so
//! this compiles. This test guards the fix.
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

//! Formerly a diagnostic wall (`ui/audit_visitor_recurse_multicycle_disjoint_params.rs`): one
//! `visitor!()` spanning two independent former-`#[recurse]` cycles whose types have DISJOINT generic
//! params (`Expr<S>` + `Foo<T>`). Under the old depth-generic design this was rejected (the depth trait
//! keyed on the union `{S, T}` was applied to each per-cycle terminator). With natural types it's an
//! ordinary union-param acyclic visitor (`Visit<S, T>`); each `visit_*` uses its own param.
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::parse::recurse;

#[recurse]
mod m {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum Expr<S> {
        Nest(Box<Expr<S>>),
        Lit(PhantomData<S>),
    }

    #[derive(Ast)]
    #[subast()]
    pub enum Foo<T> {
        Nest(Box<Foo<T>>),
        Lit(PhantomData<T>),
    }
}

mod v {
    syan::visit::visitor!(crate::m::Expr, crate::m::Foo);
}

#[test]
fn disjoint_param_cycles_one_visitor() {
    let e: m::Expr<()> = m::Expr::Nest(Box::new(m::Expr::Lit(PhantomData)));
    let f: m::Foo<u8> = m::Foo::Nest(Box::new(m::Foo::Lit(PhantomData)));
    // Closures over both cycles in one pass; the tuple fixes the union `<S = (), T = u8>` from the
    // two closure argument types.
    let mut ec = 0usize;
    let mut fc = 0usize;
    e.visit((|_: &m::Expr<()>| ec += 1, |_: &m::Foo<u8>| fc += 1));
    f.visit((|_: &m::Expr<()>| ec += 1, |_: &m::Foo<u8>| fc += 1));
    assert_eq!((ec, fc), (2, 2), "two Expr nodes + two Foo nodes");
}

//! A `where`-clause on a `#[recurse]` cycle type is **threaded** through the generated engine,
//! conversion (`__ToNat`/`__FromNat`), and delegated `Parse`/`Unparse`/`Spanned` impls — so a recurse
//! cycle may carry param bounds (`where S: Clone`) or a self-referential bound (`where Expr<S>: Marker`,
//! the old "problem 6" shape). Previously these surfaced as a cryptic undischarged `E0277` on the
//! generated items (`ui/audit_recurse_where_clause.rs`, now removed).
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::parse::{recurse, Parse, Unparse};
use template_quote::quote;

// ── (a) a param where-bound (`where S: Clone`) ─────────────────────────────────────────────────────
#[recurse]
mod param_bound {
    use core::marker::PhantomData;
    use syan::parse::{Parse, Unparse};
    use syan::source::proc_macro2::literal::Integer;

    #[derive(Parse, Unparse)]
    pub enum Expr<S>
    where
        S: Clone,
    {
        Lit(Integer, PhantomData<S>),
        Nested(Box<Expr<S>>),
    }
}

#[test]
fn where_param_bound_parse_unparse() {
    let e: param_bound::Expr<()> = Parse::parse(quote! { 5 }).unwrap();
    let mut out = Vec::<proc_macro2::TokenTree>::new();
    e.unparse(&mut (&mut out)).unwrap();
    assert_eq!(out.len(), 1);
}

// ── (b) a self-referential where-bound (`where Expr<S>: Marker`) — the old "problem 6" shape ────────
pub trait Marker {}

#[recurse]
mod self_ref {
    use core::marker::PhantomData;
    use syan::parse::{Parse, Unparse};
    use syan::source::proc_macro2::literal::Integer;

    #[derive(Parse, Unparse)]
    pub enum Expr<S>
    where
        Expr<S>: super::Marker,
    {
        Lit(Integer, PhantomData<S>),
        Nested(Box<Expr<S>>),
    }
}
// The user supplies the bound's impl. (`Expr<S>` is the natural type — depth-uniform — so unlike the old
// alias era there is no "fixed-depth alias" mismatch.)
impl<S> Marker for self_ref::Expr<S> {}

#[test]
fn where_self_referential_bound_parse_unparse() {
    let e: self_ref::Expr<()> = Parse::parse(quote! { 7 }).unwrap();
    let mut out = Vec::<proc_macro2::TokenTree>::new();
    e.unparse(&mut (&mut out)).unwrap();
    assert_eq!(out.len(), 1);
}

// Parses `proc_macro2` tokens throughout; the whole suite is skipped without the optional
// dependency that provides them.
#![cfg(feature = "proc_macro2")]

//! A leaf bound written BARE in the user's own `where`-clause must still work.
//!
//! `#[recurse]` signals "leaf, not a cycle edge" to `decycle` by spelling the bound with a
//! fully-qualified trait path; a bare single-segment path means "edge" and would be rejected as
//! un-rank-lowerable (its head, `Integer`, is not a cycle type). The derive always emits qualified
//! paths, but `append_user_where_predicates` copies the user's clause verbatim — so `contract_impl`
//! qualifies it rather than depending on how it happened to be written. Regression guard: if that
//! normalisation is lost, this file fails with decycle's "target is not a type the ranked engine can
//! rank-lower" abort pointing at the user's own bound.
use syan::parse::Parse;
use template_quote::quote;

#[syan::parse::recurse]
mod ast {
    use core::marker::PhantomData;
    use syan::parse::{Parse, Unparse};
    use syan::source::proc_macro2::literal::Integer;

    #[derive(Parse, Unparse)]
    pub enum Expr<S>
    where
        Integer: Parse<proc_macro2::TokenTree>, // ← bare `Parse`, user-written
    {
        Cons(Integer, Box<Expr<S>>),
        Nil(Integer, PhantomData<S>),
    }
}

#[test]
fn bare_user_written_leaf_bound_still_parses() {
    let e: ast::Expr<()> = Parse::parse(quote! { 1 2 3 }).unwrap();
    let mut depth = 0;
    let mut cur = &e;
    while let ast::Expr::Cons(_, inner) = cur {
        depth += 1;
        cur = inner;
    }
    assert_eq!(depth, 2, "two `Cons` levels then `Nil`");
}

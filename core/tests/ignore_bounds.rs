//! `#[ignore_bounds]` on a field suppresses the synthesized `field_ty: Trait` where-predicate in the
//! `Parse`/`Unparse` derives. Its purpose is *naturally recursive* types: without it, deriving
//! `Unparse` on a mutually-recursive pair adds `Box<Stmt<S>>: Unparse ⇐ … ⇐ Box<Expr<S>>: Unparse ⇐ …`
//! as an infinite where-bound cycle (E0275). With `#[ignore_bounds]` on the recursive-child fields the
//! impls carry only leaf bounds; each body's `.unparse()` call on a child still resolves — coinductively
//! — via the sibling type's own (leaf-bounded) impl. (Was previously a documented no-op.)
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::parse::Unparse;

#[derive(Unparse)]
pub enum Expr<S> {
    Lit(::syan::source::proc_macro2::literal::Integer, PhantomData<S>),
    Nest {
        #[ignore_bounds]
        inner: Box<Stmt<S>>,
    },
}

#[derive(Unparse)]
pub enum Stmt<S> {
    One(::syan::source::proc_macro2::literal::Integer, PhantomData<S>),
    Two {
        #[ignore_bounds]
        e: Box<Expr<S>>,
    },
}

#[test]
fn recursive_unparse_compiles_with_leaf_only_bounds() {
    use syan::source::proc_macro2::literal::Integer;
    // A tree deeper than any fixed bound — natural recursion, no depth limit.
    let deep: Expr<proc_macro2::TokenTree> = Expr::Nest {
        inner: Box::new(Stmt::Two {
            e: Box::new(Expr::Nest {
                inner: Box::new(Stmt::One(
                    Integer { value: "7".into(), suffix: None },
                    PhantomData,
                )),
            }),
        }),
    };
    let mut out = Vec::<proc_macro2::TokenTree>::new();
    deep.unparse(&mut (&mut out)).unwrap();
    assert_eq!(out.len(), 1, "the single `7` literal at the bottom of the tree");
}

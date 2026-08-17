// Parses `proc_macro2` tokens throughout; the whole suite is skipped without the optional
// dependency that provides them.
#![cfg(feature = "proc_macro2")]

//! `#[recurse(structural)]` — route the cycle through decycle's **structural** engine instead of the
//! default ranked one.
//!
//! Structural is a compile-time unroll: each cycle member gets a `#[repr(transparent)]` terminator
//! carrying the impl, and the natural type's impl layout-casts to it. No runtime, no thread-local
//! re-entry registry, no `type-leak` — but a narrower scope than ranked (see the *structural* entry
//! under *Known gaps* in CLAUDE.md).
//!
//! Two things had to be true for this to work at all, and both are worth knowing:
//! - structural adopts an impl by its trait path's **last segment**, so it reads the derive's
//!   fully-qualified impls directly — there is no re-spelling step and hence no `emit_contracts`;
//! - the graph turns on **premise sharing** in structural, exactly as it does in ranked. Without it a
//!   `#[group]` substruct's generated terminator carries no `where`-clause at all and cannot prove
//!   what the member's impl demands (`Integer: Unparse<Atom>` etc.).

use syan::parse::{recurse, Parse, Unparse};
use template_quote::quote;

mod parse_and_unparse {
    use super::*;

    #[recurse(structural)]
    mod ast {
        use syan::literal::Integer;
        use syan::nested::group::GroupBrace;
        use syan::parse::{Parse, Unparse};

        #[derive(Parse, Unparse)]
        pub enum Expr<S> {
            Lit(Integer),
            Block {
                brace: GroupBrace<(), S>,
                #[group(self.brace)]
                inner: Vec<Expr<S>>,
            },
        }
    }

    #[test]
    fn parses_a_leaf() {
        let e: ast::Expr<_> = Parse::parse(quote! { 7 }).unwrap();
        assert!(matches!(e, ast::Expr::Lit(_)));
    }

    #[test]
    fn parses_through_the_cycle() {
        let e: ast::Expr<_> = Parse::parse(quote! { { { 1 } } }).unwrap();
        let ast::Expr::Block { inner, .. } = &e else {
            panic!("expected a block")
        };
        assert_eq!(inner.len(), 1);
        assert!(matches!(&inner[0], ast::Expr::Block { .. }));
    }

    #[test]
    fn round_trips() {
        let e: ast::Expr<_> = Parse::parse(quote! { { 1 } }).unwrap();
        let mut out = Vec::new();
        Unparse::unparse(&e, &mut (&mut out)).unwrap();
        let text: proc_macro2::TokenStream = out.into_iter().collect();
        assert_eq!(text.to_string().replace(' ', ""), "{1}");
    }
}

// A two-type cycle, so the engine has to handle a cross-edge rather than only self-recursion.
mod two_type_cycle {
    use super::*;

    #[recurse(structural)]
    mod ast {
        use syan::literal::Integer;
        use syan::nested::group::GroupBrace;
        use syan::parse::{Parse, Unparse};
        use syan::symbol::Token;

        #[derive(Parse, Unparse)]
        pub enum Expr<S> {
            Lit(Integer),
            Block {
                brace: GroupBrace<(), S>,
                #[group(self.brace)]
                stmts: Vec<Stmt<S>>,
            },
        }

        #[derive(Parse, Unparse)]
        pub enum Stmt<S> {
            Semi { expr: Expr<S>, semi: Token![S => ;] },
            Expr(Expr<S>),
        }
    }

    #[test]
    fn cross_edge_parses() {
        let e: ast::Expr<_> = Parse::parse(quote! { { 1 ; 2 } }).unwrap();
        let ast::Expr::Block { stmts, .. } = &e else {
            panic!("expected a block")
        };
        assert_eq!(stmts.len(), 2);
    }

    #[test]
    fn nested_depth() {
        let e: ast::Expr<_> = Parse::parse(quote! { { { { 9 } } } }).unwrap();
        let mut depth = 0;
        let mut cur = &e;
        while let ast::Expr::Block { stmts, .. } = cur {
            depth += 1;
            match stmts.first() {
                Some(ast::Stmt::Expr(inner)) | Some(ast::Stmt::Semi { expr: inner, .. }) => {
                    cur = inner
                }
                None => break,
            }
        }
        assert_eq!(depth, 3);
    }
}

//! `#[recurse]` engine + Parse behavior: natural types, fixes, no-engine SCCs,
//! where-clauses, problem regressions.
#![allow(dead_code)]
#![allow(unused_imports)]

// Natural recursive types + Parse over mutually-recursive AST cycles.
mod basic {
    use syan::parse::{recurse, Parse};
    use syan::source::proc_macro2::literal::Integer;
    use template_quote::quote;

    #[recurse]
    mod minimal {
        use syan::nested::group::GroupBrace;
        use syan::parse::{Parse, Unparse};
        use syan::source::proc_macro2::literal::Integer;
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

        impl<S> Expr<S> {
            pub fn is_literal(&self) -> bool {
                matches!(self, Self::Lit(_))
            }

            pub fn stmt_count(&self) -> Option<usize> {
                match self {
                    Self::Block { stmts, .. } => Some(stmts.len()),
                    _ => None,
                }
            }
        }

        impl<S> Stmt<S> {
            pub fn get_expr(&self) -> Option<&Expr<S>> {
                match self {
                    Self::Expr(e) => Some(e),
                    Self::Semi { expr, .. } => Some(expr),
                }
            }

            pub fn semi_expr(&self) -> Option<&Expr<S>> {
                if let Self::Semi { expr, .. } = self {
                    Some(expr)
                } else {
                    None
                }
            }
        }
    }

    use minimal::*;

    #[test]
    fn test_literal_expr() {
        let tokens = quote! { 42 };
        let expr: Expr<_> = Parse::parse(tokens).unwrap();
        match expr {
            Expr::Lit(lit) => assert_eq!(lit.value, "42"),
            _ => panic!("expected literal"),
        }
    }

    #[test]
    fn test_nested_block() {
        let tokens = quote! { { { 99 } } };
        let expr: Expr<_> = Parse::parse(tokens).unwrap();
        match expr {
            Expr::Block { stmts, .. } => {
                assert_eq!(stmts.len(), 1);
                assert!(matches!(&stmts[0], Stmt::Expr(_) | Stmt::Semi { .. }));
            }
            _ => panic!("expected block"),
        }
    }

    #[test]
    fn test_is_literal() {
        let tokens = quote! { 42 };
        let expr: Expr<_> = Parse::parse(tokens).unwrap();
        assert!(expr.is_literal());
        assert_eq!(expr.stmt_count(), None);
    }

    #[test]
    fn test_empty_block() {
        let tokens = quote! { {} };
        let expr: Expr<_> = Parse::parse(tokens).unwrap();
        assert!(!expr.is_literal());
        assert_eq!(expr.stmt_count(), Some(0));
    }

    #[test]
    fn test_three_semis() {
        let tokens = quote! { { 1 ; 2 ; 3 ; } };
        let expr: Expr<_> = Parse::parse(tokens).unwrap();
        let Expr::Block { stmts, .. } = expr else {
            panic!("expected block")
        };
        assert_eq!(stmts.len(), 3);
        for stmt in &stmts {
            assert!(matches!(stmt, Stmt::Semi { .. }));
            assert!(stmt.semi_expr().unwrap().is_literal());
        }
    }

    #[test]
    fn test_semi_contains_block() {
        // Exercises the full Expr → Stmt → Expr recursion through field access.
        let tokens = quote! { { { 1 ; 2 } ; } };
        let expr: Expr<_> = Parse::parse(tokens).unwrap();
        let Expr::Block { stmts, .. } = expr else {
            panic!("expected outer block")
        };
        assert_eq!(stmts.len(), 1);
        let inner_expr = stmts[0].semi_expr().expect("expected semi stmt");
        assert_eq!(inner_expr.stmt_count(), Some(2));
    }

    #[test]
    fn test_mixed_stmts() {
        let tokens = quote! { { 1 ; 2 } };
        let expr: Expr<_> = Parse::parse(tokens).unwrap();
        let Expr::Block { stmts, .. } = expr else {
            panic!("expected block")
        };
        assert_eq!(stmts.len(), 2);
        assert!(matches!(&stmts[0], Stmt::Semi { .. }));
        assert!(matches!(&stmts[1], Stmt::Expr(_)));
        assert!(stmts[0].semi_expr().is_some());
        assert!(stmts[1].semi_expr().is_none());
        assert!(stmts[0].get_expr().unwrap().is_literal());
        assert!(stmts[1].get_expr().unwrap().is_literal());
    }

    // Two type params: S is the span, T is the Lit payload. The macro must thread T
    // through the depth chain so __ExprRec<S, T, __Rec> / __ExprDefault<S, T> are
    // parameterised — no E0391 cycle.
    #[recurse]
    mod multi_param {
        use syan::nested::group::GroupBrace;
        use syan::parse::{Parse, Unparse};

        #[derive(Parse, Unparse)]
        pub enum Expr<S, T> {
            Lit(T),
            Block {
                brace: GroupBrace<(), S>,
                #[group(self.brace)]
                inner: Vec<Expr<S, T>>,
            },
        }
    }

    #[test]
    fn test_multi_param_parse_lit() {
        use multi_param::Expr;
        use syan::source::proc_macro2::literal::Integer;

        let tokens = template_quote::quote! { 42 };
        let e: Expr<_, Integer> = Parse::parse(tokens).unwrap();
        match e {
            Expr::Lit(i) => assert_eq!(i.value, "42"),
            _ => panic!("expected Lit"),
        }
    }

    #[test]
    fn test_multi_param_parse_block() {
        use multi_param::Expr;
        use syan::source::proc_macro2::literal::Integer;

        let tokens = template_quote::quote! { { 1 } };
        let e: Expr<_, Integer> = Parse::parse(tokens).unwrap();
        match e {
            Expr::Block { inner, .. } => assert_eq!(inner.len(), 1),
            _ => panic!("expected Block"),
        }
    }
}

// Audit fixes: generic terminator compiles; a foreign field sharing a cycle type's
// last segment is a leaf (membership keys on the FIRST path segment).
mod fixes {
    use syan::parse::recurse;

    #[recurse]
    mod generic_limit1 {
        use syan::nested::group::GroupBrace;
        use syan::parse::{Parse, Unparse};
        use syan::source::proc_macro2::literal::Integer;

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

    // Non-generic cycle: the terminator stays the unit struct; must still compile.
    #[recurse]
    mod nongeneric_limit1 {
        use syan::parse::{Parse, Unparse};
        use syan::source::proc_macro2::literal::Integer;

        #[derive(Parse, Unparse)]
        pub enum E {
            Lit(Integer),
            Nest(Box<E>),
        }
    }

    #[test]
    fn bug6_generic_limit1_compiles() {
        use syan::source::proc_macro2::literal::Integer;
        // Naming the instantiated aliases is the regression check (they failed to compile before).
        let _e: generic_limit1::Expr<()> =
            generic_limit1::Expr::Lit(Integer { value: "1".to_string(), suffix: None });
        let _n: nongeneric_limit1::E =
            nongeneric_limit1::E::Lit(Integer { value: "2".to_string(), suffix: None });
    }

    mod other {
        pub struct Stmt;
    }

    #[recurse]
    mod ast {
        use core::marker::PhantomData;
        use syan::visit::Ast;

        #[derive(Ast)]
        #[subast()]
        pub enum Expr<S> {
            ToStmt(Box<Stmt<S>>),
            Foreign(super::other::Stmt), // unrelated leaf; last segment == cycle type name `Stmt`
            Lit(PhantomData<S>),
        }

        #[derive(Ast)]
        #[subast()]
        pub enum Stmt<S> {
            Back(Box<Expr<S>>),
            Nop(PhantomData<S>),
        }
    }

    mod v_ast {
        syan::visit::visitor!(crate::fixes::ast::Expr, crate::fixes::ast::Stmt);
    }

    #[test]
    fn bug7_foreign_field_sharing_cycle_last_segment_is_a_leaf() {
        // The generated visitor used to mis-call `visit_stmt` on the foreign `super::other::Stmt`;
        // compiling the `visitor!()` is the regression check.
        struct V;
        impl v_ast::Visit<()> for V {}
        let _ = V;
    }
}

// Name hygiene: an Ast-only cycle needs no engine; nonce-stamped internal names never
// collide with a user type named like the terminator.
mod no_engine {
    use core::marker::PhantomData;
    use syan::parse::{recurse, Parse};
    use syan::visit::Ast;
    use template_quote::quote;

    // Engine-free cycle: where-clause + user `ExprTerm` both fine.
    #[recurse]
    mod ast_only {
        use core::marker::PhantomData;
        use syan::visit::Ast;

        #[derive(Ast)]
        #[subast()]
        pub enum Expr<S>
        where
            S: Clone,
        {
            Nest(Box<Expr<S>>),
            Lit(PhantomData<S>),
        }

        pub struct ExprTerm;
    }

    // Engine-needing cycle: `ExprTerm` would have clashed with the old unstamped terminator.
    #[recurse]
    mod engine {
        use core::marker::PhantomData;
        use syan::parse::{Parse, Unparse};

        // `Lit` is tried first: a `Nest`-first grammar is left-recursive, which the now-unbounded
        // `Parse` re-entry would recurse on forever (a recursive-descent limitation the old depth
        // cap masked by truncating).
        #[derive(Parse, Unparse)]
        pub enum Expr<S> {
            Lit(::syan::source::proc_macro2::literal::Integer, PhantomData<S>),
            Nest(Box<Expr<S>>),
        }

        pub struct ExprTerm;
    }

    fn assert_ast<T: Ast>() {}

    #[test]
    fn ast_only_cycle_with_where_clause_and_exprterm_compiles() {
        assert_ast::<ast_only::Expr<()>>();
        let _e: ast_only::Expr<()> = ast_only::Expr::Lit(PhantomData);
        let _t = ast_only::ExprTerm;
    }

    #[test]
    fn engine_cycle_user_exprterm_does_not_collide() {
        let _t = engine::ExprTerm; // the user's own type, distinct from the nonce-stamped terminator
        let _e: engine::Expr<()> = Parse::parse(quote! { 5 }).unwrap();
    }
}

// A where-clause on a cycle type is threaded through the engine, conversions, and
// delegated impls — param bounds and self-referential bounds both work.
mod where_clause {
    use syan::parse::{recurse, Parse, Unparse};
    use template_quote::quote;

    // (a) a param where-bound (`where S: Clone`).
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

    // (b) a self-referential where-bound (`where Expr<S>: Marker`) — the old "problem 6" shape.
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
    // The user supplies the bound's impl (`Expr<S>` is the depth-uniform natural type).
    impl<S> Marker for self_ref::Expr<S> {}

    #[test]
    fn where_self_referential_bound_parse_unparse() {
        let e: self_ref::Expr<()> = Parse::parse(quote! { 7 }).unwrap();
        let mut out = Vec::<proc_macro2::TokenTree>::new();
        e.unparse(&mut (&mut out)).unwrap();
        assert_eq!(out.len(), 1);
    }
}

// Problem-regression compile-fail suite + non-conventional span-param warning.
mod problems {
    use syan::parse::{recurse, Parse};
    use template_quote::quote;

    #[test]
    fn compile_fail_problems() {
        let t = trybuild::TestCases::new();
        t.compile_fail("tests/ui/problem1_trait_impl.rs");
        t.compile_fail("tests/ui/problem3_pub_crate.rs");
        t.compile_fail("tests/ui/problem5_multiple_roots.rs");
        t.compile_fail("tests/ui/problem7_multiseg_path.rs");

        // `#[recurse]` takes no arguments; passing any argument is a clean compile error.
        t.compile_fail("tests/ui/recurse_takes_no_args.rs");
        // A cycle type missing one of the ROOT's params is rejected, naming it (the depth default
        // must be spellable).
        t.compile_fail("tests/ui/recurse_missing_root_param.rs");
        // A multi-root cycle whose self-referential roots are not a feedback vertex set is rejected
        // with a clear message.
        t.compile_fail("tests/ui/recurse_multiroot_rootless_subcycle.rs");
        // A non-identity generic argument on a back-edge to the root (`Expr<Vec<S>>`) is rejected —
        // the single-`__Rec` depth machinery can't thread it.
        t.compile_fail("tests/ui/recurse_complex_root_param.rs");
        // A rootless sub-cycle with ≤1 self-referential root is rejected (the `subgraph_is_cyclic`
        // guard runs on the single-root path too).
        t.compile_fail("tests/ui/recurse_rootless_subcycle_single_root.rs");
    }

    // When the first type parameter is not named `S`/`Span`, #[recurse] warns at that param's
    // span; the module still compiles.
    #[recurse]
    mod non_conventional_span_param {
        use syan::nested::group::GroupBrace;
        use syan::parse::{Parse, Unparse};
        use syan::source::proc_macro2::literal::Integer;

        #[derive(Parse, Unparse)]
        pub enum Value<Atom> {
            Lit(Integer),
            Block {
                brace: GroupBrace<(), Atom>,
                #[group(self.brace)]
                inner: Vec<Value<Atom>>,
            },
        }
    }

    #[test]
    fn non_conventional_param_still_compiles() {
        use non_conventional_span_param::Value;
        let tokens = quote! { 7 };
        let v: Value<_> = Parse::parse(tokens).unwrap();
        assert!(matches!(v, Value::Lit(_)));
    }
}

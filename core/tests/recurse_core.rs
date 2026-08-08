//! `#[recurse]` end-to-end behaviour: natural types, generic/non-generic cycles, cycles deriving no
//! routed trait, where-clause threading, and the cycle-detection scoping rules.
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

    // Two type params: `S` is the span, `T` the `Lit` payload. Both must be threaded through the
    // generated impls; `T` is a leaf payload, not a cycle edge.
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

// Audit fixes: generic and non-generic cycles compile; a foreign field sharing a cycle type's
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

    // Non-generic cycle: no type parameters to thread anywhere; must still compile.
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
    fn generic_and_non_generic_cycles_compile() {
        use syan::source::proc_macro2::literal::Integer;
        // Constructing a value of each is the check: both shapes must survive expansion.
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

// A cycle deriving NO routed trait (`Ast` only) has no obligation for decycle to break, so
// `#[recurse]` emits the module directly instead of handing it over — the `used_traits.is_empty()`
// path in `emit`. A `where`-clause on the cycle type must survive that path intact.
//
// (This module previously also asserted that a user type named `ExprTerm` did not collide with the
// generated terminator. The depth engine and its terminators are gone, so `ExprTerm` is now an
// ordinary name with no special meaning and there is nothing left to collide with.)
mod no_routed_trait {
    use core::marker::PhantomData;
    use syan::parse::recurse;
    use syan::visit::Ast;

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
    }

    fn assert_ast<T: Ast>() {}

    #[test]
    fn ast_only_cycle_with_where_clause_compiles() {
        assert_ast::<ast_only::Expr<()>>();
        let _e: ast_only::Expr<()> = ast_only::Expr::Lit(PhantomData);
    }
}
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
        // `problem3`/`problem7` pin SCOPING rules of `#[recurse]`'s cycle detection: a `pub(crate)`
        // type and a multi-segment path are both invisible to it, so the cycle is never detected and
        // rustc rejects the type itself. (`problem1`/`problem5` were retired with the finite-size
        // guard — once that abort was gone they asserted nothing but rustc's own output for an
        // infinite-size type, whose diagnostic ordering is not deterministic; they failed
        // intermittently on identical runs.)
        t.compile_fail("tests/ui/problem3_pub_crate.rs");
        // NOTE: `problem7_multiseg_path.rs` was retired, and the shape it covered is now a KNOWN GAP
        // rather than a supported rejection. It asserted that a cycle edge written through a re-export
        // (`inner::Expr`, where `inner` is `pub use super::Expr`) was not seen as a reference to
        // `Expr`, because syan keyed a reference on a path's FIRST segment — so no cycle was detected
        // and rustc rejected the by-value type on its own. Reading the cycle from the derive's
        // where-bounds keys on the head type instead (the LAST segment), so such an edge is now
        // *detected* — but decycle cannot rank it: a ranked bound needs a bare module-local head, and
        // `inner::Expr` is not one (`E0107` + an unsatisfied `shadowing_module::…` bound). It cannot be
        // pinned as a golden either: the resulting error set is order-dependent (measured: two distinct
        // outputs over three clean runs). Spell a cycle edge with a direct path.

        // `#[recurse]` takes no arguments; passing any argument is a clean compile error.
        t.compile_fail("tests/ui/recurse_takes_no_args.rs");
        // Non-regular recursion (a cycle edge wrapping the referring type's own parameter). syan no
        // longer pre-checks it — the rejection is decycle's `REACHABLE_OBLIGATIONS_CAP` abort, which
        // is matchable because decycle's nesting alias is deterministic (it was not when syan owned
        // the aliasing and keyed it on a random nonce).
        t.compile_fail("tests/ui/recurse_nonregular_arg.rs");

        // NOTE: four `ui/recurse_*` cases were retired when the depth engine was replaced by
        // `decycle` (see CLAUDE.md). They asserted engine-era diagnostics about the *recursion root*
        // and the depth parameter `__Rec` — a natural recursive type has neither, so the macro no
        // longer rejects those shapes. The multi-root / rootless-sub-cycle shapes are now simply
        // supported (see `no_root::rootless_subcycle_is_supported` below); a non-identity back-edge
        // (`Box<Expr<Vec<S>>>`) is no longer a macro-time error and instead fails, if it is ever
        // instantiated, as an ordinary rustc recursion-limit error.
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

// Shapes the old depth engine had to reject, now simply supported: with natural recursive types
// there is no "recursion root" and no depth parameter, so a cycle needs no feedback-vertex-set of
// self-referential types. Here `A` and `B` self-reference but the `C -> D -> C` sub-cycle touches
// neither — which the engine's depth machinery could not terminate, and decycle handles as an
// ordinary multi-type cycle.
mod no_root {
    use syan::parse::{recurse, Parse, Unparse};
    use template_quote::quote;

    #[recurse]
    mod ast {
        use core::marker::PhantomData;
        use syan::parse::{Parse, Unparse};
        use syan::source::proc_macro2::literal::Integer;

        #[derive(Parse, Unparse)]
        pub enum A<S> {
            Me(Box<A<S>>),
            ToC(Box<C<S>>),
            Lit(Integer, PhantomData<S>),
        }

        // Each hop consumes a token first — a cycle that consumed nothing would be genuine left
        // recursion and would recurse forever, which is the honest recursive-descent behaviour and
        // unrelated to how the obligation cycle is broken.
        #[derive(Parse, Unparse)]
        pub enum C<S> {
            ToD(Integer, Box<D<S>>),
            Lit(Integer, PhantomData<S>),
        }

        #[derive(Parse, Unparse)]
        pub enum D<S> {
            ToC(Integer, Box<C<S>>),
            Lit(Integer, PhantomData<S>),
        }
    }

    #[test]
    fn rootless_subcycle_is_supported() {
        // `C <-> D` never passes through a self-referential type; it still parses and round-trips.
        let v: ast::C<()> = Parse::parse(quote! { 1 2 3 }).unwrap();
        assert!(matches!(v, ast::C::ToD(_, _)), "descended C -> D -> C");
        let mut out = Vec::<proc_macro2::TokenTree>::new();
        v.unparse(&mut (&mut out)).unwrap();
        assert_eq!(out.len(), 3, "round-trips all three literals");
    }
}


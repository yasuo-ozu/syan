use syan::parse::{recurse, Parse};
use syan::source::proc_macro2::literal::Integer;
use template_quote::quote;

// Minimal mutual recursion: Expr contains Stmt contains Expr
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

        /// Returns a reference to the `expr` field of `Stmt::Semi { expr, semi }`.
        /// Directly accesses the named field whose type was `Expr<S>` (replaced by `__Rec`).
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
fn test_block_with_stmts() {
    let tokens = quote! { { 1 ; 2 } };
    let expr: Expr<_> = Parse::parse(tokens).unwrap();
    match expr {
        Expr::Block { stmts, .. } => assert_eq!(stmts.len(), 2),
        _ => panic!("expected block"),
    }
}

#[test]
fn test_nested_block() {
    // { { 99 } } — outer block with one Stmt::Expr containing the inner block.
    // The inner expression has a shallower depth type so we only check one level.
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
fn test_stmt_count() {
    let tokens = quote! { { 1 ; 2 } };
    let expr: Expr<_> = Parse::parse(tokens).unwrap();
    assert!(!expr.is_literal());
    assert_eq!(expr.stmt_count(), Some(2));
}

#[test]
fn test_get_expr() {
    let tokens = quote! { { 42 } };
    let expr: Expr<_> = Parse::parse(tokens).unwrap();
    let Expr::Block { stmts, .. } = expr else {
        panic!("expected block")
    };
    let inner = stmts[0].get_expr().expect("expected expr");
    assert!(inner.is_literal());
}

#[test]
fn test_semi_expr_field() {
    // { 42 ; } — one Stmt::Semi whose `expr` field is Lit(42)
    let tokens = quote! { { 42 ; } };
    let expr: Expr<_> = Parse::parse(tokens).unwrap();
    let Expr::Block { stmts, .. } = expr else {
        panic!("expected block")
    };
    // semi_expr() directly accesses the named `expr` field (the __Rec-typed field)
    let inner = stmts[0].semi_expr().expect("expected semi statement");
    assert!(inner.is_literal());
    // A bare Stmt::Expr should return None
    let tokens2 = quote! { { 7 } };
    let expr2: Expr<_> = Parse::parse(tokens2).unwrap();
    let Expr::Block { stmts: stmts2, .. } = expr2 else {
        panic!()
    };
    assert!(stmts2[0].semi_expr().is_none());
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
    // Three semicolon-terminated statements: Stmt::Semi for each
    let tokens = quote! { { 1 ; 2 ; 3 ; } };
    let expr: Expr<_> = Parse::parse(tokens).unwrap();
    let Expr::Block { stmts, .. } = expr else {
        panic!("expected block")
    };
    assert_eq!(stmts.len(), 3);
    for stmt in &stmts {
        assert!(matches!(stmt, Stmt::Semi { .. }));
        // semi_expr() accesses the __Rec-typed `expr` field of each Stmt::Semi
        assert!(stmt.semi_expr().unwrap().is_literal());
    }
}

#[test]
fn test_semi_contains_block() {
    // Outer block has one Stmt::Semi whose `expr` field is itself a block { 1 ; 2 }.
    // This exercises the full Expr → Stmt → Expr recursion chain through field access.
    let tokens = quote! { { { 1 ; 2 } ; } };
    let expr: Expr<_> = Parse::parse(tokens).unwrap();
    let Expr::Block { stmts, .. } = expr else {
        panic!("expected outer block")
    };
    assert_eq!(stmts.len(), 1);
    // semi_expr() gives back the inner block (the __Rec-typed field)
    let inner_expr = stmts[0].semi_expr().expect("expected semi stmt");
    // inner_expr is { 1 ; 2 } — stmt_count navigates into its stmts field
    assert_eq!(inner_expr.stmt_count(), Some(2));
}

#[test]
fn test_direct_init_expr_lit() {
    let lit = Integer {
        value: "123".to_string(),
        suffix: None,
    };
    let expr: Expr<proc_macro2::TokenTree> = Expr::Lit(lit);
    assert!(expr.is_literal());
    assert_eq!(expr.stmt_count(), None);
    match expr {
        Expr::Lit(i) => {
            assert_eq!(i.value, "123");
            assert_eq!(i.suffix, None);
        }
        _ => panic!("expected Lit"),
    }
}

#[test]
fn test_mixed_stmts() {
    // Block with both a semi stmt and a bare expr stmt: { 1 ; 2 }
    // 1 ; → Stmt::Semi, 2 → Stmt::Expr
    let tokens = quote! { { 1 ; 2 } };
    let expr: Expr<_> = Parse::parse(tokens).unwrap();
    let Expr::Block { stmts, .. } = expr else {
        panic!("expected block")
    };
    assert_eq!(stmts.len(), 2);
    assert!(matches!(&stmts[0], Stmt::Semi { .. }));
    assert!(matches!(&stmts[1], Stmt::Expr(_)));
    // semi_expr on the Semi stmt returns something, on the Expr stmt returns None
    assert!(stmts[0].semi_expr().is_some());
    assert!(stmts[1].semi_expr().is_none());
    // get_expr works on both variants
    assert!(stmts[0].get_expr().unwrap().is_literal());
    assert!(stmts[1].get_expr().unwrap().is_literal());
}

// ── multiple type parameters ──────────────────────────────────────────────────

// Expr<S, T> has two type params: S is the span type, T is the Lit payload.
// The macro must thread T through the depth chain so that __ExprRec<S, T, __Rec>
// and __ExprDefault<S, T> are properly parameterised — no E0391 cycle.
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
fn test_multi_param_lit() {
    use multi_param::Expr;
    use syan::source::proc_macro2::literal::Integer;

    let lit: Expr<proc_macro2::TokenTree, Integer> =
        Expr::Lit(Integer { value: "7".to_string(), suffix: None });
    assert!(matches!(lit, Expr::Lit(_)));
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

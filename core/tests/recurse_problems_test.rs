use syan::parse::{recurse, Parse};
use template_quote::quote;

// ── compile-fail tests (problems 1-3, 5, 7, 8) ───────────────────────────────

#[test]
fn compile_fail_problems() {
    let t = trybuild::TestCases::new();
    // Problem 1: trait impl for a cycle type is not transformed
    t.compile_fail("tests/ui/problem1_trait_impl.rs");
    // Problem 2: free recursive function not transformed (type mismatch on inner field)
    t.compile_fail("tests/ui/problem2_free_fn.rs");
    // Problem 3: pub(crate) types invisible to cycle detection → infinite-size error
    t.compile_fail("tests/ui/problem3_pub_crate.rs");
    // Problem 5: both types self-referential → all fields become __Rec → S unused
    t.compile_fail("tests/ui/problem5_multiple_roots.rs");
    // Problem 7: multi-segment path not detected as cycle reference → infinite type
    t.compile_fail("tests/ui/problem7_multiseg_path.rs");
    // Problem 8: qself in return type not transformed → type mismatch
    t.compile_fail("tests/ui/problem8_qself.rs");
}

// Problem 6: where clauses on cycle types are not updated by #[recurse].
// Additionally, the current #[derive(Parse, Unparse)] asserts `where_clause.is_none()`,
// so any where clause on a cycle type causes a derive panic before the recurse
// transformation even runs.  The issue is documented in tests/ui/problem6_where_clause.rs;
// no runtime assertion is possible until both limitations are resolved.

// ── fix 10: #[recurse(limit = N)] ────────────────────────────────────────────

#[recurse(limit = 2)]
mod shallow {
    use syan::nested::group::GroupBrace;
    use syan::parse::{Parse, Unparse};
    use syan::source::proc_macro2::literal::Integer;

    #[derive(Parse, Unparse)]
    pub enum Expr<S> {
        Lit(Integer),
        // GroupBrace keeps `S` in a non-recursive field so the type param is used.
        Block {
            brace: GroupBrace<(), S>,
            #[group(self.brace)]
            inner: Vec<Expr<S>>,
        },
    }
}

#[test]
fn limit_controls_max_depth() {
    use shallow::Expr;

    // limit=2 → Expr<S> = __ExprRec<S, __ExprRec<S, ExprTerm>>

    // depth 1: bare literal — Lit variant always succeeds
    let e: Expr<_> = Parse::parse(quote! { 42 }).unwrap();
    assert!(matches!(e, Expr::Lit(_)));

    // depth 2: block containing a literal — within limit, inner has 1 element
    let e: Expr<_> = Parse::parse(quote! { { 42 } }).unwrap();
    match e {
        Expr::Block { inner, .. } => assert_eq!(inner.len(), 1),
        _ => panic!("expected block"),
    }

    // depth 3: Vec<ExprTerm> stops on the first failure and leaves the literal
    // unconsumed inside the group; the parser does not error — it silently drops
    // the innermost content.  This demonstrates that the depth limit is lenient
    // rather than a hard parse error.
    let e: Expr<_> = Parse::parse(quote! { { { 1 } } }).unwrap();
    match e {
        Expr::Block { inner, .. } => {
            // The outer block has one child (the inner block was parsed).
            assert_eq!(inner.len(), 1);
            // inner[0] has type __ExprRec<S, ExprTerm>, not Expr<S>; we cannot
            // inspect its inner.len() without naming the private internal type.
        }
        _ => panic!("expected block"),
    }
}

// ── fix 9: warning for non-conventional span parameter name ──────────────────
// When the first type parameter is NOT named `S` or `Span`, #[recurse] emits a
// warning pointing at that parameter's span.  The module still compiles.

#[recurse]
mod non_conventional_span_param {
    use syan::nested::group::GroupBrace;
    use syan::parse::{Parse, Unparse};
    use syan::source::proc_macro2::literal::Integer;

    // First type parameter is `Atom` → warning emitted at `Atom`'s span.
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

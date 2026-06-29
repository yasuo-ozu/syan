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

// Problem 6 (FIXED): a where-clause on a cycle type is now threaded through the generated engine,
// conversion, and delegated impls — positive regression test in `recurse_where_clause.rs` (both a param
// bound `where S: Clone` and the self-referential `where Expr<S>: Marker` shape).

// ── fix 10: `Parse` is UNBOUNDED (the fixed engine depth is no longer a parse ceiling) ──────────
// `#[recurse]` takes no `limit` argument; the `Parse` engine has a fixed internal depth, but its
// terminator now **re-enters the top-level parser at runtime** (`core::parse::vtable`), so a tree far
// deeper than that fixed depth parses fully — no truncation. (A *group-ful* cycle's `Unparse`/`Spanned`
// remain engine-bounded; this `shallow` cycle is group-ful, so only its `Parse` is exercised here.)

#[recurse]
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
fn parse_is_unbounded() {
    use shallow::Expr;

    // The natural type is uniform at every depth, so we can count the nesting directly.
    fn block_depth<S>(e: &Expr<S>) -> usize {
        match e {
            Expr::Lit(_) => 0,
            Expr::Block { inner, .. } => 1 + inner.first().map(block_depth).unwrap_or(0),
        }
    }

    // depth 0: bare literal — the `Lit` variant.
    let e: Expr<_> = Parse::parse(quote! { 42 }).unwrap();
    assert_eq!(block_depth(&e), 0);

    // depth 8: eight nested blocks around a literal — FAR past the fixed engine depth (4). The old
    // depth-limited engine truncated past 4; the re-entry terminator now parses all eight in full.
    let e: Expr<_> = Parse::parse(quote! { { { { { { { { { 1 } } } } } } } } }).unwrap();
    assert_eq!(block_depth(&e), 8, "all eight nested blocks parsed (no truncation at the engine depth)");
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

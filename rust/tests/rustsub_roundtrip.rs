//! Parse a non-trivial snippet of the `rustsub` Rust subset, unparse it, and check the `to_string()`
//! round-trips; plus a `visitor!` that walks the parsed tree.
use syan::parse::{Parse, Unparse};
use syan_rust::rustsub::ast::{self, Expr};
use syan_rust::rustsub::visit;
use template_quote::quote;

/// The span type when parsing from a `proc_macro2::TokenTree` stream.
type Sp = syan::source::proc_macro2::Span;

fn round_trip(ts: proc_macro2::TokenStream) -> String {
    let e: Expr<Sp> = Parse::parse(ts).unwrap();
    let mut out = Vec::<proc_macro2::TokenTree>::new();
    e.unparse(&mut (&mut out)).unwrap();
    out.into_iter().collect::<proc_macro2::TokenStream>().to_string()
}

#[test]
fn round_trips_a_block_of_statements() {
    // A block with `let` bindings, a parenthesized expr, a nested-block let-value, and an expr statement.
    // (Every statement ends in `;` — a grammar simplification, so a block used as a value/statement is
    // `;`-terminated too.)
    let src = quote! {
        {
            let x = 5 ;
            let y = ( x ) ;
            let z = { let a = 1 ; a ; } ;
            y ;
        }
    };
    assert_eq!(round_trip(src.clone()), src.to_string(), "unparse must reproduce the input tokens");
}

#[test]
fn round_trips_each_construct() {
    for src in [
        quote! { { 1 ; } },
        quote! { { foo ; } },
        quote! { { ( ( 7 ) ) ; } },
        quote! { { let a = b ; } },
        quote! { { { { 0 ; } ; } ; } }, // deeply nested blocks, each `;`-terminated
    ] {
        assert_eq!(round_trip(src.clone()), src.to_string(), "{src}");
    }
}

#[test]
fn parse_deep_parens_is_unbounded() {
    // `rustsub` is **group-ful**, so its `Parse` is delegated through the fixed-depth engine — but the
    // engine's terminator re-enters the top-level parser at runtime, making `Parse` UNBOUNDED. Parsing a
    // 60-deep `( ( … 1 … ) )` (far past the fixed engine depth) succeeds, and exercises backtracking
    // through the re-entry boundaries (the `Expr` enum tries `Block`/`Paren`/`Lit`/`Var` via `dup` at each
    // level). (`Unparse` stays engine-bounded for a group-ful cycle, so this is a parse-depth check.)
    const N: usize = 60;
    let mut inner = quote! { 1 };
    for _ in 0..N {
        inner = quote! { ( #inner ) };
    }
    let e: Expr<Sp> = Parse::parse(inner).expect("deep nested parens parse");
    // Count the `Paren` nesting on the natural type (uniform at every depth).
    fn paren_depth(e: &Expr<Sp>) -> usize {
        match e {
            ast::Expr::Paren { inner, .. } => 1 + paren_depth(inner),
            _ => 0,
        }
    }
    assert_eq!(paren_depth(&e), N, "all {N} paren levels parsed (re-entry past the engine depth)");
}

#[derive(Default)]
struct Counter {
    stmts: usize,
    exprs: usize,
}
impl<S> visit::Visit<S> for Counter {
    fn visit_stmt(&mut self, i: &ast::Stmt<S>) {
        self.stmts += 1;
        visit::visit_stmt(self, i);
    }
    fn visit_expr(&mut self, i: &ast::Expr<S>) {
        self.exprs += 1;
        visit::visit_expr(self, i);
    }
}

#[test]
fn visitor_walks_the_tree() {
    // `{ let x = 5 ; foo ; }` → exprs: Block, Lit(5), Var(foo) = 3; stmts: Let, ExprStmt = 2.
    let e: Expr<Sp> = Parse::parse(quote! { { let x = 5 ; foo ; } }).unwrap();
    let mut c = Counter::default();
    e.visit(&mut c);
    assert_eq!(c.stmts, 2, "two statements");
    assert_eq!(c.exprs, 3, "the block, the `5` literal, and the `foo` variable");
}

#[test]
fn visitor_closure_counts_exprs() {
    let e: Expr<Sp> = Parse::parse(quote! { { ( 1 ) ; 2 ; } }).unwrap();
    let mut exprs = 0usize;
    e.visit(|_: &Expr<Sp>| exprs += 1);
    // Block, Paren, Lit(1), Lit(2) = 4.
    assert_eq!(exprs, 4);
}

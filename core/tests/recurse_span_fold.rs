//! Does a `#[recurse]` cycle's `Spanned` fold INCLUDE the recursive child, or silently drop it?
//! `migrate` on the string source keeps the LARGEST `loc`, so a span covering the nested child has a
//! strictly larger `loc` than one that stops at the parent's own leaf token.
use syan::parse::Parse;
use syan::source::string::Stream;
use syan::span::Spanned;

#[syan::parse::recurse]
mod ast {
    use syan::parse::{Parse, Unparse};
    use syan::source::string::Span;
    use syan::span::{Spanned, WithSpan};
    use syan::symbol::{chars, Symbol};

    // `---+` : each `Nest` consumes one `-`; the terminal consumes `+`.
    #[derive(Parse, Unparse, Spanned)]
    pub enum Expr {
        Nest(WithSpan<Symbol<chars::Minus>, Span>, Box<Expr>),
        End(WithSpan<Symbol<chars::Plus>, Span>),
    }
}

#[test]
fn recursive_child_is_included_in_span_fold() {
    let deep: ast::Expr = Parse::parse(Stream::new("---+".to_string())).unwrap();
    let shallow: ast::Expr = Parse::parse(Stream::new("+".to_string())).unwrap();

    let (deep_loc, shallow_loc) = (deep.span().loc, shallow.span().loc);

    // If the recursive child were dropped from the fold, `deep` would report only its own leading
    // `-` (loc 0) and be indistinguishable from the one-token parse.
    assert!(
        deep_loc > shallow_loc,
        "deep.span().loc = {deep_loc} must exceed shallow.span().loc = {shallow_loc} — \
         otherwise the recursive child is dropped from the span fold"
    );
    assert_eq!(deep_loc, 3, "the span should reach the final `+` at loc 3");
}

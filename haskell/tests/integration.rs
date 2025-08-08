use proc_macro2::TokenStream;
use syan_haskell::{Expression, Literal, Pattern, HaskellSpan, HaskellParse};
use syan::span::WithSpan;
use quote::quote;

#[test]
fn test_parse_integer_literal() {
    let tokens: TokenStream = quote! { 42 };
    let result = WithSpan::<Literal, HaskellSpan>::haskell_parse(tokens);
    
    assert!(result.is_ok());
    let literal = result.unwrap();
    match literal.slot {
        Literal::Integer(42) => {}
        _ => panic!("Expected integer literal 42"),
    }
}

#[test]
fn test_parse_string_literal() {
    let tokens: TokenStream = quote! { "hello" };
    let result = WithSpan::<Literal, HaskellSpan>::haskell_parse(tokens);
    
    assert!(result.is_ok());
    let literal = result.unwrap();
    match literal.slot {
        Literal::String(ref s) if s == "hello" => {}
        _ => panic!("Expected string literal 'hello'"),
    }
}

#[test]
fn test_parse_variable_expression() {
    let tokens: TokenStream = quote! { x };
    let result = WithSpan::<Expression<HaskellSpan>, HaskellSpan>::haskell_parse(tokens);
    
    assert!(result.is_ok());
    let expr = result.unwrap();
    match expr.slot {
        Expression::Var(ref name) if name.slot == "x" => {}
        _ => panic!("Expected variable expression 'x'"),
    }
}

#[test]
fn test_parse_wildcard_pattern() {
    let tokens: TokenStream = quote! { _ };
    let result = WithSpan::<Pattern<HaskellSpan>, HaskellSpan>::haskell_parse(tokens);
    
    assert!(result.is_ok());
    let pattern = result.unwrap();
    match pattern.slot {
        Pattern::Wildcard => {}
        _ => panic!("Expected wildcard pattern"),
    }
}

#[test]
fn test_parse_variable_pattern() {
    let tokens: TokenStream = quote! { x };
    let result = WithSpan::<Pattern<HaskellSpan>, HaskellSpan>::haskell_parse(tokens);
    
    assert!(result.is_ok());
    let pattern = result.unwrap();
    match pattern.slot {
        Pattern::Var(ref name) if name.slot == "x" => {}
        _ => panic!("Expected variable pattern 'x'"),
    }
}
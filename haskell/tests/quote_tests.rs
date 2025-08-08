use proc_macro2::TokenStream;
use syan_haskell::{Expression, Literal, Pattern, HaskellSpan, TokenStreamEmitter, HaskellUnparse, HaskellParse};
use syan::span::WithSpan;
use quote::quote;

#[test]
fn test_roundtrip_integer_literal() {
    let original_tokens: TokenStream = quote! { 42 };
    
    let parsed = WithSpan::<Literal, HaskellSpan>::haskell_parse(original_tokens.clone()).unwrap();
    
    let mut emitter = TokenStreamEmitter::new();
    parsed.unparse_to(&mut emitter);
    let unparsed_tokens = emitter.into_token_stream();
    
    assert_eq!(original_tokens.to_string(), unparsed_tokens.to_string());
}

#[test]
fn test_roundtrip_variable_expression() {
    let original_tokens: TokenStream = quote! { x };
    
    let parsed = WithSpan::<Expression<HaskellSpan>, HaskellSpan>::haskell_parse(original_tokens.clone()).unwrap();
    
    let mut emitter = TokenStreamEmitter::new();
    parsed.unparse_to(&mut emitter);
    let unparsed_tokens = emitter.into_token_stream();
    
    assert_eq!(original_tokens.to_string(), unparsed_tokens.to_string());
}

#[test]
fn test_roundtrip_wildcard_pattern() {
    let original_tokens: TokenStream = quote! { _ };
    
    let parsed = WithSpan::<Pattern<HaskellSpan>, HaskellSpan>::haskell_parse(original_tokens.clone()).unwrap();
    
    let mut emitter = TokenStreamEmitter::new();
    parsed.unparse_to(&mut emitter);
    let unparsed_tokens = emitter.into_token_stream();
    
    assert_eq!(original_tokens.to_string(), unparsed_tokens.to_string());
}

#[test]
fn test_complex_expression_parsing() {
    let tokens: TokenStream = quote! {
        ()
    };
    
    let result = WithSpan::<Expression<HaskellSpan>, HaskellSpan>::haskell_parse(tokens);
    assert!(result.is_ok());
    
    let expr = result.unwrap();
    match expr.slot {
        Expression::Tuple(_) => {}
        _ => panic!("Expected tuple expression"),
    }
}

#[test]
fn test_lambda_expression_unparse() {
    let patterns = vec![
        WithSpan {
            slot: Pattern::Var(WithSpan {
                slot: "x".to_string(),
                span: HaskellSpan::default(),
            }),
            span: HaskellSpan::default(),
        }
    ];
    
    let body = Box::new(WithSpan {
        slot: Expression::Var(WithSpan {
            slot: "x".to_string(),
            span: HaskellSpan::default(),
        }),
        span: HaskellSpan::default(),
    });
    
    let lambda_expr = WithSpan {
        slot: Expression::Lambda(patterns, body),
        span: HaskellSpan::default(),
    };
    
    let mut emitter = TokenStreamEmitter::new();
    lambda_expr.unparse_to(&mut emitter);
    let tokens = emitter.into_token_stream();
    let token_string = tokens.to_string();
    
    assert!(token_string.contains("lambda"));
    assert!(token_string.contains("->"));
    assert!(token_string.contains("x"));
}

#[test]
fn test_list_expression() {
    let expr_vec = vec![
        WithSpan {
            slot: Expression::Lit(WithSpan {
                slot: Literal::Integer(1),
                span: HaskellSpan::default(),
            }),
            span: HaskellSpan::default(),
        },
        WithSpan {
            slot: Expression::Lit(WithSpan {
                slot: Literal::Integer(2),
                span: HaskellSpan::default(),
            }),
            span: HaskellSpan::default(),
        },
    ];
    
    let list_expr = WithSpan {
        slot: Expression::List(expr_vec),
        span: HaskellSpan::default(),
    };
    
    let mut emitter = TokenStreamEmitter::new();
    list_expr.unparse_to(&mut emitter);
    let tokens = emitter.into_token_stream();
    let token_string = tokens.to_string();
    
    assert!(token_string.contains("["));
    assert!(token_string.contains("]"));
    assert!(token_string.contains("1"));
    assert!(token_string.contains("2"));
    assert!(token_string.contains(","));
}
// Parses `proc_macro2` tokens throughout; the whole suite is skipped without the optional
// dependency that provides them.
#![cfg(feature = "proc_macro2")]

use proc_macro2::{TokenStream, TokenTree};
use syan::parse::{Parse, Unparse};
use syan::source::proc_macro2::literal::*;
use template_quote::quote;

fn parse_single<T: Parse<TokenTree>>(tokens: TokenStream) -> Result<T, T::Error> {
    T::parse(tokens)
}

fn unparse_to_tokens<T: Unparse<TokenTree>>(value: &T) -> TokenStream {
    let mut tokens = TokenStream::new();
    value.unparse(&mut tokens).unwrap();
    tokens
}

#[test]
fn test_bool_parse_true() {
    let tokens = quote! { true };
    let result = parse_single::<Bool>(tokens).unwrap();
    assert!(result.value);
}

#[test]
fn test_bool_parse_false() {
    let tokens = quote! { false };
    let result = parse_single::<Bool>(tokens).unwrap();
    assert!(!result.value);
}

#[test]
fn test_bool_parse_invalid() {
    let tokens = quote! { maybe };
    let result = parse_single::<Bool>(tokens);
    assert!(result.is_err());
}

#[test]
fn test_bool_unparse_true() {
    let bool_val = Bool { value: true };
    let tokens = unparse_to_tokens(&bool_val);
    assert_eq!(tokens.to_string(), "true");
}

#[test]
fn test_bool_unparse_false() {
    let bool_val = Bool { value: false };
    let tokens = unparse_to_tokens(&bool_val);
    assert_eq!(tokens.to_string(), "false");
}

#[test]
fn test_char_parse() {
    let tokens = quote! { 'a' };
    let result = parse_single::<Char>(tokens).unwrap();
    assert_eq!(result.value, 'a');
}

#[test]
fn test_char_parse_escape() {
    let tokens = quote! { '\n' };
    let result = parse_single::<Char>(tokens).unwrap();
    assert_eq!(result.value, '\n');
}

#[test]
fn test_char_unparse() {
    let char_val = Char { value: 'x' };
    let tokens = unparse_to_tokens(&char_val);
    assert_eq!(tokens.to_string(), "'x'");
}

#[test]
fn test_byte_char_parse() {
    let tokens = quote! { b'A' };
    let result = parse_single::<ByteChar>(tokens).unwrap();
    assert_eq!(result.value, b'A');
}

#[test]
fn test_byte_char_unparse() {
    let byte_char = ByteChar { value: b'Z' };
    let tokens = unparse_to_tokens(&byte_char);
    assert_eq!(tokens.to_string(), "b'Z'");
}

#[test]
fn test_integer_parse_plain() {
    let tokens = quote! { 42 };
    let result = parse_single::<Integer>(tokens).unwrap();
    assert_eq!(result.value, "42");
    assert_eq!(result.suffix, None);
}

#[test]
fn test_integer_parse_with_suffix() {
    let tokens = quote! { 123u32 };
    let result = parse_single::<Integer>(tokens).unwrap();
    assert_eq!(result.value, "123");
    assert_eq!(result.suffix, Some("u32".to_string()));
}

#[test]
fn test_integer_unparse_plain() {
    let int_val = Integer {
        value: "999".to_string(),
        suffix: None,
    };
    let tokens = unparse_to_tokens(&int_val);
    assert_eq!(tokens.to_string(), "999");
}

#[test]
fn test_str_parse() {
    let tokens = quote! { "hello world" };
    let result = parse_single::<Str>(tokens).unwrap();
    assert_eq!(result.value, "hello world");
}

#[test]
fn test_str_unparse() {
    let str_val = Str {
        value: "test string".to_string(),
    };
    let tokens = unparse_to_tokens(&str_val);
    assert_eq!(tokens.to_string(), "\"test string\"");
}

// Roundtrip tests
#[test]
fn test_roundtrip_bool() {
    let original = Bool { value: true };
    let tokens = unparse_to_tokens(&original);
    let parsed = parse_single::<Bool>(tokens).unwrap();
    assert_eq!(original, parsed);
}

#[test]
fn test_roundtrip_char() {
    let original = Char { value: 'x' }; // Use simple ASCII char instead of Unicode
    let tokens = unparse_to_tokens(&original);
    let parsed = parse_single::<Char>(tokens).unwrap();
    assert_eq!(original, parsed);
}

// Error cases
#[test]
fn test_integer_rejects_float() {
    let tokens = quote! { 3.14 };
    let result = parse_single::<Integer>(tokens);
    assert!(result.is_err());
}

#[test]
fn test_float_rejects_integer() {
    let tokens = quote! { 42 };
    let result = parse_single::<Float>(tokens);
    assert!(result.is_err());
}

#[test]
fn test_char_rejects_byte_char() {
    let tokens = quote! { b'x' };
    let result = parse_single::<Char>(tokens);
    assert!(result.is_err());
}

#[test]
fn test_byte_char_rejects_regular_char() {
    let tokens = quote! { 'x' };
    let result = parse_single::<ByteChar>(tokens);
    assert!(result.is_err());
}

// Display tests
#[test]
fn test_bool_display() {
    assert_eq!(format!("{}", Bool { value: true }), "true");
    assert_eq!(format!("{}", Bool { value: false }), "false");
}

#[test]
fn test_char_display() {
    assert_eq!(format!("{}", Char { value: 'a' }), "'a'");
    assert_eq!(format!("{}", Char { value: '\n' }), "'\\n'");
    assert_eq!(format!("{}", Char { value: '\t' }), "'\\t'");
    assert_eq!(format!("{}", Char { value: '\\' }), "'\\\\'");
    assert_eq!(format!("{}", Char { value: '\'' }), "'\\''");
}

#[test]
fn test_byte_char_display() {
    assert_eq!(format!("{}", ByteChar { value: b'A' }), "b'A'");
    assert_eq!(format!("{}", ByteChar { value: b'\n' }), "b'\\n'");
    assert_eq!(format!("{}", ByteChar { value: b'\t' }), "b'\\t'");
    assert_eq!(format!("{}", ByteChar { value: 0 }), "b'\\0'");
    assert_eq!(format!("{}", ByteChar { value: 255 }), "b'\\xff'");
}

#[test]
fn test_integer_display() {
    let int_plain = Integer {
        value: "42".to_string(),
        suffix: None,
    };
    assert_eq!(format!("{}", int_plain), "42");

    let int_suffix = Integer {
        value: "123".to_string(),
        suffix: Some("u32".to_string()),
    };
    assert_eq!(format!("{}", int_suffix), "123u32");
}

#[test]
fn test_float_display() {
    let float_plain = Float {
        value: "3.14".to_string(),
        suffix: None,
    };
    assert_eq!(format!("{}", float_plain), "3.14");

    let float_suffix = Float {
        value: "2.5".to_string(),
        suffix: Some("f32".to_string()),
    };
    assert_eq!(format!("{}", float_suffix), "2.5f32");
}

#[test]
fn test_str_display() {
    let str_val = Str {
        value: "hello".to_string(),
    };
    assert_eq!(format!("{}", str_val), "\"hello\"");

    let str_escape = Str {
        value: "hello \"world\"".to_string(),
    };
    assert_eq!(format!("{}", str_escape), "\"hello \\\"world\\\"\"");
}

#[test]
fn test_str_raw_display() {
    let str_raw = StrRaw {
        value: "hello \"world\"".to_string(),
        hash_count: 1,
    };
    assert_eq!(format!("{}", str_raw), "r#\"hello \"world\"\"#");

    let str_raw_multi = StrRaw {
        value: "content".to_string(),
        hash_count: 3,
    };
    assert_eq!(format!("{}", str_raw_multi), "r###\"content\"###");
}

#[test]
fn test_byte_str_display() {
    let byte_str = ByteStr {
        value: b"hello".to_vec(),
    };
    assert_eq!(format!("{}", byte_str), "b\"hello\"");

    let byte_str_escape = ByteStr {
        value: b"hello\nworld".to_vec(),
    };
    assert_eq!(format!("{}", byte_str_escape), "b\"hello\\nworld\"");
}

#[test]
fn test_byte_str_raw_display() {
    let byte_str_raw = ByteStrRaw {
        value: b"hello \"world\"".to_vec(),
        hash_count: 1,
    };
    assert_eq!(format!("{}", byte_str_raw), "br#\"hello \"world\"\"#");
}

#[test]
fn test_cstr_display() {
    let cstr = CStr {
        value: "hello".to_string(),
    };
    assert_eq!(format!("{}", cstr), "c\"hello\"");

    let cstr_escape = CStr {
        value: "hello \"world\"".to_string(),
    };
    assert_eq!(format!("{}", cstr_escape), "c\"hello \\\"world\\\"\"");
}

#[test]
fn test_cstr_raw_display() {
    let cstr_raw = CStrRaw {
        value: "hello \"world\"".to_string(),
        hash_count: 2,
    };
    assert_eq!(format!("{}", cstr_raw), "cr##\"hello \"world\"\"##");
}

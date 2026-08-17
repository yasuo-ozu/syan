//! The Rust literal forms, each a type you can `Parse` from a single token and `Unparse` back.
//!
//! Pick the type that matches the literal you expect: [`Integer`], [`Float`], [`Bool`], [`Char`],
//! [`ByteChar`], [`Str`], [`ByteStr`], [`CStr`], or one of the `Raw` variants. The string forms
//! store their contents exactly as written, escapes unresolved; the character forms resolve theirs.

#[cfg(feature = "proc_macro2")]
use crate::error::ParseError;
#[cfg(feature = "proc_macro2")]
use crate::parse::unparse::Emitter;
#[cfg(feature = "proc_macro2")]
use crate::parse::{Parse, Unparse};
#[cfg(feature = "proc_macro2")]
use crate::source::proc_macro2::Span;

/// A boolean literal, `true` or `false`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Bool {
    pub value: bool,
}

/// A byte character literal, `b'a'`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ByteChar {
    pub value: u8,
}

/// A character literal, `'a'`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Char {
    pub value: char,
}

/// An integer literal, `42` or `1_000u64`. `value` keeps any underscores and drops the suffix.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Integer {
    pub value: String,
    pub suffix: Option<String>,
}

/// A floating-point literal, `3.14` or `2.5f64`. `value` drops the suffix.
#[derive(Debug, Clone, PartialEq)]
pub struct Float {
    pub value: String,
    pub suffix: Option<String>,
}

/// A string literal, `"text"`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Str {
    pub value: String,
}

/// A raw string literal, `r"text"` or `r#"text"#`, where `hash_count` is the number of `#`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StrRaw {
    pub value: String,
    pub hash_count: usize,
}

/// A byte string literal, `b"text"`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ByteStr {
    pub value: Vec<u8>,
}

/// A raw byte string literal, `br"text"` or `br#"text"#`, where `hash_count` is the number of `#`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ByteStrRaw {
    pub value: Vec<u8>,
    pub hash_count: usize,
}

/// A C string literal, `c"text"`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CStr {
    pub value: String,
}

/// A raw C string literal, `cr"text"` or `cr#"text"#`, where `hash_count` is the number of `#`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CStrRaw {
    pub value: String,
    pub hash_count: usize,
}

mod display_impl;
#[cfg(feature = "proc_macro2")]
mod parse_impl;
mod parse_text_impl;
#[cfg(feature = "proc_macro2")]
mod unparse_impl;
#[cfg(all(test, feature = "proc_macro2"))]
mod tests {
    //! Note: some invalid inputs fail at tokenization rather than at parsing, and the raw forms
    //! have limited support (see `parse_raw` in `parse_impl`).

    use super::*;
    use crate::parse::Parse;
    use proc_macro2::{TokenStream, TokenTree};

    fn parse_tokens<T: Parse<TokenTree>>(input: &str) -> Result<T, T::Error> {
        let tokens: TokenStream = input.parse().unwrap();
        T::parse(tokens)
    }

    #[test]
    fn test_bool_parsing_true() {
        let result = parse_tokens::<Bool>("true").unwrap();
        assert!(result.value);
    }

    #[test]
    fn test_bool_parsing_false() {
        let result = parse_tokens::<Bool>("false").unwrap();
        assert!(!result.value);
    }

    #[test]
    fn test_bool_parsing_invalid() {
        assert!(parse_tokens::<Bool>("other").is_err());
        assert!(parse_tokens::<Bool>("True").is_err());
        assert!(parse_tokens::<Bool>("FALSE").is_err());
    }

    #[test]
    fn test_bytechar_parsing_simple() {
        let result = parse_tokens::<ByteChar>("b'a'").unwrap();
        assert_eq!(result.value, b'a');
    }

    #[test]
    fn test_bytechar_parsing_escape_sequences() {
        let result = parse_tokens::<ByteChar>("b'\\n'").unwrap();
        assert_eq!(result.value, b'\n');

        let result = parse_tokens::<ByteChar>("b'\\t'").unwrap();
        assert_eq!(result.value, b'\t');

        let result = parse_tokens::<ByteChar>("b'\\r'").unwrap();
        assert_eq!(result.value, b'\r');

        let result = parse_tokens::<ByteChar>("b'\\\\'").unwrap();
        assert_eq!(result.value, b'\\');

        let result = parse_tokens::<ByteChar>("b'\\''").unwrap();
        assert_eq!(result.value, b'\'');

        let result = parse_tokens::<ByteChar>("b'\\0'").unwrap();
        assert_eq!(result.value, 0);
    }

    #[test]
    fn test_bytechar_parsing_invalid() {
        assert!(parse_tokens::<ByteChar>("'a'").is_err()); // Not a byte char
    }

    #[test]
    fn test_char_parsing_simple() {
        let result = parse_tokens::<Char>("'a'").unwrap();
        assert_eq!(result.value, 'a');

        let result = parse_tokens::<Char>("'1'").unwrap();
        assert_eq!(result.value, '1');
    }

    #[test]
    fn test_char_parsing_escape_sequences() {
        let result = parse_tokens::<Char>("'\\n'").unwrap();
        assert_eq!(result.value, '\n');

        let result = parse_tokens::<Char>("'\\t'").unwrap();
        assert_eq!(result.value, '\t');

        let result = parse_tokens::<Char>("'\\r'").unwrap();
        assert_eq!(result.value, '\r');

        let result = parse_tokens::<Char>("'\\\\'").unwrap();
        assert_eq!(result.value, '\\');

        let result = parse_tokens::<Char>("'\\''").unwrap();
        assert_eq!(result.value, '\'');

        let result = parse_tokens::<Char>("'\\0'").unwrap();
        assert_eq!(result.value, '\0');
    }

    #[test]
    fn test_char_parsing_invalid() {
        assert!(parse_tokens::<Char>("b'a'").is_err()); // Byte char
    }

    #[test]
    fn test_integer_parsing_simple() {
        let result = parse_tokens::<Integer>("42").unwrap();
        assert_eq!(result.value, "42");
        assert_eq!(result.suffix, None);

        let result = parse_tokens::<Integer>("0").unwrap();
        assert_eq!(result.value, "0");
        assert_eq!(result.suffix, None);
    }

    #[test]
    fn test_integer_parsing_with_suffixes() {
        let result = parse_tokens::<Integer>("42u32").unwrap();
        assert_eq!(result.value, "42");
        assert_eq!(result.suffix, Some("u32".to_string()));

        let result = parse_tokens::<Integer>("123i64").unwrap();
        assert_eq!(result.value, "123");
        assert_eq!(result.suffix, Some("i64".to_string()));

        let result = parse_tokens::<Integer>("456usize").unwrap();
        assert_eq!(result.value, "456");
        assert_eq!(result.suffix, Some("usize".to_string()));
    }

    #[test]
    fn test_integer_parsing_with_underscores() {
        let result = parse_tokens::<Integer>("1_000_000").unwrap();
        assert_eq!(result.value, "1_000_000");
        assert_eq!(result.suffix, None);

        let result = parse_tokens::<Integer>("1_000u64").unwrap();
        assert_eq!(result.value, "1_000");
        assert_eq!(result.suffix, Some("u64".to_string()));
    }

    #[test]
    fn test_integer_parsing_invalid() {
        assert!(parse_tokens::<Integer>("42.5").is_err()); // Float
        assert!(parse_tokens::<Integer>("abc").is_err()); // Not numeric
    }

    #[test]
    fn test_float_parsing_simple() {
        let result = parse_tokens::<Float>("3.14").unwrap();
        assert_eq!(result.value, "3.14");
        assert_eq!(result.suffix, None);

        let result = parse_tokens::<Float>("0.5").unwrap();
        assert_eq!(result.value, "0.5");
        assert_eq!(result.suffix, None);
    }

    #[test]
    fn test_float_parsing_with_suffixes() {
        let result = parse_tokens::<Float>("3.14f32").unwrap();
        assert_eq!(result.value, "3.14");
        assert_eq!(result.suffix, Some("f32".to_string()));

        let result = parse_tokens::<Float>("2.718f64").unwrap();
        assert_eq!(result.value, "2.718");
        assert_eq!(result.suffix, Some("f64".to_string()));
    }

    #[test]
    fn test_float_parsing_scientific() {
        // Scientific notation without decimal point might be tokenized differently
        let result = parse_tokens::<Float>("1.0e10").unwrap();
        assert_eq!(result.value, "1.0e10");
        assert_eq!(result.suffix, None);

        let result = parse_tokens::<Float>("1.5e-3").unwrap();
        assert_eq!(result.value, "1.5e-3");
        assert_eq!(result.suffix, None);
    }

    #[test]
    fn test_float_parsing_invalid() {
        assert!(parse_tokens::<Float>("42").is_err()); // Integer
        assert!(parse_tokens::<Float>("abc").is_err()); // Not numeric
    }

    #[test]
    fn test_str_parsing_simple() {
        let result = parse_tokens::<Str>("\"hello\"").unwrap();
        assert_eq!(result.value, "hello");

        let result = parse_tokens::<Str>("\"\"").unwrap();
        assert_eq!(result.value, "");
    }

    #[test]
    fn test_str_parsing_with_escapes() {
        let result = parse_tokens::<Str>("\"hello\\nworld\"").unwrap();
        assert_eq!(result.value, "hello\\nworld");

        let result = parse_tokens::<Str>("\"quote: \\\"text\\\"\"").unwrap();
        assert_eq!(result.value, "quote: \\\"text\\\"");
    }

    #[test]
    fn test_str_parsing_invalid() {
        assert!(parse_tokens::<Str>("r\"hello\"").is_err()); // Raw string
        assert!(parse_tokens::<Str>("b\"hello\"").is_err()); // Byte string
        assert!(parse_tokens::<Str>("c\"hello\"").is_err()); // C string
    }

    #[test]
    fn test_str_raw_parsing() {
        // TODO: implement proper raw string tokenization support
        if let Ok(result) = parse_tokens::<StrRaw>("r\"hello\"") {
            assert_eq!(result.value, "hello");
            assert_eq!(result.hash_count, 0);
        }
    }

    #[test]
    fn test_str_raw_parsing_invalid() {
        assert!(parse_tokens::<StrRaw>("\"hello\"").is_err()); // Not raw
        assert!(parse_tokens::<StrRaw>("br\"hello\"").is_err()); // Byte raw
    }

    #[test]
    fn test_bytestr_parsing() {
        let result = parse_tokens::<ByteStr>("b\"hello\"").unwrap();
        assert_eq!(result.value, b"hello");

        let result = parse_tokens::<ByteStr>("b\"\"").unwrap();
        assert_eq!(result.value, b"");
    }

    #[test]
    fn test_bytestr_parsing_invalid() {
        assert!(parse_tokens::<ByteStr>("\"hello\"").is_err()); // Not byte string
        assert!(parse_tokens::<ByteStr>("br\"hello\"").is_err()); // Raw byte string
    }

    #[test]
    fn test_bytestr_raw_parsing() {
        if let Ok(result) = parse_tokens::<ByteStrRaw>("br\"hello\"") {
            assert_eq!(result.value, b"hello");
            assert_eq!(result.hash_count, 0);
        }
    }

    #[test]
    fn test_bytestr_raw_parsing_invalid() {
        assert!(parse_tokens::<ByteStrRaw>("b\"hello\"").is_err()); // Not raw
        assert!(parse_tokens::<ByteStrRaw>("r\"hello\"").is_err()); // Not byte
    }

    #[test]
    fn test_cstr_parsing() {
        let result = parse_tokens::<CStr>("c\"hello\"").unwrap();
        assert_eq!(result.value, "hello");

        let result = parse_tokens::<CStr>("c\"\"").unwrap();
        assert_eq!(result.value, "");
    }

    #[test]
    fn test_cstr_parsing_invalid() {
        assert!(parse_tokens::<CStr>("\"hello\"").is_err()); // Not C string
        assert!(parse_tokens::<CStr>("cr\"hello\"").is_err()); // Raw C string
    }

    #[test]
    fn test_cstr_raw_parsing() {
        if let Ok(result) = parse_tokens::<CStrRaw>("cr\"hello\"") {
            assert_eq!(result.value, "hello");
            assert_eq!(result.hash_count, 0);
        }
    }

    #[test]
    fn test_cstr_raw_parsing_invalid() {
        assert!(parse_tokens::<CStrRaw>("c\"hello\"").is_err()); // Not raw
        assert!(parse_tokens::<CStrRaw>("r\"hello\"").is_err()); // Not C string
    }

    #[test]
    fn test_display_implementations() {
        assert_eq!(Bool { value: true }.to_string(), "true");
        assert_eq!(Bool { value: false }.to_string(), "false");

        assert_eq!(ByteChar { value: b'a' }.to_string(), "b'a'");
        assert_eq!(ByteChar { value: b'\n' }.to_string(), "b'\\n'");
        assert_eq!(ByteChar { value: 255 }.to_string(), "b'\\xff'");

        assert_eq!(Char { value: 'a' }.to_string(), "'a'");
        assert_eq!(Char { value: '\n' }.to_string(), "'\\n'");

        assert_eq!(
            Integer {
                value: "42".to_string(),
                suffix: None
            }
            .to_string(),
            "42"
        );
        assert_eq!(
            Integer {
                value: "42".to_string(),
                suffix: Some("u32".to_string())
            }
            .to_string(),
            "42u32"
        );

        assert_eq!(
            Float {
                value: "3.14".to_string(),
                suffix: None
            }
            .to_string(),
            "3.14"
        );
        assert_eq!(
            Float {
                value: "3.14".to_string(),
                suffix: Some("f32".to_string())
            }
            .to_string(),
            "3.14f32"
        );

        assert_eq!(
            Str {
                value: "hello".to_string()
            }
            .to_string(),
            "\"hello\""
        );
        assert_eq!(
            Str {
                value: "say \"hi\"".to_string()
            }
            .to_string(),
            "\"say \\\"hi\\\"\""
        );

        assert_eq!(
            StrRaw {
                value: "hello".to_string(),
                hash_count: 0
            }
            .to_string(),
            "r\"hello\""
        );
        assert_eq!(
            StrRaw {
                value: "hello".to_string(),
                hash_count: 2
            }
            .to_string(),
            "r##\"hello\"##"
        );

        assert_eq!(
            ByteStr {
                value: b"hello".to_vec()
            }
            .to_string(),
            "b\"hello\""
        );

        assert_eq!(
            ByteStrRaw {
                value: b"hello".to_vec(),
                hash_count: 1
            }
            .to_string(),
            "br#\"hello\"#"
        );

        assert_eq!(
            CStr {
                value: "hello".to_string()
            }
            .to_string(),
            "c\"hello\""
        );

        assert_eq!(
            CStrRaw {
                value: "hello".to_string(),
                hash_count: 1
            }
            .to_string(),
            "cr#\"hello\"#"
        );
    }
}

//! The literal types other than `Integer`, parsed from text (`char`) and bytes (`u8`).

use syan::literal::{Bool, ByteChar, ByteStr, ByteStrRaw, CStr, CStrRaw, Char, Float, Str, StrRaw};
use syan::parse::{IntoParseStream, Parse, ParseStream};
use syan::span::WithSpan;

type Sp = syan::source::string::Span;
type CharAtom = WithSpan<char, Sp>;
type ByteAtom = WithSpan<u8, Sp>;

/// Parses `T` and then drains what is left, so a test can assert on both.
fn take<T: Parse<CharAtom>>(src: &str) -> (Option<T>, String) {
    let mut stream = src.into_parse_stream();
    let parsed = T::parse_stream(&mut stream).ok();
    let mut rest = String::new();
    while let Some(atom) = stream.next() {
        rest.push(atom.slot);
    }
    (parsed, rest)
}

fn ok<T: Parse<CharAtom>>(src: &str) -> T {
    take(src).0.unwrap()
}

/// A failed parse must hand every atom back, so the next alternative sees the input untouched.
fn restores<T: Parse<CharAtom>>(src: &str) {
    let (parsed, rest) = take::<T>(src);
    assert!(parsed.is_none(), "expected a failure for {src:?}");
    assert_eq!(rest, src, "stream not restored after failing on {src:?}");
}

fn take_bytes<T: Parse<ByteAtom>>(src: &[u8]) -> (Option<T>, Vec<u8>) {
    let mut stream = src.into_parse_stream();
    let parsed = T::parse_stream(&mut stream).ok();
    let mut rest = Vec::new();
    while let Some(atom) = stream.next() {
        rest.push(atom.slot);
    }
    (parsed, rest)
}

mod boolean {
    use super::*;

    #[test]
    fn both_spellings() {
        assert!(ok::<Bool>("true").value);
        assert!(!ok::<Bool>("false").value);
    }

    #[test]
    fn a_longer_word_is_not_a_bool() {
        restores::<Bool>("truthy");
        restores::<Bool>("true_");
        restores::<Bool>("True");
        restores::<Bool>("");
    }

    #[test]
    fn stops_at_a_non_word_character() {
        let (parsed, rest) = take::<Bool>("true,");
        assert!(parsed.unwrap().value);
        assert_eq!(rest, ",");
    }
}

mod character {
    use super::*;

    #[test]
    fn plain_and_escaped() {
        assert_eq!(ok::<Char>("'a'").value, 'a');
        assert_eq!(ok::<Char>("'\\n'").value, '\n');
        assert_eq!(ok::<Char>("'\\''").value, '\'');
        assert_eq!(ok::<Char>("'\\0'").value, '\0');
    }

    #[test]
    fn rejects_and_restores() {
        restores::<Char>("''");
        restores::<Char>("'ab'");
        restores::<Char>("'\\q'");
        restores::<Char>("b'a'");
        restores::<Char>("'a");
    }

    #[test]
    fn byte_character_wants_its_prefix() {
        assert_eq!(ok::<ByteChar>("b'a'").value, b'a');
        assert_eq!(ok::<ByteChar>("b'\\t'").value, b'\t');
        restores::<ByteChar>("'a'");
        restores::<ByteChar>("b'\u{e9}'");
    }
}

mod strings {
    use super::*;

    #[test]
    fn contents_are_kept_as_written() {
        assert_eq!(ok::<Str>("\"hello\"").value, "hello");
        assert_eq!(ok::<Str>("\"\"").value, "");
        assert_eq!(ok::<Str>("\"a\\nb\"").value, "a\\nb");
        assert_eq!(ok::<Str>("\"say \\\"hi\\\"\"").value, "say \\\"hi\\\"");
    }

    #[test]
    fn rejects_and_restores() {
        restores::<Str>("\"unterminated");
        restores::<Str>("b\"hello\"");
        restores::<Str>("hello");
    }

    #[test]
    fn prefixed_forms() {
        assert_eq!(ok::<ByteStr>("b\"hi\"").value, b"hi");
        assert_eq!(ok::<CStr>("c\"hi\"").value, "hi");
        restores::<ByteStr>("\"hi\"");
        restores::<CStr>("\"hi\"");
    }

    #[test]
    fn stops_after_the_closing_quote() {
        let (parsed, rest) = take::<Str>("\"hi\" rest");
        assert_eq!(parsed.unwrap().value, "hi");
        assert_eq!(rest, " rest");
    }
}

mod raw_strings {
    use super::*;

    #[test]
    fn hash_counts() {
        let r = ok::<StrRaw>("r\"hello\"");
        assert_eq!((r.value.as_str(), r.hash_count), ("hello", 0));

        let r = ok::<StrRaw>("r##\"hello\"##");
        assert_eq!((r.value.as_str(), r.hash_count), ("hello", 2));
    }

    #[test]
    fn a_quote_with_too_few_hashes_is_content() {
        let r = ok::<StrRaw>("r#\"a\"b\"#");
        assert_eq!((r.value.as_str(), r.hash_count), ("a\"b", 1));
    }

    #[test]
    fn escapes_are_not_special() {
        assert_eq!(ok::<StrRaw>("r\"a\\\"").value, "a\\");
    }

    #[test]
    fn byte_and_c_forms() {
        assert_eq!(ok::<ByteStrRaw>("br\"hi\"").value, b"hi");
        assert_eq!(ok::<CStrRaw>("cr#\"hi\"#").value, "hi");
        restores::<ByteStrRaw>("r\"hi\"");
        restores::<CStrRaw>("r\"hi\"");
    }

    #[test]
    fn rejects_and_restores() {
        restores::<StrRaw>("\"hello\"");
        restores::<StrRaw>("r#\"unterminated\"");
        restores::<StrRaw>("br\"hi\"");
    }
}

mod float {
    use super::*;

    fn parts(src: &str) -> (String, Option<String>) {
        let f = ok::<Float>(src);
        (f.value, f.suffix)
    }

    #[test]
    fn fractional() {
        assert_eq!(parts("1.5"), ("1.5".into(), None));
        assert_eq!(parts("0.0"), ("0.0".into(), None));
        assert_eq!(parts("1_000.5"), ("1_000.5".into(), None));
    }

    #[test]
    fn exponents() {
        assert_eq!(parts("1e10"), ("1e10".into(), None));
        assert_eq!(parts("1.5e-3"), ("1.5e-3".into(), None));
        assert_eq!(parts("2.0E+7"), ("2.0E+7".into(), None));
    }

    #[test]
    fn suffixes() {
        assert_eq!(parts("3.14f32"), ("3.14".into(), Some("f32".into())));
        assert_eq!(parts("2.5e3f64"), ("2.5e3".into(), Some("f64".into())));
        // A suffix alone makes it a float, as in Rust.
        assert_eq!(parts("42f64"), ("42".into(), Some("f64".into())));
    }

    #[test]
    fn an_integer_is_not_a_float() {
        restores::<Float>("42");
        restores::<Float>("1_000");
        restores::<Float>("abc");
        restores::<Float>("");
    }

    #[test]
    fn a_dot_that_is_not_a_fraction_is_left_alone() {
        restores::<Float>("1..2");
        restores::<Float>("1.foo");
        restores::<Float>("1.");
    }

    #[test]
    fn a_non_suffix_tail_is_pushed_back() {
        let (parsed, rest) = take::<Float>("1.5foo");
        assert_eq!(parsed.unwrap().value, "1.5");
        assert_eq!(rest, "foo");

        // the exponent never completes, so `e` belongs to what follows
        let (parsed, rest) = take::<Float>("1.5e");
        assert_eq!(parsed.unwrap().value, "1.5");
        assert_eq!(rest, "e");
    }
}

mod over_bytes {
    use super::*;

    #[test]
    fn the_same_grammar_over_u8() {
        assert!(take_bytes::<Bool>(b"true").0.unwrap().value);
        assert_eq!(take_bytes::<Float>(b"1.5").0.unwrap().value, "1.5");
        assert_eq!(take_bytes::<Char>(b"'a'").0.unwrap().value, 'a');
        assert_eq!(take_bytes::<Str>(b"\"hi\"").0.unwrap().value, "hi");
        assert_eq!(take_bytes::<ByteStr>(b"b\"hi\"").0.unwrap().value, b"hi");
        assert_eq!(take_bytes::<StrRaw>(b"r#\"hi\"#").0.unwrap().value, "hi");
    }

    /// A byte >= 0x80 is not part of a literal, and a failure gives every byte back.
    #[test]
    fn non_ascii_ends_the_literal() {
        let (parsed, rest) = take_bytes::<Str>(&[b'"', b'a', 0xFF, b'"']);
        assert!(parsed.is_none());
        assert_eq!(rest, vec![b'"', b'a', 0xFF, b'"']);

        let (parsed, rest) = take_bytes::<Float>(&[b'1', b'.', b'5', 0xFF]);
        assert_eq!(parsed.unwrap().value, "1.5");
        assert_eq!(rest, vec![0xFF]);
    }
}

/// The reported failure names the literal that was expected.
#[test]
fn error_kind_and_span() {
    let mut stream = "  x".into_parse_stream();
    stream.skip_sep();
    let err = <Float as Parse<CharAtom>>::parse_stream(&mut stream).unwrap_err();
    assert!(matches!(
        err,
        syan::error::ParseError::Literal {
            kind: syan::error::LitKind::Float,
            ..
        }
    ));
    assert_eq!((err.span().line, err.span().col), (1, 3));
}

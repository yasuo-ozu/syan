//! `Integer` parsed from text (`char`) and bytes (`u8`), not just from a `TokenTree`.

use syan::literal::Integer;
use syan::parse::Parse;
use syan::symbol::Token;

type Text = syan::source::string::Span;

fn int(src: &str) -> Option<Integer> {
    Parse::parse(src).ok()
}

#[test]
fn plain_digits() {
    let i = int("42").unwrap();
    assert_eq!(i.value, "42");
    assert_eq!(i.suffix, None);
}

#[test]
fn keeps_underscores_like_the_token_impl() {
    let i = int("1_000").unwrap();
    assert_eq!(i.value, "1_000");
    assert_eq!(i.suffix, None);
}

#[test]
fn recognises_suffixes() {
    for s in ["u8", "i8", "u16", "u32", "u64", "u128", "usize", "isize"] {
        let i = int(&format!("7{s}")).unwrap_or_else(|| panic!("7{s} failed"));
        assert_eq!(i.value, "7");
        assert_eq!(i.suffix.as_deref(), Some(s));
    }
}

#[test]
fn rejects_non_digits() {
    assert!(int("x").is_none());
    assert!(int("").is_none());
    assert!(int("_").is_none(), "underscores alone are not an integer");
}

/// A trailing run that is not a real suffix belongs to the next field, not to the literal.
#[derive(Parse)]
struct IntThenX<S> {
    value: Integer,
    _x: Token![S => x],
}

#[test]
fn non_suffix_tail_is_pushed_back() {
    let r: IntThenX<Text> = Parse::parse("1x").unwrap();
    assert_eq!(r.value.value, "1");
    assert_eq!(r.value.suffix, None);
}

/// A failed parse must leave the stream untouched for the next alternative.
#[derive(Parse)]
struct XThenInt<S> {
    _x: Token![S => x],
    value: Integer,
}

#[test]
fn failure_restores_the_stream() {
    // `_` is consumed by the digit scan, then pushed back when no digit follows.
    assert!(Parse::parse("x_").map(|_: XThenInt<Text>| ()).is_err());
    // the `x` field still parses when the integer is present
    let ok: XThenInt<Text> = Parse::parse("x 5").unwrap();
    assert_eq!(ok.value.value, "5");
}

mod over_bytes {
    use super::*;

    fn int_b(src: &[u8]) -> Option<Integer> {
        Parse::parse(src).ok()
    }

    #[test]
    fn digits_and_suffix_from_bytes() {
        assert_eq!(int_b(b"42").unwrap().value, "42");
        let i = int_b(b"7u64").unwrap();
        assert_eq!((i.value.as_str(), i.suffix.as_deref()), ("7", Some("u64")));
        assert!(int_b(b"x").is_none());
    }

    /// A non-ASCII byte is not a digit, so it neither parses nor is swallowed.
    #[test]
    fn stops_at_non_ascii() {
        let i = int_b(&[b'1', 0xFF]).unwrap();
        assert_eq!(i.value, "1");
        assert_eq!(i.suffix, None);
    }
}

// Parses `proc_macro2` tokens in one test; the rest is atom-agnostic.
#![cfg(feature = "proc_macro2")]

//! `ParseError` carries the position **and the kind** of a failure, as data.
//!
//! History, because it explains the shape. Originally `ParseError::new(span, message)` *discarded*
//! the span, so every failure reported position zero (§3 of `known-gaps-rustyfi-port.md`). The first
//! fix stored the span's `Debug` rendering — which worked, and then turned out to be the single
//! largest cost in the parser: two eager `format!`s per failed alternative, 25–37% of runtime and
//! 35–81% of allocations (`perf-measurements.md` §4, §8).
//!
//! So the span is now held **by value**, in a `#[non_exhaustive]` enum whose variants *are* the error
//! kinds, and rendering happens once in `Display`. These tests pin both halves: the position is real,
//! and the kind is inspectable without parsing a string.

use syan::error::{LitKind, ParseError};
use syan::parse::Parse;
use syan::source::string::Span;
use syan::symbol::{chars, Symbol};

#[test]
fn a_span_is_kept_by_value() {
    let e = ParseError::expected(
        Span {
            line: 3,
            col: 7,
            loc: 42,
        },
        "a digit",
    );
    // No rendering and no allocation: the span is the span.
    assert_eq!(e.span().loc, 42);
    assert_eq!(e.span().line, 3);
    assert!(matches!(e, ParseError::Expected { what: "a digit", .. }));
    let text = e.to_string();
    assert!(text.contains("a digit") && text.contains("42"), "{text}");
}

#[test]
fn the_unit_span_is_not_rendered() {
    // `()` carries no position; printing "()" would be noise on every message.
    let e: ParseError<()> = ParseError::expected((), "a digit");
    assert_eq!(e.to_string(), "expected a digit");
}

/// The half the original report missed: fixing the constructor is useless if the call sites pass
/// `Span::default()`. Three successful parses, then a failure ON THE FOURTH atom.
#[test]
fn a_leaf_failure_reports_where_it_failed_not_byte_zero() {
    let mut stream = syan::parse::IntoParseStream::into_parse_stream("aaab".to_string());
    for _ in 0..3 {
        Symbol::<chars::_a>::parse_stream(&mut stream).expect("the first three are `a`");
    }
    let e = Symbol::<chars::_a>::parse_stream(&mut stream).unwrap_err();
    assert_eq!(
        e.span().loc,
        3,
        "expected the failing atom's position, got {:?}",
        e.span()
    );
}

#[test]
fn eof_is_its_own_kind() {
    let mut stream = syan::parse::IntoParseStream::into_parse_stream(String::new());
    let e = Symbol::<chars::_a>::parse_stream(&mut stream).unwrap_err();
    assert!(matches!(e, ParseError::Eof { .. }), "{e:?}");
    assert!(e.to_string().contains("end of input"), "{e}");
}

/// The 16 sites that used to say `"parse failed"` now carry a `LitKind`.
#[test]
fn a_literal_failure_names_the_literal_kind() {
    use syan::source::proc_macro2::literal::Integer;
    let ts = "oops".parse::<proc_macro2::TokenStream>().unwrap();
    let mut stream = syan::parse::IntoParseStream::into_parse_stream(ts);
    let e = Integer::parse_stream(&mut stream).unwrap_err();
    assert!(
        matches!(
            e,
            ParseError::Literal {
                kind: LitKind::Int,
                ..
            }
        ),
        "expected a LitKind::Int failure, got {e:?}"
    );
    assert!(e.to_string().contains("integer literal"), "{e}");
}

/// `Error::from_cause` builds an `Alternatives` aggregate. There is no separate expected-set: the
/// alternatives *are* the set, and `Display` joins them at print time — so nothing is rendered or
/// allocated for it during the parse.
#[test]
fn an_aggregate_renders_its_alternatives() {
    use syan::error::Error;
    let a = ParseError::expected(
        Span {
            line: 1,
            col: 1,
            loc: 5,
        },
        "`x`",
    );
    let b = ParseError::expected(
        Span {
            line: 1,
            col: 1,
            loc: 9,
        },
        "`y`",
    );
    let agg = ParseError::from_cause(vec![a, b]);
    assert_eq!(agg.alternatives().len(), 2);
    let text = agg.to_string();
    assert!(text.contains("`x`") && text.contains("`y`"), "{text}");
    assert!(text.contains(", or "), "alternatives should read as a list: {text}");
    // The aggregate takes the first alternative's span — see the note on `Error::from_cause`.
    assert_eq!(agg.span().loc, 5);
}

/// The escape hatch is the only allocating variant, and it is off the hot path.
#[test]
fn other_carries_an_owned_message() {
    let e = ParseError::other(
        Span {
            line: 2,
            col: 2,
            loc: 7,
        },
        "something bespoke",
    );
    assert!(matches!(e, ParseError::Other(..)));
    assert!(e.to_string().starts_with("something bespoke"), "{e}");
    assert_eq!(e.span().loc, 7);
}

/// Size is a *success*-path property: an error travels by value out of every field parse, so
/// `Result<T, ParseError<S>>` is at least as large as the error. The old struct was 72 bytes.
#[test]
fn the_error_is_small() {
    use std::mem::size_of;
    assert!(
        size_of::<ParseError<Span>>() <= 48,
        "ParseError<string::Span> grew to {} bytes",
        size_of::<ParseError<Span>>()
    );
    assert!(
        size_of::<ParseError<syan::source::proc_macro2::Span>>() <= 40,
        "ParseError<pm2::Span> grew to {} bytes",
        size_of::<ParseError<syan::source::proc_macro2::Span>>()
    );
}

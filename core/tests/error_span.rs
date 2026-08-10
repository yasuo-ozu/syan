// Parses `proc_macro2` tokens in one test; the rest is atom-agnostic.
#![cfg(feature = "proc_macro2")]

//! `ParseError` carries the position it was reported at — §3 of `known-gaps-rustyfi-port.md`.
//!
//! Two halves had to be fixed, and only the first is what the report describes:
//!
//! 1. `ParseError::new` took `_span: impl Span` and **discarded** it.
//! 2. 24 of the 26 in-crate call sites passed `Span::default()` rather than the atom's own span, so
//!    even after (1) every error would still have reported position zero — exactly the symptom the
//!    port hit. These now capture the span before `stream.push` consumes the atom.
//!
//! What is stored is the span's `Debug` rendering, not the span: `ParseError` is one concrete type
//! shared by every `Parse` impl, `Span` is not object-safe (`migrate` takes `self` and names `Self`),
//! and erasing through `Any` would force `'static` — which `Sp<'a>` in `recurse_borrowed_stream.rs`
//! is not. See the note on `ParseError::span`.

use syan::error::ParseError;
use syan::parse::Parse;
use syan::source::string::Span;
use syan::symbol::{chars, Symbol};

#[test]
fn a_span_survives_the_constructor() {
    let e = ParseError::new(
        Span {
            line: 3,
            col: 7,
            loc: 42,
        },
        "boom",
    );
    let s = e.span_debug().expect("a spanned error keeps its span");
    assert!(s.contains("42"), "{s}");
    assert!(
        e.to_string().contains("boom") && e.to_string().contains("42"),
        "{e}"
    );
}

#[test]
fn the_unit_span_stays_absent() {
    // `()` is the span of an unspanned atom — rendering it would prefix every message with "()".
    let e = ParseError::new((), "boom");
    assert_eq!(e.span_debug(), None);
    assert_eq!(e.to_string(), "boom");
}

/// The half the report does not mention: without fixing the call sites this reports `loc: 0` for a
/// failure at the fourth character.
#[test]
fn a_leaf_failure_reports_where_it_failed_not_byte_zero() {
    // `aaab` — the `_a` parser succeeds three times, then fails ON THE FOURTH atom.
    let mut stream = syan::parse::IntoParseStream::into_parse_stream("aaab".to_string());
    for _ in 0..3 {
        Symbol::<chars::_a>::parse_stream(&mut stream).expect("the first three are `a`");
    }
    let e = Symbol::<chars::_a>::parse_stream(&mut stream).unwrap_err();
    let s = e.span_debug().expect("a leaf failure is spanned");
    assert!(
        s.contains("loc: 3"),
        "expected the failing atom's position (loc 3), got {s:?}"
    );
    assert!(
        !s.contains("loc: 0"),
        "reported position zero — the call site is still passing Span::default(): {s:?}"
    );
}

#[test]
fn a_token_failure_reports_the_offending_token() {
    use syan::source::proc_macro2::literal::Integer;
    let tokens = "1 2 oops".parse::<proc_macro2::TokenStream>().unwrap();
    let mut stream = syan::parse::IntoParseStream::into_parse_stream(tokens);
    Integer::parse_stream(&mut stream).unwrap();
    Integer::parse_stream(&mut stream).unwrap();
    // Fails on `oops`, whose span is not the start of the stream.
    let e = Integer::parse_stream(&mut stream).unwrap_err();
    assert!(
        e.span_debug().is_some(),
        "a token-level failure must carry the offending token's span"
    );
}

/// `Error::from_cause` builds an unspanned aggregate, so the positions live on the alternatives.
/// Display has to recurse or they are invisible.
#[test]
fn an_aggregate_shows_its_alternatives_positions() {
    use syan::error::Error;
    let a = ParseError::new(
        Span {
            line: 1,
            col: 1,
            loc: 5,
        },
        "expected `x`",
    );
    let b = ParseError::new(
        Span {
            line: 1,
            col: 1,
            loc: 9,
        },
        "expected `y`",
    );
    let agg = ParseError::from_cause(vec![a, b]);
    assert_eq!(
        agg.span_debug(),
        None,
        "an aggregate has no position of its own"
    );
    assert_eq!(agg.sub_errors().len(), 2);
    let text = agg.to_string();
    assert!(text.contains("loc: 5") && text.contains("loc: 9"), "{text}");
}

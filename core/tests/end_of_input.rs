//! End of input: the [`Eof`] node, [`Parse::parse_all`], and the cursor position once the atoms
//! run out.
//!
//! Two defects, one cause. `parse` never asked whether anything was left, so a top-level `Vec<T>`
//! matched a prefix and reported success — a grammar that could not fail. And every parser took its
//! span from `stream.peek()`, which is `None` at end of input, so a failure there reported
//! `Span::default()` — line 0, column 0, a position that exists in no source, since the text source
//! numbers from 1.

use syan::literal::Integer;
use syan::nested::Eof;
use syan::parse::{Parse, ParseStream};
use syan::source::string::{Span, Stream};
use syan::span::WithSpan;

type Text = Span;
type CharAtom = WithSpan<char, Span>;
type ByteAtom = WithSpan<u8, syan::source::bytes::Span>;

// ───────────────────────────── leftover input ─────────────────────────────

/// The reported behaviour of `parse`, pinned as documented rather than fixed: it matches a prefix.
#[test]
fn parse_still_accepts_a_prefix() {
    let v: Vec<Integer> = Parse::parse("1 2 ) ) )").unwrap();
    assert_eq!(v.len(), 2);
}

#[test]
fn parse_all_rejects_what_parse_left_behind() {
    let e = <Vec<Integer> as Parse<CharAtom>>::parse_all("1 2 ) ) )").unwrap_err();
    let span = e.span();
    assert_eq!(
        (span.line, span.col, span.loc),
        (1, 5, 4),
        "the failure belongs at the first atom nothing could read"
    );
    assert!(
        matches!(
            e,
            syan::error::ParseError::Expected {
                what: "end of input",
                ..
            }
        ),
        "{e:?}"
    );
}

#[test]
fn parse_all_accepts_a_fully_consumed_input() {
    let v = <Vec<Integer> as Parse<CharAtom>>::parse_all("1 2 3  ").unwrap();
    assert_eq!(v.len(), 3, "trailing separators are still the end");
    let v = <Vec<Integer> as Parse<CharAtom>>::parse_all("").unwrap();
    assert!(v.is_empty());
}

/// `Eof` in a field is the other half: it makes "the whole input" part of the grammar itself.
#[derive(Parse, Debug)]
struct AllInts(Vec<Integer>, Eof);

#[test]
fn eof_as_a_field() {
    let AllInts(v, _) = <AllInts as Parse<CharAtom>>::parse("1 2 3").unwrap();
    assert_eq!(v.len(), 3);
    assert!(<AllInts as Parse<CharAtom>>::parse("1 2 )").is_err());
}

#[test]
fn eof_over_bytes() {
    <Eof as Parse<ByteAtom>>::parse(&b"   "[..]).unwrap();
    assert!(<Eof as Parse<ByteAtom>>::parse(&b"x"[..]).is_err());
}

#[cfg(feature = "proc_macro2")]
#[test]
fn eof_over_tokens() {
    use proc_macro2::TokenStream;
    <Eof as Parse<proc_macro2::TokenTree>>::parse(TokenStream::new()).unwrap();
    assert!(
        <Eof as Parse<proc_macro2::TokenTree>>::parse("x".parse::<TokenStream>().unwrap()).is_err()
    );
}

// ──────────────────────── where the input ran out ────────────────────────

#[derive(Parse, Debug)]
#[allow(dead_code)]
struct Pair<S> {
    open: WithSpan<syan::symbol::chars::OpenBracket, S>,
    n: Integer,
    close: WithSpan<syan::symbol::chars::CloseBracket, S>,
}

/// The reported repro: `]` is missing and the input ends, and the failure used to land on
/// `Span::default()` — line 0, column 0, loc 0.
#[test]
fn a_failure_at_end_of_input_reports_the_end() {
    let e = Pair::<Text>::parse("[7").unwrap_err();
    let span = e.span();
    assert_eq!(
        (span.line, span.col, span.loc),
        (1, 3, 2),
        "just past the last atom served, not position zero: {e}"
    );
}

/// The non-EOF failure was always right; it must stay right.
#[test]
fn a_failure_on_an_atom_still_reports_that_atom() {
    let e = Pair::<Text>::parse("[7,").unwrap_err();
    let span = e.span();
    assert_eq!((span.line, span.col, span.loc), (1, 3, 2));
}

#[test]
fn pos_is_the_next_atom_then_the_end() {
    let mut s = Stream::new("ab".to_string());
    assert_eq!((s.pos().line, s.pos().col, s.pos().loc), (1, 1, 0));
    s.next();
    assert_eq!((s.pos().line, s.pos().col, s.pos().loc), (1, 2, 1));
    s.next();
    assert_eq!((s.pos().line, s.pos().col, s.pos().loc), (1, 3, 2));
    assert!(s.next().is_none());
    assert_eq!((s.pos().line, s.pos().col, s.pos().loc), (1, 3, 2));
}

#[test]
fn pos_counts_lines() {
    let mut s = Stream::new("a\nbc".to_string());
    while s.next().is_some() {}
    let p = s.pos();
    assert_eq!((p.line, p.col, p.loc), (2, 3, 4));
}

/// A rollback puts the cursor back on a real atom, so `pos` must stop reporting the end.
#[test]
fn pos_follows_a_rollback() {
    let mut s = Stream::new("ab".to_string());
    let raw = s.checkpoint_raw();
    while s.next().is_some() {}
    assert_eq!(s.pos().loc, 2);
    s.rollback_raw(raw);
    assert_eq!(s.pos().loc, 0);
}

/// A wrapper that forgets to forward `pos` falls back to the default silently — the very failure
/// being fixed. `&mut T` and `WithSpan`'s `SubStream` are the crate's two stream wrappers.
#[test]
fn pos_survives_the_reference_wrapper() {
    let mut base = Stream::new("a".to_string());
    base.next();
    let mut through_ref = &mut base;
    assert_eq!(ParseStream::pos(&mut through_ref).loc, 1);
}

#[test]
fn pos_survives_the_substream_wrapper() {
    struct Probe;
    impl Parse<CharAtom> for Probe {
        type Error = syan::error::ParseError<Span>;
        fn parse_stream<S: ParseStream<Atom = CharAtom>>(s: &mut S) -> Result<Self, Self::Error> {
            while s.next().is_some() {}
            Err(syan::error::ParseError::eof(s.pos()))
        }
    }
    let e = <WithSpan<Probe, Span> as Parse<CharAtom>>::parse("ab")
        .err()
        .expect("`Probe` always fails");
    assert_eq!(e.span().loc, 2);
}

#[test]
fn the_byte_source_tracks_the_end_too() {
    let mut s = syan::source::bytes::Stream::new(b"ab");
    while s.next().is_some() {}
    let p = s.pos();
    assert_eq!((p.line, p.col, p.loc), (1, 3, 2));
}

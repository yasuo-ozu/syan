//! The `&[u8]` source: one `u8` per atom, no UTF-8 decoding.

use syan::parse::{Parse, ParseStream};
use syan::source::bytes::{Span, Stream};
use syan::symbol::{chars, Symbol, Token};

#[test]
fn serves_raw_bytes_with_positions() {
    let mut s = Stream::new(b"ab");
    let a = s.next().unwrap();
    assert_eq!(a.slot, b'a');
    assert_eq!((a.span.line, a.span.col, a.span.loc), (1, 1, 0));
    let b = s.next().unwrap();
    assert_eq!(b.slot, b'b');
    assert_eq!((b.span.line, b.span.col, b.span.loc), (1, 2, 1));
    assert!(s.next().is_none());
}

#[test]
fn tracks_lines() {
    let mut s = Stream::new(b"a\nb");
    s.next();
    s.next();
    let b = s.next().unwrap();
    assert_eq!(b.slot, b'b');
    assert_eq!((b.span.line, b.span.col), (2, 1));
}

/// Not valid UTF-8: it is served as the byte it is, rather than becoming U+FFFD.
#[test]
fn passes_invalid_utf8_through_untouched() {
    let mut s = Stream::new(&[0xFF, 0xFE]);
    assert_eq!(s.next().unwrap().slot, 0xFF);
    assert_eq!(s.next().unwrap().slot, 0xFE);
}

#[test]
fn into_parse_stream_for_byte_slice() {
    assert!(Symbol::<chars::_a>::parse(b"a".as_slice()).is_ok());
    assert!(Symbol::<chars::_a>::parse(b"b".as_slice()).is_err());
}

#[test]
fn skip_sep_consumes_ascii_whitespace() {
    let mut s = Stream::new(b"  \t\n x");
    assert!(s.skip_sep());
    assert_eq!(s.next().unwrap().slot, b'x');
    assert!(!s.skip_sep());
}

#[test]
fn skip_sep_reports_nothing_when_adjacent() {
    let mut s = Stream::new(b"ab");
    assert!(!s.skip_sep());
    assert_eq!(s.next().unwrap().slot, b'a');
}

#[derive(Parse)]
struct Keyword {
    _kw: Symbol!(let),
}

#[derive(Parse)]
struct Assign {
    _name: Symbol!(x),
    _space1: Vec<Symbol<chars::Space>>,
    _eq: Token![Span => =],
    _space2: Vec<Symbol<chars::Space>>,
    _value: Symbol!(1),
}

#[test]
fn symbol_and_token_macros_work_over_bytes() {
    assert!(Parse::parse(b"let".as_slice()).map(|_: Keyword| ()).is_ok());
    assert!(Parse::parse(b"lot".as_slice())
        .map(|_: Keyword| ())
        .is_err());
}

/// A multi-character symbol is `Joint`: the bytes must be adjacent.
#[test]
fn multi_byte_symbol_rejects_a_gap() {
    assert!(Parse::parse(b"l et".as_slice())
        .map(|_: Keyword| ())
        .is_err());
}

#[test]
fn assign_from_bytes() {
    assert!(Parse::parse(b"x = 1".as_slice())
        .map(|_: Assign| ())
        .is_ok());
    assert!(Parse::parse(b"x=1".as_slice()).map(|_: Assign| ()).is_ok());
    assert!(Parse::parse(b"x = 2".as_slice())
        .map(|_: Assign| ())
        .is_err());
}

/// The `chars::*` impls are generic over the span, so they apply to *any* `u8` stream -- not just
/// [`Stream`]. This one carries `()` as its span instead of [`Span`].
mod other_span {
    use super::*;
    use std::convert::Infallible;
    use syan::parse::Tape;
    use syan::span::WithSpan;

    struct Unspanned(Tape<std::vec::IntoIter<WithSpan<u8, ()>>>);

    impl Unspanned {
        fn new(src: &[u8]) -> Self {
            let v: Vec<_> = src
                .iter()
                .map(|&b| WithSpan { slot: b, span: () })
                .collect();
            Self(Tape::new(v.into_iter()))
        }
    }

    impl ParseStream for Unspanned {
        type Atom = WithSpan<u8, ()>;
        type Error = Infallible;
        fn next(&mut self) -> Option<Self::Atom> {
            self.0.next()
        }
        fn peek(&mut self) -> Option<&Self::Atom> {
            self.0.peek()
        }
        fn push(&mut self, atom: Self::Atom) {
            self.0.push(atom)
        }
        fn checkpoint_raw(&mut self) -> u64 {
            self.0.checkpoint()
        }
        fn rollback_raw(&mut self, raw: u64) {
            self.0.rollback(raw)
        }
        fn commit_raw(&mut self, raw: u64) {
            self.0.commit(raw)
        }
        fn get_error(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
        fn skip_sep(&mut self) -> bool {
            let mut skipped = false;
            while self.0.peek().is_some_and(|a| a.slot.is_ascii_whitespace()) {
                self.0.next();
                skipped = true;
            }
            skipped
        }
    }

    #[test]
    fn chars_impls_apply_to_any_u8_stream() {
        assert!(Symbol::<chars::_a>::parse_stream(&mut Unspanned::new(b"a")).is_ok());
        assert!(Symbol::<chars::_a>::parse_stream(&mut Unspanned::new(b"b")).is_err());
    }

    #[derive(Parse)]
    struct Kw {
        _kw: Symbol!(let),
    }

    #[test]
    fn symbol_macro_applies_to_any_u8_stream() {
        assert!(Kw::parse_stream(&mut Unspanned::new(b"let")).is_ok());
        assert!(Kw::parse_stream(&mut Unspanned::new(b"l et")).is_err());
    }
}

//! Parsing straight from a byte slice, one [`u8`] per atom.
//!
//! The atom type is [`WithSpan<u8, Span>`](WithSpan). Bytes are served as they are, with no UTF-8
//! decoding, so binary input and malformed text both survive the trip. Reach for
//! [`string`](super::string) instead when the input is text and you want `char`s.

use crate::error::ParseError;
use crate::parse::{IntoParseStream, Parse, ParseStream, Tape};
use crate::span::WithSpan;
use core::convert::Infallible;

pub use super::string::Span;

/// Walks a byte slice, attaching the position each byte came from. Borrows the slice and indexes
/// into it, so a checkpoint stays a plain index.
struct SpannedBytes<'a> {
    src: &'a [u8],
    idx: usize,
    line: usize,
    col: usize,
    loc: usize,
}

impl Iterator for SpannedBytes<'_> {
    type Item = WithSpan<u8, Span>;

    fn next(&mut self) -> Option<Self::Item> {
        let b = *self.src.get(self.idx)?;
        self.idx += 1;
        let span = Span {
            line: self.line,
            col: self.col,
            loc: self.loc,
        };
        if b == b'\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        self.loc += 1;
        Some(WithSpan { slot: b, span })
    }
}

/// A [`ParseStream`] over the bytes of a slice.
pub struct Stream<'a>(Tape<SpannedBytes<'a>>);

impl<'a> Stream<'a> {
    /// Starts a stream at the beginning of `src` (line 1, column 1).
    pub fn new(src: &'a [u8]) -> Self {
        Self(Tape::new(SpannedBytes {
            src,
            idx: 0,
            line: 1,
            col: 1,
            loc: 0,
        }))
    }
}

impl ParseStream for Stream<'_> {
    type Atom = WithSpan<u8, Span>;
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

    /// ASCII whitespace is the separator. Consumes a run of it and reports whether there was any,
    /// which is what tells [`Joint`](crate::nested::Joint) the atoms around it were not adjacent.
    fn skip_sep(&mut self) -> bool {
        let mut skipped = false;
        while self.0.peek().is_some_and(|a| a.slot.is_ascii_whitespace()) {
            self.0.next();
            skipped = true;
        }
        skipped
    }
}

impl<'a> IntoParseStream for &'a [u8] {
    type Atom = WithSpan<u8, Span>;
    type Output = Stream<'a>;

    fn into_parse_stream(self) -> Self::Output {
        Stream::new(self)
    }
}

macro_rules! impl_parse_for_byte {
    ($($name:ident),* $(,)?) => {
        $(
            // Generic over the span, so any `u8` stream can use these -- not just [`Stream`].
            impl<Sp: crate::span::Span> Parse<WithSpan<u8, Sp>> for crate::symbol::chars::$name {
                type Error = ParseError<Sp>;

                fn parse_stream<__S: crate::parse::parse_stream::ParseStream<Atom = WithSpan<u8, Sp>>>(
                    stream: &mut __S,
                ) -> Result<Self, Self::Error> {
                    // Every symbol in the table is ASCII, so it is exactly one byte.
                    let want = crate::symbol::chars::$name.to_string().chars().next().unwrap() as u32;
                    match stream.next() {
                        Some(WithSpan { slot: b, .. }) if u32::from(b) == want => Ok(Default::default()),
                        Some(atom) => {
                            let span = atom.span.clone();
                            stream.push(atom);
                            Err(ParseError::expected(span, concat!("the byte `", stringify!($name), "`")))
                        }
                        None => Err(ParseError::eof(Sp::default())),
                    }
                }
            }
        )*
    };
}

impl_parse_for_byte! {
    _a, _b, _c, _d, _e, _f, _g, _h, _i,
    _j, _k, _l, _m, _n, _o, _p, _q, _r,
    _s, _t, _u, _v, _w, _x, _y, _z, _A,
    _B, _C, _D, _E, _F, _G, _H, _I, _J,
    _K, _L, _M, _N, _O, _P, _Q, _R, _S,
    _T, _U, _V, _W, _X, _Y, _Z, _0, _1,
    _2, _3, _4, _5, _6, _7, _8, _9, __,
    Not, Quot, Pound, Dollar, Percnt, And, Apos, Star, Plus,
    Comma, Minus, Dot, Slash, Colon, Semi, Lt, Eq, Gt,
    Question, Commat, Backslash, Caret, Underscore, Grave, Or, Tilde, OpenParen,
    CloseParen, OpenBrace, CloseBrace, OpenBracket, CloseBracket, Space,
}

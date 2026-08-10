use crate::error::ParseError;
use crate::parse::{IntoParseStream, Parse, ParseStream, Tape};
use crate::span::WithSpan;
use crate::symbol::Symbol;
use core::convert::Infallible;

#[derive(Clone, Debug, Default)]
pub struct Span {
    pub line: usize,
    pub col: usize,
    pub loc: usize,
}

impl crate::span::Span for Span {
    fn migrate(self, other: Self) -> Self {
        if other.loc > self.loc {
            other
        } else {
            self
        }
    }
}

/// Walks the source text one `char` at a time, attaching the position it was at. Owns the `String`
/// and indexes into it, so nothing is collected up front and the line/col/loc bookkeeping lives here
/// rather than in the stream — which is what lets a checkpoint be a plain index.
struct SpannedChars {
    src: String,
    byte: usize,
    line: usize,
    col: usize,
    loc: usize,
}

impl Iterator for SpannedChars {
    type Item = WithSpan<char, Span>;

    fn next(&mut self) -> Option<Self::Item> {
        let ch = self.src[self.byte..].chars().next()?;
        self.byte += ch.len_utf8();
        let span = Span {
            line: self.line,
            col: self.col,
            loc: self.loc,
        };
        if ch == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        self.loc += 1;
        Some(WithSpan { slot: ch, span })
    }
}

pub struct Stream(Tape<SpannedChars>);

impl Stream {
    pub fn new(s: String) -> Self {
        Self(Tape::new(SpannedChars {
            src: s,
            byte: 0,
            line: 1,
            col: 1,
            loc: 0,
        }))
    }
}

impl ParseStream for Stream {
    type Atom = WithSpan<char, Span>;
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
        true
    }
}

impl IntoParseStream for String {
    type Atom = WithSpan<char, Span>;
    type Output = Stream;

    fn into_parse_stream(self) -> Self::Output {
        Stream::new(self)
    }
}

macro_rules! impl_parse_for_char {
    ($($name:ident),* $(,)?) => {
        $(
            impl Parse<WithSpan<char, Span>> for Symbol<crate::symbol::chars::$name> {
                type Error = ParseError;

                fn parse_stream<__S: crate::parse::parse_stream::ParseStream<Atom = WithSpan<char, Span>>>(
                    stream: &mut __S,
                ) -> Result<Self, Self::Error> {
                    match stream.next() {
                        Some(WithSpan { slot: ch, .. })
                            if ch == crate::symbol::chars::$name.to_string().chars().next().unwrap() =>
                        {
                            Ok(Default::default())
                        }
                        Some(atom) => {
                            let span = atom.span.clone();
                            stream.push(atom);
                            Err(ParseError::new(span, "expected character"))
                        }
                        None => Err(ParseError::new(Span::default(), "unexpected end of input")),
                    }
                }
            }
        )*
    };
}

impl_parse_for_char! {
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

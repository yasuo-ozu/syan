use crate::error::ParseError;
use crate::parse::{IntoParseStream, Parse, ParseStream};
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
    // Pick-the-later: `Span` here is a single position, not a range, so there is nothing to union —
    // keeping the operand with the greater `loc` is the whole merge. See `span::Span::migrate`'s doc
    // for why this exact shape is wrong to copy onto a range-based `Span` (it collapses to zero
    // width instead of unioning).
    fn migrate(self, other: Self) -> Self {
        if other.loc > self.loc {
            other
        } else {
            self
        }
    }
}

pub struct Stream {
    chars: std::vec::IntoIter<char>,
    buf: Vec<WithSpan<char, Span>>,
    line: usize,
    col: usize,
    loc: usize,
}

impl Stream {
    pub fn new(s: String) -> Self {
        Self {
            chars: s.chars().collect::<Vec<_>>().into_iter(),
            buf: Vec::new(),
            line: 1,
            col: 1,
            loc: 0,
        }
    }

    fn make_span(&self) -> Span {
        Span {
            line: self.line,
            col: self.col,
            loc: self.loc,
        }
    }

    fn advance(&mut self, ch: char) {
        if ch == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        self.loc += 1;
    }
}

impl ParseStream for Stream {
    type Atom = WithSpan<char, Span>;
    type Error = Infallible;

    fn next(&mut self) -> Option<Self::Atom> {
        if let Some(buffered) = self.buf.pop() {
            return Some(buffered);
        }

        let ch = self.chars.next()?;
        let span = self.make_span();
        self.advance(ch);

        Some(WithSpan { slot: ch, span })
    }

    fn peek(&mut self) -> Option<&Self::Atom> {
        if self.buf.is_empty() {
            if let Some(ch) = self.chars.next() {
                let span = self.make_span();
                self.buf.push(WithSpan { slot: ch, span });
            }
        }
        self.buf.last()
    }

    fn push(&mut self, atom: Self::Atom) {
        self.buf.push(atom);
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

/// A [`ParseError`] from [`parse_str`] rendered against this source's positional [`Span`]: `Display`
/// prints `line:col: message` from the span the error carried (recovered via
/// [`ParseError::span_of`]), which for this source is the furthest input the parse reached.
#[derive(Debug, Clone)]
pub struct SpannedError(pub ParseError);

impl SpannedError {
    /// The source position the parse failed at, if the error carried one.
    pub fn span(&self) -> Option<Span> {
        self.0.span_of::<Span>()
    }
}

impl core::fmt::Display for SpannedError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.span() {
            Some(span) => write!(f, "{}:{}: {}", span.line, span.col, self.0),
            None => write!(f, "{}", self.0),
        }
    }
}

impl std::error::Error for SpannedError {}

/// Parse `T` from a source string through the built-in char stream, wrapping a failure in a
/// [`SpannedError`] that renders `line:col: message`. The span-carrying convenience over
/// `T::parse(src)` — the position comes from the [`Span`] the failing [`ParseError`] carried.
pub fn parse_str<T>(src: impl Into<String>) -> Result<T, SpannedError>
where
    T: Parse<WithSpan<char, Span>, Error = ParseError>,
{
    T::parse(src.into()).map_err(SpannedError)
}

macro_rules! impl_parse_for_char {
    ($($name:ident),* $(,)?) => {
        $(
            impl Parse<WithSpan<char, Span>> for Symbol<crate::symbol::chars::$name> {
                type Error = ParseError;

                fn parse(
                    stream: impl IntoParseStream<Atom = WithSpan<char, Span>>,
                ) -> Result<Self, Self::Error> {
                    let mut stream = stream.into_parse_stream();
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

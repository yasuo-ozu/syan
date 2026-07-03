use crate::error::{Error, ParseError};
use crate::nested::group::{Group, GroupBrace, GroupBracket, GroupParen};
use crate::parse::{unparse::Emitter, IntoParseStream, Parse, ParseStream, Unparse};
use crate::span::WithSpan;
use crate::symbol::Symbol;

pub mod literal;

/// Wrapper around proc_macro2::Span that implements syan's Span trait
#[derive(Clone, Debug, Default)]
pub struct Span(Option<(proc_macro2::Span, proc_macro2::Span)>);

impl crate::span::Span for Span {
    // Union, not pick-the-later: `Span` here is a `(start, end)` range, so the merge keeps the
    // earlier operand's start and the later operand's end — this is the shape `span::Span::migrate`'s
    // doc recommends for any range-based `Span`.
    fn migrate(self, other: Self) -> Self {
        match (self.0, other.0) {
            (None, other) => Span(other),
            (span @ Some(_), None) => Span(span),
            (Some((lhs_start, lhs_end)), Some((rhs_start, rhs_end))) => {
                // Try to join spans, fallback to first span if joining fails
                let joined_start = lhs_start.join(rhs_start).unwrap_or(lhs_start);
                let joined_end = lhs_end.join(rhs_end).unwrap_or(rhs_end);
                Span(Some((joined_start, joined_end)))
            }
        }
    }
}

impl From<proc_macro2::Span> for Span {
    fn from(span: proc_macro2::Span) -> Self {
        Span(Some((span, span)))
    }
}

impl From<Span> for Option<proc_macro2::Span> {
    fn from(span: Span) -> Self {
        span.0.map(|(start, _end)| start)
    }
}

/// Wrapper around proc_macro2::TokenStream that implements syan's ParseStream trait
pub struct Stream {
    tokens: proc_macro2::token_stream::IntoIter,
    is_joint: bool,
    buf: Vec<proc_macro2::TokenTree>,
}

impl Stream {
    pub fn new(tokens: proc_macro2::TokenStream) -> Self {
        Self {
            tokens: tokens.into_iter(),
            is_joint: false,
            buf: Vec::new(),
        }
    }
}

impl ParseStream for Stream {
    type Atom = proc_macro2::TokenTree;
    type Error = core::convert::Infallible;

    fn next(&mut self) -> Option<Self::Atom> {
        let token = if let Some(buffered) = self.buf.pop() {
            buffered
        } else if let Some(token) = self.tokens.next() {
            token
        } else {
            return None;
        };

        if let proc_macro2::TokenTree::Punct(ref punct) = token {
            self.is_joint = punct.spacing() == proc_macro2::Spacing::Joint;
        } else {
            self.is_joint = false;
        }
        Some(token)
    }

    fn peek(&mut self) -> Option<&Self::Atom> {
        if self.buf.is_empty() {
            if let Some(token) = self.tokens.next() {
                self.buf.push(token);
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
        !self.is_joint
    }
}

impl From<proc_macro2::TokenStream> for Stream {
    fn from(tokens: proc_macro2::TokenStream) -> Self {
        Stream::new(tokens)
    }
}

impl crate::parse::into_parse_stream::IntoParseStream for proc_macro2::TokenStream {
    type Atom = proc_macro2::TokenTree;
    type Output = Stream;

    fn into_parse_stream(self) -> Self::Output {
        Stream::new(self)
    }
}

impl crate::span::Spanned for proc_macro2::TokenTree {
    type Span = Span;
    fn span(&self) -> Self::Span {
        let span = self.span();
        Span(Some((span, span)))
    }
}

impl crate::parse::unparse::Emitter<proc_macro2::TokenTree> for proc_macro2::TokenStream {
    type Error = core::convert::Infallible;
    fn write_one(&mut self, atom: proc_macro2::TokenTree) -> Result<(), Self::Error> {
        self.extend(std::iter::once(atom));
        Ok(())
    }

    // write_sep re-spaces the trailing punct as Alone to signal separation.
    fn write_sep(&mut self) -> Result<(), Self::Error> {
        let tokens: Vec<proc_macro2::TokenTree> = std::mem::take(self).into_iter().collect();

        if let Some((last_token, rest)) = tokens.split_last() {
            self.extend(rest.iter().cloned());

            match last_token {
                proc_macro2::TokenTree::Punct(punct) => {
                    let mut new_punct =
                        proc_macro2::Punct::new(punct.as_char(), proc_macro2::Spacing::Alone);
                    new_punct.set_span(punct.span());
                    self.extend(std::iter::once(proc_macro2::TokenTree::Punct(new_punct)));
                }
                other => {
                    self.extend(std::iter::once(other.clone()));
                }
            }
        }

        Ok(())
    }
}

impl<T: Default + core::fmt::Display> Parse<proc_macro2::TokenTree> for Symbol<T> {
    type Error = ParseError;

    fn parse(
        stream: impl IntoParseStream<Atom = proc_macro2::TokenTree>,
    ) -> Result<Self, Self::Error> {
        let mut stream = stream.into_parse_stream();
        match stream.next() {
            Some(proc_macro2::TokenTree::Ident(ident))
                if ident == Self::default().to_string() =>
            {
                Ok(Default::default())
            }
            Some(proc_macro2::TokenTree::Punct(punct))
                if format!("{}", punct.as_char()) == Self::default().to_string() =>
            {
                Ok(Default::default())
            }
            Some(token) => {
                stream.push(token);
                Err(ParseError::new(Span::default(), "expected symbol"))
            }
            None => Err(ParseError::new(Span::default(), "unexpected end of input")),
        }
    }
}

impl<T: Default + core::fmt::Display> Unparse<proc_macro2::TokenTree> for Symbol<T> {
    fn unparse<S: Emitter<proc_macro2::TokenTree>>(&self, sink: &mut S) -> Result<(), S::Error> {
        // A symbol may be an identifier keyword (`let`) OR punctuation (`=`, `;`, `::`, `->`). Let
        // proc-macro2's own lexer turn the symbol's text into the right token(s) — an `Ident`, a single
        // `Punct`, or a sequence of joint `Punct`s for multi-char operators — rather than forcing it
        // through `Ident::new` (which panics on punctuation).
        let text = Self::default().to_string();
        let stream: proc_macro2::TokenStream = text
            .parse()
            .expect("symbol text is not a valid Rust token sequence");
        for tt in stream {
            sink.write_one(tt)?;
        }
        Ok(())
    }
}

macro_rules! impl_for_group {
    ($($t0:ident $(:: $t:ident)*, $delim:path),* $(,)?) => {
        $(
            impl<T> Parse<proc_macro2::TokenTree> for $t0 $(::$t)*<T, Span>
            where
                T: Parse<proc_macro2::TokenTree>,
            {
                type Error = ParseError;

                fn parse(
                    stream: impl IntoParseStream<Atom = proc_macro2::TokenTree>,
                ) -> Result<Self, Self::Error> {
                    let mut stream = stream.into_parse_stream();
                    match stream.next() {
                        Some(proc_macro2::TokenTree::Group(group)) if group.delimiter() == $delim => {
                            let inner_stream = Stream::new(group.stream());
                            let slot = T::parse(inner_stream).map_err(|e| e.into_parse_error())?;
                            return Ok(Group {
                                open: WithSpan {
                                    span: group.span_open().into(),
                                    slot: Default::default(),
                                },
                                slot,
                                close: WithSpan {
                                    span: group.span_close().into(),
                                    slot: Default::default(),
                                },
                            });
                        }
                        Some(token) => {
                            stream.push(token);
                            Err(ParseError::new(Span::default(), "expected group"))
                        }
                        None => Err(ParseError::new(Span::default(), "unexpected end of input")),
                    }
                }
            }
        )*
    };
}

impl_for_group! {
    GroupParen, proc_macro2::Delimiter::Parenthesis,
    GroupBrace, proc_macro2::Delimiter::Brace,
    GroupBracket, proc_macro2::Delimiter::Bracket,
}

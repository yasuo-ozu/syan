use crate::parse::{IntoParseStream, Parse, ParseStream};
use newer_type::{implement, traits};

pub trait Span: Clone + core::fmt::Debug + Default {
    fn migrate(self, other: Self) -> Self;
}

impl Span for () {
    fn migrate(self, _: Self) -> Self {
        ()
    }
}

impl<S> Span for Option<S>
where
    S: Span,
{
    fn migrate(self, other: Self) -> Self {
        match (self, other) {
            (None, rhs) => rhs,
            (Some(lhs), None) => Some(lhs),
            (Some(lhs), Some(rhs)) => Some(lhs.migrate(rhs)),
        }
    }
}

pub trait Spanned {
    type Span: Span;
    type Map<S>: Spanned
    where
        S: Span;

    fn span(&self) -> Self::Span;
    fn map<S: Span>(self, replacement: S) -> Self::Map<S>;
}

#[derive(Default, Clone, Debug)]
#[implement]
pub struct WithSpan<T, S> {
    #[implement(
        traits::PartialEq,
        traits::Eq,
        traits::Hash,
        traits::PartialOrd,
        traits::Ord,
        traits::Display
    )]
    pub slot: T,
    pub span: S,
}

impl<T, S: Span> Spanned for WithSpan<T, S> {
    type Span = S;
    type Map<S2>
        = WithSpan<T, S2>
    where
        S2: Span;

    fn span(&self) -> Self::Span {
        self.span.clone()
    }

    fn map<S2: Span>(self, replacement: S2) -> Self::Map<S2> {
        WithSpan {
            slot: self.slot,
            span: replacement,
        }
    }
}

impl<Atom, T, S: Span> Parse<WithSpan<Atom, S>> for WithSpan<T, S>
where
    T: Parse<Atom>,
{
    type Error = T::Error;
    fn parse(stream: impl IntoParseStream<Atom = WithSpan<Atom, S>>) -> Result<Self, Self::Error> {
        struct SubStream<Slot, S>(Slot, S);

        impl<Slot, Atom, S: Span> ParseStream for SubStream<Slot, S>
        where
            Slot: ParseStream<Atom = WithSpan<Atom, S>>,
        {
            type Atom = Atom;
            type Error = Slot::Error;

            fn next(&mut self) -> Option<Self::Atom> {
                self.0.next().map(|a| a.slot)
            }

            fn peek(&mut self) -> Option<&Self::Atom> {
                self.0.peek().map(|ws| &ws.slot)
            }

            fn push(&mut self, token: Self::Atom) {
                self.0.push(WithSpan {
                    slot: token,
                    span: S::default(),
                })
            }
        }
        let mut stream = SubStream(stream.into_parse_stream(), S::default());
        let slot = T::parse(&mut stream)?;
        Ok(WithSpan {
            slot,
            span: stream.1,
        })
    }
}

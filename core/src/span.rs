use crate::error::ParseError;
use crate::parse::{IntoParseStream, Parse, ParseStream, Unparse};
use newer_type::{implement, traits};
pub use syan_macro::Spanned;

pub trait Span: Clone + core::fmt::Debug + Default {
    fn migrate(self, other: Self) -> Self;
}

impl Span for () {
    fn migrate(self, _: Self) -> Self {}
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

pub trait Map<S>: Spanned
where
    S: Span,
{
    type Output: Spanned<Span = S>;

    fn map(self, replacement: impl FnMut(Self::Span) -> S) -> Self::Output;
}

pub trait Spanned {
    type Span: Span;

    fn span(&self) -> Self::Span;
}

impl<T: Spanned> Spanned for &'_ T {
    type Span = T::Span;

    fn span(&self) -> Self::Span {
        T::span(self)
    }
}

impl<T: Spanned> Spanned for &'_ mut T {
    type Span = T::Span;

    fn span(&self) -> Self::Span {
        T::span(self)
    }
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

    fn span(&self) -> Self::Span {
        self.span.clone()
    }
}

impl<T, S: Span, S2: Span> Map<S2> for WithSpan<T, S> {
    type Output = WithSpan<T, S2>;

    fn map(self, mut replacement: impl FnMut(Self::Span) -> S2) -> Self::Output {
        WithSpan {
            slot: self.slot,
            span: replacement(self.span),
        }
    }
}

impl<T, S, Atom> Unparse<Atom> for WithSpan<T, S>
where
    T: Unparse<Atom>,
{
    fn unparse<SS: crate::parse::unparse::Emitter<Atom>>(
        &self,
        sink: &mut SS,
    ) -> Result<(), SS::Error> {
        self.slot.unparse(sink)
    }
}

impl<Atom: Spanned, T> Parse<Atom> for WithSpan<T, Atom::Span>
where
    T: Parse<Atom>,
{
    type Error = T::Error;
    fn parse(stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
        struct SubStream<Slot, S>(Slot, S);

        impl<Slot, Atom, S: Span> ParseStream for SubStream<Slot, S>
        where
            Slot: ParseStream<Atom = Atom>,
            Atom: Spanned<Span = S>,
        {
            type Atom = Atom;
            type Error = Slot::Error;

            fn next(&mut self) -> Option<Self::Atom> {
                if let Some(atom) = self.0.next() {
                    self.1 = self.1.clone().migrate(atom.span());
                    Some(atom)
                } else {
                    None
                }
            }

            fn peek(&mut self) -> Option<&Self::Atom> {
                self.0.peek()
            }

            fn push(&mut self, token: Self::Atom) {
                self.0.push(token)
            }
        }
        let mut stream = SubStream(stream.into_parse_stream(), Atom::Span::default());
        let slot = T::parse(&mut stream)?;
        Ok(WithSpan {
            slot,
            span: stream.1,
        })
    }

    fn convert_error(error: Self::Error) -> ParseError<<Atom as Spanned>::Span>
    where
        Atom: Spanned,
    {
        T::convert_error(error)
    }
}

impl<T> Spanned for Vec<T>
where
    T: Spanned,
{
    type Span = T::Span;

    fn span(&self) -> Self::Span {
        self.iter()
            .fold(T::Span::default(), |acc, item| acc.migrate(item.span()))
    }
}

impl<T, S> Map<S> for Vec<T>
where
    T: Map<S>,
    S: Span,
{
    type Output = Vec<T::Output>;

    fn map(self, mut replacement: impl FnMut(Self::Span) -> S) -> Self::Output {
        let span = self.span();
        let new_span = replacement(span);
        self.into_iter()
            .map(|item| item.map(|_| new_span.clone()))
            .collect()
    }
}

impl<T> Spanned for std::collections::VecDeque<T>
where
    T: Spanned,
{
    type Span = T::Span;

    fn span(&self) -> Self::Span {
        self.iter()
            .fold(T::Span::default(), |acc, item| acc.migrate(item.span()))
    }
}

impl<T, S> Map<S> for std::collections::VecDeque<T>
where
    T: Map<S>,
    S: Span,
{
    type Output = std::collections::VecDeque<T::Output>;

    fn map(self, mut replacement: impl FnMut(Self::Span) -> S) -> Self::Output {
        let span = self.span();
        let new_span = replacement(span);
        self.into_iter()
            .map(|item| item.map(|_| new_span.clone()))
            .collect()
    }
}

impl<T> Spanned for Option<T>
where
    T: Spanned,
{
    type Span = T::Span;

    fn span(&self) -> Self::Span {
        self.as_ref().map(|item| item.span()).unwrap_or_default()
    }
}

impl<T, S> Map<S> for Option<T>
where
    T: Map<S>,
    S: Span,
{
    type Output = Option<T::Output>;

    fn map(self, mut replacement: impl FnMut(Self::Span) -> S) -> Self::Output {
        let span = self.span();
        let new_span = replacement(span);
        self.map(|item| item.map(|_| new_span.clone()))
    }
}

impl<T, E> Spanned for Result<T, E>
where
    T: Spanned,
{
    type Span = T::Span;

    fn span(&self) -> Self::Span {
        self.as_ref()
            .ok()
            .map(|item| item.span())
            .unwrap_or_default()
    }
}

impl<T, E, S> Map<S> for Result<T, E>
where
    T: Map<S>,
    S: Span,
{
    type Output = Result<T::Output, E>;

    fn map(self, mut replacement: impl FnMut(Self::Span) -> S) -> Self::Output {
        let span = self.span();
        let new_span = replacement(span);
        self.map(|item| item.map(|_| new_span.clone()))
    }
}

impl<T> Spanned for [T]
where
    T: Spanned,
{
    type Span = T::Span;

    fn span(&self) -> Self::Span {
        self.iter()
            .fold(T::Span::default(), |acc, item| acc.migrate(item.span()))
    }
}

impl<T, const N: usize> Spanned for [T; N]
where
    T: Spanned,
{
    type Span = T::Span;

    fn span(&self) -> Self::Span {
        self.iter()
            .fold(T::Span::default(), |acc, item| acc.migrate(item.span()))
    }
}

impl<T, const N: usize, S> Map<S> for [T; N]
where
    T: Map<S>,
    S: Span,
{
    type Output = [T::Output; N];

    fn map(self, mut replacement: impl FnMut(Self::Span) -> S) -> Self::Output {
        let span = self.span();
        let new_span = replacement(span);
        self.map(|item| item.map(|_| new_span.clone()))
    }
}

macro_rules! impl_for_tup {
    (@impl $($a:ident $A:ident)*) => {
        impl<S: Span$(,$A)*> Spanned for ($($A,)*)
        where
            $($A: Spanned<Span = S>,)*
        {
            type Span = S;

            fn span(&self) -> Self::Span {
                let ($($a,)*) = self;
                let span = S::default();
                $(
                    let span = span.migrate($a.span());
                )*
                span
            }
        }
        impl<S: Span$(,$A: Map<S, Span = Self::Span>)*> Map<S> for ($($A,)*)
        where
            Self: Spanned,
        {
            type Output = ($(<$A as Map<S>>::Output,)*);

            fn map(self, mut replacement: impl FnMut(Self::Span) -> S) -> Self::Output {
                let ($($a,)*) = self;
                ($($a.map(|m| replacement(m)),)*)
            }
        }
    };
    () => {};
    ($a:ident $A:ident $($t:tt)*) => {
        impl_for_tup!(@impl $a $A $($t)*);
        impl_for_tup!($($t)*);
    };
}
impl_for_tup!(a0 A0 a1 A1 a2 A2 a3 A3 a4 A4 a5 A5 a6 A6 a7 A7 a8 A8 a9 A9 a10 A10 a11 A11 a12 A12 a13 A13);

impl<T: Spanned> Spanned for Box<T> {
    type Span = T::Span;
    
    fn span(&self) -> Self::Span {
        self.as_ref().span()
    }
}

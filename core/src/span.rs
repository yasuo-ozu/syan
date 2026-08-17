//! Source positions: the [`Span`] trait, the [`Spanned`] trait for values that carry one, and
//! [`WithSpan`] for attaching one to a value that has none of its own.

use crate::parse::{IntoParseStream, Parse, ParseStream, Unparse};
use newer_type::{implement, traits};
pub use syan_macro::Spanned;

/// The span type an atom carries — `ParseError<SpanOf<Atom>>` is the error type of any parser over
/// `Atom`, so this alias keeps those signatures readable.
pub type SpanOf<Atom> = <Atom as Spanned>::Span;

/// A source position that can grow to cover more input.
///
/// Implement it for your source's position type; `()` and `Option<S>` already implement it, so use
/// `()` when you want no span tracking at all.
pub trait Span: Clone + core::fmt::Debug + Default {
    /// Widens `self` to also cover `other`, called once per atom as a parser consumes input.
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

// Defined (and `#[decycle]`-annotated) in `crate::decycle_traits` — see that module's docs.
pub use crate::decycle_traits::Spanned;

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

/// A value paired with the span of the input it was parsed from.
///
/// Wrap a field in this when the value's own type carries no position — parsing a
/// `WithSpan<T, S>` parses a `T` and records the span of everything it consumed.
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
    fn parse_stream<__S: crate::parse::parse_stream::ParseStream<Atom = Atom>>(stream: &mut __S) -> Result<Self, Self::Error> {
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

            // Known limitation: the accumulated span in `self.1` is NOT restored by a rollback, so
            // a failed inner attempt leaves the span over-extended. A fix would save
            // `(inner_raw, span)` on a side stack, as `source::proc_macro2::Stream` does for
            // `is_joint`.
            fn checkpoint_raw(&mut self) -> u64 {
                self.0.checkpoint_raw()
            }

            fn rollback_raw(&mut self, raw: u64) {
                self.0.rollback_raw(raw)
            }

            fn commit_raw(&mut self, raw: u64) {
                self.0.commit_raw(raw)
            }

            fn get_error(&mut self) -> Result<(), Self::Error> {
                self.0.get_error()
            }

            fn skip_sep(&mut self) -> bool {
                self.0.skip_sep()
            }
        }
        let mut stream = SubStream(stream.into_parse_stream(), Atom::Span::default());
        let slot = T::parse_stream(&mut stream)?;
        Ok(WithSpan {
            slot,
            span: stream.1,
        })
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

impl Spanned for core::convert::Infallible {
    type Span = ();

    fn span(&self) -> Self::Span {
        match *self {}
    }
}

impl<T> Spanned for core::marker::PhantomData<T> {
    type Span = ();

    fn span(&self) -> Self::Span {}
}

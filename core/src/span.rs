use crate::parse::{IntoParseStream, Parse, ParseStream, Unparse};
use newer_type::{implement, traits};
pub use syan_macro::Spanned;

pub trait Span: Clone + core::fmt::Debug + Default {
    /// Merge `self` (reached earlier) with `other` (reached later during the same left-to-right
    /// walk) into the span covering both. Callers fold an arbitrary number of sub-spans this way —
    /// [`Spanned`]'s tuple/slice/`Option` impls fold a sequence left to right,
    /// [`ParseStream::validate_spacing`] merges an entry and an exit peek — so `migrate`
    /// **must be an associative merge**:
    /// `a.migrate(b).migrate(c) == a.migrate(b.migrate(c))` for any three spans reached in that
    /// order. Grouping must not change the result.
    ///
    /// It should also normally be a genuine union of the covered source range, not "pick one
    /// argument and discard the other" — a `migrate` that throws away positional information can
    /// silently collapse a span folded from several sub-spans down to zero width (see the two
    /// built-in impls below for the concrete difference).
    ///
    /// Built-in implementations:
    /// - [`source::string::Span`](crate::source::string::Span) is a single *position*
    ///   (`line`/`col`/`loc`), not a range, so its `migrate` implements **pick-the-later**: whichever
    ///   operand has the greater `loc` wins outright, the other is discarded. That is correct for a
    ///   positional span type, but copying the same "keep one side" shape onto a *range* span
    ///   (`start..end`) is a trap — merging two ranges by keeping only the later one discards the
    ///   earlier `start`, so a span folded from several sub-spans comes out zero-width instead of
    ///   covering the whole range. This is exactly the failure mode to watch for when writing a new
    ///   `Span` impl.
    /// - [`source::proc_macro2::Span`](crate::source::proc_macro2::Span) *is* a `(start, end)`
    ///   range, and its `migrate` unions: the merged span's start comes from the earlier operand's
    ///   start, its end from the later operand's end (via `proc_macro2::Span::join`, falling back to
    ///   the respective endpoint when spans aren't joinable). This is the shape a range-based `Span`
    ///   should follow.
    ///
    /// ```
    /// use syan::source::string::Span;
    /// use syan::span::Span as _;
    ///
    /// let a = Span { line: 1, col: 1, loc: 0 };
    /// let b = Span { line: 1, col: 5, loc: 4 };
    /// let c = Span { line: 2, col: 1, loc: 10 };
    ///
    /// // Grouping the folds differently must agree — the associativity every `Span` impl owes its
    /// // callers. (Compared via `Debug` since `string::Span` doesn't derive `PartialEq`.)
    /// assert_eq!(
    ///     format!("{:?}", a.clone().migrate(b.clone()).migrate(c.clone())),
    ///     format!("{:?}", a.migrate(b.migrate(c))),
    /// );
    /// ```
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

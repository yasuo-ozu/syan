use crate::error::ParseError;
use crate::parse::unparse::Emitter;
use crate::parse::{Parse, Unparse};
use crate::span::{Span, Spanned};

/// Parse a `T` and a `U` in **either order** — `T U` *or* `U T`.
///
/// Both values are always kept (in `t`/`u`); the order they appeared in the input is remembered so that
/// [`Unparse`] emits them back faithfully. Parsing is greedy `T`-first: it tries `T U`, and only on
/// failure backtracks and tries `U T`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Unordered<T, U> {
    pub t: T,
    pub u: U,
    /// `true` iff `T` appeared before `U` in the input (drives the unparse order).
    t_first: bool,
}

impl<T, U> Unordered<T, U> {
    /// Build from values, choosing `T`-then-`U` as the (re)emission order.
    pub fn new(t: T, u: U) -> Self {
        Self {
            t,
            u,
            t_first: true,
        }
    }

    /// Whether `T` appeared — and will be unparsed — before `U`.
    pub fn t_first(&self) -> bool {
        self.t_first
    }

    /// Consume into the `(T, U)` pair (in `T`/`U` order, regardless of input order).
    pub fn into_inner(self) -> (T, U) {
        (self.t, self.u)
    }
}

impl<T: Default, U: Default> Default for Unordered<T, U> {
    fn default() -> Self {
        Self {
            t: T::default(),
            u: U::default(),
            t_first: true,
        }
    }
}

impl<Atom: crate::span::Spanned, T, U> Parse<Atom> for Unordered<T, U>
where
    Atom: Clone,
    T: Parse<Atom>,
    U: Parse<Atom>,
    T::Error: Into<ParseError<crate::span::SpanOf<Atom>>>,
    U::Error: Into<ParseError<crate::span::SpanOf<Atom>>>,
{
    type Error = ParseError<crate::span::SpanOf<Atom>>;

    fn parse_stream<__S: crate::parse::parse_stream::ParseStream<Atom = Atom>>(
        stream: &mut __S,
    ) -> Result<Self, Self::Error> {
        let t_then_u = stream.dup(
            |s| -> Result<(T, U), ParseError<crate::span::SpanOf<Atom>>> {
                let t = T::parse_stream(&mut *s).map_err(Into::into)?;
                let u = U::parse_stream(&mut *s).map_err(Into::into)?;
                Ok((t, u))
            },
        );
        match t_then_u {
            Ok((t, u)) => Ok(Self {
                t,
                u,
                t_first: true,
            }),
            Err(_) => {
                let u = U::parse_stream(&mut *stream).map_err(Into::into)?;
                let t = T::parse_stream(&mut *stream).map_err(Into::into)?;
                Ok(Self {
                    t,
                    u,
                    t_first: false,
                })
            }
        }
    }
}

impl<Atom, T, U> Unparse<Atom> for Unordered<T, U>
where
    T: Unparse<Atom>,
    U: Unparse<Atom>,
{
    fn unparse<S: Emitter<Atom>>(&self, sink: &mut S) -> Result<(), S::Error> {
        if self.t_first {
            self.t.unparse(sink)?;
            self.u.unparse(sink)
        } else {
            self.u.unparse(sink)?;
            self.t.unparse(sink)
        }
    }
}

impl<T, U, S> Spanned for Unordered<T, U>
where
    S: Span,
    T: Spanned<Span = S>,
    U: Spanned<Span = S>,
{
    type Span = S;

    fn span(&self) -> S {
        // Fold the two spans in input order, so the result covers the pair as it was written.
        if self.t_first {
            Span::migrate(self.t.span(), self.u.span())
        } else {
            Span::migrate(self.u.span(), self.t.span())
        }
    }
}

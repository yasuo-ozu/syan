use crate::error::ParseError;
use crate::parse::{Parse, Unparse};
use crate::span::Spanned;
use crate::span::WithSpan;
use crate::symbol::chars as punct;
/// Parses `T` between an opening and a closing delimiter. Reach for the [`GroupParen`],
/// [`GroupBrace`] and [`GroupBracket`] aliases rather than naming `O` and `C` by hand.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Group<T, O, C> {
    pub open: O,
    pub slot: T,
    pub close: C,
}

// For a token source a delimited group is a *single* `TokenTree::Group`, not three tokens, so these
// impls are hand-written per delimiter instead of derived. `Group` itself is atom-agnostic; only these
// impls need the optional `proc_macro2` dependency.
#[cfg(feature = "proc_macro2")]
macro_rules! impl_group_unparse_tt {
    ($open:ident, $close:ident, $delim:ident) => {
        impl<T, S> Unparse<proc_macro2::TokenTree>
            for Group<T, WithSpan<punct::$open, S>, WithSpan<punct::$close, S>>
        where
            T: Unparse<proc_macro2::TokenTree>,
        {
            fn unparse<E: crate::parse::unparse::Emitter<proc_macro2::TokenTree>>(
                &self,
                sink: &mut E,
            ) -> Result<(), E::Error> {
                emit_tt_group(&self.slot, proc_macro2::Delimiter::$delim, sink)
            }
        }

        impl<S> GroupUnparse<proc_macro2::TokenTree>
            for Group<(), WithSpan<punct::$open, S>, WithSpan<punct::$close, S>>
        {
            fn unparse_group<Slot, E>(
                &self,
                slot: &Slot,
                sink: &mut E,
            ) -> Result<(), <E as crate::parse::unparse::Emitter<proc_macro2::TokenTree>>::Error>
            where
                Slot: Unparse<proc_macro2::TokenTree>,
                E: crate::parse::unparse::Emitter<proc_macro2::TokenTree>,
            {
                emit_tt_group(slot, proc_macro2::Delimiter::$delim, sink)
            }
        }
    };
}

/// Emit `slot` into a sub-stream and write it as one delimited `TokenTree::Group`.
#[cfg(feature = "proc_macro2")]
fn emit_tt_group<T, E>(
    slot: &T,
    delim: proc_macro2::Delimiter,
    sink: &mut E,
) -> Result<(), E::Error>
where
    T: Unparse<proc_macro2::TokenTree>,
    E: crate::parse::unparse::Emitter<proc_macro2::TokenTree>,
{
    let mut inner = Vec::<proc_macro2::TokenTree>::new();
    // The sub-emitter (`&mut Vec`) is `Infallible`, so this never errors.
    slot.unparse(&mut (&mut inner)).unwrap();
    let stream: proc_macro2::TokenStream = inner.into_iter().collect();
    sink.write_one(proc_macro2::TokenTree::Group(proc_macro2::Group::new(
        delim, stream,
    )))
}
#[cfg(feature = "proc_macro2")]
impl_group_unparse_tt!(OpenParen, CloseParen, Parenthesis);
#[cfg(feature = "proc_macro2")]
impl_group_unparse_tt!(OpenBrace, CloseBrace, Brace);
#[cfg(feature = "proc_macro2")]
impl_group_unparse_tt!(OpenBracket, CloseBracket, Bracket);

// The span comes from the delimiters alone, deliberately not requiring `T: Spanned` — an empty group
// (`Group<(), ..>`) still has a span. `#[derive(Spanned)]` would fold the slot in and lose that.
impl<T, O, C> Spanned for Group<T, O, C>
where
    O: Spanned,
    C: Spanned<Span = O::Span>,
{
    type Span = O::Span;

    fn span(&self) -> Self::Span {
        crate::span::Span::migrate(self.open.span(), self.close.span())
    }
}

impl<Atom: crate::span::Spanned, T, O, C> Parse<Atom> for Group<T, O, C>
where
    Atom: crate::span::Spanned,
    T: Parse<Atom>,
    T::Error: Into<ParseError<crate::span::SpanOf<Atom>>>,
    O: Parse<Atom>,
    O::Error: Into<ParseError<crate::span::SpanOf<Atom>>>,
    C: Parse<Atom>,
    C::Error: Into<ParseError<crate::span::SpanOf<Atom>>>,
{
    type Error = ParseError<crate::span::SpanOf<Atom>>;

    fn parse_stream<__S: crate::parse::parse_stream::ParseStream<Atom = Atom>>(
        stream: &mut __S,
    ) -> Result<Self, Self::Error> {
        let open = O::parse_stream(&mut *stream).map_err(Into::into)?;
        crate::parse::parse_stream::ParseStream::skip_sep(&mut *stream);
        let slot = T::parse_stream(&mut *stream).map_err(Into::into)?;
        crate::parse::parse_stream::ParseStream::skip_sep(&mut *stream);
        let close = C::parse_stream(&mut *stream).map_err(Into::into)?;
        Ok(Group { open, slot, close })
    }
}

/// Parses a delimited group whose content type the caller picks, yielding the content and the empty
/// delimiter holder. This is what `#[derive(Parse)]` uses for a `#[group(..)]` field.
///
/// `Slot` is a method generic and the result is `(Slot, Self)`, so the obligation `FieldTy:
/// GroupShape<Atom>` mentions neither the content type nor a projection — that is what lets a
/// `#[recurse]` cycle pass through a `#[group]` field. Do not lift `Slot` to the trait.
///
/// Implemented two ways: the generic sequencing impl below parses open, content and close as three
/// atoms, while `crate::source::proc_macro2` implements it per delimiter, consuming a group as a
/// single atom.
pub trait GroupShape<Atom: crate::span::Spanned>: Sized {
    /// Parse `Slot` between the delimiters.
    fn parse_group<Slot, __S: crate::parse::parse_stream::ParseStream<Atom = Atom>>(
        stream: &mut __S,
    ) -> Result<(Slot, Self), ParseError<crate::span::SpanOf<Atom>>>
    where
        Slot: Parse<Atom>,
        Slot::Error: Into<ParseError<crate::span::SpanOf<Atom>>>;
}

/// The [`GroupShape`] counterpart for emitting: writes the delimited group back out around `slot`.
/// Both the holder and the content are taken by reference, so nothing is cloned.
pub trait GroupUnparse<Atom> {
    /// Emit the delimiters around `slot`.
    fn unparse_group<Slot, E>(
        &self,
        slot: &Slot,
        sink: &mut E,
    ) -> Result<(), <E as crate::parse::unparse::Emitter<Atom>>::Error>
    where
        Slot: Unparse<Atom>,
        E: crate::parse::unparse::Emitter<Atom>;
}

impl<Atom: crate::span::Spanned, O, C> GroupShape<Atom> for Group<(), O, C>
where
    O: Parse<Atom>,
    O::Error: Into<ParseError<crate::span::SpanOf<Atom>>>,
    C: Parse<Atom>,
    C::Error: Into<ParseError<crate::span::SpanOf<Atom>>>,
{
    fn parse_group<Slot, __S: crate::parse::parse_stream::ParseStream<Atom = Atom>>(
        stream: &mut __S,
    ) -> Result<(Slot, Self), ParseError<crate::span::SpanOf<Atom>>>
    where
        Slot: Parse<Atom>,
        Slot::Error: Into<ParseError<crate::span::SpanOf<Atom>>>,
    {
        let open = O::parse_stream(&mut *stream).map_err(Into::into)?;
        crate::parse::parse_stream::ParseStream::skip_sep(&mut *stream);
        let slot = Slot::parse_stream(&mut *stream).map_err(Into::into)?;
        crate::parse::parse_stream::ParseStream::skip_sep(&mut *stream);
        let close = C::parse_stream(&mut *stream).map_err(Into::into)?;
        Ok((
            slot,
            Group {
                open,
                slot: (),
                close,
            },
        ))
    }
}

impl<Atom, O, C> GroupUnparse<Atom> for Group<(), O, C>
where
    O: Unparse<Atom>,
    C: Unparse<Atom>,
{
    fn unparse_group<Slot, E>(
        &self,
        slot: &Slot,
        sink: &mut E,
    ) -> Result<(), <E as crate::parse::unparse::Emitter<Atom>>::Error>
    where
        Slot: Unparse<Atom>,
        E: crate::parse::unparse::Emitter<Atom>,
    {
        self.open.unparse(sink)?;
        slot.unparse(sink)?;
        self.close.unparse(sink)
    }
}

/// A `T` in parentheses: `( T )`.
pub type GroupParen<T, S> = Group<T, WithSpan<punct::OpenParen, S>, WithSpan<punct::CloseParen, S>>;
/// A `T` in braces: `{ T }`.
pub type GroupBrace<T, S> = Group<T, WithSpan<punct::OpenBrace, S>, WithSpan<punct::CloseBrace, S>>;
/// A `T` in square brackets: `[ T ]`.
pub type GroupBracket<T, S> =
    Group<T, WithSpan<punct::OpenBracket, S>, WithSpan<punct::CloseBracket, S>>;

/// A pair of delimiters with no content yet, as produced by [`GroupShape::parse_group`]. Implemented
/// by `Group<(), O, C>` to move content in and out of the holder.
pub trait EmptyGroup {
    /// The same delimiters holding a `Slot`.
    type Fill<Slot>;

    /// Put `slot` between the delimiters.
    fn fill<Slot>(self, slot: Slot) -> Self::Fill<Slot>;
    /// Take the content back out, leaving the delimiters empty.
    fn unfill<Slot>(group: Self::Fill<Slot>) -> (Slot, Self);
}

impl<T, O, C> std::fmt::Display for Group<T, O, C>
where
    T: std::fmt::Display,
    O: std::fmt::Display,
    C: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.open.fmt(f)?;
        self.slot.fmt(f)?;
        self.close.fmt(f)
    }
}

impl<T, O, C> std::ops::Deref for Group<T, O, C> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.slot
    }
}

impl<O, C> EmptyGroup for Group<(), O, C> {
    type Fill<Slot> = Group<Slot, O, C>;

    fn unfill<Slot>(group: Self::Fill<Slot>) -> (Slot, Self) {
        (
            group.slot,
            Group {
                slot: (),
                open: group.open,
                close: group.close,
            },
        )
    }
    fn fill<Slot>(self, slot: Slot) -> Self::Fill<Slot> {
        Group {
            slot,
            open: self.open,
            close: self.close,
        }
    }
}

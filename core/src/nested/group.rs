use crate::error::ParseError;
use crate::parse::{Parse, Unparse};
use crate::span::Spanned;
use crate::span::WithSpan;
use crate::symbol::chars as punct;
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Group<T, O, C> {
    pub open: O,
    pub slot: T,
    pub close: C,
}

// `Unparse` to a `TokenTree`: a delimited group is a *single* `TokenTree::Group` (the delimiters aren't
// standalone tokens), so `Group` is hand-written per real delimiter rather than `#[derive(Unparse)]`d —
// the slot is unparsed into a sub-stream wrapped in the matching `Delimiter`.
//
// This whole block names `proc_macro2` types directly, so it is gated on the optional dependency —
// like `source::proc_macro2`. `Group` itself is atom-agnostic and stays available either way.
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

        // The `GroupUnparse` counterpart, on the EMPTY holder: same emission, but the content comes
        // from the caller instead of from `self.slot`, so nothing is cloned into a filled group.
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

// A group's span is the range its delimiters cover; the slot's content lies *between* `open` and
// `close`, so the span is taken from the delimiters only. This deliberately does NOT require `T:
// Spanned`, so an empty group (`Group<(), ..>`) — or one whose content isn't `Spanned` — still has a
// span (the hand-written impl replaces what `#[derive(Spanned)]` would emit, which folds the slot too).
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

    fn parse_stream<__S: crate::parse::parse_stream::ParseStream<Atom = Atom>>(stream: &mut __S) -> Result<Self, Self::Error> {
        let open = O::parse_stream(&mut *stream).map_err(Into::into)?;
        let slot = T::parse_stream(&mut *stream).map_err(Into::into)?;
        let close = C::parse_stream(&mut *stream).map_err(Into::into)?;
        Ok(Group { open, slot, close })
    }
}

/// Parse a delimited group whose **content type is chosen by the caller**, returning the content and
/// the now-empty holder.
///
/// This is what `#[derive(Parse)]` uses for a `#[group(..)]` field, and it is shaped the way it is for
/// one reason: **`Slot` is a method generic, not a trait parameter**. The resulting obligation
/// `FieldTy: GroupShape<Atom>` therefore says nothing about the content type, so it is never an edge in
/// the type's recursion — which is what lets a `#[recurse]` cycle pass through a `#[group]` field. (The
/// older formulation bounded `<FieldTy as EmptyGroup>::Fill<Substruct>: Parse<Atom>`: a projection that
/// mentions the substruct, has no head type, and so can be neither reduced nor cycle-broken. See the
/// `#[group]` entry under *Known gaps* in CLAUDE.md.)
///
/// Returning `(Slot, Self)` rather than a filled group is the other half: with no associated type there
/// is no projection anywhere in the obligation.
///
/// Two families of impl exist, matching how real sources represent a group:
/// - the **generic sequencing** impl below — open, content, close as three consecutive parses, correct
///   for a flat/char-like atom;
/// - **delimiter-specific** impls (see `crate::source::proc_macro2`) — for a token source a group is a
///   *single* atom, so the impl consumes one `TokenTree::Group`, parses the content from its inner
///   stream, and synthesizes the delimiter spans. The delimiters are never parsed as tokens there.
pub trait GroupShape<Atom: crate::span::Spanned>: Sized {
    fn parse_group<Slot, __S: crate::parse::parse_stream::ParseStream<Atom = Atom>>(
        stream: &mut __S,
    ) -> Result<(Slot, Self), ParseError<crate::span::SpanOf<Atom>>>
    where
        Slot: Parse<Atom>,
        Slot::Error: Into<ParseError<crate::span::SpanOf<Atom>>>;
}

/// The [`GroupShape`] counterpart for emitting: write the delimited group back out around `slot`.
///
/// Takes the holder by reference and the content by reference, so — unlike the `EmptyGroup::fill`
/// formulation it replaces — nothing is cloned and the holder needs no `Clone` bound.
pub trait GroupUnparse<Atom> {
    fn unparse_group<Slot, E>(
        &self,
        slot: &Slot,
        sink: &mut E,
    ) -> Result<(), <E as crate::parse::unparse::Emitter<Atom>>::Error>
    where
        Slot: Unparse<Atom>,
        E: crate::parse::unparse::Emitter<Atom>;
}

// Generic sequencing: correct whenever the delimiters really are atoms of their own.
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
        let slot = Slot::parse_stream(&mut *stream).map_err(Into::into)?;
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

pub type GroupParen<T, S> = Group<T, WithSpan<punct::OpenParen, S>, WithSpan<punct::CloseParen, S>>;
pub type GroupBrace<T, S> = Group<T, WithSpan<punct::OpenBrace, S>, WithSpan<punct::CloseBrace, S>>;
pub type GroupBracket<T, S> =
    Group<T, WithSpan<punct::OpenBracket, S>, WithSpan<punct::CloseBracket, S>>;

pub trait EmptyGroup {
    type Fill<Slot>;

    fn fill<Slot>(self, slot: Slot) -> Self::Fill<Slot>;
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

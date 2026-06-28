use crate::error::ParseError;
use crate::parse::{IntoParseStream, Parse, Unparse};
use crate::span::Spanned;
use crate::span::WithSpan;
use crate::symbol::chars as punct;
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Group<T, O, C> {
    pub open: O,
    pub slot: T,
    pub close: C,
}

// `Unparse` to a `proc_macro2::TokenTree` atom: a delimited group is a *single* `TokenTree::Group`
// carrying its slot's tokens, NOT three separate tokens — and the delimiter symbols (`{`/`}` etc.) don't
// `Unparse` to `TokenTree` on their own (they aren't standalone tokens). So `Group` is hand-written per
// real delimiter rather than `#[derive(Unparse)]`d: the slot is unparsed into a sub-stream which is
// wrapped in the matching `Delimiter`. (`#[derive(Unparse)]` is dropped from `Group`; only these
// delimiter+`TokenTree` impls exist. `GroupAngle` has no proc-macro delimiter and so no impl.)
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
                let mut inner = Vec::<proc_macro2::TokenTree>::new();
                // The sub-emitter (`&mut Vec`) is `Infallible`, so this never errors.
                self.slot.unparse(&mut (&mut inner)).unwrap();
                let stream: proc_macro2::TokenStream = inner.into_iter().collect();
                let group = proc_macro2::Group::new(proc_macro2::Delimiter::$delim, stream);
                sink.write_one(proc_macro2::TokenTree::Group(group))
            }
        }
    };
}
impl_group_unparse_tt!(OpenParen, CloseParen, Parenthesis);
impl_group_unparse_tt!(OpenBrace, CloseBrace, Brace);
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

impl<Atom, T, O, C> Parse<Atom> for Group<T, O, C>
where
    T: Parse<Atom>,
    O: Parse<Atom>,
    C: Parse<Atom>,
{
    type Error = ParseError;

    fn parse(stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
        let mut stream = stream.into_parse_stream();
        let open = O::parse(&mut stream).map_err(crate::error::Error::into_parse_error)?;
        let slot = T::parse(&mut stream).map_err(crate::error::Error::into_parse_error)?;
        let close = C::parse(&mut stream).map_err(crate::error::Error::into_parse_error)?;
        Ok(Group { open, slot, close })
    }
}

pub type GroupParen<T, S> = Group<T, WithSpan<punct::OpenParen, S>, WithSpan<punct::CloseParen, S>>;
pub type GroupBrace<T, S> = Group<T, WithSpan<punct::OpenBrace, S>, WithSpan<punct::CloseBrace, S>>;
pub type GroupBracket<T, S> =
    Group<T, WithSpan<punct::OpenBracket, S>, WithSpan<punct::CloseBracket, S>>;
pub type GroupAngle<T, S> = Group<T, WithSpan<punct::OpenAngle, S>, WithSpan<punct::CloseAngle, S>>;

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

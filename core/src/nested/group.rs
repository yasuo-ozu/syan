use crate::error::ParseError;
use crate::parse::{IntoParseStream, Parse, Unparse};
use crate::span::Spanned;
use crate::span::WithSpan;
use crate::symbol::chars as punct;
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Unparse, Spanned)]
#[syan(crate)]
pub struct Group<T, O, C> {
    pub open: O,
    pub slot: T,
    pub close: C,
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

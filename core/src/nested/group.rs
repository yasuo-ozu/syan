use crate::parse::Parse;
use crate::span::WithSpan;
use crate::symbol::chars as punct;
use std::fmt::Display;
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Group<T, O, C> {
    pub open: O,
    pub slot: T,
    pub close: C,
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

impl<K, T, O, C> Parse<K> for Group<T, O, C>
where
    T: Parse<K>,
    O: Parse<K> + Display,
    C: Parse<K> + Display,
    O::Error: crate::error::UnionWith<T::Error>,
    <O::Error as crate::error::UnionWith<T::Error>>::Output: crate::error::UnionWith<C::Error>,
{
    type Error =
        <<O::Error as crate::error::UnionWith<T::Error>>::Output as crate::error::UnionWith<
            C::Error,
        >>::Output;
    //fn parse(stream: &mut impl ParseStream<Atom = K>) -> crate::error::Result<Self, K> {
    //}

    fn parse(stream: impl crate::parse::IntoParseStream<Atom = K>) -> Result<Self, Self::Error> {
        let mut stream = stream.into_parse_stream();
        let open = O::parse(&mut stream).map_err(|o_err| {
            <<O::Error as crate::error::UnionWith<T::Error>>::Output as crate::error::UnionWith<
                C::Error,
            >>::use_left(<O::Error as crate::error::UnionWith<T::Error>>::use_left(
                o_err,
            ))
        })?;
        // TODO: The proper bracket-aware parsing logic is complex and needs more work
        // For now, just parse T directly from the stream
        let slot = T::parse(&mut stream).map_err(|t_err| {
            <<O::Error as crate::error::UnionWith<T::Error>>::Output as crate::error::UnionWith<
                C::Error,
            >>::use_left(<O::Error as crate::error::UnionWith<T::Error>>::use_right(
                t_err,
            ))
        })?;
        let close = C::parse(&mut stream).map_err(|c_err| {
            <<O::Error as crate::error::UnionWith<T::Error>>::Output as crate::error::UnionWith<
                C::Error,
            >>::use_right(c_err)
        })?;
        Ok(Group { slot, open, close })
    }
}

pub mod into_parse_stream;

#[allow(clippy::module_inception)]
pub mod parse;
pub mod parse_stream;
pub mod unparse;
#[doc(hidden)]
pub mod vtable;

mod tuple;

pub use into_parse_stream::IntoParseStream;
pub use parse::Parse;
pub use parse_stream::ParseStream;
pub use syan_macro::recurse;
pub use unparse::Unparse;

use crate::span::Span;

macro_rules! impl_for_collection {
    () => {};
    ([$item:ident $($p:tt)*] $self:ty, $($t:tt)*) => {
        impl<Atom: Clone, $item $($p)*> Parse<Atom> for $self
        where
            $item: Parse<Atom>,
        {
            type Error = $item::Error;
            fn parse(stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
                let mut v: Self = Default::default();
                let mut stream = stream.into_parse_stream();
                while let Ok(item) = stream.dup(|stream| $item::parse(stream)) {
                    v.extend(std::iter::once(item));
                }
                Ok(v)
            }
        }

        impl<Atom, $item $($p)*> Unparse<Atom> for $self
        where
            $item: Unparse<Atom>,
        {
            fn unparse<E: unparse::Emitter<Atom>>(&self, emitter: &mut E) -> Result<(), E::Error> {
                for item in self {
                    item.unparse(emitter)?;
                }
                Ok(())
            }
        }

        impl<$item $($p)*> crate::span::Spanned for $self
        where
            $item: crate::span::Spanned,
        {
            type Span = $item::Span;

            fn span(&self) -> Self::Span {
                self.iter()
                    .fold(Default::default(), |acc, item| {
                        acc.migrate(item.span())
                    })
            }
        }

        impl_for_collection!($($t)*);
    };
}

impl_for_collection!(
    [Item] Vec<Item>,
    [Item] std::collections::VecDeque<Item>,
    [Item: std::hash::Hash + Eq] std::collections::HashSet<Item>,
    [Item: std::cmp::Ord] std::collections::BTreeSet<Item>,
);

macro_rules! impl_for_map {
    () => {};
    ([$key:ident $($pk:tt)*][$value:ident $($pv:tt)*] $self:ty, $($t:tt)*) => {
        impl<Atom: Clone + crate::span::Spanned, Err, $key $($pk)*, $value $($pv)*> Parse<Atom> for $self
        where
            $key: Parse<Atom>,
            $value: Parse<Atom>,
            <$key as Parse<Atom>>::Error: crate::error::UnionWith<$value::Error, Output = Err>,
            Err: crate::error::Error,
        {
            type Error = Err;
            fn parse(stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
                let mut ret: Self = Default::default();
                let mut stream = stream.into_parse_stream();
                while let Ok((k, v)) = stream.dup(|mut stream| {
                    if let Ok(k) = $key::parse(&mut stream) {
                        if let Ok(v) = $value::parse(&mut stream) {
                            return Ok((k, v));
                        }
                    }
                    Err(())
                }) {
                    ret.insert(k, v);
                }
                Ok(ret)
            }
        }

        impl<Atom, $key $($pk)*, $value $($pv)*> Unparse<Atom> for $self
        where
            $key: Unparse<Atom>,
            $value: Unparse<Atom>,
        {
            fn unparse<E: unparse::Emitter<Atom>>(&self, emitter: &mut E) -> Result<(), E::Error> {
                for (key, value) in self {
                    key.unparse(emitter)?;
                    value.unparse(emitter)?;
                }
                Ok(())
            }
        }

        impl<$key $($pk)*, $value $($pv)*> crate::span::Spanned for $self
        where
            $key: crate::span::Spanned,
            $value: crate::span::Spanned<Span = $key::Span>,
        {
            type Span = $key::Span;

            fn span(&self) -> Self::Span {
                self.iter()
                    .fold(Default::default(), |acc, (key, value)| {
                        acc.migrate(key.span()).migrate(value.span())
                    })
            }
        }

        impl_for_map!($($t)*);
    };
}

impl_for_map! {
    [K: std::hash::Hash + Eq] [V] std::collections::HashMap<K, V>,
    [K: std::cmp::Ord] [V] std::collections::BTreeMap<K, V>,
}

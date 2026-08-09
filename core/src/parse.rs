pub mod into_parse_stream;

#[allow(clippy::module_inception)]
pub mod parse;
pub mod parse_stream;
pub mod tape;
pub mod unparse;

mod tuple;

pub use into_parse_stream::IntoParseStream;
pub use parse::Parse;
pub use parse_stream::{erase, ParseStream};
pub use syan_macro::recurse;
pub use tape::Tape;
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

use super::{IntoParseStream, ParseStream};
use crate::error::ParseError;
use crate::span::Spanned;

pub use syan_macro::Parse;

pub trait Parse<Atom>: Sized {
    type Error;
    fn parse(stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error>;

    fn convert_error(_: Self::Error) -> ParseError<<Atom as Spanned>::Span>
    where
        Atom: Spanned;

    // TODO: add rollback_subsequent_error()
}

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

            fn convert_error(error: Self::Error) -> ParseError<<Atom as Spanned>::Span>
            where
                Atom: Spanned,
            {
                $item::convert_error(error)
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
        impl<Atom: Clone + Spanned, Err, $key $($pk)*, $value $($pv)*> Parse<Atom> for $self
        where
            $key: Parse<Atom>,
            $value: Parse<Atom>,
            <$key as Parse<Atom>>::Error: crate::error::UnionWith<$value::Error, Output = Err>,
            Err: Into<ParseError<<Atom as Spanned>::Span>>,
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

            fn convert_error(error: Self::Error) -> ParseError<<Atom as Spanned>::Span>
            where
                Atom: Spanned,
            {
                error.into()
            }
        }
        impl_for_map!($($t)*);
    };
}

impl_for_map! {
    [K: std::hash::Hash + Eq] [V] std::collections::HashMap<K, V>,
    [K: std::cmp::Ord] [V] std::collections::BTreeMap<K, V>,
}

impl<Atom, Item> Parse<Atom> for Box<Item>
where
    Item: Parse<Atom>,
{
    type Error = Item::Error;
    fn parse(stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
        Ok(Box::new(Item::parse(stream)?))
    }

    fn convert_error(error: Self::Error) -> ParseError<<Atom as Spanned>::Span>
    where
        Atom: Spanned,
    {
        Item::convert_error(error)
    }
}

impl<Atom: Clone, Item> Parse<Atom> for Option<Item>
where
    Item: Parse<Atom>,
{
    type Error = core::convert::Infallible;
    fn parse(stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
        let mut stream = stream.into_parse_stream();
        Ok(stream.dup(|stream| Item::parse(stream)).ok())
    }

    fn convert_error(error: Self::Error) -> ParseError<<Atom as Spanned>::Span>
    where
        Atom: Spanned,
    {
        match error {}
    }
}

impl<const N: usize, Atom, T> Parse<Atom> for [T; N]
where
    T: Parse<Atom>,
{
    type Error = T::Error;
    fn parse(stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
        let mut stream = stream.into_parse_stream();
        let mut v = Vec::new();
        for _ in 0..N {
            v.push(T::parse(&mut stream)?);
        }
        Ok(v.try_into().unwrap_or_else(|_| panic!()))
    }

    fn convert_error(error: Self::Error) -> ParseError<<Atom as Spanned>::Span>
    where
        Atom: Spanned,
    {
        T::convert_error(error)
    }
}

impl<Atom, T, E> Parse<Atom> for Result<T, E>
where
    T: Parse<Atom, Error = E>,
{
    type Error = core::convert::Infallible;
    fn parse(stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
        Ok(T::parse(stream))
    }

    fn convert_error(error: Self::Error) -> ParseError<<Atom as Spanned>::Span>
    where
        Atom: Spanned,
    {
        match error {}
    }
}

impl<Atom: crate::span::Spanned, T> Parse<Atom> for core::marker::PhantomData<T> {
    type Error = core::convert::Infallible;
    fn parse(_stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
        Ok(Default::default())
    }

    fn convert_error(error: Self::Error) -> ParseError<<Atom as Spanned>::Span>
    where
        Atom: Spanned,
    {
        match error {}
    }
}

impl<Atom: crate::span::Spanned> Parse<Atom> for core::convert::Infallible {
    type Error = crate::error::ParseError<Atom::Span>;
    fn parse(_stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
        panic!()
    }

    fn convert_error(error: Self::Error) -> ParseError<<Atom as Spanned>::Span>
    where
        Atom: Spanned,
    {
        error
    }
}

impl<const COUNT: usize, Atom, T> crate::_imp::ParseImpl<COUNT, Atom> for T where T: Parse<Atom> {}

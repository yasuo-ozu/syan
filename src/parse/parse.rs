use super::{IntoParseStream, ParseStream};

pub trait Parse<Atom>: Sized {
    type Error;
    fn parse(stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error>;
}

mod _joint_impl {
    #[macro_export]
    macro_rules! _joint_impl {
        ($($t:ty),* $(,)?) => {
            $xrate::nested::Joint<($($t,)*)>
        };
    }
    pub use _joint_impl as Joint;
}
pub use _joint_impl::*;

macro_rules! impl_for_collection {
    () => {};
    ([$item:ident $($p:tt)*] $self:ty, $($t:tt)*) => {
        impl<Atom: Clone, $item $($p)*> Parse<Atom> for $self
        where
            $item: Parse<Atom>,
        {
            type Error = core::convert::Infallible;
            fn parse(stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
                let mut v: Self = Default::default();
                let mut stream = stream.into_parse_stream();
                while let Ok(item) = stream.dup(|stream| $item::parse(stream)) {
                    v.extend(std::iter::once(item));
                }
                Ok(v)
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
        impl<Atom: Clone, $key $($pk)*, $value $($pv)*> Parse<Atom> for $self
        where
            $key: Parse<Atom>,
            $value: Parse<Atom>,
        {
            type Error = core::convert::Infallible;
            fn parse(stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
                let mut ret: Self = Default::default();
                let mut stream = stream.into_parse_stream();
                while let Ok((k, v)) = stream.dup(|mut stream| {
                    if let Ok(k) = K::parse(&mut stream) {
                        if let Ok(v) = V::parse(&mut stream) {
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
        impl_for_map!($($t)*);
    };
}

impl_for_map! {
    [K: std::hash::Hash + Eq] [V] std::collections::HashMap<K, V>,
    [K: std::cmp::Ord] [V] std::collections::BTreeMap<K, V>,
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
}

impl<Atom, Tuple, Head, Rem, Error, HeadError> Parse<Atom> for Tuple
where
    Tuple: crate::tuple::PopHead<Head = Head, Rem = Rem>,
    Rem: Parse<Atom>,
    Head: Parse<Atom, Error = HeadError>,
    HeadError: crate::error::Merge<Rem::Error, Output = Error>,
{
    type Error = Error;
    fn parse(stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
        let mut stream = stream.into_parse_stream();
        let head = Head::parse(&mut stream).map_err(HeadError::from_left)?;
        let rem = Rem::parse(&mut stream).map_err(HeadError::from_right)?;
        Ok(Tuple::unsplit(head, rem))
    }
}

impl<Atom> Parse<Atom> for () {
    type Error = core::convert::Infallible;
    fn parse(_: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
        Ok(())
    }
}

impl<Atom, T, Error> Parse<Atom> for Result<T, Error>
where
    T: Parse<Atom, Error = Error>,
{
    type Error = core::convert::Infallible;
    fn parse(stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
        Ok(T::parse(stream))
    }
}

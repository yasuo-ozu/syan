use super::{IntoParseStream, ParseStream};

pub use syan_macro::Parse;

// The trait itself is defined (and `#[decycle]`-annotated) in `crate::decycle_traits` — see that
// module's docs: its alter macro would otherwise collide with the derive re-export just above.
pub use crate::decycle_traits::Parse;

impl<Atom, Item> Parse<Atom> for Box<Item>
where
    Item: Parse<Atom>,
{
    type Error = Item::Error;
    fn parse(stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
        Ok(Box::new(Item::parse(stream)?))
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

impl<Atom, T, E> Parse<Atom> for Result<T, E>
where
    T: Parse<Atom, Error = E>,
{
    type Error = core::convert::Infallible;
    fn parse(stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
        Ok(T::parse(stream))
    }
}

impl<Atom, T> Parse<Atom> for core::marker::PhantomData<T> {
    type Error = core::convert::Infallible;
    fn parse(_stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
        Ok(Default::default())
    }
}

impl<Atom> Parse<Atom> for core::convert::Infallible {
    type Error = crate::error::ParseError;
    fn parse(_stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
        panic!()
    }
}

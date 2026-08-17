pub use syan_macro::Parse;

// The trait itself is defined (and `#[decycle]`-annotated) in `crate::decycle_traits` — see that
// module's docs: its alter macro would otherwise collide with the derive re-export just above.
pub use crate::decycle_traits::Parse;

impl<Atom: crate::span::Spanned, Item> Parse<Atom> for Box<Item>
where
    Item: Parse<Atom>,
{
    type Error = Item::Error;
    fn parse_stream<__S: crate::parse::parse_stream::ParseStream<Atom = Atom>>(
        stream: &mut __S,
    ) -> Result<Self, Self::Error> {
        Ok(Box::new(Item::parse_stream(&mut *stream)?))
    }
}

impl<Atom: crate::span::Spanned + Clone, Item> Parse<Atom> for Option<Item>
where
    Item: Parse<Atom>,
{
    type Error = core::convert::Infallible;
    fn parse_stream<__S: crate::parse::parse_stream::ParseStream<Atom = Atom>>(
        stream: &mut __S,
    ) -> Result<Self, Self::Error> {
        Ok(stream.dup(|stream| Item::parse_stream(&mut *stream)).ok())
    }
}

impl<const N: usize, Atom: crate::span::Spanned, T> Parse<Atom> for [T; N]
where
    T: Parse<Atom>,
{
    type Error = T::Error;
    fn parse_stream<__S: crate::parse::parse_stream::ParseStream<Atom = Atom>>(
        stream: &mut __S,
    ) -> Result<Self, Self::Error> {
        let mut v = Vec::new();
        for _ in 0..N {
            v.push(T::parse_stream(&mut *stream)?);
        }
        Ok(v.try_into().unwrap_or_else(|_| panic!()))
    }
}

impl<Atom: crate::span::Spanned, T, E> Parse<Atom> for Result<T, E>
where
    T: Parse<Atom, Error = E>,
{
    type Error = core::convert::Infallible;
    fn parse_stream<__S: crate::parse::parse_stream::ParseStream<Atom = Atom>>(
        stream: &mut __S,
    ) -> Result<Self, Self::Error> {
        Ok(T::parse_stream(&mut *stream))
    }
}

impl<Atom: crate::span::Spanned, T> Parse<Atom> for core::marker::PhantomData<T> {
    type Error = core::convert::Infallible;
    fn parse_stream<__S: crate::parse::parse_stream::ParseStream<Atom = Atom>>(
        _stream: &mut __S,
    ) -> Result<Self, Self::Error> {
        Ok(Default::default())
    }
}

impl<Atom: crate::span::Spanned> Parse<Atom> for core::convert::Infallible {
    type Error = crate::error::ParseError<crate::span::SpanOf<Atom>>;
    fn parse_stream<__S: crate::parse::parse_stream::ParseStream<Atom = Atom>>(
        _stream: &mut __S,
    ) -> Result<Self, Self::Error> {
        panic!()
    }
}

use crate::error::ParseError;
use crate::parse::{IntoParseStream, Parse, ParseStream};
use crate::span::Spanned;
use crate::tuple::PopHeadRef;
use newer_type::{implement, traits};

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
#[implement(traits::Debug)]
pub struct Joint<Tuple>(pub Tuple);

impl<Tuple: Default> Default for Joint<Tuple> {
    fn default() -> Self {
        Joint(Tuple::default())
    }
}

impl<Tuple> core::ops::Deref for Joint<Tuple> {
    type Target = Tuple;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<Tuple> core::ops::DerefMut for Joint<Tuple> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<Tuple> core::convert::From<Tuple> for Joint<Tuple> {
    fn from(value: Tuple) -> Self {
        Joint(value)
    }
}

impl<Atom: Spanned, Tuple, Head, Rem> Parse<Atom> for Joint<Tuple>
where
    Tuple: crate::tuple::PopHead<Head = Head, Rem = Rem>,
    Joint<Rem>: Parse<Atom, Error = ParseError>,
    Rem: crate::tuple::PopHead,
    Head: Parse<Atom>,
    Head::Error: Into<ParseError>,
{
    type Error = ParseError;

    fn parse(stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
        let mut stream = stream.into_parse_stream();
        let head = Head::parse(&mut stream).map_err(Into::into)?;
        if stream.skip_sep() {
            // TODO:
            panic!();
        }
        let rem = <Joint<Rem>>::parse(&mut stream)?.0;
        Ok(Joint(Tuple::unsplit(head, rem)))
    }
}

impl<Atom: Spanned, T> Parse<Atom> for Joint<(T,)>
where
    T: Parse<Atom>,
    T::Error: Into<ParseError>,
{
    type Error = ParseError;
    fn parse(stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
        Ok(Joint((T::parse(stream).map_err(Into::into)?,)))
    }
}

impl<Tuple> core::fmt::Display for Joint<Tuple>
where
    Tuple: crate::tuple::AsRef,
    for<'a> <Tuple as crate::tuple::AsRef>::AsRef<'a>: DisplayImpl,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.as_ref().fmt(f)
    }
}

trait DisplayImpl {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result;
}

impl<Tuple, Head, Rem> DisplayImpl for Tuple
where
    Tuple: PopHeadRef<Head = Head, Rem = Rem>,
    Head: core::fmt::Display,
    Rem: DisplayImpl,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let (head, rem) = self.pop_head_ref();
        head.fmt(f)?;
        rem.fmt(f)
    }
}

impl DisplayImpl for () {
    fn fmt(&self, _: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Ok(())
    }
}

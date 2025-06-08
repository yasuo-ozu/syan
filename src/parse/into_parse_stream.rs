use super::ParseStream;

pub trait IntoParseStream: Sized {
    type Atom;
    type Error;
    type Output: ParseStream<Atom = Self::Atom, Error = Self::Error>;

    fn into_parse_stream(self) -> Self::Output;
}

impl<T> IntoParseStream for T
where
    T: ParseStream,
{
    type Atom = T::Atom;
    type Output = T;
    type Error = T::Error;

    fn into_parse_stream(self) -> Self::Output {
        self
    }
}

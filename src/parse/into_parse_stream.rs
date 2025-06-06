use super::ParseStream;

pub trait IntoParseStream: Sized {
    type Atom;
    type Output: ParseStream<Atom = Self::Atom>;

    fn into_parse_stream(self) -> Self::Output;
}

impl<T> IntoParseStream for T
where
    T: ParseStream,
{
    type Atom = T::Atom;
    type Output = T;

    fn into_parse_stream(self) -> Self::Output {
        self
    }
}

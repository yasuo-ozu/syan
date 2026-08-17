//! Turning a source into a [`ParseStream`].

use super::ParseStream;

/// Anything a parse can start from: a source such as `String`, or a [`ParseStream`] itself.
///
/// This is what [`Parse::parse`](crate::parse::Parse::parse) takes, so a caller never has to build
/// a stream by hand.
pub trait IntoParseStream: Sized {
    /// The unit of input the resulting stream serves.
    type Atom;
    /// The stream this becomes.
    type Output: ParseStream<Atom = Self::Atom>;

    /// Build the stream.
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

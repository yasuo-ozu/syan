use crate::error::ParseError;
use crate::parse::unparse::Emitter;
use crate::parse::{Parse, ParseStream, Unparse};
use crate::span::{SpanOf, Spanned};

/// Matches only where the input has run out.
///
/// Put it last in a grammar — or use [`Parse::parse_all`] — so that a top-level `Vec<T>` or
/// `Option<T>`, which stops at the first atom it cannot read and still reports success, cannot
/// silently accept a prefix of the input. Separators before the end are skipped, so trailing
/// whitespace still counts as the end.
///
/// It works over any atom family: `char`, `u8` and `TokenTree` alike. [`Unparse`] writes nothing.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Eof;

impl<Atom: Spanned> Parse<Atom> for Eof {
    type Error = ParseError<SpanOf<Atom>>;

    fn parse_stream<__S: ParseStream<Atom = Atom>>(stream: &mut __S) -> Result<Self, Self::Error> {
        stream.skip_sep();
        match stream.peek() {
            None => Ok(Eof),
            Some(atom) => Err(ParseError::expected(atom.span(), "end of input")),
        }
    }
}

impl<Atom> Unparse<Atom> for Eof {
    fn unparse<S: Emitter<Atom>>(&self, _: &mut S) -> Result<(), S::Error> {
        Ok(())
    }
}

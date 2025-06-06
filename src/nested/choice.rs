use crate::parse::{IntoParseStream, Parse, ParseStream};

mod _choice_impl {
    #[macro_export]
    macro_rules! _choice_impl {
        ($t:ty$(,)?) => {
            $t
        };
        ($t0:ty, $t1:ty $(,$t:ty)*$(,)?) => {
            $crate::nested::choice::Choice<
                $t0,
                $crate::nested::choice::Choice!($t1 $(,$t)*)
            >
        };
    }

    pub use _choice_impl as Choice;
}
pub use _choice_impl::*;

// TODO: implement deref

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Choice<L, R> {
    Left(L),
    Right(R),
}

impl<L: Default, R> Default for Choice<L, R> {
    fn default() -> Self {
        Self::Left(Default::default())
    }
}

impl<L: std::fmt::Display, R: std::fmt::Display> core::fmt::Display for Choice<L, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Left(left) => left.fmt(f),
            Self::Right(right) => right.fmt(f),
        }
    }
}

impl<L, R, Atom, Error, LError> Parse<Atom> for Choice<L, R>
where
    L: Parse<Atom, Error = LError>,
    R: Parse<Atom>,
    LError: crate::error::Merge<R::Error, Output = Error>,
    Atom: Clone,
{
    type Error = Error;

    fn parse(stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Error> {
        let mut stream = stream.into_parse_stream();
        match stream.dup(|mut stream| L::parse(&mut stream).map_err(LError::from_left)) {
            Ok(o) => Ok(Choice::Left(o)),
            Err(_e) => Ok(Choice::Right(
                R::parse(&mut stream).map_err(LError::from_right)?,
            )),
        }
    }
}

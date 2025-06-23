use crate::parse::{IntoParseStream, Parse, ParseStream};
pub struct Joint<Tuple>(pub Tuple);

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

impl<Atom, Tuple, Head, Rem, Error, HeadError, RemError> Parse<Atom> for Joint<Tuple>
where
    Tuple: crate::tuple::PopHead<Head = Head, Rem = Rem>,
    Joint<Rem>: Parse<Atom, Error = RemError>,
    Rem: crate::tuple::PopHead,
    Head: Parse<Atom, Error = HeadError>,
    HeadError: crate::error::Merge<RemError, Output = Error>,
{
    type Error = Error;
    fn parse(stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
        let mut stream = stream.into_parse_stream();
        let head = Head::parse(&mut stream).map_err(HeadError::from_left)?;
        if stream.skip_sep() {
            // TODO:
            panic!();
        }
        let rem = <Joint<Rem>>::parse(&mut stream)
            .map_err(HeadError::from_right)?
            .0;
        Ok(Joint(Tuple::unsplit(head, rem)))
    }
}

impl<Atom, T> Parse<Atom> for Joint<(T,)>
where
    T: Parse<Atom>,
{
    type Error = T::Error;
    fn parse(stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
        Ok(Joint((T::parse(stream)?,)))
    }
}

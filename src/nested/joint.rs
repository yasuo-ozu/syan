use crate::parse::{IntoParseStream, Parse, ParseStream};
pub struct Joint<Tuple>(pub Tuple);

mod _joint_impl {
    #[macro_export]
    #[doc(hidden)]
    macro_rules! _joint_impl {
        ($($t:ty),* $(,)?) => {
            $xrate::nested::Joint<($($t,)*)>
        };
    }
    pub use _joint_impl as Joint;
}
#[doc(inline)]
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

impl<Atom, Tuple, Head, Rem> Parse<Atom> for Joint<Tuple>
where
    Tuple: crate::tuple::PopHead<Head = Head, Rem = Rem>,
    Joint<Rem>: Parse<Atom, Error = ()>,
    Rem: crate::tuple::PopHead,
    Head: Parse<Atom, Error = ()>,
{
    type Error = ();
    fn parse(stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
        let mut stream = stream.into_parse_stream();
        let head = Head::parse(&mut stream)?;
        if stream.skip_sep() {
            // TODO:
            panic!();
        }
        let rem = <Joint<Rem>>::parse(&mut stream)?.0;
        Ok(Joint(Tuple::unsplit(head, rem)))
    }
}

impl<Atom, T> Parse<Atom> for Joint<(T,)>
where
    T: Parse<Atom, Error = ()>,
{
    type Error = ();
    fn parse(stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
        Ok(Joint((T::parse(stream)?,)))
    }
}

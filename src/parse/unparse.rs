pub trait Unparse<Atom> {
    fn unparse<S: Emitter<Atom>>(&self, sink: &mut S) -> Result<(), S::Error>;
}

pub trait Emitter<Atom> {
    type Error;
    fn write_one(&mut self, atom: Atom) -> Result<(), Self::Error>;
    fn write_sep(&mut self) -> Result<(), Self::Error>;
}

impl<Atom, T> Emitter<Atom> for &'_ mut T
where
    T: core::iter::Extend<Atom>,
{
    type Error = core::convert::Infallible;
    fn write_one(&mut self, atom: Atom) -> Result<(), Self::Error> {
        Ok(self.extend(core::iter::once(atom)))
    }

    fn write_sep(&mut self) -> Result<(), Self::Error> {
        // do nothing
        Ok(())
    }
}

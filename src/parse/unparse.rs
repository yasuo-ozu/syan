pub trait Unparse<Atom> {
    fn unparse<S: Emitter<Atom>>(&self, sink: S) -> Result<(), S::Error>;

    fn unparse_vec(&self) -> Vec<Atom> {
        let mut v = Vec::new();
        let Ok(()) = self.unparse(&mut v);
        v
    }
}

pub trait Emitter<Atom> {
    type Error;
    fn write_one(self, atom: Atom) -> Result<(), Self::Error>;
}

impl<Atom, T> Emitter<Atom> for &'_ mut T
where
    T: core::iter::Extend<Atom>,
{
    type Error = core::convert::Infallible;
    fn write_one(self, atom: Atom) -> Result<(), Self::Error> {
        Ok(self.extend(core::iter::once(atom)))
    }
}

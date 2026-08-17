//! Writing a value back out as atoms: the [`Unparse`] trait and the [`Emitter`] it writes into.

pub use syan_macro::Unparse;

// Defined (and `#[decycle]`-annotated) in `crate::decycle_traits` — see that module's docs.
pub use crate::decycle_traits::Unparse;

impl<Atom, T> Unparse<Atom> for &'_ T
where
    T: Unparse<Atom>,
{
    fn unparse<S: Emitter<Atom>>(&self, sink: &mut S) -> Result<(), S::Error> {
        (*self).unparse(sink)
    }
}

impl<Atom, T> Unparse<Atom> for &'_ mut T
where
    T: Unparse<Atom>,
{
    fn unparse<S: Emitter<Atom>>(&self, sink: &mut S) -> Result<(), S::Error> {
        (**self).unparse(sink)
    }
}

/// The sink [`Unparse`] writes atoms into, such as a `Vec<Atom>` or a token-stream builder.
///
/// Any `&mut T` where `T: Extend<Atom>` is one already.
pub trait Emitter<Atom> {
    /// What writing can fail with; `Infallible` for a sink that cannot fail.
    type Error;
    /// Write one atom.
    fn write_one(&mut self, atom: Atom) -> Result<(), Self::Error>;
    /// Write whatever separates two atoms, for a sink that needs one (whitespace, say).
    fn write_sep(&mut self) -> Result<(), Self::Error>;
}

impl<Atom, T> Emitter<Atom> for &'_ mut T
where
    T: core::iter::Extend<Atom>,
{
    type Error = core::convert::Infallible;
    fn write_one(&mut self, atom: Atom) -> Result<(), Self::Error> {
        self.extend(core::iter::once(atom));
        Ok(())
    }

    fn write_sep(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl<Atom, T: Unparse<Atom>> Unparse<Atom> for Box<T> {
    fn unparse<E: Emitter<Atom>>(&self, emitter: &mut E) -> Result<(), E::Error> {
        self.as_ref().unparse(emitter)
    }
}

impl<Atom, T: Unparse<Atom>> Unparse<Atom> for Option<T> {
    fn unparse<E: Emitter<Atom>>(&self, emitter: &mut E) -> Result<(), E::Error> {
        match self {
            Some(value) => value.unparse(emitter),
            None => Ok(()),
        }
    }
}

impl<const N: usize, Atom, T: Unparse<Atom>> Unparse<Atom> for [T; N] {
    fn unparse<E: Emitter<Atom>>(&self, emitter: &mut E) -> Result<(), E::Error> {
        for item in self {
            item.unparse(emitter)?;
        }
        Ok(())
    }
}

impl<Atom> Unparse<Atom> for core::convert::Infallible {
    fn unparse<E: Emitter<Atom>>(&self, _emitter: &mut E) -> Result<(), E::Error> {
        match *self {}
    }
}

impl<Atom, T> Unparse<Atom> for core::marker::PhantomData<T> {
    fn unparse<E: Emitter<Atom>>(&self, _emitter: &mut E) -> Result<(), E::Error> {
        Ok(())
    }
}

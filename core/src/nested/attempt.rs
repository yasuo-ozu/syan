use crate::parse::unparse::Emitter;
use crate::parse::{Parse, Unparse};
use crate::span::Spanned;
use core::ops::{Deref, DerefMut};

/// The **atomic-parse** wrapper (build one with `value.attempt()` — [`Parse::attempt`]): its [`Parse`]
/// parses `T` but on failure **rewinds** the stream to where the attempt began (the partial consumption is
/// undone) before the error propagates. Unlike `Option<T>` (which swallows the error into `None`), the
/// error still surfaces — `Attempt` only guarantees the stream isn't left half-consumed, so a later
/// alternative can retry from the same position. Use it as a derived AST field type for a field whose
/// failure should not corrupt the position.
///
/// Requires `Atom: Clone` (the rewind duplicates the stream). `Attempt` `Deref`s to `T` and forwards
/// [`Unparse`]/[`Spanned`], and a `visitor!()` walks straight through it to `T`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Attempt<T>(pub T);

impl<T> Deref for Attempt<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> DerefMut for Attempt<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T> From<T> for Attempt<T> {
    fn from(t: T) -> Self {
        Attempt(t)
    }
}

impl<Atom: Clone, T: Parse<Atom>> Parse<Atom> for Attempt<T> {
    type Error = T::Error;
    fn parse_stream<__S: crate::parse::parse_stream::ParseStream<Atom = Atom>>(stream: &mut __S) -> Result<Self, Self::Error> {
        // `dup` commits the consumed tokens on `Ok` and rewinds on `Err`, so the parse is all-or-nothing.
        stream.dup(|s| T::parse_stream(&mut *s)).map(Attempt)
    }
}

impl<Atom, T: Unparse<Atom>> Unparse<Atom> for Attempt<T> {
    fn unparse<S: Emitter<Atom>>(&self, sink: &mut S) -> Result<(), S::Error> {
        self.0.unparse(sink)
    }
}

impl<T: Spanned> Spanned for Attempt<T> {
    type Span = T::Span;
    fn span(&self) -> Self::Span {
        self.0.span()
    }
}

use crate::span::{Span, Spanned};

/// The core token stream. **Object-safe** — so `&mut dyn ParseStream<Atom = A, Error = E>` is a usable
/// type (needed to type-erase the stream at the unbounded-`#[recurse]` re-entry boundary). The generic
/// conveniences `dup`/`validate_spacing` carry `where Self: Sized`, which keeps them off the vtable (not
/// callable on `dyn ParseStream`) while preserving object safety — and they're callable on every real
/// (sized) stream (`Stream`, `Dup<…>`, `&mut T`).
pub trait ParseStream {
    type Atom;
    type Error;

    // Required
    fn next(&mut self) -> Option<Self::Atom>;
    fn peek(&mut self) -> Option<&Self::Atom>;
    fn push(&mut self, _: Self::Atom);

    fn get_error(&mut self) -> Result<(), Self::Error> {
        todo!()
    }

    /// Skip the separator atoms if exists. Returns whether we skipped some separators.
    ///
    /// This function may or may not returns `true` with multiple calling.
    /// If the input streams fall into an error, it returns `true`.
    fn skip_sep(&mut self) -> bool {
        todo!()
    }

    fn validate_spacing<S: Span + 'static>(
        &mut self,
        is_joint: bool,
    ) -> Result<(), crate::error::ParseError>
    where
        Self: Sized,
        Self::Atom: Spanned<Span = S>,
    {
        let first_peek = self.peek().map(|a| a.span()).unwrap_or_default();
        if self.skip_sep() == is_joint {
            let last_peek = self.peek().map(|a| a.span()).unwrap_or_default();
            let span = first_peek.migrate(last_peek);
            if is_joint {
                Err(crate::error::ParseError::new(span, "not joint"))
            } else {
                Err(crate::error::ParseError::new(span, "not alone"))
            }
        } else {
            Ok(())
        }
    }

    /// Run sub parser with a duplicated stream.
    /// If the given closure returns Error, then the duplicated stream is discarded and the
    /// position is not advanced in the original stream.
    /// If it returns Ok, then it replaces the duplicated stream is replaced with original one, and
    /// the original is discarded.
    fn dup<'a, T, E, F: FnOnce(&mut Dup<&'a mut Self, Self::Atom>) -> std::result::Result<T, E>>(
        &'a mut self,
        f: F,
    ) -> std::result::Result<T, E>
    where
        Self: Sized,
        Self::Atom: Clone,
    {
        let mut dup = Dup {
            slot: self,
            take_buf: Vec::new(),
            push_buf: Vec::new(),
        };
        let result = f(&mut dup);
        let Dup {
            slot,
            mut take_buf,
            mut push_buf,
        } = dup;
        match result {
            Ok(ok) => {
                while let Some(item) = push_buf.pop() {
                    slot.push(item);
                }
                Ok(ok)
            }
            Err(err) => {
                while let Some(item) = take_buf.pop() {
                    slot.push(item);
                }
                Err(err)
            }
        }
    }
}

pub struct Dup<Slot, Atom> {
    slot: Slot,
    take_buf: Vec<Atom>,
    push_buf: Vec<Atom>,
}

impl<S, A> ParseStream for Dup<S, A>
where
    S: ParseStream<Atom = A>,
    A: Clone,
{
    type Atom = A;
    type Error = S::Error;
    fn next(&mut self) -> Option<Self::Atom> {
        if let Some(item) = self.push_buf.pop() {
            Some(item)
        } else {
            let item = self.slot.next()?;
            self.take_buf.push(item.clone());
            Some(item)
        }
    }

    fn peek(&mut self) -> Option<&Self::Atom> {
        if let Some(last) = self.push_buf.last() {
            Some(last)
        } else {
            self.slot.peek()
        }
    }

    fn push(&mut self, token: Self::Atom) {
        self.push_buf.push(token);
    }
}

impl<T: ?Sized> ParseStream for &'_ mut T
where
    T: ParseStream,
{
    type Atom = T::Atom;
    type Error = T::Error;

    fn next(&mut self) -> Option<Self::Atom> {
        T::next(self)
    }

    fn peek(&mut self) -> Option<&Self::Atom> {
        T::peek(self)
    }

    fn push(&mut self, item: Self::Atom) {
        T::push(self, item)
    }
}

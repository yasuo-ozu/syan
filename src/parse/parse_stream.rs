use super::Parse;

pub trait ParseStream {
    type Atom;
    type Sep: Parse<Self::Atom>;

    // Required
    fn next(&mut self) -> Option<Self::Atom>;
    fn peek(&mut self) -> Option<&Self::Atom>;
    fn push(&mut self, _: Self::Atom);

    // Pre-defined
    fn skip_sep(&mut self) -> bool {
        Self::Sep::parse(self).is_ok()
    }

    /// Run sub parser with a duplicated stream.
    /// If the given closure returns Error, then the duplicated stream is discarded and the
    /// position is not advanced in the original stream.
    /// If it returns Ok, then it replaces the duplicated stream is replaced with original one, and
    /// the original is discarded.
    fn dup<'a, T, E, F: FnOnce(&mut Dup<&'a mut Self, Self::Atom>) -> std::result::Result<T, E>>(
        &'a mut self,
        f: F,
    ) -> std::result::Result<T, E> {
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
    type Sep = S::Sep;
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
    type Sep = T::Sep;

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

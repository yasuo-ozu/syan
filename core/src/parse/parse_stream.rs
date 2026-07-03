use crate::span::{Span, Spanned};

/// The core token stream. **Object-safe** — so `&mut dyn ParseStream<Atom = A, Error = E>` is a usable
/// type (needed to type-erase the stream at the unbounded-`#[recurse]` re-entry boundary). The generic
/// conveniences `dup`/`validate_spacing` carry `where Self: Sized`, which keeps them off the vtable (not
/// callable on `dyn ParseStream`) while preserving object safety — and they're callable on every real
/// (sized) stream (`Stream`, `Dup<…>`, `&mut T`).
///
/// # Backtracking and stateful producers
///
/// [`dup`](Self::dup) implements backtracking by *replaying atoms*: on failure it feeds every atom
/// it had pulled via [`next`](Self::next) back through [`push`](Self::push), in reverse order, so
/// the next `next()`/`peek()` sees them again. That rewinds the stream's **atom sequence** only — it
/// does nothing to whatever *produced* those atoms. If a `ParseStream` is a thin cursor over an
/// already-built `Vec<Atom>` (or any producer with no memory of what it already emitted), this is
/// exactly a pointer decrement and `push` is a true inverse of `next`.
///
/// But if the stream is backed by a **stateful producer** — e.g. a lazy lexer that pushes/pops a
/// mode stack as it emits atoms (seeing `<` switches the lexer into "tag" mode before the atom for
/// `<` is even returned to the caller) — pushing an atom back does **not** undo whatever
/// producer-side effect emitting it caused. The pending-atom buffer rewinds; the lexer's mode stack
/// does not. A `dup` that fails after crossing a mode transition leaves the producer permanently
/// desynchronized from the atoms handed back by `push`, and every atom pulled afterward is lexed in
/// the wrong mode — **silently**, with no error at the point of divergence, only a confusing failure
/// (or a wrong-but-successful parse) downstream.
///
/// **The safe pattern is eager tokenization**: run the stateful producer to completion up front,
/// collect every atom into a `Vec<Atom>`, and implement `ParseStream` over that `Vec` (index in and
/// out — no lazy production left to desynchronize). `push` is then always a true inverse of `next`,
/// no matter how much backtracking `dup` does. If a lazy stream is unavoidable, `push` must fully
/// restore *every* piece of producer-observable state that changed while producing the atom being
/// pushed back — not just hand back the atom's value — for `dup` (and hence [`Parse`](crate::parse::Parse)'s
/// ordered-choice backtracking, and [`Attempt`](crate::nested::Attempt)) to be sound.
pub trait ParseStream {
    type Atom;
    type Error;

    // Required
    fn next(&mut self) -> Option<Self::Atom>;
    fn peek(&mut self) -> Option<&Self::Atom>;
    /// Push an atom back onto the stream so the next `next()`/`peek()` sees it again — the primitive
    /// [`dup`](Self::dup) backtracking is built from. See "Backtracking and stateful producers" above:
    /// for a stream backed by a stateful producer, `push` must fully undo any producer-side effect of
    /// having emitted the atom, not just return the atom's value.
    fn push(&mut self, _: Self::Atom);

    fn get_error(&mut self) -> Result<(), Self::Error> {
        todo!()
    }

    /// Skip the separator atoms (if any) at the current position.
    ///
    /// Returns whether **at least one** separator atom was consumed. `skip_sep` is **idempotent at
    /// a fixed stream position**: calling it again right after, with nothing else having advanced
    /// the stream in between, finds nothing left to skip and returns `false` — it does not need to
    /// keep returning `true` just because an earlier call did.
    ///
    /// A stream whose atoms are already separator-free by construction — pre-tokenized, or produced
    /// by a lexer that strips whitespace/comments before emitting atoms — has no separators to skip
    /// at *any* position, so an implementation that always returns `false` is fully conforming.
    /// `false` forever is not a bug to work around; it is a truthful report that this stream never
    /// has separators, and callers (chiefly [`validate_spacing`](Self::validate_spacing)) must treat
    /// it as such.
    ///
    /// If the stream is in an error state, this returns `true` (an error state is indistinguishable
    /// from "some separator was consumed" for the purposes of the one caller that checks this, since
    /// both suppress a spurious "not joint"/"not alone" failure — see `validate_spacing`).
    fn skip_sep(&mut self) -> bool {
        todo!()
    }

    /// Check that the next atom is (`is_joint = true`) or isn't (`is_joint = false`) joint with the
    /// atom just consumed, per [`skip_sep`](Self::skip_sep), erroring otherwise.
    ///
    /// ## Sharp edge: span defaulting
    ///
    /// When the stream is empty at the entry or exit peek, the missing span is defaulted to
    /// `S::default()` before the two peeks are merged via [`migrate`](crate::span::Span::migrate)
    /// into the error's span. For a **positional** span type (line/column/offset — see
    /// [`source::string::Span`](crate::source::string::Span), whose `Default` is `line: 0, col: 0,
    /// loc: 0` even though real positions start at line 1) that default is a real, addressable
    /// location, not a sentinel — so an end-of-stream `validate_spacing` failure reports a
    /// **fabricated position** (line 0) rather than "end of input". Don't rely on the returned
    /// error's span being meaningful when the stream was empty at either peek.
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

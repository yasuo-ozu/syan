//! The [`ParseStream`] trait: the rewindable stream of atoms a parser reads from.

use crate::span::{Span, Spanned};

/// The stream a parser reads from: pull atoms, look one ahead, and rewind.
///
/// Backtracking is expressed by the checkpoint trio ([`checkpoint_raw`](Self::checkpoint_raw) and
/// friends) rather than by a wrapper type, so [`dup`](Self::dup) hands its closure `&mut Self` and
/// nesting transactions never grows the stream type.
///
/// The trait is object-safe, so `&mut dyn ParseStream<Atom = A, Error = E>` is usable; the generic
/// conveniences carry `where Self: Sized` to keep it that way.
pub trait ParseStream {
    /// The unit of input this stream serves.
    type Atom;
    /// What the underlying source reports when it fails; see [`get_error`](Self::get_error).
    type Error;

    /// Consume and return the next atom, or `None` at end of input.
    fn next(&mut self) -> Option<Self::Atom>;
    /// The atom [`next`](Self::next) would return, without consuming it.
    fn peek(&mut self) -> Option<&Self::Atom>;
    /// Hand an atom back; it is served before anything still unread.
    fn push(&mut self, _: Self::Atom);

    /// Open a transaction; the returned token identifies it.
    ///
    /// Every token must be handed to exactly one of [`rollback_raw`](Self::rollback_raw) or
    /// [`commit_raw`](Self::commit_raw), and scopes must nest (LIFO). Prefer [`dup`](Self::dup),
    /// which enforces both; reach for the raw trio only when implementing a stream or a combinator
    /// whose control flow `dup` cannot express.
    ///
    /// The token is **opaque and stream-specific** — it is not a position, and passing one stream's
    /// token to another is meaningless.
    ///
    /// Deliberately required with no default, so that a wrapper type cannot silently inherit a
    /// rewind primitive that does not rewind it.
    fn checkpoint_raw(&mut self) -> u64;

    /// Undo everything consumed since `raw` was taken, and close that scope.
    fn rollback_raw(&mut self, raw: u64);

    /// Keep everything consumed since `raw` was taken, and close that scope.
    fn commit_raw(&mut self, raw: u64);

    /// Report an error the *source* has accumulated (a lexer that failed mid-stream, say), as opposed
    /// to a parse failure. `Ok(())` for a source that cannot fail.
    fn get_error(&mut self) -> Result<(), Self::Error>;

    /// Skip the separator atoms if exists. Returns whether we skipped some separators.
    ///
    /// This function may or may not returns `true` with multiple calling.
    /// If the input streams fall into an error, it returns `true`.
    fn skip_sep(&mut self) -> bool;

    /// Check the spacing before the next atom: `is_joint` demands no separator here, `false` demands
    /// one. Fails with [`ParseError::Spacing`](crate::error::ParseError::Spacing) if the input
    /// disagrees.
    fn validate_spacing<S: Span>(
        &mut self,
        is_joint: bool,
    ) -> Result<(), crate::error::ParseError<S>>
    where
        Self: Sized,
        Self::Atom: Spanned<Span = S>,
    {
        let first_peek = self.peek().map(|a| a.span()).unwrap_or_default();
        if self.skip_sep() == is_joint {
            let last_peek = self.peek().map(|a| a.span()).unwrap_or_default();
            let span = first_peek.migrate(last_peek);
            if is_joint {
                Err(crate::error::ParseError::spacing(span, true))
            } else {
                Err(crate::error::ParseError::spacing(span, false))
            }
        } else {
            Ok(())
        }
    }

    /// Run a sub-parser as a transaction on **this** stream.
    ///
    /// If the closure returns `Err`, everything it consumed is rolled back and the stream is exactly
    /// where it started. If it returns `Ok`, the consumption is kept.
    ///
    /// The closure receives `&mut Self` — the same stream type, not a wrapper — so `dup` scopes can
    /// nest arbitrarily without changing the type a recursive descent instantiates.
    fn dup<T, E, F: FnOnce(&mut Self) -> std::result::Result<T, E>>(
        &mut self,
        f: F,
    ) -> std::result::Result<T, E>
    where
        Self: Sized,
    {
        let raw = self.checkpoint_raw();
        // A panic escaping `f` deliberately leaves the scope open (a bounded leak) rather than
        // rewinding from a drop guard during unwind, which could itself assert and abort.
        match f(self) {
            Ok(ok) => {
                self.commit_raw(raw);
                Ok(ok)
            }
            Err(err) => {
                self.rollback_raw(raw);
                Err(err)
            }
        }
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

    fn checkpoint_raw(&mut self) -> u64 {
        T::checkpoint_raw(self)
    }

    fn rollback_raw(&mut self, raw: u64) {
        T::rollback_raw(self, raw)
    }

    fn commit_raw(&mut self, raw: u64) {
        T::commit_raw(self, raw)
    }

    fn get_error(&mut self) -> std::result::Result<(), Self::Error> {
        T::get_error(self)
    }

    fn skip_sep(&mut self) -> bool {
        T::skip_sep(self)
    }
}

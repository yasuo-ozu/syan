use crate::span::{Span, Spanned};

/// The core token stream. **Object-safe** — so `&mut dyn ParseStream<Atom = A, Error = E>` is a usable
/// type (needed to type-erase the stream at the unbounded-`#[recurse]` re-entry boundary). The generic
/// conveniences `dup`/`validate_spacing` carry `where Self: Sized`, which keeps them off the vtable (not
/// callable on `dyn ParseStream`) while preserving object safety — and they're callable on every real
/// (sized) stream (`Stream`, `&mut T`).
///
/// Backtracking is expressed by the **checkpoint trio** below rather than by a wrapper type. That is
/// what keeps the stream type fixed across a `dup` scope: `dup` hands the closure `&mut Self`, not a
/// `Dup<&mut Self>`, so nesting transactions cannot grow the type. See [`erase`] for the *other*
/// growth source, which this does not address.
pub trait ParseStream {
    type Atom;
    type Error;

    // Required
    fn next(&mut self) -> Option<Self::Atom>;
    fn peek(&mut self) -> Option<&Self::Atom>;
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
    /// The trio is deliberately *required*, with no default: a defaulted rewind primitive would let a
    /// wrapper type silently inherit a broken one. `get_error`/`skip_sep` are required for the same
    /// reason — they used to default to `todo!()`, which turned a forgotten forward into a runtime
    /// panic instead of a compile error.
    fn checkpoint_raw(&mut self) -> u64;

    /// Undo everything consumed since `raw` was taken, and close that scope.
    fn rollback_raw(&mut self, raw: u64);

    /// Keep everything consumed since `raw` was taken, and close that scope.
    fn commit_raw(&mut self, raw: u64);

    /// Report an error the *source* has accumulated (a lexer that failed mid-stream, say), as opposed
    /// to a parse failure. `Ok(())` for a source that cannot fail.
    ///
    /// Required — see the note on [`checkpoint_raw`](Self::checkpoint_raw).
    fn get_error(&mut self) -> Result<(), Self::Error>;

    /// Skip the separator atoms if exists. Returns whether we skipped some separators.
    ///
    /// This function may or may not returns `true` with multiple calling.
    /// If the input streams fall into an error, it returns `true`.
    fn skip_sep(&mut self) -> bool;

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
        // A panic escaping `f` leaves the scope open. That is deliberate: the alternative is a drop
        // guard that rewinds during unwind, which turns "this parse panicked" into "this parse
        // panicked and then quietly rewound", and can abort the process if the rewind itself
        // asserts. An abandoned scope is only a bounded leak in `saves`.
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

/// Type-erase a concrete stream to **one fixed `&mut dyn ParseStream` layer**.
///
/// `#[recurse]` wraps every field-parse call of a cycle member's derived `Parse` in this. The reason is
/// that [`Parse::parse`](crate::parse::Parse::parse) takes `impl IntoParseStream` — a generic parameter,
/// which *moves* rather than reborrows — so a recursive descent that passes `&mut local` at each level
/// asks for `Expr::parse::<&mut &mut …>`: an infinite monomorphization chain (E0275) that no
/// trait-obligation engine can break. Erasing at the call site pins the callee's stream type to
/// `&mut dyn ParseStream<…>`, and erasing *that* yields the same type again — a fixed point, so the
/// instantiation set is finite while recursion depth stays bounded only by the call stack.
///
/// `erase` is also a *depth normaliser*: whatever `&mut` tower the caller happens to hold, the callee
/// sees exactly one layer.
///
/// Backtracking used to be a second, independent growth source — `dup` wrapped the stream in a `Dup<…>`
/// that was itself a `ParseStream`. The checkpoint trio removed that one; this function addresses only
/// the `Parse::parse`-by-value one, which remains.
///
/// The blanket `impl<T: ?Sized + ParseStream> ParseStream for &mut T` (plus the blanket
/// `T: ParseStream ⇒ T: IntoParseStream`) is what makes the erased stream usable as a parser input.
pub fn erase<'a, Atom, S>(
    stream: &'a mut S,
) -> &'a mut (dyn ParseStream<Atom = Atom, Error = S::Error> + 'a)
where
    S: ParseStream<Atom = Atom>,
{
    stream
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

//! Rewind machinery for a stream built on a one-shot iterator.
//!
//! Every [`ParseStream`](super::ParseStream) must supply the checkpoint trio
//! (`checkpoint_raw`/`rollback_raw`/`commit_raw`), because that is what
//! [`ParseStream::dup`](super::ParseStream::dup) is built from. An iterator cannot rewind, so
//! something has to remember what it produced. [`Tape`] is that something, and it is what both of
//! syan's built-in sources delegate to.
//!
//! The state is an iterator plus three buffers:
//!
//! * `buf` — atoms already pulled from the iterator. `buf[pos..]` are pulled-but-not-yet-served
//!   (lookahead from `peek`, or re-serves after a rollback); `buf[..pos]` are served atoms kept
//!   *only* so an open transaction can rewind onto them.
//! * `extra` — atoms handed back by [`push`](Tape::push), served LIFO before `buf`/the iterator. The
//!   leaf idiom is next-then-push-back-on-mismatch, so this is almost always empty or a singleton.
//! * `saves` — one `(pos, extra)` snapshot per open checkpoint.
//!
//! **Retention is scoped to open transactions.** With no checkpoint open, a served atom is dropped
//! immediately and `buf` is emptied — nothing can rewind onto it, so nothing needs to hold it. Only
//! once `checkpoint` is called does the tape start accumulating, and the moment the outermost scope
//! resolves the accumulated prefix is released. So a parse that never backtracks holds O(1) atoms,
//! and one that does holds exactly the span of its outermost open transaction.
//!
//! A checkpoint costs one `Vec` push of a `usize` plus a clone of `extra`, and a rollback costs one
//! pop — no replay, and nothing proportional to how much the transaction consumed. That is the point
//! of the design: the previous `Dup` wrapper cloned *every consumed atom* into a replay buffer and
//! pushed them back one at a time on failure, which is both slower and where the pushback-ordering
//! bug lived. Here a failed transaction cannot reorder anything, because it does not move atoms at
//! all — it restores an index.

/// Makes a one-shot iterator rewindable, with O(1) checkpoints.
///
/// This is a *helper*, not a `ParseStream`: it deliberately has no `Error` type and no `skip_sep`
/// policy, so a source can embed it and keep whatever error and separator semantics it wants. See
/// [`crate::source::string::Stream`] for the shape.
pub struct Tape<I: Iterator> {
    iter: I,
    /// Atoms pulled from `iter`. `buf[pos..]` are still to be served; `buf[..pos]` are retained only
    /// for rollback, and are dropped as soon as `saves` empties.
    buf: Vec<I::Item>,
    pos: usize,
    /// Pushed-back atoms, served LIFO before `buf` and the iterator.
    extra: Vec<I::Item>,
    /// `(pos, extra)` at each open checkpoint. A raw token is an index into this.
    saves: Vec<(usize, Vec<I::Item>)>,
    /// `iter` has returned `None`. Kept because `Iterator` does not promise to stay exhausted, and a
    /// rollback makes it entirely normal to reach the end more than once.
    done: bool,
}

impl<I: Iterator> Tape<I> {
    pub fn new(iter: I) -> Self {
        Self {
            iter,
            buf: Vec::new(),
            pos: 0,
            extra: Vec::new(),
            saves: Vec::new(),
            done: false,
        }
    }

    /// The atom `next` will return, without consuming it.
    ///
    /// Takes `&mut self` because it may have to pull from the iterator to answer — the pulled atom
    /// is parked in `buf` and served by the following `next`.
    pub fn peek(&mut self) -> Option<&I::Item> {
        if let Some(atom) = self.extra.last() {
            return Some(atom);
        }
        if self.pos == self.buf.len() && !self.done {
            match self.iter.next() {
                Some(atom) => self.buf.push(atom),
                None => self.done = true,
            }
        }
        self.buf.get(self.pos)
    }

    /// Hand an atom back to the stream. It is served before anything still unread.
    ///
    /// This accepts an *arbitrary* atom, not just the one last read.
    pub fn push(&mut self, atom: I::Item) {
        self.extra.push(atom);
    }

    /// Number of checkpoints currently open.
    pub fn depth(&self) -> usize {
        self.saves.len()
    }

    /// How many atoms are currently held in memory — lookahead, pushbacks, and whatever the open
    /// transactions could rewind onto. Zero-ish between transactions; useful in tests to pin that
    /// retention is actually released.
    pub fn retained(&self) -> usize {
        self.buf.len() + self.extra.len()
    }

    /// Drop the served prefix once nothing can rewind onto it.
    fn release(&mut self) {
        if self.saves.is_empty() && self.pos > 0 {
            self.buf.drain(..self.pos);
            self.pos = 0;
        }
    }
}

impl<I: Iterator> Tape<I>
where
    I::Item: Clone,
{
    // Named to mirror `ParseStream::next`, which is what every caller is implementing. Making this an
    // `Iterator` instead would be worse than the name clash: a type that is both `Iterator` and
    // `ParseStream` makes every bare `s.next()` ambiguous (E0034) at each call site.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<I::Item> {
        if let Some(atom) = self.extra.pop() {
            return Some(atom);
        }
        if self.pos < self.buf.len() {
            let atom = self.buf[self.pos].clone();
            self.pos += 1;
            self.release();
            return Some(atom);
        }
        self.release();
        if self.done {
            return None;
        }
        match self.iter.next() {
            None => {
                self.done = true;
                None
            }
            // Retain it only if some open transaction could rewind onto it. This arm is a
            // PERFORMANCE path, not a correctness one — `release` would drop the atom on the next
            // call anyway; skipping the clone-push-drain round trip per atom is the point. (Measured
            // by mutation: removing either this arm or `release` alone keeps retention bounded;
            // removing both makes `reading_outside_a_transaction_retains_nothing` fail.)
            Some(atom) if self.saves.is_empty() => Some(atom),
            Some(atom) => {
                self.buf.push(atom.clone());
                self.pos += 1;
                Some(atom)
            }
        }
    }

    /// Open a transaction. The returned token is the caller's to pass to exactly one of
    /// [`rollback`](Self::rollback) or [`commit`](Self::commit), and scopes must nest (LIFO).
    pub fn checkpoint(&mut self) -> u64 {
        self.saves.push((self.pos, self.extra.clone()));
        (self.saves.len() - 1) as u64
    }

    /// Undo everything read or pushed since `raw` was taken, and close that scope.
    pub fn rollback(&mut self, raw: u64) {
        let idx = raw as usize;
        debug_assert!(
            idx < self.saves.len(),
            "rollback of an already-resolved checkpoint (or non-LIFO nesting)"
        );
        // Truncating to `idx + 1` first discards any inner scope the caller leaked, so a leak
        // degrades to "the inner scope was rolled back too" rather than corrupting `saves`.
        self.saves.truncate(idx + 1);
        if let Some((pos, extra)) = self.saves.pop() {
            self.pos = pos;
            self.extra = extra;
        }
        self.release();
    }

    /// Keep everything read since `raw` was taken, and close that scope.
    pub fn commit(&mut self, raw: u64) {
        let idx = raw as usize;
        debug_assert!(
            idx < self.saves.len(),
            "commit of an already-resolved checkpoint (or non-LIFO nesting)"
        );
        self.saves.truncate(idx);
        self.release();
    }
}

#[cfg(test)]
mod tests {
    use super::Tape;
    use std::cell::Cell;
    use std::rc::Rc;

    /// Counts how many atoms have actually been pulled, so laziness is observable.
    fn counted(n: u32) -> (Tape<impl Iterator<Item = u32>>, Rc<Cell<usize>>) {
        let pulls = Rc::new(Cell::new(0));
        let seen = Rc::clone(&pulls);
        let iter = (0..n).inspect(move |_| seen.set(seen.get() + 1));
        (Tape::new(iter), pulls)
    }

    #[test]
    fn pulls_only_what_is_asked_for() {
        let (mut t, pulls) = counted(1000);
        assert_eq!(pulls.get(), 0, "construction must not touch the iterator");
        t.next();
        t.next();
        assert_eq!(pulls.get(), 2);
        t.peek();
        assert_eq!(pulls.get(), 3, "peek pulls exactly one lookahead atom");
        t.peek();
        assert_eq!(pulls.get(), 3, "a second peek reuses it");
    }

    /// The reason this is a `Tape` over an iterator rather than a `Vec`: reading without an open
    /// transaction must not accumulate. Holding the input was previously unavoidable.
    #[test]
    fn reading_outside_a_transaction_retains_nothing() {
        let (mut t, _) = counted(1000);
        for _ in 0..1000 {
            t.next();
        }
        assert_eq!(t.next(), None);
        assert!(
            t.retained() <= 1,
            "retained {} atoms with no checkpoint open",
            t.retained()
        );
    }

    #[test]
    fn retention_is_scoped_to_the_outermost_open_transaction() {
        let (mut t, _) = counted(1000);
        let outer = t.checkpoint();
        for _ in 0..100 {
            t.next();
        }
        assert!(
            t.retained() >= 100,
            "an open scope must retain what it could rewind onto"
        );
        t.commit(outer);
        assert_eq!(t.depth(), 0);
        assert!(
            t.retained() <= 1,
            "resolving the outermost scope must release the retained prefix, held {}",
            t.retained()
        );
    }

    #[test]
    fn rollback_re_serves_atoms_the_iterator_can_no_longer_produce() {
        let (mut t, pulls) = counted(10);
        let raw = t.checkpoint();
        let first: Vec<u32> = (0..4).filter_map(|_| t.next()).collect();
        assert_eq!(first, vec![0, 1, 2, 3]);
        assert_eq!(pulls.get(), 4);
        t.rollback(raw);
        let again: Vec<u32> = (0..4).filter_map(|_| t.next()).collect();
        assert_eq!(again, first, "the rewound atoms must come back in order");
        assert_eq!(
            pulls.get(),
            4,
            "and must NOT be pulled from the iterator twice"
        );
    }

    #[test]
    fn nested_rollback_keeps_the_outer_scope_rewindable() {
        let (mut t, _) = counted(10);
        let outer = t.checkpoint();
        t.next(); // 0
        let inner = t.checkpoint();
        t.next(); // 1
        t.next(); // 2
        t.rollback(inner);
        assert_eq!(t.next(), Some(1), "inner rollback returns to just after 0");
        t.rollback(outer);
        assert_eq!(t.next(), Some(0), "outer rollback returns to the start");
    }

    #[test]
    fn pushback_survives_rollback_and_commit() {
        let (mut t, _) = counted(10);
        let raw = t.checkpoint();
        let a = t.next().unwrap();
        t.push(a); // leaf idiom: un-consume
        assert_eq!(t.peek(), Some(&0));
        t.rollback(raw);
        assert_eq!(
            t.next(),
            Some(0),
            "the pushback must not duplicate the atom"
        );
        assert_eq!(t.next(), Some(1));
    }

    /// `Iterator` does not promise to keep returning `None`, and a rollback makes reaching the end
    /// more than once entirely ordinary.
    #[test]
    fn the_end_is_reached_at_most_once() {
        struct Restarting(u32);
        impl Iterator for Restarting {
            type Item = u32;
            fn next(&mut self) -> Option<u32> {
                // Yields 0, then None, then 99 forever — a badly behaved but legal iterator.
                self.0 += 1;
                match self.0 {
                    1 => Some(0),
                    2 => None,
                    _ => Some(99),
                }
            }
        }
        let mut t = Tape::new(Restarting(0));
        assert_eq!(t.next(), Some(0));
        assert_eq!(t.next(), None);
        assert_eq!(t.next(), None, "the tape must stay exhausted");
    }
}

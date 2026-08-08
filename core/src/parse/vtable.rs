//! Runtime fn-pointer registry backing **unbounded `#[recurse]` `Parse`/`Unparse`/`Spanned`**.
//!
//! A `#[recurse]` cycle's engine-delegated traits go through an internal fixed-depth *engine*; at the
//! bottom of the depth chain sits a *terminator* whose trait impl, instead of erroring/panicking,
//! **re-enters** the top-level natural impl at runtime through a type-erased fn pointer stored here. That
//! cuts the E0275 type-recursion cycles (the per-field `where`-bound cycle, the `Dup<…>` stream growth for
//! `Parse`, and the group `Fill` HRTB cycle for group-ful `Unparse`/`Spanned`) at a runtime boundary, so
//! the only depth ceiling becomes the OS call stack — like any recursive-descent parser. See
//! `docs/recurse-unbounded-plan.md`.
//!
//! The natural type's delegated impl **registers** its own erased top-level fn here (keyed per
//! root/terminator type + atom + error) before descending; the terminator **looks it up** and calls it.
//! `Parse` erases the stream to `&mut dyn ParseStream`; group-ful `Unparse` erases the sink to
//! `&mut dyn Emitter` (re-wrapped by [`DynSink`]); group-ful `Spanned` needs no erasure ([`SpanReentry`]
//! keys it). The group-ful U/S engine is a *depth-1 borrow* engine, so the terminator borrows the natural
//! remainder (`&'a Root`) rather than cloning it — no `Clone` requirement.
//!
//! ## Soundness notes
//! - **Key.** Keyed on `core::any::type_name::<K>()`'s *string content* — robustly per-type and
//!   independent of pointer interning (cf. ICF/`-Zshare-generics`, which could fold equal-bodied fns). `K`
//!   is the [`ReKey`] marker carrying `(terminator, atom, stream-error)`, so the stored `usize`'s concrete
//!   fn type is unambiguous at every lookup. The terminator component is **nonce-stamped per
//!   `#[recurse]` expansion** (a fresh, uniquely-named type per compilation — `macro/recurse/names.rs`)
//!   rather than the bare natural root type, so keys are unique per compilation: even two independently
//!   linked versions of one AST crate, whose root type's `type_name` would otherwise be byte-identical,
//!   cannot collide.
//! - **Value.** The fn pointer is stored as `usize` and **copied out** on lookup, so a `HashMap` rehash
//!   never dangles a borrow into the map (no entry-stability hazard). The caller transmutes the `usize`
//!   back to the one concrete fn type that key denotes (an audited `unsafe` in the generated terminator).
//! - **Locking.** Each lookup briefly locks a global `Mutex`. This is a compile-time parser (the depth is
//!   the *syntactic* nesting of the input), so the per-re-entry lock is negligible; a lock-free cached
//!   cell is a possible future optimization (`docs/recurse-unbounded-plan.md` §8.4). The lock is
//!   poison-tolerant (`unwrap_or_else(|e| e.into_inner())`): the map holds only `Copy` `usize` values, so
//!   there is no invariant a poisoned lock would need to protect.

use crate::parse::unparse::Emitter;
use core::marker::PhantomData;
use std::any::type_name;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// A zero-sized marker keying the registry per `(root/terminator type, atom, error)`. Never instantiated
/// — only `type_name`d. The three type arguments make each `(T, A, E)` combination a distinct key,
/// matching the one concrete fn-pointer type stored for it. `T` is keyed lifetime-free (the natural root
/// type), since `type_name` erases lifetimes and needs no `'static` bound — so re-entry works for a
/// non-`'static` parameter.
pub struct ReKey<T: ?Sized, A, E>(PhantomData<(A, E)>, PhantomData<*const T>);

/// A marker used as the `A` of [`ReKey`] for the **`Spanned`** re-entry registry, so a cycle's `Spanned`
/// re-entry fn is keyed distinctly from its `Unparse` one (whose `A` is the real atom).
pub struct SpanReentry;

/// A `Sized` [`Emitter`] re-wrapping a type-erased `&mut dyn Emitter`, so a generic `Unparse::unparse<E>`
/// re-entry can accept the erased sink. Used by the unbounded **group-ful `Unparse`** terminator re-entry:
/// the terminator erases its concrete sink to `&mut dyn Emitter`, calls the registered fn, which wraps it
/// back in a `DynSink` to satisfy the generic `unparse<E: Emitter>`. (`Emitter` is already object-safe —
/// `write_one`/`write_sep` take `&mut self` — so no library trait change is needed.)
pub struct DynSink<'e, A, E>(pub &'e mut (dyn Emitter<A, Error = E> + 'e));

impl<A, E> Emitter<A> for DynSink<'_, A, E> {
    type Error = E;
    fn write_one(&mut self, atom: A) -> Result<(), Self::Error> {
        self.0.write_one(atom)
    }
    fn write_sep(&mut self) -> Result<(), Self::Error> {
        self.0.write_sep()
    }
}

static REG: OnceLock<Mutex<HashMap<&'static str, usize>>> = OnceLock::new();

/// Register the erased top-level re-entry parser (as `fn`-pointer-cast-to-`usize`) for key `K`. Idempotent
/// — the same key always maps to the same fn, so a re-register (e.g. a re-entrant or concurrent parse) is
/// a harmless overwrite.
pub fn register<K: ?Sized>(f: usize) {
    let map = REG.get_or_init(|| Mutex::new(HashMap::new()));
    map.lock().unwrap_or_else(|e| e.into_inner()).insert(type_name::<K>(), f);
}

/// Look up the re-entry parser registered for key `K`, as a `usize` to transmute back to its concrete fn
/// type. Panics only if the terminator is reached before the top-level delegated `Parse` registered — an
/// internal invariant the generated code always upholds.
pub fn lookup<K: ?Sized>() -> usize {
    let map = REG.get_or_init(|| Mutex::new(HashMap::new()));
    // Copy the `usize` out and drop the guard before `.expect()` — panicking while the guard is still
    // held would poison the global `REG` and cascade a `PoisonError` into unrelated, later parses.
    let found = map.lock().unwrap_or_else(|e| e.into_inner()).get(type_name::<K>()).copied();
    found.expect(
        "#[recurse] internal: re-entry parser not registered before the terminator was reached",
    )
}

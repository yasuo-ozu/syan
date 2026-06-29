//! Runtime fn-pointer registry backing **unbounded `#[recurse]` `Parse`**.
//!
//! A `#[recurse]` cycle's `Parse` is delegated through an internal fixed-depth *engine*; at the bottom of
//! that depth chain sits a *terminator* type whose `Parse` impl, instead of erroring, **re-enters** the
//! top-level natural parser at runtime through a type-erased fn pointer stored here. That cuts the two
//! E0275 type-recursion cycles (the per-field `where`-bound cycle and the `Dup<…>` stream-monomorphization
//! growth) at a runtime boundary, so the only depth ceiling becomes the OS call stack — like any
//! recursive-descent parser. See `docs/recurse-unbounded-plan.md`.
//!
//! The natural type's delegated `Parse` **registers** its own erased top-level parse fn here (keyed per
//! terminator type + atom + stream-error) before descending; the terminator **looks it up** and calls it.
//!
//! ## Soundness notes
//! - **Key.** Keyed on `core::any::type_name::<K>()`'s *string content* — robustly per-type and
//!   independent of pointer interning (cf. ICF/`-Zshare-generics`, which could fold equal-bodied fns). `K`
//!   is the [`ReKey`] marker carrying `(terminator, atom, stream-error)`, so the stored `usize`'s concrete
//!   fn type is unambiguous at every lookup.
//! - **Value.** The fn pointer is stored as `usize` and **copied out** on lookup, so a `HashMap` rehash
//!   never dangles a borrow into the map (no entry-stability hazard). The caller transmutes the `usize`
//!   back to the one concrete fn type that key denotes (an audited `unsafe` in the generated terminator).
//! - **Locking.** Each lookup briefly locks a global `Mutex`. This is a compile-time parser (the depth is
//!   the *syntactic* nesting of the input), so the per-re-entry lock is negligible; a lock-free cached
//!   cell is a possible future optimization (`docs/recurse-unbounded-plan.md` §8.4).

use core::marker::PhantomData;
use std::any::type_name;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// A zero-sized marker keying the registry per `(terminator type, atom, stream-error)`. Never
/// instantiated — only `type_name`d. The three type arguments make each `(T, A, E)` combination a
/// distinct key, matching the one concrete fn-pointer type stored for it.
pub struct ReKey<T: ?Sized, A, E>(PhantomData<(A, E)>, PhantomData<*const T>);

static REG: OnceLock<Mutex<HashMap<&'static str, usize>>> = OnceLock::new();

/// Register the erased top-level re-entry parser (as `fn`-pointer-cast-to-`usize`) for key `K`. Idempotent
/// — the same key always maps to the same fn, so a re-register (e.g. a re-entrant or concurrent parse) is
/// a harmless overwrite.
pub fn register<K: ?Sized>(f: usize) {
    let map = REG.get_or_init(|| Mutex::new(HashMap::new()));
    map.lock().unwrap().insert(type_name::<K>(), f);
}

/// Look up the re-entry parser registered for key `K`, as a `usize` to transmute back to its concrete fn
/// type. Panics only if the terminator is reached before the top-level delegated `Parse` registered — an
/// internal invariant the generated code always upholds.
pub fn lookup<K: ?Sized>() -> usize {
    let map = REG.get_or_init(|| Mutex::new(HashMap::new()));
    *map.lock().unwrap().get(type_name::<K>()).expect(
        "#[recurse] internal: re-entry parser not registered before the terminator was reached",
    )
}

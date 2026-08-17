//! Visitor-system support items.
//!
//! The generated visitor modules and the `#[derive(Ast)]` metadata macros live in user crates;
//! this module only holds the two cross-crate primitives they rely on:
//!
//! * [`Ast`] — an empty marker trait implemented by `#[derive(Ast)]` for every AST node type.
//! * [`Repeater`] — the `type-leak` indirection trait. `#[derive(Ast)]` emits one
//!   `impl Repeater<N> for <the AST type>` per field type that depends on the definition's type
//!   context, so a consumer can name those types portably as
//!   `<T as ::syan::visit::Repeater<N>>::Type` regardless of which crate/module it expands in.
//!

pub use syan_macro::Ast;

/// Define a visitor over the given AST types, used *inside* an (otherwise empty) module:
///
/// ```ignore
/// pub mod my_visitor {
///     syan::visit::visitor!(Type, Expr);          // or: visitor!(super::base => Stmt);
/// }
/// ```
///
/// This captures `$crate` (the path to `syan` from the caller) and forwards it to the proc-macro,
/// so the syan crate is resolved automatically (no `#[syan(..)]` needed).
#[macro_export]
macro_rules! visitor {
    ($($t:tt)*) => {
        $crate::_imp::syan_macro::__visitor_entry! { @syan { $crate } $($t)* }
    };
}

#[doc(hidden)]
pub use crate::visitor;

/// Marker trait implemented by every type carrying `#[derive(Ast)]`.
///
/// It carries no methods; its only purpose is to let generic code (and the `#[visitor]` generator)
/// bound on "is an AST node".
pub trait Ast {}

/// `type-leak` repeater: passes a single type out of the leaker's type context to a referrer.
///
/// `INDEX` distinguishes the type references collected from one definition (in declaration order,
/// matching `type_leak::Referrer::iter`). The `#[derive(Ast)]` macro implements this directly on
/// the AST type; a consumer refers back through it.
pub trait Repeater<const INDEX: usize> {
    /// The leaked type, valid in the referrer's context via `<T as Repeater<INDEX>>::Type`.
    type Type: ?Sized;
}

// ── Structural-edit views ───────────────────────────────────────────────────────────────────────
//
// A generated `visit_mut` traversal hands a *node held inside another AST in a collection / Option slot*
// a **view of that slot** as an argument, through which the visitor edits the parent **in place** (no
// cloning of existing nodes). The view is a trait implemented directly on the container types — so the
// descent passes `&mut self.field` with no wrapper. Two dedicated interfaces: [`SeqView`] (Vec-like,
// unbounded) and [`OptView`] (Option-like, ≤1).
//
// The element type is a **type parameter** (`SeqView<T>`, not an associated type); the traits are
// bare-element only — a wrapper like `Box<T>`/`Attempt<T>` implements `OptView<T>` directly (single-slot,
// always-full) and the visitor descends *through* wrapped shapes by recursing per layer, not via any
// wrapped-element `SeqView`/`OptView` impl.

/// A mutable, **sequence-like** view of an AST collection field (`Vec`/`VecDeque`/`Punctuated`),
/// bare-element — the element type is `T` itself, never a wrapped `Box<T>`. A generated
/// `visit_<t>_seq(&mut self, &mut impl SeqView<T>)` receives one; override it to edit the collection in
/// place. The required core (`len`/`get`/`get_mut`/`insert`/`remove`) is object-safe; the ergonomic
/// helpers are `Self: Sized` provided methods.
pub trait SeqView<T> {
    fn len(&self) -> usize;
    fn get(&self, i: usize) -> Option<&T>;
    /// Edit an element in place — no clone.
    fn get_mut(&mut self, i: usize) -> Option<&mut T>;
    /// Insert before index `i` (`i == len` appends).
    fn insert(&mut self, i: usize, value: T);
    fn remove(&mut self, i: usize) -> T;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn push(&mut self, value: T)
    where
        Self: Sized,
    {
        let n = self.len();
        self.insert(n, value);
    }
    /// Visit each element in place, then drop those for which `f` returns `false`.
    fn retain_mut(&mut self, mut f: impl FnMut(&mut T) -> bool)
    where
        Self: Sized,
    {
        let mut i = 0;
        while i < self.len() {
            let keep = match self.get_mut(i) {
                Some(e) => f(e),
                None => true,
            };
            if keep {
                i += 1;
            } else {
                self.remove(i);
            }
        }
    }
    /// Iterate the elements by shared ref (`for x in v.view_iter() { … }`). Default impl over `get`.
    /// Named `view_iter` (not `iter`) so it never shadows the slice `iter` on a concrete `Vec` when
    /// `SeqView` is in scope.
    fn view_iter(&self) -> SeqIter<'_, T>
    where
        Self: Sized,
    {
        SeqIter {
            seq: self,
            idx: 0,
            len: self.len(),
        }
    }
    /// Iterate the elements by `&mut` for in-place edits (`for x in v.view_iter_mut() { … }`). For
    /// structural changes use `push`/`insert`/`remove`/`retain_mut`. Default impl over the by-index
    /// `get_mut`. Named `view_iter_mut` to avoid shadowing the slice `iter_mut`.
    fn view_iter_mut(&mut self) -> SeqIterMut<'_, T>
    where
        Self: Sized,
    {
        let len = self.len();
        SeqIterMut {
            seq: self,
            idx: 0,
            len,
        }
    }
}

/// The shared iterator returned by [`SeqView::view_iter`] — yields each element by index.
pub struct SeqIter<'a, T> {
    seq: &'a dyn SeqView<T>,
    idx: usize,
    len: usize,
}

impl<'a, T> Iterator for SeqIter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<&'a T> {
        if self.idx >= self.len {
            return None;
        }
        let i = self.idx;
        self.idx += 1;
        // `self.seq` is a `&'a` (Copy) borrow, so `get` returns `&'a T` — no lifetime widening needed
        // (shared borrows may coexist).
        self.seq.get(i)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.len - self.idx;
        (n, Some(n))
    }
}

impl<'a, T> ExactSizeIterator for SeqIter<'a, T> {}

/// The `&mut` iterator returned by [`SeqView::view_iter_mut`] — yields each element once, by index.
pub struct SeqIterMut<'a, T> {
    seq: &'a mut dyn SeqView<T>,
    idx: usize,
    len: usize,
}

impl<'a, T> Iterator for SeqIterMut<'a, T> {
    type Item = &'a mut T;
    fn next(&mut self) -> Option<&'a mut T> {
        if self.idx >= self.len {
            return None;
        }
        let i = self.idx;
        self.idx += 1;
        // SAFETY: each index is yielded exactly once, so the returned `&mut T`s are pairwise disjoint, and
        // all borrow from `self.seq` (a `&'a mut` collection that outlives them) — so widening the element
        // borrow to `'a` is sound.
        self.seq.get_mut(i).map(|r| unsafe { &mut *(r as *mut T) })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.len - self.idx;
        (n, Some(n))
    }
}

impl<'a, T> ExactSizeIterator for SeqIterMut<'a, T> {}

/// A mutable, **Option-like** view (≤1 element) of an AST `Option` field, bare-element (nested
/// `Box`/`Attempt` layers descend separately). A generated
/// `visit_<t>_opt(&mut self, &mut impl OptView<T>)` receives one.
pub trait OptView<T> {
    fn is_some(&self) -> bool;
    fn get(&self) -> Option<&T>;
    /// Edit the node in place — no clone.
    fn get_mut(&mut self) -> Option<&mut T>;
    /// Fill or replace the node (works on an empty slot).
    fn set(&mut self, value: T);
    fn take(&mut self) -> Option<T>;

    fn is_none(&self) -> bool {
        !self.is_some()
    }
    fn clear(&mut self) {
        let _ = self.take();
    }
    /// Iterate the node by shared ref — 0 or 1 items. Named `view_iter` (not `iter`) to mirror
    /// [`SeqView::view_iter`] and stay non-shadowing.
    fn view_iter(&self) -> core::option::IntoIter<&T> {
        self.get().into_iter()
    }
    /// Iterate the node by `&mut` — 0 or 1 items (in-place edit). Mirrors [`SeqView::view_iter_mut`].
    fn view_iter_mut(&mut self) -> core::option::IntoIter<&mut T> {
        self.get_mut().into_iter()
    }
}

// `SeqView`/`OptView` are **bare-element**: the container holds the viewed node `T` directly (no element
// adapter). A transparent single-slot wrapper (`Box<T>`/`Attempt<T>`/user wrappers) implements `OptView<T>`
// (always present, so a 1-element view) so the visitor descends *through* it uniformly via
// `view_iter_mut`, recursing per level. `take` can't empty a fixed single slot, so it is unreachable — a
// single wrapper is descent-only, never a `#[seq]`/`#[opt]` edit target.

impl<T> SeqView<T> for Vec<T> {
    fn len(&self) -> usize {
        <[T]>::len(self)
    }
    fn get(&self, i: usize) -> Option<&T> {
        <[T]>::get(self, i)
    }
    fn get_mut(&mut self, i: usize) -> Option<&mut T> {
        <[T]>::get_mut(self, i)
    }
    fn insert(&mut self, i: usize, value: T) {
        Vec::insert(self, i, value);
    }
    fn remove(&mut self, i: usize) -> T {
        Vec::remove(self, i)
    }
}

impl<T> SeqView<T> for std::collections::VecDeque<T> {
    fn len(&self) -> usize {
        std::collections::VecDeque::len(self)
    }
    fn get(&self, i: usize) -> Option<&T> {
        std::collections::VecDeque::get(self, i)
    }
    fn get_mut(&mut self, i: usize) -> Option<&mut T> {
        std::collections::VecDeque::get_mut(self, i)
    }
    fn insert(&mut self, i: usize, value: T) {
        std::collections::VecDeque::insert(self, i, value);
    }
    fn remove(&mut self, i: usize) -> T {
        std::collections::VecDeque::remove(self, i).expect("index in bounds")
    }
}

// `insert`/`push` synthesize the separator via `Punct::default()`, hence `P: Default`.
impl<T, P: Default> SeqView<T> for crate::nested::Punctuated<T, P> {
    fn len(&self) -> usize {
        crate::nested::Punctuated::len(self)
    }
    fn get(&self, i: usize) -> Option<&T> {
        crate::nested::Punctuated::get(self, i)
    }
    fn get_mut(&mut self, i: usize) -> Option<&mut T> {
        crate::nested::Punctuated::get_mut(self, i)
    }
    fn insert(&mut self, i: usize, value: T) {
        crate::nested::Punctuated::insert(self, i, value);
    }
    fn remove(&mut self, i: usize) -> T {
        crate::nested::Punctuated::remove(self, i).expect("index in bounds")
    }
}

impl<T> OptView<T> for Option<T> {
    fn is_some(&self) -> bool {
        Option::is_some(self)
    }
    fn get(&self) -> Option<&T> {
        self.as_ref()
    }
    fn get_mut(&mut self) -> Option<&mut T> {
        self.as_mut()
    }
    fn set(&mut self, value: T) {
        *self = Some(value);
    }
    fn take(&mut self) -> Option<T> {
        Option::take(self)
    }
}

impl<T> OptView<T> for Box<T> {
    fn is_some(&self) -> bool {
        true
    }
    fn get(&self) -> Option<&T> {
        Some(&**self)
    }
    fn get_mut(&mut self) -> Option<&mut T> {
        Some(&mut **self)
    }
    fn set(&mut self, value: T) {
        **self = value;
    }
    fn take(&mut self) -> Option<T> {
        unreachable!("Box<T> is a single-slot view; `take` would empty it")
    }
}

impl<T> OptView<T> for crate::nested::Attempt<T> {
    fn is_some(&self) -> bool {
        true
    }
    fn get(&self) -> Option<&T> {
        Some(&self.0)
    }
    fn get_mut(&mut self) -> Option<&mut T> {
        Some(&mut self.0)
    }
    fn set(&mut self, value: T) {
        self.0 = value;
    }
    fn take(&mut self) -> Option<T> {
        unreachable!("Attempt<T> is a single-slot view; `take` would empty it")
    }
}

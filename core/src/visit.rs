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
//! See `CLAUDE.md` for the full design.

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
// unbounded) and [`OptView`] (Option-like, ≤1). See `docs/visitor-edit-plan.md`.
//
// The element type is a **type parameter** (`SeqView<T>`, not an associated type) so the `Box`-wrapped
// element forms (`Vec<Box<T>>`, `Option<Box<T>>`) can implement the *unboxed* view (`SeqView<T>`) without
// colliding with the plain `SeqView<Box<T>>` impl — a `Vec<Box<U>>` implements both `SeqView<Box<U>>` and
// `SeqView<U>` (distinct trait parameters), and the generated `visit_t_seq` selects `SeqView<T>` by `T`.

/// A mutable, **sequence-like** view of an AST collection field (`Vec`/`VecDeque`/`Punctuated`, and their
/// `Box`-wrapped element forms — box-transparent, so the element type is `T`, not `Box<T>`). A generated
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
    /// Iterate the elements by `&mut` for in-place edits (`for x in v.iter_mut() { … }`). For structural
    /// changes use `push`/`insert`/`remove`/`retain_mut`. Default impl over the by-index `get_mut`.
    fn iter_mut(&mut self) -> SeqIterMut<'_, T>
    where
        Self: Sized,
    {
        let len = self.len();
        SeqIterMut { seq: self, idx: 0, len }
    }
}

/// The `&mut` iterator returned by [`SeqView::iter_mut`] — yields each element once, by index.
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

/// A mutable, **Option-like** view (≤1 element) of an AST `Option` field (and `Option<Box<T>>`,
/// box-transparent). A generated `visit_<t>_opt(&mut self, &mut impl OptView<T>)` receives one.
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
}

/// How a stored element wraps the viewed node `T`: identity, `Box<T>`, or `Attempt<T>` (a transparent
/// `Deref` newtype). One `SeqView`/`OptView` impl per container is generic over the wrapper, so the
/// `Box`/`Attempt` element forms are covered box-transparently without separate impls.
#[doc(hidden)]
pub trait Wrap<T> {
    fn as_node(&self) -> &T;
    fn as_node_mut(&mut self) -> &mut T;
    fn wrap(node: T) -> Self;
    fn into_node(self) -> T;
}
impl<T> Wrap<T> for T {
    fn as_node(&self) -> &T {
        self
    }
    fn as_node_mut(&mut self) -> &mut T {
        self
    }
    fn wrap(node: T) -> T {
        node
    }
    fn into_node(self) -> T {
        self
    }
}
impl<T> Wrap<T> for Box<T> {
    fn as_node(&self) -> &T {
        self
    }
    fn as_node_mut(&mut self) -> &mut T {
        self
    }
    fn wrap(node: T) -> Box<T> {
        Box::new(node)
    }
    fn into_node(self) -> T {
        *self
    }
}
impl<T> Wrap<T> for crate::nested::Attempt<T> {
    fn as_node(&self) -> &T {
        &self.0
    }
    fn as_node_mut(&mut self) -> &mut T {
        &mut self.0
    }
    fn wrap(node: T) -> Self {
        crate::nested::Attempt(node)
    }
    fn into_node(self) -> T {
        self.0
    }
}

impl<T, W: Wrap<T>> SeqView<T> for Vec<W> {
    fn len(&self) -> usize {
        <[W]>::len(self)
    }
    fn get(&self, i: usize) -> Option<&T> {
        <[W]>::get(self, i).map(W::as_node)
    }
    fn get_mut(&mut self, i: usize) -> Option<&mut T> {
        <[W]>::get_mut(self, i).map(W::as_node_mut)
    }
    fn insert(&mut self, i: usize, value: T) {
        Vec::insert(self, i, W::wrap(value));
    }
    fn remove(&mut self, i: usize) -> T {
        Vec::remove(self, i).into_node()
    }
}

// A `Box` forwards to the boxed view (only where the boxed type is itself a view) — so a
// `Box`-around-a-container field (`#[seq] Box<Vec<T>>` / `#[opt] Box<Option<T>>`) views the inner one.
impl<E, T: SeqView<E> + ?Sized> SeqView<E> for Box<T> {
    fn len(&self) -> usize {
        <T as SeqView<E>>::len(&**self)
    }
    fn get(&self, i: usize) -> Option<&E> {
        <T as SeqView<E>>::get(&**self, i)
    }
    fn get_mut(&mut self, i: usize) -> Option<&mut E> {
        <T as SeqView<E>>::get_mut(&mut **self, i)
    }
    fn insert(&mut self, i: usize, value: E) {
        <T as SeqView<E>>::insert(&mut **self, i, value)
    }
    fn remove(&mut self, i: usize) -> E {
        <T as SeqView<E>>::remove(&mut **self, i)
    }
}

impl<E, T: OptView<E> + ?Sized> OptView<E> for Box<T> {
    fn is_some(&self) -> bool {
        <T as OptView<E>>::is_some(&**self)
    }
    fn get(&self) -> Option<&E> {
        <T as OptView<E>>::get(&**self)
    }
    fn get_mut(&mut self) -> Option<&mut E> {
        <T as OptView<E>>::get_mut(&mut **self)
    }
    fn set(&mut self, value: E) {
        <T as OptView<E>>::set(&mut **self, value)
    }
    fn take(&mut self) -> Option<E> {
        <T as OptView<E>>::take(&mut **self)
    }
}

impl<T, W: Wrap<T>> SeqView<T> for std::collections::VecDeque<W> {
    fn len(&self) -> usize {
        std::collections::VecDeque::len(self)
    }
    fn get(&self, i: usize) -> Option<&T> {
        std::collections::VecDeque::get(self, i).map(W::as_node)
    }
    fn get_mut(&mut self, i: usize) -> Option<&mut T> {
        std::collections::VecDeque::get_mut(self, i).map(W::as_node_mut)
    }
    fn insert(&mut self, i: usize, value: T) {
        std::collections::VecDeque::insert(self, i, W::wrap(value));
    }
    fn remove(&mut self, i: usize) -> T {
        std::collections::VecDeque::remove(self, i).expect("index in bounds").into_node()
    }
}

// `insert`/`push` synthesize the separator via `Punct::default()`, hence `P: Default`.
impl<T, W: Wrap<T>, P: Default> SeqView<T> for crate::nested::Punctuated<W, P> {
    fn len(&self) -> usize {
        crate::nested::Punctuated::len(self)
    }
    fn get(&self, i: usize) -> Option<&T> {
        crate::nested::Punctuated::get(self, i).map(W::as_node)
    }
    fn get_mut(&mut self, i: usize) -> Option<&mut T> {
        crate::nested::Punctuated::get_mut(self, i).map(W::as_node_mut)
    }
    fn insert(&mut self, i: usize, value: T) {
        crate::nested::Punctuated::insert(self, i, W::wrap(value));
    }
    fn remove(&mut self, i: usize) -> T {
        crate::nested::Punctuated::remove(self, i).expect("index in bounds").into_node()
    }
}

impl<T, W: Wrap<T>> OptView<T> for Option<W> {
    fn is_some(&self) -> bool {
        Option::is_some(self)
    }
    fn get(&self) -> Option<&T> {
        self.as_ref().map(W::as_node)
    }
    fn get_mut(&mut self) -> Option<&mut T> {
        self.as_mut().map(W::as_node_mut)
    }
    fn set(&mut self, value: T) {
        *self = Some(W::wrap(value));
    }
    fn take(&mut self) -> Option<T> {
        Option::take(self).map(W::into_node)
    }
}

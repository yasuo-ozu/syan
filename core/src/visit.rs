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
    /// Number of elements.
    fn len(&self) -> usize;
    /// Shared access to element `i` (`None` if out of range).
    fn get(&self, i: usize) -> Option<&T>;
    /// Mutable access to element `i` — edit a node in place, no clone (`None` if out of range).
    fn get_mut(&mut self, i: usize) -> Option<&mut T>;
    /// Insert `value` before index `i` (`i == len` appends).
    fn insert(&mut self, i: usize, value: T);
    /// Remove and return element `i`, shifting the rest down.
    fn remove(&mut self, i: usize) -> T;

    /// Whether the collection is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Append `value` to the end.
    fn push(&mut self, value: T)
    where
        Self: Sized,
    {
        let n = self.len();
        self.insert(n, value);
    }
    /// Edit every element in place (no structural change).
    fn for_each_mut(&mut self, mut f: impl FnMut(&mut T))
    where
        Self: Sized,
    {
        let n = self.len();
        for i in 0..n {
            if let Some(e) = self.get_mut(i) {
                f(e);
            }
        }
    }
    /// Keep only the elements for which `f` returns `true` (visiting each in place first).
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
    /// Walk the collection handing each element a [`SeqCursor`] — a positioned handle that can edit the
    /// node in place and structurally `remove` / `replace` / `insert_before` / `insert_after` it, with the
    /// index bookkeeping handled here. (Elements inserted *after* the current one are not re-visited.)
    fn edit_each(&mut self, mut f: impl FnMut(&mut SeqCursor<'_, T>))
    where
        Self: Sized,
    {
        let mut i = 0;
        while i < self.len() {
            let mut cur = SeqCursor { seq: self, idx: i, step: 1 };
            f(&mut cur);
            i = cur.idx + cur.step;
        }
    }
}

/// A positioned handle over one element of a [`SeqView`], handed out by [`SeqView::edit_each`]. Edits the
/// current node in place (`get_mut`) or structurally; the parent walk advances correctly afterwards.
/// Do at most one structural op (`remove`/`replace`/`insert_*`) per element.
pub struct SeqCursor<'a, T> {
    seq: &'a mut dyn SeqView<T>,
    idx: usize,
    step: usize,
}

impl<'a, T> SeqCursor<'a, T> {
    /// Shared access to the current node.
    pub fn get(&self) -> &T {
        self.seq.get(self.idx).expect("cursor in bounds")
    }
    /// Mutable access to the current node — edit in place, no clone.
    pub fn get_mut(&mut self) -> &mut T {
        self.seq.get_mut(self.idx).expect("cursor in bounds")
    }
    /// Replace the current node with `value`.
    pub fn replace(&mut self, value: T) {
        *self.seq.get_mut(self.idx).expect("cursor in bounds") = value;
    }
    /// Remove the current node (the next element shifts into this position and is visited next).
    pub fn remove(&mut self) {
        self.seq.remove(self.idx);
        self.step = 0;
    }
    /// Insert `value` immediately before the current node.
    pub fn insert_before(&mut self, value: T) {
        self.seq.insert(self.idx, value);
        self.idx += 1;
    }
    /// Insert `value` immediately after the current node (it is not re-visited).
    pub fn insert_after(&mut self, value: T) {
        self.seq.insert(self.idx + 1, value);
        self.step += 1;
    }
}

/// A mutable, **Option-like** view (≤1 element) of an AST `Option` field (and `Option<Box<T>>`,
/// box-transparent). A generated `visit_<t>_opt(&mut self, &mut impl OptView<T>)` receives one.
pub trait OptView<T> {
    /// Whether a node is present.
    fn is_some(&self) -> bool;
    /// Shared access to the node, if present.
    fn get(&self) -> Option<&T>;
    /// Mutable access to the node, if present — edit in place, no clone.
    fn get_mut(&mut self) -> Option<&mut T>;
    /// Set (fill or replace) the node.
    fn set(&mut self, value: T);
    /// Remove and return the node, leaving the slot empty.
    fn take(&mut self) -> Option<T>;

    /// Whether the slot is empty.
    fn is_none(&self) -> bool {
        !self.is_some()
    }
    /// Empty the slot.
    fn clear(&mut self) {
        let _ = self.take();
    }
}

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

// Box-transparent: the view element is the inner `T`, the `Box` is managed here.
impl<T> SeqView<T> for Vec<Box<T>> {
    fn len(&self) -> usize {
        <[Box<T>]>::len(self)
    }
    fn get(&self, i: usize) -> Option<&T> {
        <[Box<T>]>::get(self, i).map(|b| &**b)
    }
    fn get_mut(&mut self, i: usize) -> Option<&mut T> {
        <[Box<T>]>::get_mut(self, i).map(|b| &mut **b)
    }
    fn insert(&mut self, i: usize, value: T) {
        Vec::insert(self, i, Box::new(value));
    }
    fn remove(&mut self, i: usize) -> T {
        *Vec::remove(self, i)
    }
}

// A `Box` is transparent for views: it forwards to the boxed view, so a `Box`-around-a-container field
// (e.g. `#[seq] Box<Vec<T>>` / `#[opt] Box<Option<T>>`) views the inner collection/Option. Only where the
// boxed type is itself a view (so a bare `Box<Leaf>` whose `Leaf` is not a view is *not* a view).
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

impl<T> SeqView<T> for std::collections::VecDeque<Box<T>> {
    fn len(&self) -> usize {
        std::collections::VecDeque::len(self)
    }
    fn get(&self, i: usize) -> Option<&T> {
        std::collections::VecDeque::get(self, i).map(|b| &**b)
    }
    fn get_mut(&mut self, i: usize) -> Option<&mut T> {
        std::collections::VecDeque::get_mut(self, i).map(|b| &mut **b)
    }
    fn insert(&mut self, i: usize, value: T) {
        std::collections::VecDeque::insert(self, i, Box::new(value));
    }
    fn remove(&mut self, i: usize) -> T {
        *std::collections::VecDeque::remove(self, i).expect("index in bounds")
    }
}

// `Attempt<T>` is a transparent `Deref` newtype (a parse-backtracking wrapper), peeled like `Box` by the
// visitor — so the views see through it too.
impl<T> SeqView<T> for Vec<crate::nested::Attempt<T>> {
    fn len(&self) -> usize {
        <[crate::nested::Attempt<T>]>::len(self)
    }
    fn get(&self, i: usize) -> Option<&T> {
        <[crate::nested::Attempt<T>]>::get(self, i).map(|a| &**a)
    }
    fn get_mut(&mut self, i: usize) -> Option<&mut T> {
        <[crate::nested::Attempt<T>]>::get_mut(self, i).map(|a| &mut **a)
    }
    fn insert(&mut self, i: usize, value: T) {
        Vec::insert(self, i, crate::nested::Attempt(value));
    }
    fn remove(&mut self, i: usize) -> T {
        Vec::remove(self, i).0
    }
}

impl<T> SeqView<T> for std::collections::VecDeque<crate::nested::Attempt<T>> {
    fn len(&self) -> usize {
        std::collections::VecDeque::len(self)
    }
    fn get(&self, i: usize) -> Option<&T> {
        std::collections::VecDeque::get(self, i).map(|a| &**a)
    }
    fn get_mut(&mut self, i: usize) -> Option<&mut T> {
        std::collections::VecDeque::get_mut(self, i).map(|a| &mut **a)
    }
    fn insert(&mut self, i: usize, value: T) {
        std::collections::VecDeque::insert(self, i, crate::nested::Attempt(value));
    }
    fn remove(&mut self, i: usize) -> T {
        std::collections::VecDeque::remove(self, i).expect("index in bounds").0
    }
}

// `Punctuated` insert/push synthesize the separator via `Punct::default()`, hence `P: Default`.
impl<T, P: Default> SeqView<T> for crate::nested::Punctuated<T, P> {
    fn len(&self) -> usize {
        crate::nested::Punctuated::len(self)
    }
    fn get(&self, i: usize) -> Option<&T> {
        crate::nested::Punctuated::iter(self).nth(i)
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

impl<T, P: Default> SeqView<T> for crate::nested::Punctuated<Box<T>, P> {
    fn len(&self) -> usize {
        crate::nested::Punctuated::len(self)
    }
    fn get(&self, i: usize) -> Option<&T> {
        crate::nested::Punctuated::iter(self).nth(i).map(|b| &**b)
    }
    fn get_mut(&mut self, i: usize) -> Option<&mut T> {
        crate::nested::Punctuated::get_mut(self, i).map(|b| &mut **b)
    }
    fn insert(&mut self, i: usize, value: T) {
        crate::nested::Punctuated::insert(self, i, Box::new(value));
    }
    fn remove(&mut self, i: usize) -> T {
        *crate::nested::Punctuated::remove(self, i).expect("index in bounds")
    }
}

impl<T, P: Default> SeqView<T> for crate::nested::Punctuated<crate::nested::Attempt<T>, P> {
    fn len(&self) -> usize {
        crate::nested::Punctuated::len(self)
    }
    fn get(&self, i: usize) -> Option<&T> {
        crate::nested::Punctuated::iter(self).nth(i).map(|a| &**a)
    }
    fn get_mut(&mut self, i: usize) -> Option<&mut T> {
        crate::nested::Punctuated::get_mut(self, i).map(|a| &mut **a)
    }
    fn insert(&mut self, i: usize, value: T) {
        crate::nested::Punctuated::insert(self, i, crate::nested::Attempt(value));
    }
    fn remove(&mut self, i: usize) -> T {
        crate::nested::Punctuated::remove(self, i).expect("index in bounds").0
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

impl<T> OptView<T> for Option<Box<T>> {
    fn is_some(&self) -> bool {
        Option::is_some(self)
    }
    fn get(&self) -> Option<&T> {
        self.as_deref()
    }
    fn get_mut(&mut self) -> Option<&mut T> {
        self.as_deref_mut()
    }
    fn set(&mut self, value: T) {
        *self = Some(Box::new(value));
    }
    fn take(&mut self) -> Option<T> {
        Option::take(self).map(|b| *b)
    }
}

impl<T> OptView<T> for Option<crate::nested::Attempt<T>> {
    fn is_some(&self) -> bool {
        Option::is_some(self)
    }
    fn get(&self) -> Option<&T> {
        self.as_ref().map(|a| &**a)
    }
    fn get_mut(&mut self) -> Option<&mut T> {
        self.as_mut().map(|a| &mut **a)
    }
    fn set(&mut self, value: T) {
        *self = Some(crate::nested::Attempt(value));
    }
    fn take(&mut self) -> Option<T> {
        Option::take(self).map(|a| a.0)
    }
}

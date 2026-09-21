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
///
/// # What it generates
///
/// Everything below is emitted twice: once by shared reference, and once by `&mut` with a `_mut`
/// suffix.
///
/// * **`Visit`** — one `visit_<type>` method per listed type. Each default recurses, so you
///   override only the nodes you care about.
/// * **`visit_<type>`** — a free function that walks one node's children. The trait method is the
///   hook; this is the descent. Call it from an override to keep going.
/// * **`Hook`, `Driver`, `<Type>Hook`, `IntoVisitor`** — adapters that let a closure act as a
///   visitor. The closure's argument type picks the node it sees. A tuple of closures runs them all
///   in one traversal.
/// * **`visit`** — an inherent method on each listed type, so a walk starts with `node.visit(..)`.
///
/// # Generated names
///
/// For a listed type `T`, written `t` in snake_case:
///
/// | | shared (`Visit`) | by `&mut` (`VisitMut`) |
/// |---|---|---|
/// | the node | `visit_t` | `visit_t_mut` |
/// | a `#[seq]` field | — | `visit_t_seq` |
/// | a `#[opt]` field | — | `visit_t_opt` |
/// | closure hook | `hook_t` | `hook_t_mut` |
/// | entry point | `T::visit` | `T::visit_mut` |
///
/// The `_seq` and `_opt` methods are on `VisitMut` only. A view exists to *edit* the parent slot;
/// reading needs nothing beyond the element, which `visit_t` already gives. They take no extra
/// `_mut` suffix, because the trait they sit on is already the `&mut` one.
///
/// ```
/// mod ast {
///     use syan::visit::Ast;
///
///     #[derive(Ast)]
///     #[subast(crate::ast::Expr)]
///     pub enum Expr { Lit(u32), Neg(Box<Expr>), Many(#[seq] Vec<Expr>) }
///
///     pub mod visit { syan::visit::visitor!(super::Expr); }
/// }
///
/// fn main() {
///     use ast::{visit, Expr};
///     use syan::visit::SeqView;
///
///     // A closure sees one node type.
///     let mut n = 0;
///     Expr::Neg(Box::new(Expr::Lit(1))).visit(|_: &Expr| n += 1);
///     assert_eq!(n, 2);
///
///     // `#[seq]` adds `visit_expr_seq`, which hands you the parent slot, not just the element.
///     struct DropLits;
///     impl visit::VisitMut for DropLits {
///         fn visit_expr_seq<V: SeqView<Expr>>(&mut self, v: &mut V) {
///             v.retain_mut(|e| !matches!(e, Expr::Lit(_)));
///         }
///     }
///     let mut e = Expr::Many(vec![Expr::Lit(1), Expr::Neg(Box::new(Expr::Lit(2)))]);
///     e.visit_mut(&mut DropLits);
///     assert!(matches!(&e, Expr::Many(v) if v.len() == 1));
///
///     // A struct overrides one method and calls the free fn to keep descending.
///     struct Depth { max: usize, at: usize }
///     impl visit::Visit for Depth {
///         fn visit_expr(&mut self, i: &Expr) {
///             self.at += 1;
///             self.max = self.max.max(self.at);
///             visit::visit_expr(self, i);
///             self.at -= 1;
///         }
///     }
///     let mut d = Depth { max: 0, at: 0 };
///     Expr::Neg(Box::new(Expr::Lit(1))).visit(&mut d);
///     assert_eq!(d.max, 2);
/// }
/// ```
///
/// # What it expands to
///
/// For the `Expr` above, the module gets about 390 lines. The parts that matter:
///
/// ```ignore
/// pub trait Visit {
///     fn visit_expr(&mut self, i: &super::Expr) { visit_expr(self, i) }
/// }
///
/// pub fn visit_expr<__V: Visit + ?Sized>(this: &mut __V, i: &super::Expr) {
///     match i {
///         super::Expr::Lit(_) => {}
///         super::Expr::Neg(__f0_0) => {
///             for __nc1 in __f0_0.view_iter() { this.visit_expr(__nc1); }
///         }
///     }
/// }
///
/// pub trait Hook { fn hook_expr(&mut self, i: &super::Expr) { let _ = i; } }
/// pub struct Driver<__H>(pub __H);            // Hook   -> Visit
/// pub struct ExprHook<__F>(pub __F);          // FnMut  -> Hook
///
/// impl<__F: FnMut(&super::Expr)> IntoVisitor<super::Expr> for __F {
///     fn into_visitor(self) -> impl Visit { Driver(ExprHook(self)) }
/// }
///
/// impl<__V: Visit> Visit for &mut __V { .. }                       // pass `&mut pass`
/// impl<__V0: Visit, __V1: Visit> Visit for (__V0, __V1) { .. }     // several passes, one walk
///
/// // a visitor inside a `Slot` (`Box<MyPass>`, `Attempt`, your own wrapper)
/// impl<__S: SlotMut> Visit for syan::visit::SlotDriver<__S> where __S::Target: Visit { .. }
/// impl<__S: SlotMut> IntoVisitor<syan::visit::WrappedVisitor> for __S
///     where __S::Target: Visit { .. }
///
/// impl super::Expr {
///     pub fn visit<__T>(&self, visitor: impl IntoVisitor<__T>) -> &Self { .. }
/// }
///
/// // On `VisitMut`, because a `#[seq]` field can be edited, not just read:
/// pub trait VisitMut {
///     fn visit_expr_mut(&mut self, i: &mut super::Expr) { visit_expr_mut(self, i) }
///
///     fn visit_expr_seq<__VW: SeqView<super::Expr>>(&mut self, v: &mut __VW) {
///         for __syan_e in SeqView::view_iter_mut(v) { self.visit_expr_mut(__syan_e); }
///     }
/// }
/// ```
///
/// `Lit(u32)` produces an empty arm because `u32` is not a visited type. `Neg(Box<Expr>)` produces
/// one loop per wrapper layer. There are 8 traits and 20 `into_visitor` impls in all — the tuple
/// arities, and a `_mut` twin for each.
///
/// Visitors compose two ways. A **tuple of visitors** (arity 2..=8) implements `Visit` itself, so
/// every element sees every node in one traversal and the tuple can go anywhere one visitor can. A
/// visitor held in a [`Slot`] arrives through `IntoVisitor` rather than `Visit`, because a blanket
/// `impl Visit for T where T: SlotMut` and the tuple impls overlap as far as coherence can tell.
/// `node.visit(Box::new(pass))` therefore works, but the wrapper is taken by value.
///
/// A `#[seq]` field adds `visit_<type>_seq`, and `#[opt]` adds `visit_<type>_opt`. Both are on
/// `VisitMut` only, and both hand you a view of the *parent slot* — a [`SeqView`] or [`OptView`] —
/// so an override can `push`, `remove` or `retain_mut` rather than only read each element. The
/// default just descends. The marked field must be a bare `Vec<T>` or `Option<T>`: a wrapped one
/// such as `Option<Box<T>>` cannot be edited in place, and the macro says so.
///
/// The walk never names a container type. A field is stepped through `view_iter`, which the
/// compiler resolves to [`SeqView`], [`OptView`], [`MapView`] or [`Slot`]. So `Box<T>` and
/// `Vec<T>` generate the same code, and nested wrappers nest the loops.
///
/// Every listed type needs `#[derive(Ast)]`: the macro reads its shape from the metadata that
/// derive emits. Generated names come from a type's last path segment, so two listed types ending
/// in the same ident are rejected.
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
// descent passes `&mut self.field` with no wrapper. Two are edit targets: [`SeqView`] (Vec-like,
// unbounded) and [`OptView`] (Option-like, ≤1). Two are descent-only: [`MapView`], because a map holds
// the node in its VALUE slot, which no positional view can address; and [`Slot`], because a
// transparent wrapper always holds exactly one node and can neither be emptied nor filled.
//
// The element type is a **type parameter** (`SeqView<T>`, not an associated type); the traits are
// bare-element only — a wrapper like `Box<T>`/`Attempt<T>` is a [`Slot`] instead, and the visitor
// descends *through* wrapped shapes by recursing per layer, not via any wrapped-element impl.

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

/// A **map-like** view of an AST map field (`HashMap`/`BTreeMap`): the node sits in the VALUE slot, so
/// the viewed element is the map's value type rather than its first type argument. Descent-only — a map
/// slot is keyed, so there is no positional structural edit and hence no `#[seq]`/`#[opt]` counterpart;
/// it exists so a `View` level resolves `view_iter[_mut]()` on a map just as it does on a
/// [`SeqView`]/[`OptView`] container, still without the macro naming any container type.
pub trait MapView<T> {
    fn view_iter<'a>(&'a self) -> impl Iterator<Item = &'a T>
    where
        T: 'a;
    /// Iterate the values by `&mut` for in-place edits (keys are untouched).
    fn view_iter_mut<'a>(&'a mut self) -> impl Iterator<Item = &'a mut T>
    where
        T: 'a;
}

impl<K, V, S> MapView<V> for std::collections::HashMap<K, V, S> {
    fn view_iter<'a>(&'a self) -> impl Iterator<Item = &'a V>
    where
        V: 'a,
    {
        self.values()
    }
    fn view_iter_mut<'a>(&'a mut self) -> impl Iterator<Item = &'a mut V>
    where
        V: 'a,
    {
        self.values_mut()
    }
}

impl<K, V> MapView<V> for std::collections::BTreeMap<K, V> {
    fn view_iter<'a>(&'a self) -> impl Iterator<Item = &'a V>
    where
        V: 'a,
    {
        self.values()
    }
    fn view_iter_mut<'a>(&'a mut self) -> impl Iterator<Item = &'a mut V>
    where
        V: 'a,
    {
        self.values_mut()
    }
}

/// A mutable, **Option-like** view (≤1 element) of an AST `Option` field, bare-element (a nested
/// `Box`/`Attempt` layer descends separately, as a [`Slot`]). A generated
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
// adapter). A transparent single-slot wrapper (`Box<T>`/`Attempt<T>`/user wrappers) is reached by
// [`Slot`] instead, so the visitor descends *through* it uniformly via `view_iter_mut`, recursing
// per level. Such a slot can be neither emptied nor filled, so it is descent-only — never a
// `#[seq]`/`#[opt]` edit target.

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

/// A **transparent single-slot** wrapper: one that holds exactly one value, such as `Box<T>` or
/// [`Attempt<T>`](crate::nested::Attempt). A consumer's own wrapper joins the walk with one impl.
///
/// Two roles, both served by the same impl. On an AST field it is the fourth container shape
/// alongside [`SeqView`], [`OptView`] and [`MapView`] — descent-only, since a fixed slot can be
/// neither emptied nor filled. On a *visitor* it lets a wrapped visitor stand in for the visitor
/// itself, the way `&mut V` already does.
///
/// Deliberately **not** blanket-implemented over [`Deref`](core::ops::Deref): std's
/// `impl<T: ?Sized> Deref for &T` would make every shared reference a `Slot` holding its referent,
/// so a field whose container has no view impl would resolve `view_iter` to the reference itself and
/// fail with a baffling type mismatch instead of "no method named `view_iter`". No reference type is a
/// `Slot`.
pub trait Slot {
    /// The value in the slot.
    type Target: ?Sized;
    /// Borrow the value.
    fn get(&self) -> &Self::Target;
    /// Iterate the value by shared ref — always exactly one. Mirrors [`SeqView::view_iter`].
    fn view_iter(&self) -> core::iter::Once<&Self::Target> {
        core::iter::once(self.get())
    }
}

/// The `&mut` half of [`Slot`].
pub trait SlotMut: Slot {
    /// Borrow the value mutably.
    fn get_mut(&mut self) -> &mut Self::Target;
    /// Iterate the value by `&mut` — always exactly one. Mirrors [`SeqView::view_iter_mut`].
    fn view_iter_mut(&mut self) -> core::iter::Once<&mut Self::Target> {
        core::iter::once(self.get_mut())
    }
}

/// Adapts a visitor held in a [`Slot`] — `Box<MyPass>`, an [`Attempt`](crate::nested::Attempt), or a
/// consumer's own wrapper — to a `visitor!`-generated visitor trait, so `node.visit(wrapped)` works.
///
/// Lives here rather than in the generated module so that an *extending* visitor
/// (`visitor!(base => ..)`) and its base can each implement their own trait for the same type; a type
/// minted per module could not satisfy the base trait as a supertrait.
pub struct SlotDriver<S>(pub S);

/// Marker selecting the [`SlotDriver`] `IntoVisitor` impl, distinguishing it from the closure and
/// tuple-of-closures impls.
pub struct WrappedVisitor;

impl<T: ?Sized> Slot for Box<T> {
    type Target = T;
    fn get(&self) -> &T {
        self
    }
}

impl<T: ?Sized> SlotMut for Box<T> {
    fn get_mut(&mut self) -> &mut T {
        self
    }
}

impl<T> Slot for crate::nested::Attempt<T> {
    type Target = T;
    fn get(&self) -> &T {
        &self.0
    }
}

impl<T> SlotMut for crate::nested::Attempt<T> {
    fn get_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

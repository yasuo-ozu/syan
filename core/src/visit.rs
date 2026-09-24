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
/// impl<__V: Visit> Visit for Box<__V> { .. }                     // node.visit(Box::new(pass))
///
/// // the descent side: this node hands itself to the visitor
/// impl<__V: Visit> syan::visit::Walk<__SyanWalkTag, __V> for super::Expr {
///     fn walk(&self, v: &mut __V) { v.visit_expr(self) }
/// }
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
/// `Box` around a visitor is also a visitor, so `node.visit(Box::new(pass))` works — taken by value.
/// A wrapper of your own forwards in one line, the same way `Box` does.
///
/// A `#[seq]` field adds `visit_<type>_seq`, and `#[opt]` adds `visit_<type>_opt`. Both are on
/// `VisitMut` only, and both hand you a view of the *parent slot* — a [`SeqView`] or [`OptView`] —
/// so an override can `push`, `remove` or `retain_mut` rather than only read each element. The
/// default just descends. The marked field must be a bare `Vec<T>` or `Option<T>`: a wrapped one
/// such as `Option<Box<T>>` cannot be edited in place, and the macro says so.
///
/// The walk never names a container type, or a leaf type. Each followed field is one [`Walk`] call
/// carrying an [`indicator`] computed from the field's shape — `Vec<(Length, Line)>` is walked at
/// `Thru<(Skip, Here)>` — and the container impls in this module peel it one level at a time until a
/// node's generated impl hands the node to its `visit_*` method. So `Box<T>` and `Vec<T>` generate
/// the same code, and a leaf sharing a tuple with a node is simply `Skip`, needing no impl of its own.
///
/// Every listed type needs `#[derive(Ast)]`: the macro reads its shape from the metadata that
/// derive emits. Generated names come from a type's last path segment, so two listed types ending
/// in the same ident are rejected.
///
/// A type may be listed in only one `visitor!` per crate: each listing gives it an inherent
/// `visit`/`visit_mut`, so a second is `E0592: duplicate definitions`. To widen a visitor, extend it
/// with `visitor!(base => More)` rather than writing a second one.
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
// the node in its VALUE slot, which no positional view can address.
//
// These are **edit** views only: the descent itself goes through [`Walk`]. The element type is a
// **type parameter** (`SeqView<T>`, not an associated type), and the traits are bare-element — a
// `#[seq]`/`#[opt]` field must be a bare `Vec<T>`/`Option<T>`, never a wrapped one.

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
/// `Box`/`Attempt` layer descends separately, through [`Walk`]). A generated
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
// [`Walk`] instead. Such a slot can be neither emptied nor filled, so it is descent-only — never a
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

/// Which parts of a value a walk descends into, as a type.
///
/// `visitor!` computes one of these per field from the field's shape and spells it at the call site,
/// so the descent never names a leaf type or matches a container by name. The language is small:
///
/// | indicator | meaning |
/// |---|---|
/// | [`Here`](indicator::Here) | `Self` is a visited node — hand it to the visitor |
/// | [`Skip`](indicator::Skip) | do not descend |
/// | [`Thru<H>`](indicator::Thru) | `Self` is a container — delegate `H` to what it holds |
/// | `(H0, .., Hn)` | `Self` is a tuple — delegate each `Hi` to slot `i` |
///
/// So `Vec<(Length, Line)>` is walked at `Thru<(Skip, Here)>`: through the `Vec`, past the `Length`,
/// into the `Line`.
pub mod indicator {
    /// `Self` is a visited node: hand it to the visitor.
    pub struct Here;
    /// Do not descend into `Self`.
    pub struct Skip;
    /// `Self` is a container: delegate `H` to the value(s) it holds.
    pub struct Thru<H>(core::marker::PhantomData<H>);
}

use indicator::{Skip, Thru};

/// How an AST value hands itself to a visitor — the descent half of the walk, where the trait
/// `visitor!` generates is the dispatch half.
///
/// `V` is the visitor. `M` is the tag minted by the `visitor!` module the walk belongs to, so two
/// visitor modules over the same node do not write conflicting impls. `H` is the [`indicator`]:
/// which parts of `Self` to descend into.
///
/// `H` is what lets the container impls below stay generic in the types they hold — `(Length, Line)`
/// and `(Line, Line)` are the *same* impl at different indicators — so no leaf type is ever named and
/// no shape needs an impl of its own.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be walked at `{H}`",
    label = "this field's shape has no `Walk` impl",
    note = "a visited node needs `#[derive(Ast)]` and an entry in the owning type's `#[subast(..)]`; \
            a container of one needs a `Walk` impl, which `syan::visit` has for `Vec`, `VecDeque`, \
            `Punctuated`, `Option`, `Box`, `Attempt`, maps, slices, arrays and tuples"
)]
pub trait Walk<M, H, V: ?Sized> {
    /// Offer the parts of `self` that `H` selects to `v`.
    fn walk(&self, v: &mut V);
}

/// The `&mut` half of [`Walk`].
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be walked mutably at `{H}`",
    label = "this field's shape has no `WalkMut` impl",
    note = "a field behind a shared `&` is walkable on the shared side only"
)]
pub trait WalkMut<M, H, V: ?Sized> {
    /// Offer the parts of `self` that `H` selects to `v`, by `&mut`.
    fn walk_mut(&mut self, v: &mut V);
}

// `Skip` descends nowhere, whatever it is applied to. A blanket over every type — but at one fixed
// indicator, so it overlaps nothing else. This is what frees a leaf type from having to say anything
// about itself, even when it shares a tuple with a node.
impl<M, V: ?Sized, T: ?Sized> Walk<M, Skip, V> for T {
    fn walk(&self, _: &mut V) {}
}

impl<M, V: ?Sized, T: ?Sized> WalkMut<M, Skip, V> for T {
    fn walk_mut(&mut self, _: &mut V) {}
}

macro_rules! walk_seq {
    ($($ty:ty $(where $($p:ident),+)?),* $(,)?) => {
        $(
            impl<M, H, V: ?Sized, T: Walk<M, H, V> $(, $($p),+)?> Walk<M, Thru<H>, V> for $ty {
                fn walk(&self, v: &mut V) {
                    for x in self.iter() {
                        x.walk(v);
                    }
                }
            }
            impl<M, H, V: ?Sized, T: WalkMut<M, H, V> $(, $($p),+)?> WalkMut<M, Thru<H>, V> for $ty {
                fn walk_mut(&mut self, v: &mut V) {
                    for x in self.iter_mut() {
                        x.walk_mut(v);
                    }
                }
            }
        )*
    };
}

walk_seq!(Vec<T>, [T], std::collections::VecDeque<T>, crate::nested::Punctuated<T, P> where P);

impl<M, H, V: ?Sized, T: Walk<M, H, V>, const N: usize> Walk<M, Thru<H>, V> for [T; N] {
    fn walk(&self, v: &mut V) {
        for x in self.iter() {
            x.walk(v);
        }
    }
}

impl<M, H, V: ?Sized, T: WalkMut<M, H, V>, const N: usize> WalkMut<M, Thru<H>, V> for [T; N] {
    fn walk_mut(&mut self, v: &mut V) {
        for x in self.iter_mut() {
            x.walk_mut(v);
        }
    }
}

impl<M, H, V: ?Sized, T: Walk<M, H, V>> Walk<M, Thru<H>, V> for Option<T> {
    fn walk(&self, v: &mut V) {
        if let Some(x) = self {
            x.walk(v);
        }
    }
}

impl<M, H, V: ?Sized, T: WalkMut<M, H, V>> WalkMut<M, Thru<H>, V> for Option<T> {
    fn walk_mut(&mut self, v: &mut V) {
        if let Some(x) = self {
            x.walk_mut(v);
        }
    }
}

// A map holds the node in its VALUE slot, so only the values are walked.
macro_rules! walk_map {
    ($($ty:ty),* $(,)?) => {
        $(
            impl<M, H, V: ?Sized, K, T: Walk<M, H, V>> Walk<M, Thru<H>, V> for $ty {
                fn walk(&self, v: &mut V) {
                    for x in self.values() {
                        x.walk(v);
                    }
                }
            }
            impl<M, H, V: ?Sized, K, T: WalkMut<M, H, V>> WalkMut<M, Thru<H>, V> for $ty {
                fn walk_mut(&mut self, v: &mut V) {
                    for x in self.values_mut() {
                        x.walk_mut(v);
                    }
                }
            }
        )*
    };
}

walk_map!(std::collections::HashMap<K, T>, std::collections::BTreeMap<K, T>);

// A field behind a shared reference is walkable on the shared side only — there is no `&mut` to be had
// through a `&`, so there is deliberately no `WalkMut` twin.
impl<M, H, V: ?Sized, T: Walk<M, H, V> + ?Sized> Walk<M, Thru<H>, V> for &T {
    fn walk(&self, v: &mut V) {
        (**self).walk(v);
    }
}

/// Transparent single-slot wrappers. A consumer's own wrapper joins the same way:
///
/// ```
/// # use syan::visit::{Walk, WalkMut, indicator::Thru};
/// struct MyPtr<T>(Box<T>);
/// impl<M, H, V: ?Sized, T: Walk<M, H, V>> Walk<M, Thru<H>, V> for MyPtr<T> {
///     fn walk(&self, v: &mut V) { <T as Walk<M, H, V>>::walk(&self.0, v) }
/// }
/// impl<M, H, V: ?Sized, T: WalkMut<M, H, V>> WalkMut<M, Thru<H>, V> for MyPtr<T> {
///     fn walk_mut(&mut self, v: &mut V) { <T as WalkMut<M, H, V>>::walk_mut(&mut self.0, v) }
/// }
/// ```
impl<M, H, V: ?Sized, T: Walk<M, H, V> + ?Sized> Walk<M, Thru<H>, V> for Box<T> {
    fn walk(&self, v: &mut V) {
        (**self).walk(v);
    }
}

impl<M, H, V: ?Sized, T: WalkMut<M, H, V> + ?Sized> WalkMut<M, Thru<H>, V> for Box<T> {
    fn walk_mut(&mut self, v: &mut V) {
        (**self).walk_mut(v);
    }
}

impl<M, H, V: ?Sized, T: Walk<M, H, V>> Walk<M, Thru<H>, V> for crate::nested::Attempt<T> {
    fn walk(&self, v: &mut V) {
        self.0.walk(v);
    }
}

impl<M, H, V: ?Sized, T: WalkMut<M, H, V>> WalkMut<M, Thru<H>, V> for crate::nested::Attempt<T> {
    fn walk_mut(&mut self, v: &mut V) {
        self.0.walk_mut(v);
    }
}

// One impl per arity, fully generic in the slot types: a slot's indicator decides whether it is walked,
// so `(Length, Line)` and `(Line, Line)` are this same impl at `(Skip, Here)` and `(Here, Here)`.
macro_rules! walk_tuple {
    ($(($($p:ident . $h:ident . $i:tt),+),)*) => {
        $(
            impl<M, V: ?Sized, $($h,)+ $($p: Walk<M, $h, V>),+> Walk<M, ($($h,)+), V> for ($($p,)+) {
                fn walk(&self, v: &mut V) {
                    $( self.$i.walk(v); )+
                }
            }
            impl<M, V: ?Sized, $($h,)+ $($p: WalkMut<M, $h, V>),+> WalkMut<M, ($($h,)+), V> for ($($p,)+) {
                fn walk_mut(&mut self, v: &mut V) {
                    $( self.$i.walk_mut(v); )+
                }
            }
        )*
    };
}

walk_tuple! {
    (A.HA.0),
    (A.HA.0, B.HB.1),
    (A.HA.0, B.HB.1, C.HC.2),
    (A.HA.0, B.HB.1, C.HC.2, D.HD.3),
    (A.HA.0, B.HB.1, C.HC.2, D.HD.3, E.HE.4),
    (A.HA.0, B.HB.1, C.HC.2, D.HD.3, E.HE.4, F.HF.5),
    (A.HA.0, B.HB.1, C.HC.2, D.HD.3, E.HE.4, F.HF.5, G.HG.6),
    (A.HA.0, B.HB.1, C.HC.2, D.HD.3, E.HE.4, F.HF.5, G.HG.6, I.HI.7),
    (A.HA.0, B.HB.1, C.HC.2, D.HD.3, E.HE.4, F.HF.5, G.HG.6, I.HI.7, J.HJ.8),
    (A.HA.0, B.HB.1, C.HC.2, D.HD.3, E.HE.4, F.HF.5, G.HG.6, I.HI.7, J.HJ.8, K.HK.9),
    (A.HA.0, B.HB.1, C.HC.2, D.HD.3, E.HE.4, F.HF.5, G.HG.6, I.HI.7, J.HJ.8, K.HK.9, L.HL.10),
    (A.HA.0, B.HB.1, C.HC.2, D.HD.3, E.HE.4, F.HF.5, G.HG.6, I.HI.7, J.HJ.8, K.HK.9, L.HL.10, N.HN.11),
}

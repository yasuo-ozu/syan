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

/// Marks a type as an AST node and emits the metadata [`visitor!`] reads.
///
/// # Types are matched by their last path segment
///
/// **This is the main rule to remember.** A field's type is known by the last part of its path.
/// Nothing else is read. So `other::Node`, `crate::ast::Node` and plain `Node` all mean the same
/// `Node` here. This derive sees one type at a time. It cannot look a path up.
///
/// If two types share a name, give one an alias and write the alias in the field:
///
/// ```ignore
/// #[derive(Ast)]
/// #[subast(crate::other::Node as OtherNode)]
/// pub struct Wrapper {
///     inner: OtherNode,
/// }
/// ```
///
/// # Attributes
///
/// | attribute | on | meaning |
/// |---|---|---|
/// | `#[subast(path, ..)]` | the type | types this node reaches that `visitor!(..)` does **not** list, so the walk can pass through them. A listed type needs no entry. |
/// | `#[seq]` / `#[opt]` | a field | reach the parent slot through a [`SeqView`] or [`OptView`]. Plain `Vec<T>` / `Option<T>` only. |
/// | `#[skip]` | a field | never walk into this field, whatever its type. An error together with `#[seq]` or `#[opt]`. |
///
/// Take a field's type and remove wrappers like `Box`, `Vec` and `Option`. What is left is the
/// **head**. The walk goes into a field when the visitor knows its head. It knows a type if you
/// listed it in `visitor!(..)`, or if it comes from a base visitor. Those need no `#[subast(..)]`
/// entry. That list is only for types the walk passes *through* on the way to a known one.
///
/// A `#[subast(..)]` path must start at a crate: `crate::path::to::Type`, or
/// `other_crate::path::Type` for a type in another crate. A plain name, or a `self::`/`super::`
/// path, is an error. Here is why. This derive puts the path inside a macro. That macro runs later,
/// in the module where the visitor lives. A short path would mean something else there.
/// `visitor!(..)` has no such rule. Only `#[subast(..)]` does.
///
/// A `#[seq]` or `#[opt]` field must be a plain `Vec<T>` or `Option<T>`. A wrapped one, like
/// `Option<Box<T>>`, cannot be edited in place. The macro tells you so, instead of building a view
/// that would not work.
///
/// `#[skip]` takes a field out of the walk, whatever its type:
///
/// ```ignore
/// #[derive(Ast)]
/// pub enum Expr<S> {
///     // `&mut Box<Stmt<'_, S>>` is invariant, so a mut walk can never descend into a concrete
///     // lifetime fill — say so here rather than leaving it to an error inside the macro.
///     Stmt(#[skip] Box<Stmt<'static, S>>),
///     Lit(PhantomData<S>),
/// }
/// ```
pub use syan_macro::Ast;

/// Define a visitor over the given AST types. Put it in its own empty module:
///
/// ```ignore
/// pub mod my_visitor {
///     syan::visit::visitor!(Type, Expr);          // or: visitor!(super::base => Stmt);
/// }
/// ```
///
/// # What it generates
///
/// You get everything below twice. One half is for reading. The other is for editing: it adds a
/// `_mut` suffix, and takes `&mut` where the reading half takes `&`.
///
/// * **`Visit`** — a trait with one `visit_<type>` method per listed type. Each method already has
///   a body that walks that node's children. Override only the nodes you care about.
/// * **`visit_<type>`** — a free function that walks one node's children. The trait method is where
///   you hook in. This function does the walking. Call it from your override to keep going down.
/// * **`IntoVisitor`** — what `visit` accepts. See
///   [What you can pass to `visit()`](#what-you-can-pass-to-visit).
/// * **`visit`** — a method on each listed type, so a walk starts with `node.visit(..)`.
///
/// A few hidden items come along too. You never name them.
///
/// # Generated names
///
/// For a listed type `T`, written `t` in snake_case:
///
/// | | reading (`Visit`) | editing (`VisitMut`) |
/// |---|---|---|
/// | the node | `visit_t` | `visit_t_mut` |
/// | a `#[seq]` field | `visit_t_seq` | `visit_t_seq_mut` |
/// | a `#[opt]` field | `visit_t_opt` | `visit_t_opt_mut` |
/// | closure hook | `hook_t` | `hook_t_mut` |
/// | entry point | `T::visit` | `T::visit_mut` |
///
/// A `_seq` or `_opt` method gives you the whole slot, not one element. When reading, you get a
/// `&impl SeqView<T>`. It shows you what `visit_t` cannot: how many elements there are, what sits
/// next to an element, which index it has. When editing, you get a `&mut`, so you can `push`,
/// `remove` or `retain_mut`. Both already have a body that just walks on.
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
///     // `#[seq]` adds `visit_expr_seq_mut`, which hands you the parent slot, not just the element.
///     struct DropLits;
///     impl visit::VisitMut for DropLits {
///         fn visit_expr_seq_mut<V: SeqView<Expr>>(&mut self, v: &mut V) {
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
/// Here are two small nodes. One has a `#[seq]` list and an `#[opt]` slot:
///
/// ```ignore
/// #[derive(Ast)]
/// pub struct Doc {
///     #[seq] items: Vec<Item>,
///     #[opt] footer: Option<Item>,
/// }
///
/// #[derive(Ast)]
/// pub struct Item(u32);
///
/// pub mod visit { syan::visit::visitor!(crate::Doc, crate::Item); }
/// ```
///
/// That module is about a thousand lines. Here are the parts that matter:
///
/// ```ignore
/// pub trait Visit {
///     fn visit_doc(&mut self, i: &crate::Doc) { visit_doc(self, i) }
///     fn visit_item(&mut self, i: &crate::Item) { visit_item(self, i) }
///
///     // One method per marker. It takes the parent slot, not an element.
///     fn visit_item_seq<__VW: SeqView<crate::Item>>(&mut self, v: &__VW) {
///         for e in SeqView::view_iter(v) { self.visit_item(e); }
///     }
///     fn visit_item_opt<__OW: OptView<crate::Item>>(&mut self, v: &__OW) {
///         if let Some(e) = OptView::get(v) { self.visit_item(e); }
///     }
/// }
///
/// pub fn visit_doc<__V: Visit + ?Sized>(this: &mut __V, i: &crate::Doc) {
///     let crate::Doc { items, footer, .. } = i;
///     this.visit_item_seq(items);
///     this.visit_item_opt(footer);
/// }
///
/// pub fn visit_item<__V: Visit + ?Sized>(this: &mut __V, i: &crate::Item) {}
///
/// // A visitor behind a wrapper is still a visitor. So is a tuple of them.
/// impl<__V: Visit> Visit for &mut __V { .. }                       // node.visit(&mut pass)
/// impl<__V: Visit + ?Sized> Visit for Box<__V> { .. }              // node.visit(Box::new(pass))
/// impl<__V0: Visit, __V1: Visit> Visit for (__V0, __V1) { .. }     // two passes, one walk
///
/// // What makes a closure a visitor. These names are hidden; you never write them.
/// impl<__F: FnMut(&crate::Item)> IntoVisitor<crate::Item> for __F {
///     fn into_visitor(self) -> impl Visit { .. }
/// }
///
/// // The entry points, one per listed type.
/// impl crate::Doc {
///     pub fn visit<__T>(&self, visitor: impl IntoVisitor<__T>) -> &Self {
///         let mut visitor = visitor.into_visitor();
///         visitor.visit_doc(self);
///         self
///     }
/// }
/// impl crate::Item {
///     pub fn visit<__T>(&self, visitor: impl IntoVisitor<__T>) -> &Self {
///         let mut visitor = visitor.into_visitor();
///         visitor.visit_item(self);
///         self
///     }
/// }
///
/// // `VisitMut` is the same again, with `_mut` names and `&mut` everywhere. That is how the
/// // views edit the slot instead of only reading it.
/// pub trait VisitMut {
///     fn visit_doc_mut(&mut self, i: &mut crate::Doc) { visit_doc_mut(self, i) }
///     fn visit_item_mut(&mut self, i: &mut crate::Item) { visit_item_mut(self, i) }
///
///     fn visit_item_seq_mut<__VW: SeqView<crate::Item>>(&mut self, v: &mut __VW) {
///         for e in SeqView::view_iter_mut(v) { self.visit_item_mut(e); }
///     }
///     fn visit_item_opt_mut<__OW: OptView<crate::Item>>(&mut self, v: &mut __OW) {
///         if let Some(e) = OptView::get_mut(v) { self.visit_item_mut(e); }
///     }
/// }
///
/// pub fn visit_doc_mut<__V: VisitMut + ?Sized>(this: &mut __V, i: &mut crate::Doc) {
///     let crate::Doc { items, footer, .. } = i;
///     this.visit_item_seq_mut(items);
///     this.visit_item_opt_mut(footer);
/// }
///
/// pub fn visit_item_mut<__V: VisitMut + ?Sized>(this: &mut __V, i: &mut crate::Item) {}
///
/// impl crate::Doc {
///     pub fn visit_mut<__T>(&mut self, visitor: impl IntoVisitorMut<__T>) -> &mut Self {
///         let mut visitor = visitor.into_visitor_mut();
///         visitor.visit_doc_mut(self);
///         self
///     }
/// }
/// impl crate::Item {
///     pub fn visit_mut<__T>(&mut self, visitor: impl IntoVisitorMut<__T>) -> &mut Self {
///         let mut visitor = visitor.into_visitor_mut();
///         visitor.visit_item_mut(self);
///         self
///     }
/// }
///
/// // .. and the `&mut` versions of the wrapper, tuple and closure impls above.
/// ```
///
/// `visit_item` has an empty body. `u32` is not a visited type, so there is nothing inside an
/// `Item` to walk into.
///
/// `Doc`'s two fields differ only in their marker. Take the markers away and both fields give the
/// *same* code. A `Vec<Item>` and an `Option<Item>` are walked the same way, and no container type
/// is ever named.
///
/// Four traits are public: `Visit`, `VisitMut`, `IntoVisitor` and `IntoVisitorMut`. The rest is
/// hidden.
///
/// # Extending a visitor
///
/// You can list a type in only one `visitor!` per crate. So to cover more types, add to the
/// visitor you have. Write the base module, then `=>`, then the new types:
///
/// ```ignore
/// #[derive(Ast)] pub enum Type<S> { .. }
/// #[derive(Ast)] pub enum Expr<S> { Typed(Box<Type<S>>), .. }
/// #[derive(Ast)] pub enum Stmt<S> { E(Box<Expr<S>>), .. }
///
/// pub mod base { syan::visit::visitor!(super::Type, super::Expr); }
/// pub mod ext  { syan::visit::visitor!(super::base => super::Stmt); }
/// ```
///
/// `base::Visit` is a supertrait of `ext::Visit`. So your visitor implements both traits, and has
/// methods for all three types:
///
/// ```ignore
/// impl<S> base::Visit<S> for Counter {
///     fn visit_expr(&mut self, i: &Expr<S>) { self.exprs += 1; base::visit_expr(self, i); }
/// }
/// impl<S> ext::Visit<S> for Counter {
///     fn visit_stmt(&mut self, i: &Stmt<S>) { self.stmts += 1; ext::visit_stmt(self, i); }
/// }
///
/// stmt.visit(&mut counter);   // walks Stmt, then Expr, then Type
/// ```
///
/// A field holding one of the base's types needs no `#[subast(..)]` entry. `ext` asks the base for
/// its list. Those nodes go to the base's `visit_*`, through the supertrait.
///
/// Chains can be as long as you like. `base => mid => new` works, and a visitor for `new`
/// implements every trait in the chain. Closures still work at any depth. The new visitor can also
/// have more generic parameters than its base.
///
/// There are two limits. First, you must be able to name the base module from where you add to it.
/// Second, a type from the base gets no `#[seq]`/`#[opt]` view, because its `visit_*` method lives
/// in the base, not here. Mark that field in the base's own `visitor!` instead, or drop the marker
/// and let the walk take the elements one at a time.
///
/// # What you can pass to `visit()`
///
/// `node.visit(..)` takes anything that implements `IntoVisitor`. The macro implements it for five
/// things:
///
/// * **a visitor** — any type that implements `Visit`, by value or by `&mut`.
/// * **a closure** that takes `&T`, for one listed type `T`. The argument type picks which node it
///   sees. It runs for every `T` in the tree.
/// * **a tuple of 2 to 8 closures**. All of them run in one walk.
/// * **a tuple of 2 to 8 visitors**. The same: each one sees every node.
/// * **a `Box` around a visitor**, by value.
///
/// ```ignore
/// node.visit(&mut pass);                          // a visitor
/// node.visit(Box::new(pass));                     // boxed
/// node.visit(|e: &Expr| exprs += 1);              // one closure
/// node.visit((|e: &Expr| .., |t: &Type| ..));     // two closures, one walk
/// node.visit(&mut (first, second));               // two visitors, one walk
/// ```
///
/// A tuple must be all closures or all visitors. You cannot mix the two in one tuple. If you need
/// both, call `visit` twice, or wrap the closure in a visitor of your own.
///
/// A type of your own becomes a visitor as soon as it forwards `Visit`, which takes one line. It
/// then goes anywhere `Box` goes.
///
/// `visit_mut` takes the same five things through `IntoVisitorMut`. Its closures take `&mut T`.
///
/// # Rules
///
/// Every listed type needs `#[derive(Ast)]`. That derive writes the metadata this macro reads.
///
/// A type is known by the last segment of its path (see [`Ast`]). The generated names come from
/// that segment too, so two listed types ending in the same name are rejected.
///
/// You can list a type in only one `visitor!` per crate. Each listing gives it its own
/// `visit`/`visit_mut` method, so a second one is `E0592: duplicate definitions`. See
/// [Extending a visitor](#extending-a-visitor).
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
/// `visit_<t>_seq_mut(&mut self, &mut impl SeqView<T>)` receives one (and `visit_<t>_seq` a `&`); override it to edit the collection in
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
/// slot is keyed, so there is no positional structural edit and hence no `#[seq]`/`#[opt]` counterpart.
///
/// **No longer part of the walk.** A map field descends through [`Walk`], whose `Thru<H>` impl for
/// `HashMap`/`BTreeMap` iterates the values directly. This trait is kept as a way to write that
/// traversal by hand; nothing `visitor!` generates refers to it.
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
/// `visit_<t>_opt_mut(&mut self, &mut impl OptView<T>)` receives one, and `visit_<t>_opt` a `&`.
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
/// | [`Skip`] | do not descend |
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

// A delimited group walks its content and not its delimiters: `O` and `C` are punctuation tokens,
// never AST nodes. Descent-only like every other single-slot wrapper — a `Group` holds exactly one
// `T`, so there is nothing for `SeqView`'s `push`/`remove` or `OptView`'s `take` to mean, and
// `#[seq]`/`#[opt]` on such a field stays the error it already was.
impl<M, H, V: ?Sized, T: Walk<M, H, V>, O, C> Walk<M, Thru<H>, V>
    for crate::nested::group::Group<T, O, C>
{
    fn walk(&self, v: &mut V) {
        self.slot.walk(v);
    }
}

impl<M, H, V: ?Sized, T: WalkMut<M, H, V>, O, C> WalkMut<M, Thru<H>, V>
    for crate::nested::group::Group<T, O, C>
{
    fn walk_mut(&mut self, v: &mut V) {
        self.slot.walk_mut(v);
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

//! Multi-level inheritance across the crate boundary with an **upstream intermediate**:
//! `base => mid => new`, where BOTH `base` and `mid` live in the upstream `syan_rust` library and
//! only `new` is built here (downstream). This is the realistic shape — a library ships a base
//! visitor and a richer intermediate, and a downstream crate extends the intermediate.
//!
//! The subtlety this exercises: `mid` (upstream) recorded its ancestor `base` as the
//! `crate::inherit::base` path it was given — *relative to `mid`'s own crate*. A downstream `new`
//! must still satisfy the transitive `base::Visit` supertrait, so it must emit
//! `impl syan_rust::inherit::base::Visit for new::Driver`. `$crate` cannot carry that path (emitted
//! by a proc-macro into a generated `macro_rules` it resolves only for fetch paths, not for a trait
//! path in final code), so the macro instead *requalifies* the `crate::`-relative ancestor against
//! the direct base's host crate (`syan_rust`, taken from the `syan_rust::inherit::mid` path `new`
//! was given) — making it concrete and resolvable downstream.
//!
//! The visitor descends `File -> Item (mid) -> Expr (base) -> Type (base)`.

use core::marker::PhantomData;
use syan::visit::Ast;
use syan_rust::inherit::{Expr, Item, Type};

// A downstream top-level node referencing the UPSTREAM `Item`.
#[derive(Debug, Ast)]
#[subast(syan_rust::inherit::Item)]
pub enum File<S> {
    It(Box<Item<S>>),
    Empty(PhantomData<S>),
}

// `new` inherits the UPSTREAM `mid` (which transitively carries the UPSTREAM `base`).
pub mod new {
    syan::visit::visitor!(syan_rust::inherit::mid => crate::File);
}

#[derive(Default)]
struct Counter {
    types: u32,
    exprs: u32,
    items: u32,
    files: u32,
    wides: u32,
}

// Base trait — over the upstream `Type`/`Expr`.
impl<S> syan_rust::inherit::base::Visit<S> for Counter {
    fn visit_type(&mut self, i: &Type<S>) {
        self.types += 1;
        syan_rust::inherit::base::visit_type(self, i);
    }
    fn visit_expr(&mut self, i: &Expr<S>) {
        self.exprs += 1;
        syan_rust::inherit::base::visit_expr(self, i);
    }
}

// Mid trait — upstream, supertrait of base.
impl<S> syan_rust::inherit::mid::Visit<S> for Counter {
    fn visit_item(&mut self, i: &Item<S>) {
        self.items += 1;
        syan_rust::inherit::mid::visit_item(self, i);
    }
}

// New trait — downstream, supertrait of mid (transitively base).
impl<S> new::Visit<S> for Counter {
    fn visit_file(&mut self, i: &File<S>) {
        self.files += 1;
        new::visit_file(self, i);
    }
}

#[test]
fn multilevel_inheritance_through_upstream_intermediate() {
    // File::It -> Item::Ex -> Expr::Typed -> Type::Unit
    let ast: File<()> = File::It(Box::new(Item::Ex(Box::new(Expr::Typed(Box::new(
        Type::Unit(PhantomData),
    ))))));
    let mut counter = Counter::default();
    ast.visit(&mut counter);
    assert_eq!(counter.files, 1, "the new (downstream) node");
    assert_eq!(counter.items, 1, "inherited mid method (upstream)");
    assert_eq!(counter.exprs, 1, "transitively inherited base method (upstream, requalified)");
    assert_eq!(counter.types, 1, "transitively inherited base method (upstream, requalified)");
}

// ── Cross-crate multi-level inheritance WITH arity widening at the leaf ───────────────────────────
// `base`/`mid` are arity-1 (`Visit<S>`); the downstream `File2<S, T>` adds a second param `T`, so
// `new2::Visit<S, T>` extends the *requalified* upstream `mid::Visit<S>` / `base::Visit<S>` with the
// base's own arity (S only) on an arity-2 `Driver` — the requalified ancestor path and the
// per-ancestor generic-param subset must both be right at once (a leak of `T` into the base
// obligation would be E0107/E0207).
#[derive(Debug, Ast)]
#[subast(syan_rust::inherit::Item)]
pub enum File2<S, T> {
    It(Box<Item<S>>),
    Tag(PhantomData<T>),
}

pub mod new2 {
    syan::visit::visitor!(syan_rust::inherit::mid => crate::File2);
}

impl<S, T> new2::Visit<S, T> for Counter {
    fn visit_file2(&mut self, i: &File2<S, T>) {
        self.wides += 1;
        new2::visit_file2(self, i);
    }
}

#[test]
fn multilevel_upstream_intermediate_with_wider_arity() {
    // File2::<(), u8>::It -> Item::Ex -> Expr::Typed -> Type::Unit
    let ast: File2<(), u8> = File2::It(Box::new(Item::Ex(Box::new(Expr::Typed(Box::new(
        Type::Unit(PhantomData),
    ))))));
    let mut counter = Counter::default();
    ast.visit(&mut counter);
    assert_eq!(counter.wides, 1, "the new arity-2 node");
    assert_eq!(counter.items, 1, "inherited mid (base arity 1) through arity-2 leaf");
    assert_eq!(counter.exprs, 1, "requalified base supertrait emitted with base's own arity");
    assert_eq!(counter.types, 1, "requalified base supertrait emitted with base's own arity");
}

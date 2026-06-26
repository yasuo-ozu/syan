//! 3-crate cross-crate inheritance through an upstream intermediate that recorded its ancestor via a
//! **`super::`-relative** path (the previously-residual hole):
//!
//!   `syan` (macro) → `syan-rust` (defines `base` + `mid_ss = visitor!(super::base => ItemSs)`)
//!                  → THIS test crate (`visitor!(syan_rust::inherit::mid_ss => Down)`).
//!
//! `mid_ss` recorded its `base` ancestor as `super::base` — relative to `mid_ss`'s own module
//! upstream. The `__syan_visited` macro replays that to the downstream extender, which must satisfy
//! the transitive `base::Visit` supertrait (`impl … : syan_rust::inherit::base::Visit`). Downstream
//! the macro is *given* `mid_ss`'s full path (`syan_rust::inherit::mid_ss`), and `super::base` was
//! invoked inside that module — so it requalifies `super::base` → `syan_rust::inherit::base` by
//! popping `mid_ss` off the base path (the analogue of the `crate::` requalification that `mid` uses,
//! see `cross_crate_inherit_multilevel.rs`).
//!
//! Descends `Down -> ItemSs (mid_ss) -> Expr (base) -> Type (base)`.

use core::marker::PhantomData;
use syan::visit::Ast;
use syan_rust::inherit::{Expr, ItemSs, Type};

// A downstream top-level node referencing the UPSTREAM `ItemSs`.
#[derive(Debug, Ast)]
#[subast(syan_rust::inherit::ItemSs)]
pub enum Down<S> {
    It(Box<ItemSs<S>>),
    Nil(PhantomData<S>),
}

// `nv` inherits the UPSTREAM `mid_ss`, which recorded its `base` ancestor `super::`-relative.
pub mod nv {
    syan::visit::visitor!(syan_rust::inherit::mid_ss => crate::Down);
}

#[derive(Default)]
struct Counter {
    downs: u32,
    items: u32,
    exprs: u32,
    types: u32,
}

// Base trait — over the upstream `Type`/`Expr` (the `super::`-relative transitive ancestor).
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

// Mid_ss trait — upstream, supertrait of base.
impl<S> syan_rust::inherit::mid_ss::Visit<S> for Counter {
    fn visit_item_ss(&mut self, i: &ItemSs<S>) {
        self.items += 1;
        syan_rust::inherit::mid_ss::visit_item_ss(self, i);
    }
}

// New trait — downstream, supertrait of mid_ss (transitively base).
impl<S> nv::Visit<S> for Counter {
    fn visit_down(&mut self, i: &Down<S>) {
        self.downs += 1;
        nv::visit_down(self, i);
    }
}

#[test]
fn downstream_extends_super_relative_intermediate() {
    // Down::It -> ItemSs::Ex -> Expr::Typed -> Type::Unit
    let ast: Down<()> = Down::It(Box::new(ItemSs::Ex(Box::new(Expr::Typed(Box::new(Type::Unit(
        PhantomData,
    )))))));
    let mut counter = Counter::default();
    ast.visit(&mut counter);
    assert_eq!(counter.downs, 1, "the new (downstream) node");
    assert_eq!(counter.items, 1, "inherited mid_ss method (upstream)");
    assert_eq!(counter.exprs, 1, "transitively inherited base method (upstream, super::-requalified)");
    assert_eq!(counter.types, 1, "transitively inherited base method (upstream, super::-requalified)");
}

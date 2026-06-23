//! Multi-level inheritance where the intermediate is built DOWNSTREAM — the case requalification
//! must correctly leave ALONE. `base` lives upstream (`syan_rust::inherit::base`); both the
//! intermediate `dmid` and the leaf `dnew` are built here.
//!
//! `dnew`'s direct base is `crate::dmid` (a same-crate path), so `base_host_crate` returns `None`
//! and the ancestor loop takes its `a.path.clone()` no-op branch. And `dmid` — built downstream with
//! the concrete `syan_rust::inherit::base` path — records its ancestor `base` *already concrete*, so
//! even the per-ancestor `requalify_ancestor` would decline (no leading bare `crate`). This proves
//! requalification is suppressed exactly when the intermediate is downstream (its ancestor is already
//! resolvable), the complement of `cross_crate_inherit_multilevel.rs`'s upstream-intermediate case.
//!
//! Descends `DItem -> DStmt (dmid) -> Expr (base) -> Type (base)`.

use core::marker::PhantomData;
use syan::visit::Ast;
use syan_rust::inherit::{Expr, Type};

#[derive(Debug, Ast)]
#[subast(syan_rust::inherit::Expr)]
pub enum DStmt<S> {
    E(Box<Expr<S>>),
    Nil(PhantomData<S>),
}

// Intermediate built DOWNSTREAM, inheriting the upstream base (records `base` concretely).
pub mod dmid {
    syan::visit::visitor!(syan_rust::inherit::base => crate::DStmt);
}

#[derive(Debug, Ast)]
#[subast(crate::DStmt)]
pub enum DItem<S> {
    S(Box<DStmt<S>>),
    Nil(PhantomData<S>),
}

// Leaf inheriting the downstream `dmid` (direct base `crate::dmid` => host None => no requalify).
pub mod dnew {
    syan::visit::visitor!(crate::dmid => crate::DItem);
}

#[derive(Default)]
struct Counter {
    types: u32,
    exprs: u32,
    stmts: u32,
    items: u32,
}

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

impl<S> dmid::Visit<S> for Counter {
    fn visit_d_stmt(&mut self, i: &DStmt<S>) {
        self.stmts += 1;
        dmid::visit_d_stmt(self, i);
    }
}

impl<S> dnew::Visit<S> for Counter {
    fn visit_d_item(&mut self, i: &DItem<S>) {
        self.items += 1;
        dnew::visit_d_item(self, i);
    }
}

#[test]
fn downstream_intermediate_leaves_concrete_ancestor_untouched() {
    // DItem::S -> DStmt::E -> Expr::Typed -> Type::Unit
    let ast: DItem<()> = DItem::S(Box::new(DStmt::E(Box::new(Expr::Typed(Box::new(
        Type::Unit(PhantomData),
    ))))));
    let mut counter = Counter::default();
    ast.visit(&mut counter);
    assert_eq!(counter.items, 1, "dnew (downstream leaf)");
    assert_eq!(counter.stmts, 1, "inherited downstream dmid method");
    assert_eq!(counter.exprs, 1, "transitively inherited base (concrete ancestor, no requalify)");
    assert_eq!(counter.types, 1, "transitively inherited base (concrete ancestor, no requalify)");
}

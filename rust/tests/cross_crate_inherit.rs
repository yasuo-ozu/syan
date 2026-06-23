//! Cross-crate visitor **inheritance**: the base visitor (`syan_rust::inherit::base`) and its AST
//! (`Type`, `Expr`) live in the upstream `syan_rust` library; this downstream crate defines a new
//! AST node `Stmt` (which references the upstream `Expr`) and a visitor that *inherits* the upstream
//! base via `visitor!(syan_rust::inherit::base => crate::Stmt)`.
//!
//! Everything inheritance needs is keyed on the base **path**: the supertrait
//! `ext::Visit: syan_rust::inherit::base::Visit`, the inherited free fns
//! `syan_rust::inherit::base::visit_{type,expr}`, and the base's `__syan_visited` macro (exported
//! `#[macro_export]` + `pub use`, so it resolves downstream). So the new visitor descends through
//! its own `Stmt` into the inherited (cross-crate) `Expr` and `Type`.

use core::marker::PhantomData;
use syan::visit::Ast;
use syan_rust::inherit::{Expr, Type};

#[derive(Debug, Ast)]
#[subast(syan_rust::inherit::Expr)]
pub enum Stmt<S> {
    E(Box<Expr<S>>),
    Empty(PhantomData<S>),
}

pub mod ext {
    // Inherit the upstream base visitor; add a method for the downstream `Stmt`.
    syan::visit::visitor!(syan_rust::inherit::base => crate::Stmt);
}

#[derive(Default)]
struct Counter {
    types: u32,
    exprs: u32,
    stmts: u32,
    wrapped: u32,
}

// The base trait is implemented for the inherited (upstream) types...
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

// ...and the extending trait (supertrait of the base) for the new type.
impl<S> ext::Visit<S> for Counter {
    fn visit_stmt(&mut self, i: &Stmt<S>) {
        self.stmts += 1;
        ext::visit_stmt(self, i);
    }
}

#[test]
fn inheriting_visitor_descends_into_cross_crate_base_types() {
    // Stmt::E -> Expr::Typed -> Type::Unit
    let ast: Stmt<()> = Stmt::E(Box::new(Expr::Typed(Box::new(Type::Unit(PhantomData)))));
    let mut counter = Counter::default();
    ast.visit(&mut counter);
    assert_eq!(counter.stmts, 1);
    assert_eq!(counter.exprs, 1, "reached the inherited (cross-crate) Expr method");
    assert_eq!(counter.types, 1, "reached the inherited (cross-crate) Type method");
}

#[test]
fn inheriting_closure_targets_new_type() {
    let ast: Stmt<()> = Stmt::E(Box::new(Expr::Typed(Box::new(Type::Unit(PhantomData)))));
    let mut stmts = 0;
    ast.visit(|_s: &Stmt<()>| stmts += 1);
    assert_eq!(stmts, 1);
}

// ── Differing arity across the crate boundary ────────────────────────────────────────────────────
// The base visitor is `Visit<S>` (one param). The downstream `Wrapped<S, T>` adds a second param
// `T`, so the new trait is `ext2::Visit<S, T>` extending `syan_rust::inherit::base::Visit<S>` with
// the *base's* own arity (carried via the base's `@bg` generic union). `Wrapped` references the
// upstream `Expr<S>`, so traversal still reaches the inherited base methods.
#[derive(Debug, Ast)]
#[subast(syan_rust::inherit::Expr)]
pub enum Wrapped<S, T> {
    E(Box<Expr<S>>),
    Extra(PhantomData<T>),
    Lit(PhantomData<S>),
}

pub mod ext2 {
    syan::visit::visitor!(syan_rust::inherit::base => crate::Wrapped);
}

impl<S, T> ext2::Visit<S, T> for Counter {
    fn visit_wrapped(&mut self, i: &Wrapped<S, T>) {
        self.wrapped += 1;
        ext2::visit_wrapped(self, i);
    }
}

#[test]
fn cross_crate_inheritance_with_wider_arity() {
    // Wrapped<(), u8>::E -> Expr::Typed -> Type::Unit
    let ast: Wrapped<(), u8> =
        Wrapped::E(Box::new(Expr::Typed(Box::new(Type::Unit(PhantomData)))));
    let mut counter = Counter::default();
    ast.visit(&mut counter);
    assert_eq!(counter.wrapped, 1, "the new arity-2 node");
    assert_eq!(counter.exprs, 1, "inherited Expr (base arity 1)");
    assert_eq!(counter.types, 1, "inherited Type");
}

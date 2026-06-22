//! Drill-in: a visited type's `#[subast]` lists an *unlisted intermediate* (`Cast`), which the
//! visitor has no `visit_*` for. `visit_expr` must drill *through* `Cast` inline to reach the
//! visited `Type` nested inside it — `Expr::Cast(c) => this.visit_type(&c.0)` — while `Cast` itself
//! is never visitable (no `visit_cast`).

use core::marker::PhantomData;
use syan::visit::Ast;

#[derive(Debug, Ast)]
pub enum Type<S> {
    Unit(PhantomData<S>),
}

// Unlisted intermediate: followed by `Expr` but absent from `visitor!(..)`, so it carries no method
// and is drilled through to its `Type` child.
#[derive(Debug, Ast)]
#[subast(crate::Type)]
pub struct Cast<S>(pub Type<S>);

#[derive(Debug, Ast)]
#[subast(crate::Cast)]
pub enum Expr<S> {
    Cast(Cast<S>),
    Lit(PhantomData<S>),
}

pub mod visit {
    // `Type` is visited, `Cast` is NOT — `Cast` is reached only by drilling through it.
    syan::visit::visitor!(super::Expr, super::Type);
}

fn sample() -> Expr<()> {
    Expr::Cast(Cast(Type::Unit(PhantomData)))
}

#[test]
fn closure_reaches_type_through_unlisted_cast() {
    let mut types = 0usize;
    sample().visit(|_t: &Type<()>| types += 1);
    assert_eq!(types, 1, "visit_expr drilled through Cast to the Type");
}

#[test]
fn struct_visitor_has_expr_and_type_methods_only() {
    // `Visit` carries `visit_expr` and `visit_type`; there is no `visit_cast` (Cast is not visited).
    #[derive(Default)]
    struct Counter {
        exprs: usize,
        types: usize,
    }
    impl<S> visit::Visit<S> for Counter {
        fn visit_expr(&mut self, i: &Expr<S>) {
            self.exprs += 1;
            visit::visit_expr(self, i);
        }
        fn visit_type(&mut self, i: &Type<S>) {
            self.types += 1;
            visit::visit_type(self, i);
        }
    }
    let mut c = Counter::default();
    sample().visit(&mut c);
    assert_eq!(c.exprs, 1);
    assert_eq!(c.types, 1, "reached via drilling, not a visit_cast hop");
}

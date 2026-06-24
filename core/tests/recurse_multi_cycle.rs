//! Step 1 of multi-root support: several *independent* cycles (separate SCCs) in one `#[recurse]`
//! module. Each cycle gets its own root, depth chain, terminator, public aliases, and — under
//! `#[recurse(visit)]` — its own (root-prefixed) visitor traits, so the cycles don't interfere.
//!
//! Previously this was either a miscompile (plain `#[recurse]` collapsed both cycles into one
//! `__Rec`, so one cycle's recursion wrongly bottomed out via the other's terminator) or a hard
//! abort (`#[recurse(visit)]` rejected >1 self-referential type module-wide).
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::parse::recurse;

#[derive(Default)]
struct Counter(usize);

// ── two independent cycles, plain #[recurse] ─────────────────────────────────────
// Expr and Type are disjoint self-referential cycles. Each must regenerate against its OWN depth
// default; if they collapsed into one __Rec this would mistype.
#[recurse(limit = 3)]
mod plain {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum Expr<S> {
        Nest(Box<Expr<S>>),
        Lit(PhantomData<S>),
    }

    #[derive(Ast)]
    #[subast()]
    pub enum Type<S> {
        Arrow(Box<Type<S>>),
        Unit(PhantomData<S>),
    }
}

#[test]
fn two_independent_cycles_build() {
    // Both public aliases exist and typecheck independently (a leaf of each). If the two cycles had
    // collapsed into one `__Rec`/one root, these aliases would be cross-wired and fail to resolve.
    let _e: plain::Expr<()> = plain::Expr::Lit(PhantomData);
    let _t: plain::Type<()> = plain::Type::Unit(PhantomData);
}

// ── two independent cycles, #[recurse(visit)] ────────────────────────────────────
// Each cycle gets its own root-prefixed visitor: `ExprVisit`/`ExprVisitRec` and
// `TypeVisit`/`TypeVisitRec`, plus `visit_expr`/`visit_type` and `ExprNode`/`TypeNode`.
#[recurse(visit)]
mod vis {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum Expr<S> {
        Nest(Box<Expr<S>>),
        Lit(PhantomData<S>),
    }

    #[derive(Ast)]
    #[subast()]
    pub enum Type<S> {
        Arrow(Box<Type<S>>),
        Unit(PhantomData<S>),
    }
}

impl<S> vis::ExprVisit<S> for Counter {
    fn visit_expr<R: vis::ExprVisitRec<S, Self>>(&mut self, i: &vis::ExprNode<S, R>) {
        self.0 += 10;
        vis::visit_expr(self, i);
    }
}

impl<S> vis::TypeVisit<S> for Counter {
    fn visit_type<R: vis::TypeVisitRec<S, Self>>(&mut self, i: &vis::TypeNode<S, R>) {
        self.0 += 1;
        vis::visit_type(self, i);
    }
}

#[test]
fn independent_visitors_are_separate() {
    // Expr depth 2 (Nest + Lit) → +10 twice = 20; Type depth 2 → +1 twice = 2. Each cycle's visitor
    // descends only its own type — they don't bleed into each other.
    let e: vis::Expr<()> = vis::Expr::Nest(Box::new(vis::ExprNode::Lit(PhantomData)));
    let t: vis::Type<()> = vis::Type::Arrow(Box::new(vis::TypeNode::Unit(PhantomData)));

    let mut c = Counter::default();
    vis::ExprVisit::visit_expr(&mut c, &e);
    assert_eq!(c.0, 20, "two Expr nodes at +10 each");

    let mut c2 = Counter::default();
    vis::TypeVisit::visit_type(&mut c2, &t);
    assert_eq!(c2.0, 2, "two Type nodes at +1 each");
}

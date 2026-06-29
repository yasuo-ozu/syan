//! Step 1 of multi-root support: several *independent* cycles (separate SCCs) in one `#[recurse]`
//! module. Each cycle gets its own root, depth chain, terminator, and public aliases, so the cycles
//! don't interfere; a `visitor!()` over both then keeps each cycle's depth dimension separate.
//!
//! Previously this was a miscompile (plain `#[recurse]` collapsed both cycles into one `__Rec`, so
//! one cycle's recursion wrongly bottomed out via the other's terminator).
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::parse::recurse;

#[derive(Default)]
struct Counter(usize);

// ── two independent cycles, plain #[recurse] ─────────────────────────────────────
// Expr and Type are disjoint self-referential cycles. Each must regenerate against its OWN depth
// default; if they collapsed into one __Rec this would mistype.
#[recurse]
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

// ── two independent cycles, visited via one visitor!() ───────────────────────────
// A single unified `visitor!()` over both cycles: one acyclic `Visit` trait carrying `visit_expr` +
// `visit_type`. Each cycle is a self-recursive natural type, so the visitors descend only their own
// type and don't bleed.
#[recurse]
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

mod v_vis {
    syan::visit::visitor!(crate::vis::Expr, crate::vis::Type);
}

impl<S> v_vis::Visit<S> for Counter {
    fn visit_expr(&mut self, i: &vis::Expr<S>) {
        self.0 += 10;
        v_vis::visit_expr(self, i);
    }
    fn visit_type(&mut self, i: &vis::Type<S>) {
        self.0 += 1;
        v_vis::visit_type(self, i);
    }
}

#[test]
fn independent_visitors_are_separate() {
    // Expr depth 2 (Nest + Lit) → +10 twice = 20; Type depth 2 → +1 twice = 2. Each cycle's visitor
    // descends only its own type — they don't bleed into each other.
    let e: vis::Expr<()> = vis::Expr::Nest(Box::new(vis::Expr::Lit(PhantomData)));
    let t: vis::Type<()> = vis::Type::Arrow(Box::new(vis::Type::Unit(PhantomData)));

    let mut c = Counter::default();
    v_vis::Visit::visit_expr(&mut c, &e);
    assert_eq!(c.0, 20, "two Expr nodes at +10 each");

    let mut c2 = Counter::default();
    v_vis::Visit::visit_type(&mut c2, &t);
    assert_eq!(c2.0, 2, "two Type nodes at +1 each");
}

//! Depth-generic traversal through container shapes and tuples — the cycle uses `#[recurse]` and the
//! visitor is built by a sibling `visitor!()`. Regression tests for the fixes to audit problems #3 (a
//! `Box` *around* an `Option`, via `Peeled::cont_box`) and #7 (a tuple-typed field, destructured and
//! dispatched element-by-element), plus the already-working `Vec<Box<_>>` / `Option<Box<_>>` shapes
//! for good measure.
//!
//! Back-edge slots are filled with `v_v_ast::ExprNode::Lit(PhantomData)` (the depth-generic node alias),
//! letting each call site infer the one-level-shallower depth — as in `visitor_recurse_cycle.rs`.
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::parse::recurse;

#[recurse]
mod ast {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum Expr<S> {
        // #3: a Box wrapping the Option (`cont_box`) — patterns don't auto-deref Box.
        Opt(Box<Option<Box<Expr<S>>>>),
        // #7: a single tuple-typed field holding two cycle refs.
        Pair((Box<Expr<S>>, Box<Expr<S>>)),
        // A tuple mixing a followed element with a leaf.
        Tagged((Box<Expr<S>>, PhantomData<S>)),
        // Sequence and plain-Option shapes (these already worked; pin them).
        Many(Vec<Box<Expr<S>>>),
        OptIn(Option<Box<Expr<S>>>),
        Lit(PhantomData<S>),
    }
}

mod v_ast {
    syan::visit::visitor!(crate::ast::Expr);
}

#[derive(Default)]
struct Nodes(usize);

impl<S> v_ast::Visit<S> for Nodes {
    fn visit_expr<R: v_ast::VisitRec<S, Self>>(&mut self, i: &v_ast::ExprNode<S, R>) {
        self.0 += 1;
        v_ast::visit_expr(self, i);
    }
}

fn count(e: &ast::Expr<()>) -> usize {
    let mut n = Nodes::default();
    v_ast::Visit::visit_expr(&mut n, e);
    n.0
}

#[test]
fn box_around_option_some() {
    // #3: Opt(Some(Lit)) → outer Opt-Expr + inner Lit = 2.
    let e: ast::Expr<()> =
        ast::Expr::Opt(Box::new(Some(Box::new(v_ast::ExprNode::Lit(PhantomData)))));
    assert_eq!(count(&e), 2, "Box<Option<Box<Expr>>> descends through the Some");
}

#[test]
fn box_around_option_none() {
    // #3: Opt(None) → just the outer Opt-Expr = 1.
    let e: ast::Expr<()> = ast::Expr::Opt(Box::new(None));
    assert_eq!(count(&e), 1, "None stops the descent");
}

#[test]
fn tuple_field_both_operands() {
    // #7: Pair((Lit, Lit)) → outer Pair-Expr + both operands = 3.
    let e: ast::Expr<()> = ast::Expr::Pair((
        Box::new(v_ast::ExprNode::Lit(PhantomData)),
        Box::new(v_ast::ExprNode::Lit(PhantomData)),
    ));
    assert_eq!(count(&e), 3, "tuple field visits both cycle-ref operands");
}

#[test]
fn tuple_field_with_leaf() {
    // #7: Tagged((Lit, PhantomData)) → outer + the one followed element = 2.
    let e: ast::Expr<()> =
        ast::Expr::Tagged((Box::new(v_ast::ExprNode::Lit(PhantomData)), PhantomData));
    assert_eq!(count(&e), 2, "leaf tuple element is skipped, cycle ref visited");
}

#[test]
fn vec_of_boxed_exprs() {
    let e: ast::Expr<()> = ast::Expr::Many(vec![
        Box::new(v_ast::ExprNode::Lit(PhantomData)),
        Box::new(v_ast::ExprNode::Lit(PhantomData)),
        Box::new(v_ast::ExprNode::Lit(PhantomData)),
    ]);
    assert_eq!(count(&e), 4, "outer Many + 3 elements");
}

#[test]
fn option_of_boxed_expr() {
    let e: ast::Expr<()> = ast::Expr::OptIn(Some(Box::new(v_ast::ExprNode::Lit(PhantomData))));
    assert_eq!(count(&e), 2, "outer OptIn + inner");
}

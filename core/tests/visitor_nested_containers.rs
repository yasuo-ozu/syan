//! Nested containers (`Vec<Option<T>>`, `Option<Vec<T>>`, `Vec<Vec<T>>`) are traversed — previously a
//! clean `abort!` telling you to wrap the inner part in its own `#[derive(Ast)]` type.
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::visit::Ast;

#[derive(Ast)]
#[subast()]
pub struct Leaf<S> {
    pub _p: PhantomData<S>,
}

#[derive(Ast)]
#[subast(crate::Leaf)]
pub struct Holder<S> {
    pub vo: Vec<Option<Leaf<S>>>,
    pub ov: Option<Vec<Leaf<S>>>,
    pub vv: Vec<Vec<Leaf<S>>>,
}

mod v {
    syan::visit::visitor!(crate::Leaf, crate::Holder);
}

fn leaf<S>() -> Leaf<S> {
    Leaf { _p: PhantomData }
}

#[test]
fn nested_containers_are_traversed() {
    let h: Holder<()> = Holder {
        vo: vec![Some(leaf()), None, Some(leaf())], // 2
        ov: Some(vec![leaf(), leaf()]),             // 2
        vv: vec![vec![leaf()], vec![leaf(), leaf()]], // 3
    };
    let mut n = 0usize;
    h.visit(|_: &Leaf<()>| n += 1);
    assert_eq!(n, 7, "2 (Vec<Option>) + 2 (Option<Vec>) + 3 (Vec<Vec>)");
}

#[test]
fn nested_containers_visit_mut() {
    let mut h: Holder<()> = Holder { vo: vec![Some(leaf())], ov: None, vv: vec![] };
    let mut n = 0usize;
    h.visit_mut(|_: &mut Leaf<()>| n += 1);
    assert_eq!(n, 1);
}

// Nested containers in a `#[recurse]` cycle traverse too (back-edges still via the depth param).
#[syan::parse::recurse]
mod rec {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum Expr<S> {
        Many(Vec<Option<Expr<S>>>), // nested container of the back-edge
        Lit(PhantomData<S>),
    }
}

mod rv {
    syan::visit::visitor!(crate::rec::Expr);
}

#[derive(Default)]
struct Counter(usize);
impl<S> rv::Visit<S> for Counter {
    fn visit_expr<R: rv::VisitRec<S, Self>>(&mut self, i: &rv::ExprNode<S, R>) {
        self.0 += 1;
        rv::visit_expr(self, i);
    }
}

#[test]
fn recurse_nested_container_is_traversed() {
    // Expr::Many([Some(Lit), None, Some(Lit)]) → outer Expr + 2 inner Exprs.
    let e: rec::Expr<()> = rec::Expr::Many(vec![
        Some(rv::ExprNode::Lit(PhantomData)),
        None,
        Some(rv::ExprNode::Lit(PhantomData)),
    ]);
    let mut c = Counter::default();
    rv::Visit::visit_expr(&mut c, &e);
    assert_eq!(c.0, 3, "outer Expr + 2 inner (Vec<Option<Expr>> back-edges)");
}

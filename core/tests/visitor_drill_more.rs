//! More drill-in scenarios: a chain of several unlisted intermediates, drilling inside containers,
//! and a finite dead-end (an unlisted intermediate that reaches no visited type — a no-op, not an
//! error).

use core::marker::PhantomData;
use syan::visit::Ast;

// ── A chain of unlisted intermediates: Expr -> Wrap -> Cast -> Type ──────────────────────────────

#[derive(Debug, Ast)]
pub enum Type<S> {
    Unit(PhantomData<S>),
}

#[derive(Debug, Ast)]
#[subast(crate::Type)]
pub struct Cast<S>(pub Type<S>);

#[derive(Debug, Ast)]
#[subast(crate::Cast)]
pub struct Wrap<S>(pub Cast<S>);

#[derive(Debug, Ast)]
#[subast(crate::Wrap)]
pub enum Expr<S> {
    W(Wrap<S>),
    Lit(PhantomData<S>),
}

pub mod chain {
    // Only `Expr` and `Type` are visited; `Wrap` and `Cast` are drilled through transitively.
    syan::visit::visitor!(super::Expr, super::Type);
}

#[test]
fn drills_through_a_chain_of_intermediates() {
    let ast: Expr<()> = Expr::W(Wrap(Cast(Type::Unit(PhantomData))));
    let mut types = 0usize;
    ast.visit(|_t: &Type<()>| types += 1);
    assert_eq!(types, 1, "reached Type through Wrap -> Cast");
}

// ── Drilling inside Vec / Option containers ──────────────────────────────────────────────────────

#[derive(Debug, Ast)]
pub enum Leaf<S> {
    U(PhantomData<S>),
}

#[derive(Debug, Ast)]
#[subast(crate::Leaf)]
pub struct Item<S>(pub Leaf<S>);

#[derive(Debug, Ast)]
#[subast(crate::Item)]
pub struct Block<S> {
    pub items: Vec<Item<S>>,
    pub opt: Option<Item<S>>,
}

pub mod container {
    // `Item` is an unlisted intermediate drilled per element; `Leaf` is the visited target.
    syan::visit::visitor!(super::Block, super::Leaf);
}

#[test]
fn drills_through_intermediates_in_containers() {
    let block: Block<()> = Block {
        items: vec![Item(Leaf::U(PhantomData)), Item(Leaf::U(PhantomData))],
        opt: Some(Item(Leaf::U(PhantomData))),
    };
    let mut leaves = 0usize;
    block.visit(|_l: &Leaf<()>| leaves += 1);
    assert_eq!(leaves, 3, "2 in the Vec + 1 in the Option, each drilled through Item");
}

// ── Finite dead-end: an unlisted intermediate that reaches no visited type ───────────────────────

#[derive(Debug, Ast)]
pub struct Dead<S>(pub i64, pub PhantomData<S>);

#[derive(Debug, Ast)]
#[subast(crate::Dead)]
pub enum ExprD<S> {
    D(Dead<S>),
    Lit(PhantomData<S>),
}

pub mod deadend {
    // `Dead` is followed but unlisted and contains no visited type — drilling it lowers to nothing.
    syan::visit::visitor!(super::ExprD);
}

#[test]
fn finite_dead_end_is_a_noop_not_an_error() {
    let ast: ExprD<()> = ExprD::D(Dead(7, PhantomData));
    let mut exprs = 0usize;
    ast.visit(|_e: &ExprD<()>| exprs += 1);
    assert_eq!(exprs, 1, "the root ExprD; drilling Dead reached no visited node");
}

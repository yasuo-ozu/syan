//! TODO follow-up: a single visitor over types with *different* generic arities
//! (`Expr<S, Tokens>` and `BinOp<S>`). The trait is parameterized by the union `<S, Tokens>`.

use core::marker::PhantomData;
use syan::visit::{visitor, Ast};

#[derive(Ast)]
pub enum Expr<S, Tokens> {
    Bin(Box<Expr<S, Tokens>>, BinOp<S>, Box<Expr<S, Tokens>>),
    Lit(i64, PhantomData<(S, Tokens)>),
}

#[derive(Ast)]
pub enum BinOp<S> {
    Add(PhantomData<S>),
    Mul(PhantomData<S>),
}

#[visitor(Expr, BinOp)]
pub mod visit {}

#[test]
fn visitor_over_mixed_arity_types() {
    let ast: Expr<(), ()> = Expr::Bin(
        Box::new(Expr::Lit(1, PhantomData)),
        BinOp::Add(PhantomData),
        Box::new(Expr::Bin(
            Box::new(Expr::Lit(2, PhantomData)),
            BinOp::Mul(PhantomData),
            Box::new(Expr::Lit(3, PhantomData)),
        )),
    );

    let mut exprs = 0usize;
    let mut ops = 0usize;
    ast.visit((
        |_e: &Expr<(), ()>| exprs += 1,
        |_o: &BinOp<()>| ops += 1,
    ));
    assert_eq!(exprs, 5, "2 Bin + 3 Lit");
    assert_eq!(ops, 2, "Add + Mul");
}

//! chumsky 0.13: `recursive` + `foldl`, the idiomatic shape.

use crate::ast::{Expr, Op};
use chumsky::prelude::*;

pub fn parser<'a>() -> impl Parser<'a, &'a str, Expr, extra::Err<Rich<'a, char>>> {
    recursive(|expr| {
        let int = text::int(10)
            .from_str::<i64>()
            .unwrapped()
            .map(Expr::Int)
            .labelled("integer");

        let atom = int
            .or(expr.delimited_by(just('('), just(')')).padded())
            .padded();

        let term = atom.clone().foldl(
            choice((just('*').to(Op::Mul), just('/').to(Op::Div)))
                .padded()
                .then(atom)
                .repeated(),
            |l, (op, r)| Expr::Bin(Box::new(l), op, Box::new(r)),
        );

        term.clone()
            .foldl(
                choice((just('+').to(Op::Add), just('-').to(Op::Sub)))
                    .padded()
                    .then(term)
                    .repeated(),
                |l, (op, r)| Expr::Bin(Box::new(l), op, Box::new(r)),
            )
            .labelled("expression")
    })
    .then_ignore(end())
}

pub fn parse(src: &str) -> Result<Expr, String> {
    parser().parse(src).into_result().map_err(|es| {
        es.iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    })
}

//! nom 8: hand-written precedence climbing over `&str`.

use crate::ast::{Expr, Op};
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::{digit1, multispace0};
use nom::combinator::map_res;
use nom::sequence::delimited;
use nom::{IResult, Parser};

fn ws<'a, O, F>(inner: F) -> impl Parser<&'a str, Output = O, Error = nom::error::Error<&'a str>>
where
    F: Parser<&'a str, Output = O, Error = nom::error::Error<&'a str>>,
{
    delimited(multispace0, inner, multispace0)
}

fn int(i: &str) -> IResult<&str, Expr> {
    map_res(digit1, |s: &str| s.parse::<i64>().map(Expr::Int)).parse(i)
}

fn atom(i: &str) -> IResult<&str, Expr> {
    ws(alt((int, delimited(tag("("), ws(expr), tag(")"))))).parse(i)
}

fn term(i: &str) -> IResult<&str, Expr> {
    let (mut i, mut lhs) = atom(i)?;
    loop {
        let op = ws(alt((tag::<_, &str, _>("*"), tag("/")))).parse(i);
        match op {
            Ok((rest, t)) => {
                let (rest, rhs) = atom(rest)?;
                let op = if t == "*" { Op::Mul } else { Op::Div };
                lhs = Expr::Bin(Box::new(lhs), op, Box::new(rhs));
                i = rest;
            }
            Err(_) => return Ok((i, lhs)),
        }
    }
}

fn expr(i: &str) -> IResult<&str, Expr> {
    let (mut i, mut lhs) = term(i)?;
    loop {
        let op = ws(alt((tag::<_, &str, _>("+"), tag("-")))).parse(i);
        match op {
            Ok((rest, t)) => {
                let (rest, rhs) = term(rest)?;
                let op = if t == "+" { Op::Add } else { Op::Sub };
                lhs = Expr::Bin(Box::new(lhs), op, Box::new(rhs));
                i = rest;
            }
            Err(_) => return Ok((i, lhs)),
        }
    }
}

/// Parse and require full consumption, so a partial parse cannot be mistaken for success.
pub fn parse(src: &str) -> Result<Expr, String> {
    match expr(src) {
        Ok((rest, e)) if rest.trim().is_empty() => Ok(e),
        Ok((rest, _)) => Err(format!("trailing input: {rest:?}")),
        Err(e) => Err(format!("{e}")),
    }
}

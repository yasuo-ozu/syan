//! combine 4.6: Parsec-style, `chainl1` for the left-associative folds.
//!
//! Like `nom_impl` and `chumsky_impl` this is written concretely over `&str` rather than generically
//! over `Stream`, so all three backends see the same input type and the same zero-copy slicing.
//! Two things differ from the other backends by necessity, both noted so the numbers are read
//! correctly:
//!
//! * combine's `or`/`choice` does **not** backtrack once a branch has consumed input (it returns
//!   `CommitErr`), so alternatives that can consume before failing must be wrapped in `attempt`.
//!   `atom`'s two branches are distinguished by their first token — `int` fails on `(` without
//!   consuming, and the parenthesised branch fails on a digit without consuming — so no `attempt`
//!   is needed here and none is paid for. nom and chumsky backtrack unconditionally.
//! * A recursion point is needed to break the infinite `impl Parser` type, the same role
//!   `recursive` plays in chumsky. combine's options are the `parser!`/`opaque!` macros and
//!   `parser(f)`; `parser(atom_)` is the cheapest, because a plain `fn` item is a ZST, so
//!   `FnParser` adds no indirection and no boxing (`opaque!` boxes). The function must return
//!   `StdParseResult`, i.e. `Result<(O, Commit<()>), Commit<Tracked<E>>>`, not `ParseResult`.
//!
//! Whitespace convention: every token parser skips the whitespace *after* itself, and `parse` skips
//! whitespace once at the very start. That is one `spaces()` per token, as in the other backends.

use crate::ast::{Expr, Op};
use combine::parser::char::{char, spaces};
use combine::parser::range::take_while1;
use combine::{between, chainl1, from_str, parser, Parser, StdParseResult};

fn int<'a>() -> impl Parser<&'a str, Output = Expr> {
    from_str(take_while1(|c: char| c.is_ascii_digit()))
        .map(Expr::Int)
        .skip(spaces())
}

/// The recursion point: a non-generic `fn` whose signature mentions no parser type, so the
/// `atom -> expr -> term -> atom` cycle never has to be spelled as a type.
fn atom_<'a>(input: &mut &'a str) -> StdParseResult<Expr, &'a str> {
    let paren = between(char('(').skip(spaces()), char(')').skip(spaces()), expr());
    int().or(paren).parse_stream(input).into_result()
}

fn atom<'a>() -> impl Parser<&'a str, Output = Expr> {
    parser(atom_)
}

fn term<'a>() -> impl Parser<&'a str, Output = Expr> {
    let op = char('*')
        .map(|_| Op::Mul)
        .or(char('/').map(|_| Op::Div))
        .skip(spaces())
        .map(|op| move |l, r| Expr::Bin(Box::new(l), op, Box::new(r)));
    chainl1(atom(), op)
}

fn expr<'a>() -> impl Parser<&'a str, Output = Expr> {
    let op = char('+')
        .map(|_| Op::Add)
        .or(char('-').map(|_| Op::Sub))
        .skip(spaces())
        .map(|op| move |l, r| Expr::Bin(Box::new(l), op, Box::new(r)));
    chainl1(term(), op)
}

/// Parse and require full consumption, so a partial parse cannot be mistaken for success.
pub fn parse(src: &str) -> Result<Expr, String> {
    match spaces().with(expr()).parse(src) {
        Ok((e, "")) => Ok(e),
        Ok((_, rest)) => Err(format!("trailing input: {rest:?}")),
        Err(e) => Err(format!("{e}")),
    }
}

//! The single AST all three parsers produce, so results are comparable and checkable.
//!
//! Grammar (left-associative, `*`/`/` binding tighter than `+`/`-`):
//!
//! ```text
//! expr   := term (('+' | '-') term)*
//! term   := atom (('*' | '/') atom)*
//! atom   := int | '(' expr ')'
//! ```

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Int(i64),
    Bin(Box<Expr>, Op, Box<Expr>),
}

impl Expr {
    /// Evaluated to give every backend a cheap, total correctness check that does not
    /// depend on tree shape being spelled identically.
    pub fn eval(&self) -> i64 {
        match self {
            Expr::Int(n) => *n,
            Expr::Bin(l, op, r) => {
                let (l, r) = (l.eval(), r.eval());
                match op {
                    Op::Add => l + r,
                    Op::Sub => l - r,
                    Op::Mul => l * r,
                    Op::Div => l / r,
                }
            }
        }
    }

    pub fn nodes(&self) -> usize {
        match self {
            Expr::Int(_) => 1,
            Expr::Bin(l, _, r) => 1 + l.nodes() + r.nodes(),
        }
    }
}

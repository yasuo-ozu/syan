//! syan with `Atom = WithSpan<char, Span>` — the char source, same starting point as nom/chumsky.
//!
//! Both decycle engines are instantiated from one grammar definition: [`ranked`] (`#[recurse]`) and
//! [`structural`] (`#[recurse(structural)]`). They must agree on results; `tests/agree.rs` checks it.
//!
//! Two things the char source does not give you, both written out here because a benchmark that hid
//! them would be measuring a grammar nobody can write:
//!
//! * **no integer leaf** — `impl_parse_for_char!` only provides `Symbol<chars::X>` for one literal
//!   char, so multi-digit integers need a hand-written `Parse`;
//! * **no whitespace skipping** — `source::string::Stream::skip_sep()` returns `true`
//!   unconditionally, so `#[joint]`/`#[alone]` cannot express padding and every token position needs
//!   an explicit wrapper.

use syan::error::ParseError;
use syan::parse::{IntoParseStream, Parse};
use syan::source::string::Span;
use syan::span::WithSpan;

pub type Atom = WithSpan<char, Span>;

/// Skip leading whitespace, then parse `T`.
pub struct Ws<T>(pub T);

impl<T: Parse<Atom>> Parse<Atom> for Ws<T> {
    type Error = T::Error;
    fn parse_stream<__S: syan::parse::ParseStream<Atom = Atom>>(stream: &mut __S) -> Result<Self, Self::Error> {
        let s = stream.into_parse_stream();
        loop {
            let ws = matches!(s.peek(), Some(a) if a.slot.is_whitespace());
            if ws {
                s.next();
            } else {
                break;
            }
        }
        T::parse_stream(&mut *s).map(Ws)
    }
}

/// A multi-digit decimal integer.
pub struct Int(pub i64);

impl Parse<Atom> for Int {
    type Error = ParseError;
    fn parse_stream<__S: syan::parse::ParseStream<Atom = Atom>>(stream: &mut __S) -> Result<Self, Self::Error> {
        let s = stream.into_parse_stream();
        let mut buf = String::new();
        loop {
            let d = match s.peek() {
                Some(a) if a.slot.is_ascii_digit() => a.slot,
                _ => break,
            };
            buf.push(d);
            s.next();
        }
        if buf.is_empty() {
            return Err(ParseError::new(Span::default(), "expected a digit"));
        }
        buf.parse::<i64>()
            .map(Int)
            .map_err(|e| ParseError::new(Span::default(), e))
    }
}

/// Instantiate the grammar under one decycle engine. `engine!(ranked)` uses `#[recurse]`;
/// `engine!(structural, structural)` uses `#[recurse(structural)]`.
macro_rules! engine {
    ($name:ident $(, $arg:ident)?) => {
        pub mod $name {
            use crate::ast::{Expr, Op};
            use crate::syan_char::{Atom, Int, Ws};
            use syan::parse::{Parse, ParseStream};
            use syan::source::string::Stream;

            // `AddOp`/`MulOp` are ACYCLIC, so they live outside the `#[recurse]` module.
            // Keeping them inside works under the ranked engine but not the structural one, which
            // leaves a non-cycle member's `Parse::Error` associated type unpinned — see README.
            #[derive(Parse)]
            pub enum AddOp {
                Add(Ws<syan::symbol::Symbol<syan::symbol::chars::Plus>>),
                Sub(Ws<syan::symbol::Symbol<syan::symbol::chars::Minus>>),
            }

            #[derive(Parse)]
            pub enum MulOp {
                Mul(Ws<syan::symbol::Symbol<syan::symbol::chars::Star>>),
                Div(Ws<syan::symbol::Symbol<syan::symbol::chars::Slash>>),
            }

            #[syan::parse::recurse $(($arg))?]
            mod g {
                use syan::parse::Parse;
                use syan::symbol::{chars, Symbol};

                #[derive(Parse)]
                pub struct AddTail {
                    pub op: super::AddOp,
                    pub rhs: Term,
                }

                #[derive(Parse)]
                pub struct MulTail {
                    pub op: super::MulOp,
                    pub rhs: Atom,
                }

                #[derive(Parse)]
                pub struct Expr {
                    pub head: Term,
                    pub tail: Vec<AddTail>,
                }

                #[derive(Parse)]
                pub struct Term {
                    pub head: Atom,
                    pub tail: Vec<MulTail>,
                }

                #[derive(Parse)]
                pub enum Atom {
                    Int(crate::syan_char::Ws<crate::syan_char::Int>),
                    Paren {
                        open: crate::syan_char::Ws<Symbol<chars::OpenParen>>,
                        inner: Box<Expr>,
                        close: crate::syan_char::Ws<Symbol<chars::CloseParen>>,
                    },
                }
            }

            fn lower_expr(e: &g::Expr) -> Expr {
                let mut acc = lower_term(&e.head);
                for t in &e.tail {
                    let op = match t.op {
                        AddOp::Add(_) => Op::Add,
                        AddOp::Sub(_) => Op::Sub,
                    };
                    acc = Expr::Bin(Box::new(acc), op, Box::new(lower_term(&t.rhs)));
                }
                acc
            }

            fn lower_term(t: &g::Term) -> Expr {
                let mut acc = lower_atom(&t.head);
                for m in &t.tail {
                    let op = match m.op {
                        MulOp::Mul(_) => Op::Mul,
                        MulOp::Div(_) => Op::Div,
                    };
                    acc = Expr::Bin(Box::new(acc), op, Box::new(lower_atom(&m.rhs)));
                }
                acc
            }

            fn lower_atom(a: &g::Atom) -> Expr {
                match a {
                    g::Atom::Int(Ws(Int(n))) => Expr::Int(*n),
                    g::Atom::Paren { inner, .. } => lower_expr(inner),
                }
            }

            pub fn parse(src: &str) -> Result<Expr, String> {
                let mut stream = Stream::new(src.to_string());
                let parsed =
                    <g::Expr as Parse<Atom>>::parse_stream(&mut stream).map_err(|e| e.to_string())?;
                // require full consumption, minus trailing whitespace
                loop {
                    let ws = matches!(stream.peek(), Some(a) if a.slot.is_whitespace());
                    if ws {
                        stream.next();
                    } else {
                        break;
                    }
                }
                if let Some(a) = stream.peek() {
                    return Err(format!("trailing input at {:?}", a.slot));
                }
                Ok(lower_expr(&parsed))
            }
        }
    };
}

engine!(ranked);
engine!(structural, structural);

/// The default engine, so existing call sites keep working.
pub use ranked::parse;

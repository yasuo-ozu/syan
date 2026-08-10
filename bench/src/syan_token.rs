//! syan with `Atom = proc_macro2::TokenTree` — the idiomatic source.
//!
//! Both decycle engines are instantiated from one grammar definition: [`ranked`] (`#[recurse]`) and
//! [`structural`] (`#[recurse(structural)]`).
//!
//! Two structural differences from the char version, both of which the numbers must account for:
//!
//! * **proc-macro2 does the lexing**, so `parse` here starts from an already-tokenised stream.
//!   `lex_then_parse` includes that cost; `parse_pretokenised` excludes it. Only the former is
//!   comparable with nom/chumsky/syan-char, which all start from `&str`.
//! * **a parenthesised group is ONE atom.** `( 1 + 2 )` is a single `TokenTree::Group`, so this
//!   parser walks far fewer atoms and recursion happens through `#[group]` rather than through a
//!   delimiter pair.

pub type Atom = proc_macro2::TokenTree;

/// Instantiate the grammar under one decycle engine. `engine!(ranked)` uses `#[recurse]`;
/// `engine!(structural, structural)` uses `#[recurse(structural)]`.
macro_rules! engine {
    ($name:ident $(, $arg:ident)?) => {
        pub mod $name {
            use crate::ast::{Expr, Op};
            use crate::syan_token::Atom;
            use proc_macro2::TokenStream;
            use syan::parse::Parse;
            use syan::source::proc_macro2::Stream;
            use syan::symbol::Token;

            // ACYCLIC, so outside the `#[recurse]` module: a non-cycle type inside it compiles
            // under the ranked engine but leaves its `Parse::Error` unpinned under structural.
            #[derive(Parse)]
            pub enum AddOp {
                Add(Token![syan::source::proc_macro2::Span => +]),
                Sub(Token![syan::source::proc_macro2::Span => -]),
            }

            #[derive(Parse)]
            pub enum MulOp {
                Mul(Token![syan::source::proc_macro2::Span => *]),
                Div(Token![syan::source::proc_macro2::Span => /]),
            }

            #[syan::parse::recurse $(($arg))?]
            mod g {
                use syan::nested::group::GroupParen;
                use syan::parse::Parse;
                use syan::source::proc_macro2::literal::Integer;
                use syan::source::proc_macro2::Span;

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
                    Int(Integer),
                    Paren {
                        paren: GroupParen<(), Span>,
                        #[group(self.paren)]
                        inner: Box<Expr>,
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
                    g::Atom::Int(i) => Expr::Int(i.value.parse::<i64>().expect("integer literal")),
                    g::Atom::Paren { inner, .. } => lower_expr(inner),
                }
            }

            /// Tokenise, then parse. This is the number to compare against the `&str` parsers.
            pub fn lex_then_parse(src: &str) -> Result<Expr, String> {
                let ts: TokenStream = src.parse().map_err(|e| format!("lex: {e}"))?;
                parse_pretokenised(ts)
            }

            /// Parse an already-tokenised stream. Excludes lexing; not comparable with the `&str`
            /// backends.
            pub fn parse_pretokenised(ts: TokenStream) -> Result<Expr, String> {
                let mut stream = Stream::new(ts);
                let parsed =
                    <g::Expr as Parse<Atom>>::parse_stream(&mut stream).map_err(|e| e.to_string())?;
                if syan::parse::ParseStream::peek(&mut stream).is_some() {
                    return Err("trailing input".into());
                }
                Ok(lower_expr(&parsed))
            }
        }
    };
}

engine!(ranked);
engine!(structural, structural);

/// The default engine, so existing call sites keep working.
pub use ranked::{lex_then_parse, parse_pretokenised};

pub fn tokenise(src: &str) -> proc_macro2::TokenStream {
    src.parse().expect("lexable")
}

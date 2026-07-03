//! §2A — the two `Group` atom families cohere and both are served by core with no downstream orphan
//! impl.
//!
//! * A **flat-token** atom (each character is its own atom; `(` and `)` are ordinary leaf atoms) uses
//!   core's flat `Group` `Parse`/`Unparse` blankets directly: this test defines its own open/close leaf
//!   tokens and **no `Group` impl of its own** — exactly the SATySFi-port scenario the field report's
//!   `define_groups!` workaround existed to serve.
//! * A **tree-shaped** atom (`proc_macro2::TokenTree`, where a delimited group is a single atom) still
//!   parses/round-trips a `GroupParen` as one atom via the delimiter-generic tree impls.

use std::convert::Infallible;
use syan::error::ParseError;
use syan::nested::group::{Group, GroupParen};
use syan::parse::into_parse_stream::IntoParseStream;
use syan::parse::unparse::Emitter;
use syan::parse::{Parse, ParseStream, Unparse};

// ── A flat toy atom: a single character. `(`/`)` are separate leaf atoms, so groups parse/unparse
//    delimiter-wise (open, slot, close). ──────────────────────────────────────────────────────────

type Tok = char;

/// Character stream over a `Vec<char>` used as a stack (whitespace stripped up front).
struct CharStream {
    buf: Vec<Tok>,
}

impl CharStream {
    fn new(src: &str) -> Self {
        Self {
            buf: src.chars().filter(|c| !c.is_whitespace()).rev().collect(),
        }
    }
}

impl ParseStream for CharStream {
    type Atom = Tok;
    type Error = Infallible;

    fn next(&mut self) -> Option<Tok> {
        self.buf.pop()
    }

    fn peek(&mut self) -> Option<&Tok> {
        self.buf.last()
    }

    fn push(&mut self, atom: Tok) {
        self.buf.push(atom);
    }

    fn get_error(&mut self) -> Result<(), Infallible> {
        Ok(())
    }

    fn skip_sep(&mut self) -> bool {
        false
    }
}

/// The downstream's own `(` leaf token.
#[derive(Debug, PartialEq)]
struct LParen;
/// The downstream's own `)` leaf token.
#[derive(Debug, PartialEq)]
struct RParen;
/// An alphanumeric payload leaf.
#[derive(Debug, PartialEq)]
struct Word(String);

macro_rules! impl_delim_leaf {
    ($ty:ident, $ch:literal) => {
        impl Parse<Tok> for $ty {
            type Error = ParseError;
            fn parse(stream: impl IntoParseStream<Atom = Tok>) -> Result<Self, ParseError> {
                let mut stream = stream.into_parse_stream();
                match stream.next() {
                    Some($ch) => Ok($ty),
                    Some(c) => {
                        stream.push(c);
                        Err(ParseError::new((), concat!("expected `", $ch, "`")))
                    }
                    None => Err(ParseError::new((), "unexpected end of input")),
                }
            }
        }

        impl Unparse<Tok> for $ty {
            fn unparse<E: Emitter<Tok>>(&self, sink: &mut E) -> Result<(), E::Error> {
                sink.write_one($ch)
            }
        }
    };
}

impl_delim_leaf!(LParen, '(');
impl_delim_leaf!(RParen, ')');

impl Parse<Tok> for Word {
    type Error = ParseError;
    fn parse(stream: impl IntoParseStream<Atom = Tok>) -> Result<Self, ParseError> {
        let mut stream = stream.into_parse_stream();
        let mut out = String::new();
        while let Some(c) = stream.peek().copied() {
            if c.is_alphanumeric() {
                out.push(c);
                stream.next();
            } else {
                break;
            }
        }
        if out.is_empty() {
            Err(ParseError::new((), "expected word"))
        } else {
            Ok(Word(out))
        }
    }
}

impl Unparse<Tok> for Word {
    fn unparse<E: Emitter<Tok>>(&self, sink: &mut E) -> Result<(), E::Error> {
        for c in self.0.chars() {
            sink.write_one(c)?;
        }
        Ok(())
    }
}

/// A flat paren group over the toy atom — its `Parse`/`Unparse` come entirely from core's flat blankets.
type FlatParen<T> = Group<T, LParen, RParen>;

fn unparse_chars<T: Unparse<Tok>>(value: &T) -> String {
    let mut out: Vec<Tok> = Vec::new();
    value.unparse(&mut (&mut out)).unwrap();
    out.into_iter().collect()
}

#[test]
fn flat_atom_group_round_trip() {
    let g: FlatParen<Word> = Parse::parse(CharStream::new("( hello )")).unwrap();
    assert_eq!(g.open, LParen);
    assert_eq!(g.slot, Word("hello".to_string()));
    assert_eq!(g.close, RParen);

    // Delimiter-wise unparse: open, slot, close — three ordinary leaf writes.
    assert_eq!(unparse_chars(&g), "(hello)");
}

#[test]
fn flat_atom_group_nests() {
    // Nesting comes for free: the inner `Group` is just another flat-family `Parse`/`Unparse`.
    let g: FlatParen<FlatParen<Word>> = Parse::parse(CharStream::new("(( inner ))")).unwrap();
    assert_eq!(g.slot.slot, Word("inner".to_string()));
    assert_eq!(unparse_chars(&g), "((inner))");
}

// ── The tree-shaped `proc_macro2::TokenTree` atom keeps group-as-one-atom semantics. ───────────────

#[test]
fn pm2_group_is_one_atom_round_trip() {
    use syan::source::proc_macro2::literal::Integer;
    use template_quote::quote;
    type Sp = syan::source::proc_macro2::Span;

    // `( 42 )` is a single `TokenTree::Group`; the delimiter-generic tree impl matches it and parses the
    // inner sub-stream.
    let tokens = quote! { ( 42 ) };
    let g: GroupParen<Integer, Sp> = Parse::parse(tokens).unwrap();
    assert_eq!(g.slot.value, "42");
    assert_eq!(format!("{}", g.open), "(");
    assert_eq!(format!("{}", g.close), ")");

    let mut out = Vec::<proc_macro2::TokenTree>::new();
    g.unparse(&mut (&mut out)).unwrap();
    let s = out
        .into_iter()
        .collect::<proc_macro2::TokenStream>()
        .to_string();
    assert!(
        s.contains('(') && s.contains(')') && s.contains("42"),
        "pm2 group round-trips as one atom: {s}"
    );
}

//! `#[derive(TokenLeaves)]` on a custom token enum generates, per `#[leaf]`-annotated variant, a leaf
//! struct with the standard `Parse`/`Unparse`/`Spanned` trio (peek one atom → match this variant → push
//! the atom back and error on mismatch). Here a small `Token` enum (a keyword unit variant, an unnamed
//! payload variant renamed via `field`, and a named payload variant) is driven end-to-end over a
//! `Vec<WithSpan<Token, Sp>>` stream, round-tripping parse + unparse.
use core::convert::Infallible;
use syan::parse::{Parse, ParseStream, Unparse};
use syan::span::{Span as SpanTrait, Spanned, WithSpan};
use syan::TokenLeaves;

#[derive(Clone, Debug, Default, PartialEq)]
struct Sp(usize);

impl SpanTrait for Sp {
    fn migrate(self, other: Self) -> Self {
        Sp(self.0.max(other.0))
    }
}

type Atom = WithSpan<Token, Sp>;

#[derive(Clone, Debug, PartialEq, TokenLeaves)]
#[token_leaf(atom = "Atom", span = "|a| a.span.clone()")]
enum Token {
    #[leaf(name = "KwLet", expect = "'let'")]
    Let,
    #[leaf(name = "VarTok", expect = "a variable", field = "name")]
    Var(String),
    #[leaf(name = "NumTok", expect = "a number")]
    Num { value: i64 },
    // No `#[leaf]`: no leaf struct is generated for `Eq`.
    Eq,
}

/// A `Vec`-backed atom stream (a stand-in for the eager-lexer output the derive is meant to consume).
struct VecStream {
    atoms: std::vec::IntoIter<Atom>,
    buf: Vec<Atom>,
}

impl VecStream {
    fn new(atoms: Vec<Atom>) -> Self {
        Self {
            atoms: atoms.into_iter(),
            buf: Vec::new(),
        }
    }
}

impl ParseStream for VecStream {
    type Atom = Atom;
    type Error = Infallible;

    fn next(&mut self) -> Option<Atom> {
        self.buf.pop().or_else(|| self.atoms.next())
    }

    fn peek(&mut self) -> Option<&Atom> {
        if self.buf.is_empty() {
            if let Some(atom) = self.atoms.next() {
                self.buf.push(atom);
            }
        }
        self.buf.last()
    }

    fn push(&mut self, atom: Atom) {
        self.buf.push(atom);
    }
}

fn atom(token: Token, loc: usize) -> Atom {
    WithSpan {
        slot: token,
        span: Sp(loc),
    }
}

#[test]
fn round_trip_unit_and_payloads() {
    let source = vec![
        atom(Token::Let, 1),
        atom(Token::Var("x".into()), 2),
        atom(Token::Num { value: 42 }, 3),
    ];
    let mut stream = VecStream::new(source.clone());

    let kw: KwLet = Parse::parse(&mut stream).unwrap();
    assert_eq!(kw.span(), Sp(1));

    let var: VarTok = Parse::parse(&mut stream).unwrap();
    assert_eq!(var.name, "x");
    assert_eq!(var.span(), Sp(2));

    let num: NumTok = Parse::parse(&mut stream).unwrap();
    assert_eq!(num.value, 42);
    assert_eq!(num.span(), Sp(3));

    // Unparse each leaf back into an atom vec; the emitted atoms match the originals (slot + span).
    let mut out = Vec::<Atom>::new();
    kw.unparse(&mut (&mut out)).unwrap();
    var.unparse(&mut (&mut out)).unwrap();
    num.unparse(&mut (&mut out)).unwrap();
    assert_eq!(out, source);
}

#[test]
fn mismatch_pushes_atom_back() {
    // Parsing the wrong leaf must not consume: `VarTok` on a `Let` head errors, then `KwLet` still sees it.
    let mut stream = VecStream::new(vec![atom(Token::Let, 7)]);

    let wrong: Result<VarTok, _> = Parse::parse(&mut stream);
    let err = wrong.unwrap_err();
    assert_eq!(format!("{err}"), "expected a variable");

    let kw: KwLet = Parse::parse(&mut stream).unwrap();
    assert_eq!(kw.span(), Sp(7));
}

#[test]
fn end_of_input_errors() {
    let mut stream = VecStream::new(vec![]);
    let err = <KwLet as Parse<Atom>>::parse(&mut stream).unwrap_err();
    assert_eq!(format!("{err}"), "unexpected end of input, expected 'let'");
}

#[test]
fn non_leaf_variant_has_no_leaf() {
    // `Eq` carries no `#[leaf]`, so no `EqTok` struct is generated; an `Eq` atom just fails to match any
    // leaf (and is pushed back, so a second leaf attempt still sees it).
    let mut stream = VecStream::new(vec![atom(Token::Eq, 9)]);
    assert!(<KwLet as Parse<Atom>>::parse(&mut stream).is_err());
    assert!(<VarTok as Parse<Atom>>::parse(&mut stream).is_err());
}

#[test]
fn sequence_via_tuple_of_leaves() {
    // Leaves compose with the built-in combinators: a tuple parses them in order.
    let source = vec![atom(Token::Let, 10), atom(Token::Var("y".into()), 11)];
    let (kw, var): (KwLet, VarTok) = Parse::parse(VecStream::new(source.clone())).unwrap();
    assert_eq!(kw.span(), Sp(10));
    assert_eq!(var.name, "y");

    let mut out = Vec::<Atom>::new();
    (kw, var).unparse(&mut (&mut out)).unwrap();
    assert_eq!(out, source);
}

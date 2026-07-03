use syan::parse::{BufStream, IntoParseStream, Parse, ParseStream};
use syan::source::string::Span;
use syan::span::WithSpan;
use syan::symbol::{chars, Symbol};

type Atom = WithSpan<char, Span>;

fn atom(ch: char) -> Atom {
    WithSpan {
        slot: ch,
        span: Span::default(),
    }
}

// A `Vec<Atom>` is the canonical eager-lexer output — `into_parse_stream` must accept it directly.
#[test]
fn parse_leaves_from_vec() {
    let atoms: Vec<Atom> = "a+b".chars().map(atom).collect();
    let mut stream = atoms.into_parse_stream();

    assert!(Symbol::<chars::_a>::parse(&mut stream).is_ok());
    assert!(Symbol::<chars::Plus>::parse(&mut stream).is_ok());
    assert!(Symbol::<chars::_b>::parse(&mut stream).is_ok());
    assert!(stream.next().is_none());
}

// The `&[Atom]` impl clones into an owned `BufStream`.
#[test]
fn parse_leaves_from_slice() {
    let atoms: Vec<Atom> = "xy".chars().map(atom).collect();
    let mut stream = atoms.as_slice().into_parse_stream();

    assert!(Symbol::<chars::_x>::parse(&mut stream).is_ok());
    assert!(Symbol::<chars::_y>::parse(&mut stream).is_ok());
    assert!(stream.next().is_none());
}

// A mismatch pushes the atom back (LIFO buf), so the next leaf still sees it.
#[test]
fn pushback_on_mismatch() {
    let atoms: Vec<Atom> = "ab".chars().map(atom).collect();
    let mut stream: BufStream<Atom> = atoms.into_parse_stream();

    assert!(Symbol::<chars::_b>::parse(&mut stream).is_err());
    assert!(Symbol::<chars::_a>::parse(&mut stream).is_ok());
    assert!(Symbol::<chars::_b>::parse(&mut stream).is_ok());
    assert!(stream.next().is_none());
}

// peek is non-consuming and BufStream reports no separators (pre-tokenized).
#[test]
fn peek_and_skip_sep() {
    let atoms: Vec<Atom> = "z".chars().map(atom).collect();
    let mut stream = BufStream::new(atoms);

    assert_eq!(stream.peek().unwrap().slot, 'z');
    assert_eq!(stream.peek().unwrap().slot, 'z');
    assert!(!stream.skip_sep());
    assert_eq!(stream.next().unwrap().slot, 'z');
    assert!(stream.next().is_none());
}

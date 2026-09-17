//! `Ident`, the general word leaf, over text (`char`) and bytes (`u8`).

use syan::error::ParseError;
use syan::literal::ident::{Dotted, IdentClass, Rust};
use syan::literal::{Ident, Integer};
use syan::parse::{IntoParseStream, Parse, ParseStream};
use syan::span::{Spanned, WithSpan};
use syan::symbol::Token;

type Sp = syan::source::string::Span;
type CharAtom = WithSpan<char, Sp>;
type ByteAtom = WithSpan<u8, Sp>;

fn take<T: Parse<CharAtom>>(src: &str) -> (Option<T>, String) {
    let mut stream = src.into_parse_stream();
    let parsed = T::parse_stream(&mut stream).ok();
    let mut rest = String::new();
    while let Some(atom) = stream.next() {
        rest.push(atom.slot);
    }
    (parsed, rest)
}

#[test]
fn a_word_and_what_follows_it() {
    let (word, rest) = take::<Ident<Sp>>("total_count(");
    assert_eq!(word.unwrap().as_str(), "total_count");
    assert_eq!(rest, "(");

    let (word, rest) = take::<Ident<Sp>>("_x1 y");
    assert_eq!(word.unwrap().as_str(), "_x1");
    assert_eq!(rest, " y");
}

#[test]
fn a_failure_leaves_the_stream_untouched() {
    for src in ["42", "", ".name", "(x)"] {
        let (word, rest) = take::<Ident<Sp>>(src);
        assert!(word.is_none(), "expected a failure for {src:?}");
        assert_eq!(rest, src, "stream not restored after failing on {src:?}");
    }
}

/// The failure is what lets a following alternative match the same input.
#[test]
fn an_alternative_still_matches_after_the_failure() {
    let mut stream = "42".into_parse_stream();
    assert!(<Ident<Sp> as Parse<CharAtom>>::parse_stream(&mut stream).is_err());
    let int = <Integer as Parse<CharAtom>>::parse_stream(&mut stream).unwrap();
    assert_eq!(int.value, "42");
}

#[test]
fn the_span_is_where_the_word_starts() {
    let mut stream = "\n  name".into_parse_stream();
    stream.skip_sep();
    let word = <Ident<Sp> as Parse<CharAtom>>::parse_stream(&mut stream).unwrap();
    assert_eq!((word.span.line, word.span.col), (2, 3));
    assert_eq!(
        (Spanned::span(&word).line, Spanned::span(&word).col),
        (2, 3)
    );
}

#[test]
fn the_error_names_the_class() {
    let mut stream = "42".into_parse_stream();
    let err = <Ident<Sp> as Parse<CharAtom>>::parse_stream(&mut stream).unwrap_err();
    assert!(matches!(
        err,
        ParseError::Expected {
            what: "an identifier",
            ..
        }
    ));
}

#[test]
fn dotted_takes_dots_too() {
    let (word, rest) = take::<Ident<Sp, Dotted>>("core.mem.swap(");
    assert_eq!(word.unwrap().as_str(), "core.mem.swap");
    assert_eq!(rest, "(");

    // a dot cannot open one
    let (word, rest) = take::<Ident<Sp, Dotted>>(".mem");
    assert!(word.is_none());
    assert_eq!(rest, ".mem");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Hex;

impl IdentClass for Hex {
    const EXPECTED: &'static str = "a hex word";
    fn is_start(c: char) -> bool {
        c.is_ascii_hexdigit()
    }
    fn is_continue(c: char) -> bool {
        c.is_ascii_hexdigit()
    }
}

#[test]
fn a_grammar_can_bring_its_own_class() {
    let (word, rest) = take::<Ident<Sp, Hex>>("beefg");
    assert_eq!(word.unwrap().as_str(), "beef");
    assert_eq!(rest, "g");

    let mut stream = "zz".into_parse_stream();
    let err = <Ident<Sp, Hex> as Parse<CharAtom>>::parse_stream(&mut stream).unwrap_err();
    assert!(matches!(
        err,
        ParseError::Expected {
            what: "a hex word",
            ..
        }
    ));
}

#[test]
fn unicode_letters_count_for_the_rust_class() {
    assert!(Rust::is_start('é'));
    let (word, rest) = take::<Ident<Sp>>("naïve-");
    assert_eq!(word.unwrap().as_str(), "naïve");
    assert_eq!(rest, "-");
}

/// A word in a derived node, which is what a text grammar actually writes.
#[derive(Parse)]
struct Labelled<S> {
    name: Ident<S>,
    _colon: Token![S => :],
    value: Integer,
}

#[test]
fn inside_a_derived_node() {
    let node: Labelled<Sp> = Parse::parse("count : 42").unwrap();
    assert_eq!(node.name.as_str(), "count");
    assert_eq!(node.value.value, "42");
    assert!(Labelled::<Sp>::parse("42 : 42").is_err());
}

#[test]
fn value_semantics() {
    let word: Ident<Sp> = Parse::parse("abc").unwrap();
    let copy = word.clone();
    assert_eq!(copy.to_string(), "abc");
    assert_eq!(
        format!("{:?}", Ident::<()>::new("abc", ())),
        "Ident { value: \"abc\", span: () }"
    );
    assert_eq!(Ident::<()>::new("a", ()), Ident::<()>::new("a", ()));
}

mod over_bytes {
    use super::*;

    #[test]
    fn words_from_a_byte_source() {
        let mut stream = (&b"mov r1"[..]).into_parse_stream();
        let word = <Ident<Sp> as Parse<ByteAtom>>::parse_stream(&mut stream).unwrap();
        assert_eq!(word.as_str(), "mov");

        let mut stream = (&[0xFFu8, b'a'][..]).into_parse_stream();
        assert!(<Ident<Sp> as Parse<ByteAtom>>::parse_stream(&mut stream).is_err());
        assert_eq!(stream.next().map(|a| a.slot), Some(0xFF));
    }
}

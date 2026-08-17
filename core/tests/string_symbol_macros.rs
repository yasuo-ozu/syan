//! `Symbol!` and `Token!` over a `char` source.
//!
//! Both expand to `_Symbol<Joint<..>>`, which needs `Atom: AtomParsedToAllChars` -- satisfied only
//! because `source::string` implements `Parse` for the bare `chars::*` types. Multi-character
//! symbols additionally need `Joint` to see the atoms as adjacent, which is what `skip_sep`
//! returning `false` reports for text.

use syan::parse::Parse;
use syan::source::string::Span;
use syan::symbol::{chars, Symbol, Token};

#[derive(Parse)]
struct ViaSymbolMacro {
    _x: Symbol!(x),
}

#[derive(Parse)]
struct ViaTokenMacro {
    _eq: Token![Span => =],
}

#[derive(Parse)]
struct Keyword {
    _kw: Symbol!(let),
}

#[derive(Parse)]
struct Explicit {
    _x: Symbol<chars::_x>,
}

/// The README's `Assign`, written with the macros rather than by hand.
#[derive(Parse)]
struct Assign {
    _name: Symbol!(x),
    _space1: Vec<Symbol<chars::Space>>,
    _eq: Token![Span => =],
    _space2: Vec<Symbol<chars::Space>>,
    _value: Symbol!(1),
}

#[test]
fn symbol_macro_single_char() {
    assert!(Parse::parse("x".to_string())
        .map(|_: ViaSymbolMacro| ())
        .is_ok());
    assert!(Parse::parse("y".to_string())
        .map(|_: ViaSymbolMacro| ())
        .is_err());
}

#[test]
fn token_macro_carries_a_span() {
    assert!(Parse::parse("=".to_string())
        .map(|_: ViaTokenMacro| ())
        .is_ok());
    assert!(Parse::parse("+".to_string())
        .map(|_: ViaTokenMacro| ())
        .is_err());
}

#[test]
fn multi_char_keyword() {
    assert!(Parse::parse("let".to_string()).map(|_: Keyword| ()).is_ok());
    assert!(Parse::parse("lot".to_string())
        .map(|_: Keyword| ())
        .is_err());
}

/// A multi-character symbol is `Joint`: the characters must be adjacent.
#[test]
fn multi_char_keyword_rejects_a_gap() {
    assert!(Parse::parse("l et".to_string())
        .map(|_: Keyword| ())
        .is_err());
}

#[test]
fn explicit_symbol_form_still_works() {
    assert!(Parse::parse("x".to_string()).map(|_: Explicit| ()).is_ok());
}

#[test]
fn assign_from_text() {
    assert!(Parse::parse("x = 1".to_string())
        .map(|_: Assign| ())
        .is_ok());
    assert!(Parse::parse("x=1".to_string()).map(|_: Assign| ()).is_ok());
    assert!(Parse::parse("x = 2".to_string())
        .map(|_: Assign| ())
        .is_err());
}

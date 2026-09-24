//! `serde` feature: a derived AST round-trips through JSON.
//!
//! Structs whose fields use `Token!`/`Symbol!` cannot carry the *built-in* derives (`Debug`,
//! `PartialEq`, …) — rustc rejects `derive` on an item containing a type macro. serde's derive is a
//! proc macro and is not restricted, but it cannot see through the macro to infer bounds, so a
//! span parameter needs an explicit `#[serde(bound(..))]`. Round-trips are therefore checked by
//! comparing the encoded form rather than with `PartialEq`.

#![cfg(feature = "serde")]
// `Punctuated<Integer, Token![S => ,]>` is the grammar, not an accident of nesting.
#![allow(clippy::type_complexity)]

use serde::{Deserialize, Serialize};
use syan::literal::Integer;
use syan::nested::group::GroupParen;
use syan::nested::Punctuated;
use syan::parse::Parse;
use syan::symbol::Token;

type Text = syan::source::string::Span;

#[derive(Parse, Serialize, Deserialize)]
#[serde(bound(serialize = "S: Serialize", deserialize = "S: Deserialize<'de>"))]
struct Assign<S> {
    name: Token![S => x],
    eq: Token![S => =],
    value: Integer,
}

#[derive(Parse, Serialize, Deserialize)]
#[serde(bound(serialize = "S: Serialize", deserialize = "S: Deserialize<'de>"))]
struct Call<S> {
    name: Token![S => f],
    paren: GroupParen<(), S>,
    #[group(self.paren)]
    args: Punctuated<Integer, Token![S => ,]>,
}

#[derive(Parse, Serialize, Deserialize)]
#[serde(bound(serialize = "S: Serialize", deserialize = "S: Deserialize<'de>"))]
struct Many<S> {
    items: Vec<Integer>,
    end: Option<Token![S => ;]>,
}

#[derive(Parse, Serialize, Deserialize)]
#[serde(bound(serialize = "S: Serialize", deserialize = "S: Deserialize<'de>"))]
struct Whole<S> {
    items: Vec<Integer>,
    eof: syan::nested::Eof,
    #[serde(skip)]
    _s: core::marker::PhantomData<S>,
}

/// Encode, decode, re-encode: equal encodings mean the value survived intact.
fn roundtrip<T>(v: &T) -> (String, String)
where
    T: Serialize + for<'de> Deserialize<'de>,
{
    let first = serde_json::to_string(v).expect("serialize");
    let back: T = serde_json::from_str(&first).expect("deserialize");
    let second = serde_json::to_string(&back).expect("re-serialize");
    (first, second)
}

#[test]
fn a_derived_ast_round_trips() {
    let a: Assign<Text> = Parse::parse("x = 1").unwrap();
    let (first, second) = roundtrip(&a);
    assert_eq!(first, second);
}

#[test]
fn text_spans_are_real_data_and_survive() {
    let a: Assign<Text> = Parse::parse("x = 1").unwrap();
    let json = serde_json::to_string(&a).unwrap();
    let back: Assign<Text> = serde_json::from_str(&json).unwrap();
    assert_eq!(
        (back.eq.span.line, back.eq.span.col, back.eq.span.loc),
        (1, 3, 2)
    );
    assert_eq!(back.value.value, "1");
}

#[test]
fn groups_and_punctuated_round_trip() {
    let c: Call<Text> = Parse::parse("f( 1, 2, 3 )").unwrap();
    assert_eq!(c.args.len(), 3);
    let json = serde_json::to_string(&c).unwrap();
    let back: Call<Text> = serde_json::from_str(&json).unwrap();
    assert_eq!(back.args.len(), 3, "the separated list survives");
    assert_eq!(serde_json::to_string(&back).unwrap(), json);
}

#[test]
fn optional_and_repeated_fields_round_trip() {
    let m: Many<Text> = Parse::parse("1 2 3;").unwrap();
    assert_eq!(m.items.len(), 3);
    assert!(m.end.is_some());
    let (first, second) = roundtrip(&m);
    assert_eq!(first, second);

    let empty: Many<Text> = Parse::parse(";").unwrap();
    assert_eq!(empty.items.len(), 0);
    let (a, b) = roundtrip(&empty);
    assert_eq!(a, b);
}

/// A symbol is a zero-sized type: it costs one `null`, and carries no `chars::*` bound.
#[test]
fn a_symbol_encodes_as_unit() {
    let a: Assign<Text> = Parse::parse("x = 1").unwrap();
    let json = serde_json::to_string(&a).unwrap();
    assert!(json.contains(r#""slot":null"#), "symbol is unit: {json}");
    assert!(json.contains(r#""line":1"#), "span is data: {json}");
}

/// `ParseError` is `Serialize` only — its `what` is a `&'static str`, which cannot be deserialized.
#[test]
fn a_parse_error_serializes_for_reporting() {
    let e = Parse::parse("y = 1").map(|_: Assign<Text>| ()).unwrap_err();
    let json = serde_json::to_string(&e).expect("errors serialize");
    assert!(json.contains("Expected"), "{json}");
}

/// The encoding is `{items, puncts}`, not the private inner layout, and the invariant is enforced.
#[test]
fn punctuated_has_a_stable_encoding() {
    let c: Call<Text> = Parse::parse("f(1, 2, 3)").unwrap();
    let json = serde_json::to_string(&c.args).unwrap();
    assert!(json.starts_with(r#"{"items":["#), "{json}");
    assert!(json.contains(r#""puncts":["#), "{json}");
    assert!(
        !json.contains("inner"),
        "private layout must not leak: {json}"
    );

    let empty: Punctuated<Integer, Token![Text => ,]> = Default::default();
    assert_eq!(
        serde_json::to_string(&empty).unwrap(),
        r#"{"items":[],"puncts":[]}"#
    );

    // One item must carry zero puncts; two puncts is malformed. (`Integer` as the punct type here
    // only so the malformed input is easy to write by hand.)
    let one = r#"{"value":"1","suffix":null}"#;
    let bad = format!(r#"{{"items":[{one}],"puncts":[{one},{one}]}}"#);
    let e = serde_json::from_str::<Punctuated<Integer, Integer>>(&bad).unwrap_err();
    assert!(e.to_string().contains("one fewer punct"), "{e}");
}

#[test]
fn a_node_ending_in_eof_round_trips() {
    // `Eof` is a marker: it carries nothing, so it encodes as a unit struct and reads back as
    // itself. A grammar that ends with one still round-trips whole.
    let w: Whole<Text> = Parse::parse("1 2 3").expect("parses to the end");
    assert_eq!(w.items.len(), 3);
    let (first, second) = roundtrip(&w);
    assert_eq!(first, second);
    let back: Whole<Text> = serde_json::from_str(&first).unwrap();
    assert_eq!(back.items.len(), 3);
    assert_eq!(back.eof, syan::nested::Eof);
    assert_eq!(
        serde_json::to_string(&w.eof).unwrap(),
        "null",
        "a marker encodes as unit"
    );
}

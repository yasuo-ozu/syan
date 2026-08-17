// Parses `proc_macro2` tokens throughout; the whole suite is skipped without the optional
// dependency that provides them.
#![cfg(feature = "proc_macro2")]

//! `Unordered<T, U>` parses `T` and `U` in either order, keeps both, and unparses back in the order it
//! saw them.
use syan::literal::{Bool, Integer};
use syan::nested::Unordered;
use syan::parse::{Parse, Unparse};
use syan::span::{Spanned, WithSpan};
use template_quote::quote;

type IntBool = Unordered<Integer, Bool>;

fn unparsed(u: &IntBool) -> String {
    let mut out = Vec::<proc_macro2::TokenTree>::new();
    u.unparse(&mut (&mut out)).unwrap();
    out.iter()
        .map(|t| t.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn parses_t_then_u() {
    let u: IntBool = Parse::parse(quote! { 5 true }).unwrap();
    assert!(u.t_first(), "`Integer` came first");
    assert_eq!(u.t.value, "5");
    assert!(u.u.value);
    // Round-trips in the order it was written.
    assert_eq!(unparsed(&u), "5 true");
}

#[test]
fn parses_u_then_t() {
    // `Integer`-first fails on `false`, so it backtracks and parses `U T` (`Bool` then `Integer`).
    let u: IntBool = Parse::parse(quote! { false 9 }).unwrap();
    assert!(!u.t_first(), "`Bool` came first");
    assert_eq!(u.t.value, "9");
    assert!(!u.u.value);
    // Unparse preserves the input order (`Bool` before `Integer`).
    assert_eq!(unparsed(&u), "false 9");
}

#[test]
fn new_chooses_t_first_order() {
    let u = IntBool::new(
        Integer {
            value: "1".into(),
            suffix: None,
        },
        Bool { value: true },
    );
    assert!(u.t_first());
    assert_eq!(unparsed(&u), "1 true");
    let (t, b) = u.into_inner();
    assert_eq!(t.value, "1");
    assert!(b.value);
}

#[test]
fn span_folds_both_in_input_order() {
    // Constructed (not parsed) so the span type can be `()`; just assert `Spanned` is callable.
    let u = Unordered::new(
        WithSpan {
            slot: 1u32,
            span: (),
        },
        WithSpan {
            slot: 2u64,
            span: (),
        },
    );
    let _s: () = u.span();
}

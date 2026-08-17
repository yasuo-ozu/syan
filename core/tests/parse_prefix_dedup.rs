// Parses `proc_macro2` tokens throughout; the whole suite is skipped without the optional
// dependency that provides them.
#![cfg(feature = "proc_macro2")]

//! `#[derive(Parse)]` prefix-dedup: when enum variants share a leading field (`E | E!`), the shared
//! prefix is parsed ONCE up front, not re-parsed inside each variant's backtracking attempt.
#![allow(dead_code)] // variants are matched, not field-read
use std::sync::atomic::{AtomicUsize, Ordering};
use syan::literal::{Bool, Integer};
use syan::parse::Parse;
use template_quote::quote;

static PREFIX_PARSES: AtomicUsize = AtomicUsize::new(0);

// A parse-counting stand-in for a shared first field, so we can SEE how many times it is parsed.
#[derive(Debug)]
struct Counted(Integer);
impl Parse<proc_macro2::TokenTree> for Counted {
    type Error = syan::error::ParseError<syan::source::proc_macro2::Span>;
    fn parse_stream<__S: syan::parse::ParseStream<Atom = proc_macro2::TokenTree>>(
        stream: &mut __S,
    ) -> Result<Self, Self::Error> {
        PREFIX_PARSES.fetch_add(1, Ordering::SeqCst);
        Integer::parse(stream).map(Counted)
    }
}

// Both variants begin with `Counted`; `Tagged` is declared first so it's tried first (longest match).
#[derive(syan::parse::Parse, Debug)]
enum E {
    Tagged(Counted, Bool), // `E <bool>`
    Plain(Counted),        // `E`
}

// ── 3-variant prefix chain (declared longest-first) ──────────────────────────────────────────────
#[derive(syan::parse::Parse, Debug)]
enum Chain {
    Three(Integer, Bool, Integer),
    Two(Integer, Bool),
    One(Integer),
}

#[test]
fn three_variant_chain() {
    assert!(matches!(Chain::parse(quote! { 5 }).unwrap(), Chain::One(_)));
    assert!(matches!(
        Chain::parse(quote! { 5 true }).unwrap(),
        Chain::Two(_, _)
    ));
    assert!(matches!(
        Chain::parse(quote! { 5 true 6 }).unwrap(),
        Chain::Three(_, _, _)
    ));
}

// ── divergent suffixes after a shared prefix (no empty-suffix fallback) ───────────────────────────
#[derive(syan::parse::Parse, Debug)]
enum Div {
    Flag(Integer, Bool),    // `<int> <bool>`
    Pair(Integer, Integer), // `<int> <int>`
}

#[test]
fn divergent_suffixes() {
    assert!(matches!(
        Div::parse(quote! { 5 true }).unwrap(),
        Div::Flag(_, _)
    ));
    assert!(matches!(
        Div::parse(quote! { 5 6 }).unwrap(),
        Div::Pair(_, _)
    ));
    assert!(
        Div::parse(quote! { 5 }).is_err(),
        "neither suffix matches → error"
    );
}

// ── named-field shared prefix ─────────────────────────────────────────────────────────────────────
#[derive(syan::parse::Parse, Debug)]
enum Named {
    Tagged { a: Integer, b: Bool },
    Plain { a: Integer },
}

#[test]
fn named_field_prefix() {
    assert!(matches!(
        Named::parse(quote! { 5 true }).unwrap(),
        Named::Tagged { .. }
    ));
    assert!(matches!(
        Named::parse(quote! { 5 }).unwrap(),
        Named::Plain { .. }
    ));
}

// ── a prefix-sharing enum INSIDE a `#[recurse]` cycle (factored codegen in the engine) ───────────
#[syan::parse::recurse]
mod rec {
    use core::marker::PhantomData;
    use syan::literal::{Bool, Integer};
    use syan::parse::{Parse, Unparse};

    #[derive(Parse, Unparse)]
    pub enum Expr<S> {
        // Shared leading `Integer`; `Tagged` continues with a `Bool` then recurses.
        Tagged(Integer, Bool, Box<Expr<S>>),
        Lit(Integer, PhantomData<S>),
    }
}

#[test]
fn prefix_sharing_recurse_cycle() {
    use rec::Expr;
    // `5` → Lit; `5 true 6` → Tagged(5, true, Lit(6)). The factored enum codegen runs inside the recurse
    // engine (which has its own backtracking + terminator re-entry) and still parses correctly.
    assert!(matches!(
        Expr::<()>::parse(quote! { 5 }).unwrap(),
        Expr::Lit(..)
    ));
    let e: Expr<()> = Parse::parse(quote! { 5 true 6 }).unwrap();
    match e {
        Expr::Tagged(_, _, inner) => assert!(matches!(*inner, Expr::Lit(..))),
        _ => panic!("expected Tagged"),
    }
}

#[test]
fn shared_prefix_parsed_once() {
    let parse_counting = |ts: proc_macro2::TokenStream| -> (E, usize) {
        PREFIX_PARSES.store(0, Ordering::SeqCst);
        let e: E = Parse::parse(ts).unwrap();
        (e, PREFIX_PARSES.load(Ordering::SeqCst))
    };

    // `5` → Plain (Tagged's `Bool` fails, rewinds the *suffix* only), prefix parsed exactly ONCE.
    let (e, n) = parse_counting(quote! { 5 });
    assert!(matches!(e, E::Plain(_)));
    assert_eq!(
        n, 1,
        "prefix parsed once for `5` (no re-parse on the Tagged→Plain fallback)"
    );

    // `5 true` → Tagged, also one prefix parse.
    let (e, n) = parse_counting(quote! { 5 true });
    assert!(matches!(e, E::Tagged(_, _)));
    assert_eq!(n, 1, "prefix parsed once for `5 true`");
}

// Parses `proc_macro2` tokens throughout; the whole suite is skipped without the optional
// dependency that provides them.
#![cfg(feature = "proc_macro2")]

//! The `Group<T, O, C>` **type form** of `#[group]` inside a `#[recurse]` cycle. `ty` and `attr` are
//! documented as two spellings of one thing; before the fix only `attr` compiled, because decycle
//! peeled `GroupBracket<Vec<Item<S>>, S>: Parse<A>` down to `Item<S>: Parse<A>` and the delimiters'
//! premise went with it (likewise for `Unparse` and `Spanned`).

use syan::parse::{Parse, Unparse};
use syan::span::Spanned;
use template_quote::quote;

#[syan::parse::recurse]
mod ty {
    use syan::literal::Integer;
    use syan::nested::group::GroupBracket;
    use syan::parse::{Parse, Unparse};

    #[derive(Parse, Unparse, Debug)]
    pub struct List<S> {
        pub group: GroupBracket<Vec<Item<S>>, S>,
    }
    #[derive(Parse, Unparse, Debug)]
    pub enum Item<S> {
        Nested(List<S>),
        Num(Integer),
    }
}

#[syan::parse::recurse]
mod attr {
    use syan::literal::Integer;
    use syan::nested::group::GroupBracket;
    use syan::parse::{Parse, Unparse};

    #[derive(Parse, Unparse, Debug)]
    pub struct List<S> {
        pub b: GroupBracket<(), S>,
        #[group(self.b)]
        pub items: Vec<Item<S>>,
    }
    #[derive(Parse, Unparse, Debug)]
    pub enum Item<S> {
        Nested(List<S>),
        Num(Integer),
    }
}

#[syan::parse::recurse]
mod spanned {
    use syan::literal::Integer;
    use syan::nested::group::GroupParen;
    use syan::parse::Parse;
    use syan::source::proc_macro2::Span;
    use syan::span::{Spanned, WithSpan};

    #[derive(Parse, Spanned)]
    pub struct List {
        pub group: GroupParen<Vec<Item>, Span>,
    }
    #[derive(Parse, Spanned)]
    pub enum Item {
        Nested(List),
        Num(WithSpan<Integer, Span>),
    }
}

#[syan::parse::recurse(structural)]
mod structural {
    use syan::literal::Integer;
    use syan::nested::group::GroupBrace;
    use syan::parse::{Parse, Unparse};

    #[derive(Parse, Unparse, Debug)]
    pub struct List<S> {
        pub group: GroupBrace<Vec<Item<S>>, S>,
    }
    #[derive(Parse, Unparse, Debug)]
    pub enum Item<S> {
        Nested(List<S>),
        Num(Integer),
    }
}

fn round_trip<T: Parse<proc_macro2::TokenTree> + Unparse<proc_macro2::TokenTree>>(
    src: proc_macro2::TokenStream,
) -> T
where
    T::Error: std::fmt::Debug,
{
    let v: T = Parse::parse(src.clone()).unwrap();
    let mut out = proc_macro2::TokenStream::new();
    v.unparse(&mut out).unwrap();
    assert_eq!(out.to_string(), src.to_string());
    v
}

#[test]
fn type_form_parses_nested_input() {
    let l: ty::List<_> = round_trip(quote! { [1 [2] 3] });
    assert_eq!(l.group.slot.len(), 3);
    assert!(matches!(&l.group.slot[1], ty::Item::Nested(inner) if inner.group.slot.len() == 1));
}

#[test]
fn attribute_form_parses_nested_input() {
    let l: attr::List<_> = round_trip(quote! { [1 [2] 3] });
    assert_eq!(l.items.len(), 3);
}

#[test]
fn type_form_is_deep() {
    let mut src = quote! { 7 };
    for _ in 0..40 {
        src = quote! { [ #src ] };
    }
    let _: ty::List<_> = round_trip(src);
}

#[test]
fn type_form_under_the_structural_engine() {
    let l: structural::List<_> = round_trip(quote! { {1 {2} 3} });
    assert_eq!(l.group.slot.len(), 3);
}

#[test]
fn type_form_spans_its_delimiters() {
    let l: spanned::List = Parse::parse(quote! { (1 (2) 3) }).unwrap();
    let span: Option<proc_macro2::Span> = l.span().into();
    assert!(span.is_some());
    assert_eq!(l.group.slot.len(), 3);
}

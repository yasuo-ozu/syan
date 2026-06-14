use proc_macro::TokenStream as TokenStream1;
use proc_macro_error::proc_macro_error;
use syn::*;

mod ast;
mod attribute;
mod recurse;
mod symbol;
mod visitor;

use crate::attribute::FindAttribute;

fn random() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    std::collections::hash_map::RandomState::new()
        .build_hasher()
        .finish()
}

#[proc_macro_error]
#[proc_macro_derive(Parse, attributes(group, syan, joint, alone, ignore_bounds,))]
pub fn parse_derive(input: TokenStream1) -> TokenStream1 {
    let input: DeriveInput = parse_macro_input!(input);
    let syan = input.attrs.get_syan();
    let trait_path: Path = parse_quote!(#syan::parse::parse::Parse);
    attribute::parse(
        &input.ident,
        &input.generics,
        &input.data,
        random(),
        &syan,
        &trait_path,
    )
    .into()
}

#[proc_macro_error]
#[proc_macro_derive(Unparse, attributes(group, syan, joint, alone, ignore_bounds,))]
pub fn unparse(input: TokenStream1) -> TokenStream1 {
    let input: DeriveInput = parse_macro_input!(input);
    let syan = input.attrs.get_syan();
    let trait_path: Path = parse_quote!(#syan::parse::unparse::Unparse);
    attribute::unparse(
        &input.ident,
        &input.generics,
        &input.data,
        random(),
        &syan,
        &trait_path,
    )
    .into()
}

#[proc_macro_error]
#[proc_macro_derive(Ast, attributes(syan))]
pub fn ast_derive(input: TokenStream1) -> TokenStream1 {
    let input: DeriveInput = parse_macro_input!(input);
    let syan = input.attrs.get_syan();
    ast::derive_ast(&input, random(), &syan).into()
}

#[proc_macro_error]
#[proc_macro_derive(Spanned)]
pub fn spanned(input: TokenStream1) -> TokenStream1 {
    let input: DeriveInput = parse_macro_input!(input);
    let syan = input.attrs.get_syan();
    let trait_path: Path = parse_quote!(#syan::span::Spanned);
    attribute::spanned(&input, trait_path).into()
}

#[proc_macro_error]
#[proc_macro]
pub fn symbol(input: TokenStream1) -> TokenStream1 {
    let args = parse_macro_input!(input as symbol::SymbolArgs);
    symbol::symbol(args).into()
}

#[proc_macro_error]
#[proc_macro_attribute]
pub fn recurse(attr: TokenStream1, input: TokenStream1) -> TokenStream1 {
    recurse::recurse(attr, input)
}

#[proc_macro_error]
#[doc(hidden)]
#[proc_macro]
pub fn __visitor_entry(input: TokenStream1) -> TokenStream1 {
    visitor::entry(input.into(), random()).into()
}

#[proc_macro_error]
#[doc(hidden)]
#[proc_macro]
pub fn __visitor_build(input: TokenStream1) -> TokenStream1 {
    visitor::build(input.into()).into()
}

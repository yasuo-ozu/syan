use proc_macro::TokenStream as TokenStream1;
use proc_macro_error::proc_macro_error;
use syn::*;

mod attribute;
mod recurse;
mod symbol;

fn random() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    std::collections::hash_map::RandomState::new()
        .build_hasher()
        .finish()
}

#[proc_macro_error]
#[proc_macro_attribute]
pub fn recurse(_attr: TokenStream1, input: TokenStream1) -> TokenStream1 {
    recurse::recurse(parse_macro_input!(input)).into()
}

#[proc_macro_error]
#[proc_macro_derive(
    Parse,
    attributes(
        group,
        syan,
        joint,
        alone,
        ignore_bounds,
        fundamental_tys,
        predicate,
        predicate_parse,
        predicate_unparse
    )
)]
pub fn parse_derive(input: TokenStream1) -> TokenStream1 {
    attribute::parse(&parse_macro_input!(input), random()).into()
}

#[proc_macro_error]
#[proc_macro_derive(
    Unparse,
    attributes(
        group,
        syan,
        joint,
        alone,
        ignore_bounds,
        fundamental_tys,
        predicate,
        predicate_parse,
        predicate_unparse
    )
)]
pub fn unparse(input: TokenStream1) -> TokenStream1 {
    attribute::unparse(&parse_macro_input!(input), random()).into()
}

#[proc_macro_error]
#[proc_macro_derive(Spanned)]
pub fn spanned(input: TokenStream1) -> TokenStream1 {
    attribute::spanned(&parse_macro_input!(input)).into()
}

#[proc_macro_error]
#[proc_macro]
pub fn symbol(input: TokenStream1) -> TokenStream1 {
    let args = parse_macro_input!(input as symbol::SymbolArgs);
    symbol::symbol(args).into()
}

use proc_macro::TokenStream as TokenStream1;
use proc_macro_error::proc_macro_error;
use syn::*;

mod attribute;

fn random() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    std::collections::hash_map::RandomState::new()
        .build_hasher()
        .finish()
}

#[proc_macro_error]
#[proc_macro_derive(Parse, attributes(group, syan, joint, alone))]
pub fn parse_derive(input: TokenStream1) -> TokenStream1 {
    attribute::parse(&parse_macro_input!(input), random()).into()
}

#[proc_macro_error]
#[proc_macro_derive(Unparse, attributes(group, syan, joint, alone))]
pub fn unparse(input: TokenStream1) -> TokenStream1 {
    attribute::unparse(&parse_macro_input!(input), random()).into()
}

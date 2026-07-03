use proc_macro2::{Spacing, Span, TokenStream};
use proc_macro_error::abort;
use std::collections::VecDeque;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::*;
use template_quote::quote;

mod adt;
mod find;
mod substruct;
mod token_leaves;
// `pub(crate)` so the `#[recurse]` decycle path (feature `recurse-decycle`) can reach `adt::Adt`
// (`Adt::extract_parse_dyn`) — a re-export, so it is never "unused" regardless of feature.
pub(crate) use adt::*;
use find::*;
use substruct::*;

pub(crate) use find::FindAttribute;
pub fn parse(
    ident: &Ident,
    generics: &Generics,
    input: &Data,
    nonce: u64,
    syan: &Path,
    trait_path: &Path,
) -> TokenStream {
    match input {
        Data::Struct(data_struct) => {
            data_struct.extract_parse(syan, generics, ident, nonce, trait_path)
        }
        Data::Enum(data_enum) => data_enum.extract_parse(syan, generics, ident, nonce, trait_path),
        _ => abort!(ident, "Bad data"),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn unparse(
    ident: &Ident,
    generics: &Generics,
    input: &Data,
    attrs: &[Attribute],
    nonce: u64,
    syan: &Path,
    trait_path: &Path,
) -> TokenStream {
    match &input {
        Data::Struct(data_struct) => {
            data_struct.extract_unparse(syan, generics, ident, attrs, nonce, trait_path)
        }
        Data::Enum(data_enum) => {
            data_enum.extract_unparse(syan, generics, ident, attrs, nonce, trait_path)
        }
        _ => abort!(ident, "Bad data"),
    }
}

pub fn token_leaves(input: &DeriveInput, nonce: u64, syan: &Path) -> TokenStream {
    token_leaves::token_leaves(input, nonce, syan)
}

pub fn spanned(input: &DeriveInput, trait_path: Path) -> TokenStream {
    let syan = input.attrs.get_syan();
    match &input.data {
        Data::Struct(data_struct) => {
            data_struct.extract_spanned(&syan, &input.generics, &input.ident, &input.attrs, &trait_path)
        }
        Data::Enum(data_enum) => {
            data_enum.extract_spanned(&syan, &input.generics, &input.ident, &input.attrs, &trait_path)
        }
        _ => abort!(input, "Bad data"),
    }
}

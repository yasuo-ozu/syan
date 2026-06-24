use proc_macro::TokenStream as TokenStream1;
use proc_macro_error::proc_macro_error;
use syn::*;

mod ast;
mod attribute;
mod recurse;
mod symbol;
mod util;
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
#[proc_macro_derive(Ast, attributes(syan, subast))]
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

/// Turn a module of mutually-recursive AST types (a *cycle*) into depth-limited concrete types.
/// `#[recurse(visit)]` additionally emits a depth-generic visitor; `limit = N` sets the depth.
///
/// # The recursion root
///
/// The **root** is the single cycle type that controls recursion depth (its back-edges become the
/// depth parameter `__Rec`). It is chosen automatically: a directly **self-referential** cycle type
/// if one exists (the alphabetically-first when several), otherwise the cycle type most referenced by
/// the others (as a bare field; ties broken by total references, then alphabetically). Any directly
/// self-referential type is *also* treated as a root and collapses to `__Rec`.
///
/// # Generic arguments on a reference to the recursion root
///
/// A reference to a **root** type is the cycle's back-edge and collapses to the single depth
/// parameter `__Rec`, so it must repeat the root's own parameters **verbatim (identity)**. A complex
/// or substituted argument there is *non-regular* recursion (the param would grow at every level),
/// which the single-`__Rec` depth machinery cannot express, so it is **rejected**:
///
/// ```text
/// // root `Expr<S>`:
/// Box<Expr<S>>          // OK   — identity back-edge
/// Box<Expr<Vec<S>>>     // ERROR — wrapped param
/// Box<Expr<u8>>         // ERROR — concrete substitution
/// Stmt<Expr<Vec<S>>>    // ERROR — even nested inside a cross-edge
/// ```
///
/// Complex arguments are only unsupported on a root back-edge. They are fine on a **cross-edge** to a
/// non-root cycle type (`Box<Stmt<S, u8>>` — `u8` fills `Stmt`'s own param) and on **non-cycle** types
/// (`Vec<S>`, `Option<S>`). Workaround for the rejected case: move the differing part into its own
/// `#[derive(Ast)]` type.
#[proc_macro_error]
#[proc_macro_attribute]
pub fn recurse(attr: TokenStream1, input: TokenStream1) -> TokenStream1 {
    recurse::recurse(attr, input, random())
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

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

/// Derive `Parse` — build the type from a token stream, field by field.
///
/// # Field attributes
///
/// - `#[group(self.field)]`, `#[joint]`, `#[alone]`, `#[default]` — grouping/spacing/skip controls.
///
/// A **mutually recursive** AST needs `#[recurse]` on the enclosing module: the per-field bounds this
/// derive synthesizes would otherwise make each type's impl conditional on its sibling's, so neither
/// is usable. `#[recurse]` routes `Parse` through `decycle`, which contracts the cyclic bound. The
/// stream type needs no separate fix: `Parse::parse_stream` takes `&mut S` and recursive calls
/// reborrow, so `S` is a fixed point.
#[proc_macro_error]
#[proc_macro_derive(Parse, attributes(group, syan, joint, alone,))]
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

/// Derive `Unparse` — emit the type back to a token sink, field by field.
///
/// # Field attributes
///
/// - `#[group(self.field)]`, `#[joint]`, `#[alone]`, `#[default]` — grouping/spacing/skip controls.
///
/// As for `Parse`, a **mutually recursive** AST needs `#[recurse]` on the enclosing module — the
/// per-field bounds synthesized here would otherwise make each impl conditional on its sibling's.
#[proc_macro_error]
#[proc_macro_derive(Unparse, attributes(group, syan, joint, alone,))]
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
#[proc_macro_derive(Ast, attributes(syan, subast, seq, opt))]
pub fn ast_derive(input: TokenStream1) -> TokenStream1 {
    let input: DeriveInput = parse_macro_input!(input);
    let syan = input.attrs.get_syan();
    ast::derive_ast(&input, random(), &syan).into()
}

/// Derive `Spanned` — compute the type's span by folding (`Span::migrate`) its fields' spans.
///
/// # Field attributes
///
/// - `#[default]` — excludes a field from the span fold (and from the synthesized predicate).
///
/// Every folded field gets a `FieldTy: Spanned<Span = __Syan_Span>` predicate, which is what pins the
/// invented span param. As for `Parse`/`Unparse`, a mutually recursive AST needs `#[recurse]`.
#[proc_macro_error]
#[proc_macro_derive(Spanned, attributes(group, syan, joint, alone,))]
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

/// Turn a module of mutually-recursive AST types (a *cycle*) into **natural recursive public types**
/// whose circular trait obligations are broken by the external
/// [`decycle`](https://docs.rs/decycle) crate.
///
/// # Arguments
///
/// - **`#[recurse]`** — decycle's **ranked** engine (the default): twin `TrRanked<Rank>` traits and a
///   rank ladder, with unbounded depth via decycle's re-entry registry. The ladder discharges the
///   *obligation* only; a recursive call re-enters through the un-ranked delegating impl at full
///   height, so runtime depth is bounded only by the OS call stack.
/// - **`#[recurse(structural)]`** — decycle's **structural** engine: a compile-time unroll with a
///   `#[repr(transparent)]` terminator, so there is no runtime registry and no `type-leak`. Narrower
///   scope in exchange.
///
/// Both produce the same public types and the same parse results; they differ in how the cyclic
/// obligation is discharged, and therefore in compile time and in generated-code shape.
/// `bench/` measures both against `nom` and `chumsky`.
///
/// (`limit = N` was removed; the ranked ladder's height is not user-tunable.)
///
/// The user's types are emitted verbatim — genuine recursive enums/structs, one type at all depths, no
/// engine type and no depth parameter. `#[derive(Ast)]`, `Debug`, `#[subast]`, user `impl`s and every
/// non-structural derive land on them directly, and `syan::visit::visitor!(<cycle types>)` over them is
/// an **ordinary acyclic visitor** (closures included).
///
/// # What it does
///
/// 1. Find the cycles (SCCs over the module's type-reference graph) and check the preconditions below.
/// 2. **Expand `#[derive(Parse/Unparse/Spanned)]` itself**, for every type in the module, through the
///    same entry points the ordinary derives use — so bodies, `#[group]` handling, prefix-dedup and
///    spacing are byte-identical to a non-`#[recurse]` derive.
/// 3. Reshape each cycle member's generated `impl` into the form `decycle` can contract, and hand the
///    module to `decycle`.
///
/// # Which traits are routed
///
/// **`Parse`, `Unparse` and `Spanned`** all go through `decycle`. Depth is **unbounded** modulo the
/// OS call stack: a recursive call re-enters through the un-ranked delegating impl at full height, so
/// decycle's rank ladder only discharges the *obligation*. (Deriving `Parse` naively cannot work —
/// the per-field bounds form an E0275 cycle. A *second*, independent cycle used to exist as well:
/// `Parse::parse` took `impl IntoParseStream` — a generic parameter, which *moves* rather than
/// reborrows, so each descent level asked for `parse::<&mut &mut …>`, an infinite *monomorphization*
/// chain no obligation engine can break. The required method is now `parse_stream(&mut S)` and
/// recursive calls reborrow (`&mut *stream`), so `S` is a genuine fixed point and no erasure is
/// involved.) As for any recursive-descent parser, a *left-recursive*
/// grammar recurses forever rather than being truncated. `Spanned`'s `Span = _` associated-type
/// constraint travels through unchanged — verbatim on the peeled cyclic bounds (admitted when the
/// target is a cycle self head), and bound through the `SpannedBound` supertrait alias on the leaf
/// bounds.
///
/// # Preconditions
///
/// **Finite size.** A natural recursive type must be finite-size, so a **pure by-value cycle** (no
/// `Box`/`Vec`/… on any cycle edge) is rejected with a clean error rather than E0072.
///
/// **Regular recursion.** A cycle edge must not *grow* its generic arguments:
///
/// ```text
/// Box<Expr<S>>          // OK    — parameters passed through unchanged
/// Box<Stmt<S, u8>>      // OK    — parameter-free concrete substitution (finite family)
/// Box<Expr<Vec<S>>>     // ERROR — `S` grows every level
/// ```
///
/// The last shape is unsupportable by anything: its instantiation family is infinite, so the *type* is
/// already unusable (`E0320` on any concrete binding) and the parse fn family exceeds the
/// monomorphization limit. Move the differing part into its own type outside the cycle.
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

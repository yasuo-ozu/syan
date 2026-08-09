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
/// - **`#[ignore_bounds]`** — suppress the `FieldTy: Parse<_>` where-predicate this derive would
///   otherwise synthesize for the field. The field is still parsed (the obligation is just not added to
///   the impl's `where`-clause), so its type must implement `Parse` at the call site by other means.
///   The intended use is a **naturally-recursive child** (e.g. `Box<Expr<S>>` in a mutually-recursive
///   AST), where the per-field bound would otherwise form an infinite `where`-clause cycle
///   (`Expr: Parse ⇐ … ⇐ Expr: Parse`, E0275); with the bound dropped the recursion is discharged
///   coinductively via the sibling type's own impl. (Caveat for `Parse` specifically: a
///   *generic* field type with no other impl in scope, e.g. a bare `T`, will then fail to parse — the
///   bound was the only thing satisfying it.) `#[recurse]` routes `Parse` through `decycle` rather than
///   using this, and separately fixes the stream-monomorphization cycle with `syan::parse::erase`.
/// - `#[group(self.field)]`, `#[joint]`, `#[alone]`, `#[default]` — grouping/spacing/skip controls.
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

/// Derive `Unparse` — emit the type back to a token sink, field by field.
///
/// # Field attributes
///
/// - **`#[ignore_bounds]`** — suppress the `FieldTy: Unparse<_>` where-predicate this derive would
///   otherwise synthesize for the field. The field is still unparsed (`.unparse(sink)` is still called),
///   so its type must implement `Unparse`; the bound is merely omitted from the impl's `where`-clause.
///   The intended use is a **naturally-recursive child**, where the per-field bound would form an
///   infinite `where`-clause cycle (E0275); with it dropped, the recursive `.unparse()` resolves
///   coinductively against the sibling type's own (leaf-only-bounded) impl. This makes a hand-written
///   natural recursive `Unparse` compile (arbitrary depth — `Unparse` has no backtracking).
///   `#[recurse]` no longer needs it: every structural trait (`Unparse`, `Parse` and `Spanned`
///   alike) is routed through `decycle`, which contracts the cyclic bound rather than dropping it.
/// - `#[group(self.field)]`, `#[joint]`, `#[alone]`, `#[default]` — grouping/spacing/skip controls.
#[proc_macro_error]
#[proc_macro_derive(Unparse, attributes(group, syan, joint, alone, ignore_bounds, predicate_unparse,))]
pub fn unparse(input: TokenStream1) -> TokenStream1 {
    let input: DeriveInput = parse_macro_input!(input);
    let syan = input.attrs.get_syan();
    let trait_path: Path = parse_quote!(#syan::parse::unparse::Unparse);
    attribute::unparse(
        &input.ident,
        &input.generics,
        &input.data,
        &input.attrs,
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
/// - **`#[ignore_bounds]`** — suppress the `FieldTy: Spanned<Span = _>` where-predicate this derive
///   would otherwise synthesize. The field is **still folded** into the span, exactly as for
///   `Parse`/`Unparse` — only the predicate is dropped, and the field's associated `Span` is then pinned
///   by inference at the call site instead (which is why `#[ignore_bounds]` on a `Spanned` field only
///   type-checks when that `Span` is otherwise inferable — for a `#[recurse]` child it is, since the
///   child's own impl fixes it). The intended use is a **naturally-recursive child**,
///   whose per-field bound would otherwise form an infinite `where`-clause cycle (E0275); dropping it
///   lets a hand-written natural recursive `Spanned` compile (the resulting span covers the non-ignored
///   leaves plus, by inference, the recursive children). (`#[recurse]` itself no longer needs this:
///   it routes `Spanned` through `decycle` like `Parse`/`Unparse`, which contracts the cyclic bound
///   instead of dropping it.)
/// - `#[default]` — also excludes a field from the span fold.
#[proc_macro_error]
#[proc_macro_derive(Spanned, attributes(group, syan, joint, alone, ignore_bounds, predicate_spanned,))]
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
/// [`decycle`](https://docs.rs/decycle) crate. Takes **no arguments**.
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
/// the per-field bounds form an E0275 cycle, and `Parse::parse` takes `impl IntoParseStream` — a
/// generic parameter, which *moves* rather than reborrows, so each descent level asks for
/// `parse::<&mut &mut …>`, an infinite *monomorphization* chain no obligation engine can break. The
/// latter is fixed by wrapping every recursive call's stream in `syan::parse::erase`, pinning it to
/// one fixed `&mut dyn ParseStream` layer.) As for any recursive-descent parser, a *left-recursive*
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

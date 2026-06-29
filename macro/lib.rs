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
///   bound was the only thing satisfying it.) Note `#[recurse]` delegates `Parse` to its internal
///   fixed-depth engine rather than using this (the engine also bottoms out a second `Parse`-only E0275,
///   the `stream.dup(…)` stream-monomorphization cycle); it *does* use `#[ignore_bounds]` for a
///   **group-free** cycle's direct `Unparse`/`Spanned`.
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
///   natural recursive `Unparse` compile (arbitrary depth — `Unparse` has no backtracking). This is
///   exactly how `#[recurse]` derives `Unparse` directly on a **group-free** cycle's natural type
///   (pairing it with an injected `#[predicate_unparse(<leaf union>)]`); a **group-ful** cycle can't use
///   it (the `#[group]` `Fill: Unparse` HRTB cycle survives `#[ignore_bounds]`) so it delegates `Unparse`
///   to the engine instead.
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
#[proc_macro_derive(Ast, attributes(syan, subast))]
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
///   would otherwise synthesize, AND exclude the field from the span fold. (Unlike `Parse`/`Unparse`,
///   the field is *not* visited — the dropped predicate is what pins the field's associated `Span` type,
///   so the field cannot be folded without it.) The intended use is a **naturally-recursive child**,
///   whose per-field bound would otherwise form an infinite `where`-clause cycle (E0275); dropping it
///   lets a hand-written natural recursive `Spanned` compile (the resulting span reflects the non-ignored
///   leaves). This is how `#[recurse]` derives `Spanned` directly on a **group-free** cycle's natural
///   type (with an injected `#[predicate_spanned(<leaf union>)]`); a **group-ful** cycle delegates
///   `Spanned` to its internal engine instead.
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
/// plus an internal fixed-depth **engine** used to satisfy `Parse`. Takes **no arguments** (the former
/// `limit = N` was removed — the engine depth is a fixed internal constant). The user's cycle types stay
/// genuine recursive enums/structs (one type at all depths — the public API); `#[derive(Ast)]`/`Debug`/…
/// land on the natural type directly. A `syan::visit::visitor!(<cycle types>)` over them is an **ordinary
/// acyclic visitor** (closures included).
///
/// # Which traits go through the engine
///
/// - **`Parse`** is *always* delegated through the engine (parse the engine, convert back to the natural
///   type). Deriving `Parse` directly on a natural recursive type can't work — the per-field
///   `field_ty: Parse` where-bounds form an infinite cycle (E0275), and backtracking `stream.dup(…)`
///   wraps the stream in a fresh `Dup<…>` per descent level (infinite stream-type monomorphization). The
///   fixed-depth engine bottoms both out, so `Parse` is **depth-limited** (a tree deeper than the engine
///   depth is silently truncated).
/// - **`Unparse`/`Spanned`** are derived **directly on the natural type** for a **group-free** cycle
///   (`#[ignore_bounds]` on recursive children drops the per-field where-cycle, and an injected
///   `#[predicate_unparse/spanned(<cycle leaf union>)]` supplies the leaf bounds a member's body needs to
///   unparse its siblings). These are **unbounded** — any tree depth works. For a **group-ful** cycle
///   (a self-recursive `#[group]` field, whose `for<'a> Fill<Substruct>: Unparse` HRTB forms a
///   trait-solver cycle `#[ignore_bounds]` can't break) `Unparse`/`Spanned` stay **engine-delegated**,
///   hence depth-limited like `Parse`.
///
/// # The recursion root (engine-internal)
///
/// The **root** is the cycle type whose back-edges drive the engine's depth parameter `__Rec`. It is
/// chosen automatically: a directly **self-referential** cycle type if one exists (alphabetically-first
/// when several), else the cycle type most referenced by the others. This is purely an internal-engine
/// concept; the public types are uniform recursive types with no depth parameter.
///
/// # Generic arguments on a reference to the recursion root
///
/// In the *engine*, a back-edge to a root collapses to the depth parameter `__Rec`, so it must repeat
/// the root's own parameters **verbatim (identity)** — a substituted argument is non-regular recursion
/// the depth machinery can't express, and is **rejected** (an engine constraint, kept):
///
/// ```text
/// // root `Expr<S>`:
/// Box<Expr<S>>          // OK   — identity back-edge
/// Box<Expr<Vec<S>>>     // ERROR — wrapped param
/// Box<Expr<u8>>         // ERROR — concrete substitution
/// ```
///
/// Complex args are fine on a **cross-edge** to a non-root cycle type (`Box<Stmt<S, u8>>`) and on
/// **non-cycle** types (`Vec<S>`). Workaround for the rejected case: move the differing part into its
/// own `#[derive(Ast)]` type.
///
/// # Finite-size precondition
///
/// A natural recursive type must be finite-size, so a **pure by-value cycle** (no `Box`/`Vec`/… on any
/// cycle edge) is rejected with a clean error (it would be E0072) — put a `Box<…>` on a cycle edge.
///
/// # Visiting & the one limitation
///
/// `visitor!(<cycle types>)` builds an ordinary acyclic visitor over the natural types — closures,
/// tuples-of-closures, `visit_mut`, and inheritance (`visitor!(base => New)`) all work, and it may span
/// acyclic/outer types in one `Visit` trait. `#[subast]` on each cycle type lists its cross-edge
/// children. `Parse` is delegated through the internal fixed-depth engine; `Unparse`/`Spanned` are direct
/// (unbounded) for a group-free cycle and engine-delegated (depth-limited) for a group-ful one (see
/// "Which traits go through the engine" above and CLAUDE.md).
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

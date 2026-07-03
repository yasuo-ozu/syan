## Slice `macro-rest` — detailed design

All 8 findings verified against `/home/yasuo/ghq/github.com/yasuo-ozu/syan2-reduce` (HEAD `e5d0576`,
plus uncommitted split of `macro/attribute.rs` into `macro/attribute/{find,substruct,adt}.rs`); zero
rejected. Estimated ~71 lines saved, matching the plan header. Execution order (dead-code, then
dup-code, then comment, matching the repo-wide policy; within `find.rs` the dup-code step depends on
the dead-code step run first): **(1) symbol.rs dead-code → (2) find.rs dead-code → (3) adt.rs
preamble+substruct helper → (4) adt.rs `fields_pattern` helper → (5) find.rs array collapse → (6)
ast.rs `param_name` reuse → (7) adt.rs comment trim → (8) attribute.rs + ast.rs comment trim**. None
of the 8 changes emitted tokens or diagnostic text — every step is either unreachable code, a
byte-identical mechanical refactor, or a comment-only edit — so no trybuild `.stderr` is expected to
move.

### 1. `macro/symbol.rs:7,125-147` — dead code in `create_joint_type` + unused `Debug` derive

Verified as described. `create_joint_type`'s `else` branch only runs when `char_types.len() >
MAX_TUPLE_SIZE` (12), so `char_types.chunks(12)` always yields `≥2` chunks, so `joint_types.len() ==
1` (lines 141-143) can never hold — confirmed by re-reading the recursion (line 128 `if len <=
MAX_TUPLE_SIZE` takes the `if`; only overflow reaches the `else`). `SymbolToken` (line 7
`#[derive(Debug)]`) is never `{:?}`-formatted anywhere in `macro/`: `grep -n '{:?}' macro/symbol.rs`
hits only line 159, which formats a `char`, not `SymbolToken`.

**Code shape** — collapse the intermediate `Vec<Vec<TokenStream>>` and drop the unreachable arm:

```rust
fn create_joint_type(char_types: Vec<TokenStream>, syan_path: &Ident) -> TokenStream {
    const MAX_TUPLE_SIZE: usize = 12;
    if char_types.len() <= MAX_TUPLE_SIZE {
        quote! { #syan_path::nested::Joint<(#(#char_types,)*)> }
    } else {
        let joint_types: Vec<TokenStream> = char_types
            .chunks(MAX_TUPLE_SIZE)
            .map(|chunk| create_joint_type(chunk.to_vec(), syan_path))
            .collect();
        quote! { #syan_path::nested::Joint<(#(#joint_types),*)> }
    }
}
```

Drop `#[derive(Debug)]` from `SymbolToken` (line 7).

**Invariant**: emission unchanged — the deleted branch never executed, and `Debug` on a proc-macro-
internal parse type has no effect on emitted tokens.

**Risk**: low. **Verify**: `cargo build -p syan-macro && cargo test --workspace` (also
`cargo build -p syan-macro 2>&1 | grep -i dead_code` to confirm no new dead-code lint appears from the
collapsed `else`).

### 2. `macro/attribute/find.rs:61-63` — dead `is_derive_helper_attr` arms

Verified: `fundamental_tys` occurs nowhere else in the worktree; `predicate`/`predicate_parse` occur
only in this strip list and in `macro/recurse/items.rs`'s separate, out-of-scope
`strip_field_helper_attrs` list. None of the four `#[proc_macro_derive(..., attributes(...))]`
registrations in `macro/lib.rs` (Parse: `group,syan,joint,alone,ignore_bounds`; Unparse: `+
predicate_unparse`; Spanned: `+ predicate_spanned`; Ast: `syan,subast,seq,opt`) registers these three
names, so a field bearing them fails to compile before `strip_derive_helper_attrs` ever runs — the
arms are unreachable. Confirmed drift noted by the plan: `predicate_spanned` (registered on `Spanned`)
is *not* in this list — out of scope for this finding (not proposed for addition), left as-is.

**Code shape** — delete 3 of the (then 12-, becoming 9-) arms:

```rust
fn is_derive_helper_attr(attr: &Attribute) -> bool {
    attr.path().is_ident("group")
        || attr.path().is_ident("syan")
        || attr.path().is_ident("joint")
        || attr.path().is_ident("alone")
        || attr.path().is_ident("ignore_bounds")
        || attr.path().is_ident("default")
        || attr.path().is_ident("predicate_unparse")
        // `#[derive(Ast)]`'s view markers: … (comment kept verbatim)
        || attr.path().is_ident("seq")
        || attr.path().is_ident("opt")
}
```

**Invariant**: emission unchanged — the removed arms never matched any field reaching this function.

**Risk**: low. **Verify**: `cargo build -p syan-macro && cargo test --workspace`.

### 3. `macro/attribute/adt.rs:29-35,133-151,155-161` vs `186-192,268-287,295-301` — `extract_parse`/`extract_unparse` preamble + substruct-emission dedup

Verified byte-identical: `29-35` (7 lines: `tp_atom`, `trait_path_owned`, `trait_fullpath`,
`generic_params` clone+strip+push, `ty_generics`) equals `186-192` verbatim; `136-140`
(`DataStruct{struct_token,fields,semi_token}` construction) equals `271-275`; `150-151`
(`substructs_for_emit` map) equals `286-287`; the `quote!` substruct loops `156-161` equal `296-301`.
Only the inner call differs (`data_struct.extract_parse(syan, generics, ident, nonce, trait_path)` vs
`data_struct.extract_unparse(syan, generics, ident, &[], nonce, &trait_path_owned)`).

**Code shape** — two small helpers on `impl Adt` (or free fns in `adt.rs`):

```rust
fn atom_impl_header<'g>(
    generics: &'g Generics,
    trait_path: &Path,
) -> (Ident, Path, Punctuated<GenericParam, Token![,]>, syn::TypeGenerics<'g>) {
    let tp_atom: Ident = parse_quote!(__SyanMacro_Atom);
    let trait_fullpath: Path = parse_quote!(#trait_path<#tp_atom>);
    let mut generic_params = generics.params.clone();
    strip_param_defaults(&mut generic_params);
    generic_params.push(parse_quote!(#tp_atom));
    let ty_generics = generics.split_for_impl().1;
    (tp_atom, trait_fullpath, generic_params, ty_generics)
}

fn substruct_items(
    substructs: &[ItemStruct],
    mut derive: impl FnMut(&DataStruct, &Generics, &Ident) -> TokenStream,
) -> TokenStream {
    let substructs_for_emit: Vec<ItemStruct> =
        substructs.iter().map(strip_derive_helper_attrs).collect();
    let substruct_impls: Vec<TokenStream> = substructs
        .iter()
        .map(|substruct| {
            let data_struct = DataStruct {
                struct_token: Default::default(),
                fields: substruct.fields.clone(),
                semi_token: substruct.semi_token,
            };
            derive(&data_struct, &substruct.generics, &substruct.ident)
        })
        .collect();
    quote! {
        #(for substruct in &substructs_for_emit) { #substruct }
        #(for substruct_impl in &substruct_impls) { #substruct_impl }
    }
}
```

Call sites — `extract_parse` (replaces 29-35 and 133-161):

```rust
let (tp_atom, trait_fullpath, generic_params, ty_generics) = atom_impl_header(generics, trait_path);
// …
let substruct_defs = substruct_items(&substructs, |data_struct, generics, ident| {
    data_struct.extract_parse(syan, generics, ident, nonce, trait_path)
});
// then `quote! { #substruct_defs #[automatically_derived] impl … }`
```

`extract_unparse` (replaces 186-192 and 268-287/295-301) is identical except
`data_struct.extract_unparse(syan, generics, ident, &[], nonce, trait_path)`. Note this drops the
local `trait_path_owned` clone at the old line 282 call site in favor of the original `trait_path: &Path`
parameter already in scope — `syn::Path::clone()` preserves spans exactly, so `trait_path` and
`&trait_path_owned` denote token-identical values; this is required by extracting the preamble (which
no longer exposes a `trait_path_owned` local) and is token-neutral.

**Invariant**: emission unchanged — pure extract-helper refactor; token order and content identical,
including spans (verified the one behavioral wrinkle: dropping `trait_path_owned` in favor of
`trait_path` at the substruct-recursion call site).

**Risk**: low. **Verify**: `cargo build -p syan-macro && cargo test --workspace` (pay particular
attention to `core/tests/where_clause_attribute.rs` and `visitor_edit_group.rs`, which exercise
`#[group]` substruct emission through both `extract_parse` and `extract_unparse`).

### 4. `macro/attribute/adt.rs:466-508,530-540,655-669` — `fields_pattern` helper for the four Named/Unnamed template blocks

Verified: `DataStruct::extract_parse_inner`'s `Fields::Unnamed`/`Fields::Named` template (lines
480-485), `DataStruct::extract_inner`'s (500-504), `DataEnum::extract_parse_inner`'s `construct_of`
closure (533-538), and `DataEnum::extract_inner`'s match-arm head (659-663) all emit the identical
`#(if let Fields::Unnamed(_) = ..) { (..) } #(if let Fields::Named(_) = ..) { {..} }` shape, differing
only in the `Fields` source (`&self.fields`/`&variant.fields`) and the per-item binding name
(`field_ident`/`id`) — confirmed by re-reading all four sites; `grep -rn 'Fields::Unnamed\|Fields::Named'
macro/` shows no other quote-template use of this shape anywhere in the crate (the other hits are
plain `match`/`if let` on `Fields`, not token templates).

**Code shape**:

```rust
/// `(a, b,)` for tuple fields, `{a, b,}` for named fields, or nothing for a unit shape.
fn fields_pattern(shape: &Fields, fields: &[MappedField]) -> TokenStream {
    quote! {
        #(if let Fields::Unnamed(_) = shape) {
            (#(for (_, id, _) in fields) {#id,})
        }
        #(if let Fields::Named(_) = shape) {
            {#(for (_, id, _) in fields) {#id,}}
        }
    }
}
```

Call sites (all four, byte-identical composition since `#{expr}` splices a `TokenStream` verbatim):
`DataStruct::extract_parse_inner`: `#ident #{fields_pattern(&self.fields, &fields)}`;
`DataStruct::extract_inner`: `let #ident #{fields_pattern(&self.fields, &fields)} = #v_self;`;
`DataEnum::extract_parse_inner`'s `construct_of`: `#ident :: #{&variant.ident} #{fields_pattern(&variant.fields, fields)}`;
`DataEnum::extract_inner`: `#ident :: #{&variant.ident} #{fields_pattern(&variant.fields, &fields)} => { #inner }`.
`MappedField<'a>` (the existing `type MappedField<'a> = (Member, Ident, &'a Field);` alias at line 432)
is already module-private and in scope for all four sites.

**Invariant**: emission unchanged — same conditional structure and same iteration source per call
site, only the enclosing variable names differ (irrelevant to emitted tokens).

**Risk**: low. **Verify**: `cargo build -p syan-macro && cargo test --workspace` (covers both tuple-
and named-field structs/enums via `core/tests/ast_derive.rs`, `recurse_core.rs`, and the
`parse_prefix_dedup.rs` enum-construction paths).

### 5. `macro/attribute/find.rs:54-70` — collapse `is_derive_helper_attr` to an array + `.any`

Verified: after step 2, the chain has 9 identical-shape arms (`attr.path().is_ident("…")`). The
array-based style is already proven working in this exact crate:
`macro/recurse/items.rs:70-74`'s `is_struct_helper` uses `[…].iter().any(|n| attr.path().is_ident(n))`
on a `&[&str]`, the same pattern proposed here.

**Code shape**:

```rust
fn is_derive_helper_attr(attr: &Attribute) -> bool {
    [
        "group", "syan", "joint", "alone", "ignore_bounds", "default", "predicate_unparse",
        // `#[derive(Ast)]`'s view markers: strip them off a `#[group]`-cloned substruct (which carries no
        // `Ast` derive to register them), else `#[group] #[seq] Punctuated<..>` fails with "cannot find
        // attribute `seq`".
        "seq", "opt",
    ]
    .iter()
    .any(|n| attr.path().is_ident(n))
}
```

**Invariant**: emission unchanged — same predicate, same matched name set.

**Risk**: low. **Verify**: `cargo build -p syan-macro && cargo test --workspace`.

### 6. `macro/ast.rs:331-340` — reuse `crate::util::param_name`

Verified: `macro/util.rs:27-33`'s `param_name(p: &GenericParam) -> String` has an arm-for-arm identical
body to the inline closure at `ast.rs:335-339` (`GenericParam::Type => t.ident.to_string()`, `Const =>
c.ident.to_string()`, `Lifetime => l.lifetime.ident.to_string()`). `ast.rs` already imports from
`crate::util` (line 1: `angle, gargs, gparams, to_snake`) and already calls it fully qualified
elsewhere (`crate::util::peel` at line 274), so either an added import or an inline fully-qualified
call works.

**Code shape**:

```rust
let param_names: std::collections::HashSet<String> = input
    .generics
    .params
    .iter()
    .map(crate::util::param_name)
    .collect();
```

(`.iter()` over `Punctuated<GenericParam,_>` yields `&GenericParam`, matching `param_name`'s
`&GenericParam` parameter — direct fn-pointer coercion, no closure needed.)

**Invariant**: emission unchanged — this only feeds the nightly "follows nothing" lint's suspect-name
filter; the computed `HashSet<String>` contents are identical, so the lint fires on the same fields
with the same message.

**Risk**: low. **Verify**: `cargo build -p syan-macro && cargo test --workspace` (nightly-gated lint
path is exercised by the `ui/` diagnostics suite per CLAUDE.md; re-run
`TRYBUILD=overwrite cargo test --test visitor_diagnostics -- --ignored` only if a `.stderr` drifts,
which is not expected here).

### 7. `macro/attribute/adt.rs` comments — trim restatements of helper docs

Verified all eleven locations exactly as claimed, re-reading each: line 60 and its twins at 211, 351,
374 all read `// Skip fields with #[default] attribute — …` immediately above `field.has_default()`/
`attrs.has_default()` checks; 152-153 and 292-293 and 391-392 all restate
`append_user_where_predicates`'s own doc (`find.rs:129-131`); 288 and 387 both restate
`predicate_tys`'s doc (`find.rs:143-148`, which already states the `Ty: Unparse<atom>` /
`Ty: Spanned<Span=span>` mapping for both `predicate_unparse` and `predicate_spanned`); 329 restates
`add_spanned_param_predicates`'s doc (`find.rs:109-112`); 529 restates the `construct_of` closure's own
name/purpose immediately below it. The constraint comments the plan says to keep (355-367 E0207/E0308
rationale, 542-546, 575-586, 594, 603) are untouched by this deletion list.

**Code shape**: delete the 11 listed single/double-line `//` comments verbatim; no code lines change.

**Invariant**: emission unchanged — comment-only edit.

**Risk**: low. **Verify**: `cargo build -p syan-macro && cargo test --workspace` (a comment-only diff
cannot itself change behavior; the build simply confirms no stray line was caught in the edit).

### 8. `macro/attribute.rs:16` + `macro/ast.rs:43-44,306-307` — stale/duplicate comments

Verified: `attribute.rs:16` (`// FindAttribute is imported directly by lib.rs.`) sits directly above
`pub(crate) use find::FindAttribute;` (line 17), and `macro/lib.rs:12` does indeed
`use crate::attribute::FindAttribute;` — but the comment only restates the next line, adding nothing
`grep` can't already show. `ast.rs:43-44`'s "Shared by `#[derive(Ast)]` and `#[recurse]`'s per-cycle-
type metadata macros" is stale: `grep -rn 'subast_tokens\|crate_rooted_tokens\|cleaned_definition\|parse_subast'
macro/recurse.rs macro/recurse/` returns zero hits — all four callers of these functions are in
`ast.rs` itself (self-calls at lines 366/368/308), confirming `#[recurse]` cycle types now carry a
plain `#[derive(Ast)]` with no `#[recurse]`-specific metadata path, per CLAUDE.md. `ast.rs:306-307`
narrates the `#[subast(..)]` allowlist immediately above `parse_subast(&input.attrs)`, restating
`parse_subast`'s own doc at `ast.rs:91-93`.

**Code shape**: delete `attribute.rs:16` entirely; in `ast.rs:43-44` delete the trailing sentence
"Shared by `#[derive(Ast)]` and `#[recurse]`'s per-cycle-type metadata macros." from `subast_tokens`'s
doc comment (keep the rest of the doc); delete `ast.rs:306-307` entirely.

**Invariant**: emission unchanged — comment-only edit.

**Risk**: low. **Verify**: `cargo build -p syan-macro && cargo test --workspace`.

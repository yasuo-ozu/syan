use crate::util::{first_ty_arg, param_tokens};
use proc_macro::TokenStream as TokenStream1;
use proc_macro2::{Ident, Span, TokenStream};
use proc_macro_error::{abort, set_dummy};
use std::collections::{HashMap, HashSet};
use syn::{
    punctuated::Punctuated, AngleBracketedGenericArguments, Field, Fields, FieldsNamed,
    FieldsUnnamed, FnArg, GenericArgument, GenericParam, Generics, ImplItem, Item, ItemMod, Path,
    PathArguments, ReturnType, Token, Type, TypePath, Visibility,
};
use template_quote::quote;

mod build;
mod convert;
#[cfg(feature = "recurse-decycle")]
mod decycle;
mod emit;
mod graph;
mod items;
mod names;
mod transform;
use build::*;
use convert::*;
use emit::*;
use graph::*;
use items::*;
use names::*;
use transform::*;

/// The fixed type-depth of the internal engine (the depth chain `__XxxDefault`). The engine backs
/// `Parse` always, and `Unparse`/`Spanned` for a group-ful cycle; all three are nonetheless unbounded —
/// the depth-floor terminator re-enters the top-level impl at runtime via `core::parse::vtable`, so this
/// depth is not a ceiling. This is *not* user-tunable — `#[recurse]` takes no arguments (the former
/// `limit = N` was removed).
const DEFAULT_RECURSION_DEPTH: usize = 4;

pub fn recurse(attr: TokenStream1, input: TokenStream1, nonce: u64) -> TokenStream1 {
    // `#[recurse]` takes no arguments. (The former `limit = N` is gone — `Unparse`/`Spanned` are now
    // unbounded for group-free cycles, and the `Parse` engine uses a fixed internal depth.)
    if !attr.is_empty() {
        return syn::Error::new(
            proc_macro2::TokenStream::from(attr)
                .into_iter()
                .next()
                .map(|t| t.span())
                .unwrap_or_else(proc_macro2::Span::call_site),
            "`#[recurse]` takes no arguments (the `limit = N` argument was removed)",
        )
        .to_compile_error()
        .into();
    }
    let recursion_depth = DEFAULT_RECURSION_DEPTH;

    let module: ItemMod = match syn::parse(input) {
        Ok(m) => m,
        Err(e) => return e.to_compile_error().into(),
    };

    // On any later `abort!` (missing root param, non-identity root arg, multi-root + visit, …) emit
    // the *original* module unchanged instead of nothing. The user's definitions are valid Rust on
    // their own, so downstream `mod::Type` references still resolve — the diagnostic stands alone
    // rather than triggering a cascade of "cannot find type/module" errors.
    set_dummy(quote!(#module));

    let Some((_, items)) = module.content else {
        return quote!(#module).into();
    };

    let mod_attrs = &module.attrs;
    let mod_vis = &module.vis;
    let mod_ident = &module.ident;

    // Collect all pub type names
    let pub_types: HashSet<String> = items
        .iter()
        .filter_map(|item| match item {
            Item::Enum(e) if matches!(e.vis, Visibility::Public(_)) => Some(e.ident.to_string()),
            Item::Struct(s) if matches!(s.vis, Visibility::Public(_)) => Some(s.ident.to_string()),
            _ => None,
        })
        .collect();

    // Build both reference maps in a single pass over the items:
    //   * `type_refs`        — *all* references (nested too, via `collect_refs_item`); this is the
    //                          adjacency that drives cycle detection.
    //   * `direct_type_refs` — only outermost-constructor references (`collect_direct_refs_item`);
    //                          the primary signal for root selection (a bare field, not `Vec`/`Box`).
    // `type_refs` is kept a plain adjacency `HashMap` rather than a `safegraph` graph: it is built
    // straight from the AST and is also queried as a map for self-reference and degree counting;
    // `find_cycle_sccs` lifts it into a `safegraph` graph for the one operation that needs graph
    // algorithms (Tarjan SCC). A `Copy`-keyed graph here would just push name<->id bookkeeping outward.
    let mut type_refs: HashMap<String, HashSet<String>> = HashMap::new();
    let mut direct_type_refs: HashMap<String, HashSet<String>> = HashMap::new();
    for item in &items {
        let name = match item {
            Item::Enum(e) if matches!(e.vis, Visibility::Public(_)) => e.ident.to_string(),
            Item::Struct(s) if matches!(s.vis, Visibility::Public(_)) => s.ident.to_string(),
            _ => continue,
        };
        type_refs.insert(name.clone(), collect_refs_item(item, &pub_types));
        direct_type_refs.insert(name, collect_direct_refs_item(item, &pub_types));
    }

    // Partition the cycle types into independent cycles (SCCs). A module may hold several cycles
    // that don't reference one another; each is handled on its own.
    let sccs = find_cycle_sccs(&type_refs);

    if sccs.is_empty() {
        return quote!(
            #(#mod_attrs)* #mod_vis mod #mod_ident { #(#items)* }
        )
        .into();
    }

    // Finite-size precondition (natural-type design): each cycle is exposed as a *natural* recursive
    // type, which Rust admits only if it is finite-size — i.e. every cycle passes through a heap
    // indirection (`Box`/`Vec`/…). Detect a pure value-cycle as a cyclic *direct-edge* subgraph
    // (`direct_type_refs`, the by-value references) and reject it with guidance rather than emitting an
    // infinite-size type (E0072). `subgraph_is_cyclic` with no removed roots tests exactly this.
    for scc in &sccs {
        if subgraph_is_cyclic(scc, &HashSet::new(), &direct_type_refs) {
            let mut names: Vec<&String> = scc.iter().collect();
            names.sort();
            abort!(
                mod_ident,
                "#[recurse]: the cycle ({}) has no heap indirection on a by-value reference cycle, so \
                 its natural recursive type would be infinite-size. Wrap a cyclic field in `Box<…>` \
                 (or `Vec`/`Option<Box<…>>`) to break the value cycle.",
                names.iter().map(|n| n.as_str()).collect::<Vec<_>>().join(", ")
            );
        }
    }

    // Map each cycle type to the index of its SCC, so an engine type is transformed with ONLY its own
    // cycle's `ctx` (a cross-SCC reference stays the *natural* type of the other cycle — a finite type
    // reached via a DAG edge, parsed through its own delegated `Parse`).
    let mut type_to_scc: HashMap<String, usize> = HashMap::new();
    for (i, scc) in sccs.iter().enumerate() {
        for n in scc {
            type_to_scc.insert(n.clone(), i);
        }
    }

    let item_in_scc = |item: &Item, scc: &HashSet<String>| match item {
        Item::Enum(e) => scc.contains(&e.ident.to_string()),
        Item::Struct(s) => scc.contains(&s.ident.to_string()),
        _ => false,
    };
    fn item_attrs(item: &Item) -> &[syn::Attribute] {
        match item {
            Item::Enum(e) => &e.attrs,
            Item::Struct(s) => &s.attrs,
            _ => &[],
        }
    }
    // Does any member of this SCC have a `#[group(...)]` field? For a **group-free** cycle `Unparse`/
    // `Spanned` are derived directly on the natural type (unbounded). A **group-ful** cycle keeps them on
    // the depth-limited engine (delegated): a self-recursive group field's derive-generated
    // `for<'a> Fill<Substruct>: Unparse` bound forms a trait-solver cycle that `#[ignore_bounds]` can't
    // break (it only suppresses the per-field bound, not the group `Fill` bound).
    let field_has_group = |f: &Field| f.attrs.iter().any(|a| a.path().is_ident("group"));
    let item_has_group = |item: &Item| match item {
        Item::Enum(e) => e.variants.iter().any(|v| v.fields.iter().any(&field_has_group)),
        Item::Struct(s) => s.fields.iter().any(&field_has_group),
        _ => false,
    };
    let scc_has_group: Vec<bool> = sccs
        .iter()
        .map(|scc| items.iter().any(|it| item_in_scc(it, scc) && item_has_group(it)))
        .collect();

    // The engine backs `Parse` always; for a **group-ful** cycle it also backs `Unparse`/`Spanned` (which
    // are delegated). So a cycle needs the engine iff it derives `Parse`, or is group-ful and derives
    // `Unparse`/`Spanned`.
    let scc_needs_engine: Vec<bool> = sccs
        .iter()
        .enumerate()
        .map(|(i, scc)| {
            items.iter().any(|item| {
                item_in_scc(item, scc)
                    && (derives_any(item_attrs(item), &["Parse"])
                        || (scc_has_group[i] && derives_any(item_attrs(item), &["Unparse", "Spanned"])))
            })
        })
        .collect();

    // Cycle types that derive `Parse` → get a delegated `impl Parse for X` (parse the engine, `__to_nat`).
    let parse_types: HashSet<String> = items
        .iter()
        .filter_map(|item| match item {
            Item::Enum(e) if derives_any(&e.attrs, &["Parse"]) => Some(e.ident.to_string()),
            Item::Struct(s) if derives_any(&s.attrs, &["Parse"]) => Some(s.ident.to_string()),
            _ => None,
        })
        .collect();

    // `Unparse`/`Spanned` are delegated through the engine (`__FromNat`) ONLY for a group-ful cycle; a
    // group-free cycle derives them directly on the natural type (unbounded) — see `make_natural_item`.
    let delegated_us = |trait_name: &str| -> HashSet<String> {
        items
            .iter()
            .filter_map(|item| {
                let (id, attrs) = match item {
                    Item::Enum(e) => (e.ident.to_string(), &e.attrs),
                    Item::Struct(s) => (s.ident.to_string(), &s.attrs),
                    _ => return None,
                };
                let idx = *type_to_scc.get(&id)?;
                (scc_has_group[idx] && derives_any(attrs, &[trait_name])).then_some(id)
            })
            .collect()
    };
    let deleg = DelegSets {
        parse: parse_types,
        unparse: delegated_us("Unparse"),
        spanned: delegated_us("Spanned"),
    };

    // Feature `recurse-decycle`: which SCCs host the decycle cycle-mode `Parse` (group-free, S-free
    // leaves, direct/`Box` recursive edges — the shapes decycle can express). Their `Parse` is removed
    // from the engine's delegation (`deleg.parse`) and supplied by a `#[decycle]` module instead; being
    // group-free their `Unparse`/`Spanned` are already direct on the natural type, so they need *no*
    // engine at all. Feature-off: no SCC qualifies (byte-identical engine path).
    #[cfg(feature = "recurse-decycle")]
    let decycle_scc: Vec<bool> = sccs
        .iter()
        .enumerate()
        .map(|(i, scc)| decycle::scc_is_decycleable(scc, &items, &deleg.parse, scc_has_group[i]))
        .collect();
    #[cfg(not(feature = "recurse-decycle"))]
    let decycle_scc: Vec<bool> = vec![false; sccs.len()];

    #[cfg(feature = "recurse-decycle")]
    let deleg = {
        let mut d = deleg;
        for (scc, &dc) in sccs.iter().zip(&decycle_scc) {
            if dc {
                for n in scc {
                    d.parse.remove(n);
                }
            }
        }
        d
    };

    // A decycle-hosted SCC needs no engine at all (its `Parse` is decycle's, its `Unparse`/`Spanned`
    // are direct). Feature-off `decycle_scc` is all-false, so this is a no-op.
    let scc_needs_engine: Vec<bool> = scc_needs_engine
        .iter()
        .zip(&decycle_scc)
        .map(|(&e, &d)| e && !d)
        .collect();

    // Per SCC: the transform `ctx` + the engine/conversion/delegated-`Parse`/`Unparse`/`Spanned` tail.
    let plans: Vec<(TransformCtx, TokenStream)> = sccs
        .iter()
        .map(|scc| {
            build_scc(
                scc,
                &items,
                &type_refs,
                &direct_type_refs,
                recursion_depth,
                mod_ident,
                &deleg,
                nonce,
            )
        })
        .collect();

    // Emit, for each cycle enum/struct: the natural public type (derives split, `#[ignore_bounds]`
    // injected) AND the internal `pub(crate)` engine type. Non-cycle items (incl. user `impl` blocks on
    // cycle types — now plain impls on the natural type) pass through unchanged.
    // Per SCC, the UNION of every member's leaf field types — injected as `#[predicate_unparse/spanned]`
    // on each member's natural type so a member's `Unparse`/`Spanned` body can call its siblings'
    // (whose bounds reduce to these leaves). Deduped by rendered text.
    let scc_union_leaf: Vec<Vec<Type>> = sccs
        .iter()
        .map(|scc| {
            let mut seen = HashSet::new();
            items
                .iter()
                .filter(|it| item_in_scc(it, scc))
                .flat_map(|it| leaf_field_types(it, scc))
                .filter(|t| seen.insert(quote!(#t).to_string()))
                .collect()
        })
        .collect();

    let mut out_items: Vec<TokenStream> = Vec::new();
    for item in &items {
        let cycle_name = match item {
            Item::Enum(e) if matches!(e.vis, Visibility::Public(_)) => Some(e.ident.to_string()),
            Item::Struct(s) if matches!(s.vis, Visibility::Public(_)) => Some(s.ident.to_string()),
            _ => None,
        }
        .filter(|n| type_to_scc.contains_key(n));

        match cycle_name {
            Some(name) => {
                let idx = type_to_scc[&name];
                let scc = &sccs[idx];
                // Group-free: `Unparse`/`Spanned` direct on the natural type (unbounded). Group-ful:
                // engine-delegated (depth-limited), like `Parse`.
                let (natural, engine_paths, engine_md) =
                    make_natural_item(item, scc, &scc_union_leaf[idx], scc_has_group[idx]);
                if scc_needs_engine[idx] {
                    let ctx = &plans[idx].0;
                    let engine = make_engine_item(item, ctx, &engine_paths, engine_md);
                    out_items.push(quote!(#natural #engine));
                } else {
                    // No engine (derives none of Parse/Unparse/Spanned): just the natural type (+ Ast/…).
                    out_items.push(quote!(#natural));
                }
            }
            None => out_items.push(quote!(#item)),
        }
    }
    // Emit each cycle's engine/conversion/delegated-Parse tail only when that cycle needs the engine.
    let tails: Vec<TokenStream> = plans
        .into_iter()
        .enumerate()
        .filter_map(|(i, (_, tail))| scc_needs_engine[i].then_some(tail))
        .collect();

    // Feature `recurse-decycle`: the `#[decycle]` module attribute + the per-SCC `__ParseDyn` trait /
    // impls / `Parse` facades spliced into the module. `recurse_level` = the widest decycle SCC (≥ its
    // width — level < width panics at the re-entry floor). Feature-off: both empty (byte-identical).
    #[cfg(feature = "recurse-decycle")]
    let (decycle_attr, decycle_items): (TokenStream, TokenStream) = {
        let mut items_out = TokenStream::new();
        let mut level = 1usize;
        for (scc, &dc) in sccs.iter().zip(&decycle_scc) {
            if dc {
                items_out.extend(decycle::emit_scc(scc, &items, nonce));
                level = level.max(scc.len().max(1));
            }
        }
        if decycle_scc.iter().any(|&b| b) {
            (
                quote!( #[::syan::__decycle::decycle(recurse_level = #level, decycle = ::syan::__decycle)] ),
                items_out,
            )
        } else {
            (quote!(), quote!())
        }
    };
    #[cfg(not(feature = "recurse-decycle"))]
    let (decycle_attr, decycle_items): (TokenStream, TokenStream) = (quote!(), quote!());

    quote! {
        #(#mod_attrs)*
        #decycle_attr
        #mod_vis mod #mod_ident {
            #(#out_items)*

            #(#tails)*

            #decycle_items
        }
    }
    .into()
}

use super::*;

/// If `attr` is a derive-like list attribute, return its name (`"derive"` or `"macro_derive"`), else
/// `None`. `#[macro_derive]` (from the `type-macro-derive-tricks` crate) is the form a cycle type must use
/// when it has `Token![..]`-style *type-macro* fields, which rustc forbids under a plain `#[derive]`;
/// `#[recurse]` handles either the same way.
pub(crate) fn derive_attr_name(attr: &syn::Attribute) -> Option<&'static str> {
    if attr.path().is_ident("derive") {
        Some("derive")
    } else if attr.path().is_ident("macro_derive") {
        Some("macro_derive")
    } else {
        None
    }
}

/// Partition a cycle type's derive list into (kept-on-natural attrs, engine-routed derive paths), routing
/// the `engine_routed` traits to the engine. Recognizes both `#[derive(...)]` and `#[macro_derive(...)]`,
/// re-emitting the kept derives under the *same* attribute name. Also returns whether any engine-routed
/// trait came from a `#[macro_derive]` — the engine type carries the same `Token!` fields, so its
/// engine-routed derives must use `#[macro_derive]` too.
pub(crate) fn split_cycle_derives(
    attrs: &[syn::Attribute],
    engine_routed: &[&str],
) -> (Vec<syn::Attribute>, Vec<Path>, bool) {
    let mut natural = Vec::new();
    let mut engine_paths = Vec::new();
    let mut engine_macro_derive = false;
    for attr in attrs {
        if let Some(name) = derive_attr_name(attr) {
            if let syn::Meta::List(list) = &attr.meta {
                let paths: Punctuated<Path, Token![,]> = list
                    .parse_args_with(Punctuated::parse_terminated)
                    .unwrap_or_default();
                let mut kept: Vec<Path> = Vec::new();
                for p in paths {
                    if p.segments.last().is_some_and(|s| engine_routed.iter().any(|t| s.ident == t)) {
                        engine_paths.push(p);
                        engine_macro_derive |= name == "macro_derive";
                    } else {
                        kept.push(p);
                    }
                }
                if !kept.is_empty() {
                    let name = Ident::new(name, Span::call_site());
                    natural.push(syn::parse_quote!( #[#name(#(#kept),*)] ));
                }
                continue;
            }
        }
        natural.push(attr.clone());
    }
    (natural, engine_paths, engine_macro_derive)
}

/// Whether `attrs` contains a `#[derive(...)]` / `#[macro_derive(...)]` mentioning any of `names`.
pub(crate) fn derives_any(attrs: &[syn::Attribute], names: &[&str]) -> bool {
    attrs.iter().any(|a| {
        derive_attr_name(a).is_some()
            && matches!(&a.meta, syn::Meta::List(l)
                if l.parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)
                    .map(|ps| ps.iter().any(|p| p.segments.last().is_some_and(|s| names.iter().any(|n| s.ident == n))))
                    .unwrap_or(false))
    })
}

/// Strip the structural-derive field helper attributes from a field set. Used on the natural type when
/// it carries NO structural derive (else the attrs would be unregistered "cannot find attribute").
pub(crate) fn strip_field_helper_attrs(fields: &mut Fields) {
    fn is_struct_helper(attr: &syn::Attribute) -> bool {
        ["group", "joint", "alone", "ignore_bounds", "default", "predicate", "predicate_parse", "predicate_unparse"]
            .iter()
            .any(|n| attr.path().is_ident(n))
    }
    let go = |f: &mut Field| f.attrs.retain(|a| !is_struct_helper(a));
    match fields {
        Fields::Named(n) => n.named.iter_mut().for_each(go),
        Fields::Unnamed(u) => u.unnamed.iter_mut().for_each(go),
        Fields::Unit => {}
    }
}

/// The public **natural** form of a cycle type. `Parse` is always routed to the depth-limited engine and
/// re-supplied on the natural type by delegation (`gen_natural_extras`): a natural `Parse` overflows
/// (per-field where-bound cycle + backtracking `Dup<…>` stream recursion). `Unparse`/`Spanned` are routed
/// to the engine (delegated) only for a **group-ful** cycle; a group-free cycle derives them directly on
/// the natural type (unbounded — see the body). Everything else (`Ast`, `Debug`, `Default`, `#[subast]`,
/// docs, …) stays on the natural type; the engine-routed structural-derive field
/// helper attrs (`#[group]`/`#[ignore_bounds]`/…) are stripped from the natural type (it carries no
/// structural derive that would consume them — they live on the engine, built from the original item).
/// Returns `(natural item, engine-routed derive paths, engine-uses-`#[macro_derive]`)`.
pub(crate) fn make_natural_item(
    item: &Item,
    scc: &HashSet<String>,
    union_leaf_tys: &[Type],
    group_ful: bool,
) -> (Item, Vec<Path>, bool) {
    // `Parse` is always engine-routed (it overflows on a natural type — Dup stream growth). For a
    // **group-free** cycle `Unparse`/`Spanned` stay on the natural type and are derived directly
    // (unbounded): `#[ignore_bounds]` on recursive-child fields drops the per-field `field_ty: Trait`
    // bound (no E0275 where-cycle), and an injected `#[predicate_unparse/spanned(<union of all cycle leaf
    // types>)]` supplies the bounds a member's body needs to call its siblings' `unparse`/`span` (which
    // reduce to those leaves). A **group-ful** cycle engine-routes them too (delegated, depth-limited) —
    // the self-recursive group `Fill<Substruct>: Unparse` bound forms a where-cycle a direct impl can't
    // break.
    let engine_routed: &[&str] = if group_ful {
        &["Parse", "Unparse", "Spanned"]
    } else {
        &["Parse"]
    };
    let mut it = item.clone();
    let prep = |attrs: &[syn::Attribute], fields_iter: &mut dyn FnMut(bool)| -> (Vec<syn::Attribute>, Vec<Path>, bool) {
        let (mut nat, ep, md) = split_cycle_derives(attrs, engine_routed);
        let structural = derives_any(&nat, &["Unparse", "Spanned"]);
        if !union_leaf_tys.is_empty() {
            if derives_any(&nat, &["Unparse"]) {
                nat.push(syn::parse_quote!(#[predicate_unparse(#(#union_leaf_tys),*)]));
            }
            if derives_any(&nat, &["Spanned"]) {
                nat.push(syn::parse_quote!(#[predicate_spanned(#(#union_leaf_tys),*)]));
            }
        }
        fields_iter(structural);
        (nat, ep, md)
    };
    let (engine_paths, engine_md) = match &mut it {
        Item::Enum(e) => {
            let variants = &mut e.variants;
            let (nat, ep, md) = prep(&e.attrs, &mut |structural| {
                for v in variants.iter_mut() {
                    if structural {
                        inject_ignore_bounds(&mut v.fields, scc);
                    } else {
                        strip_field_helper_attrs(&mut v.fields);
                    }
                }
            });
            e.attrs = nat;
            (ep, md)
        }
        Item::Struct(s) => {
            let fields = &mut s.fields;
            let (nat, ep, md) = prep(&s.attrs, &mut |structural| {
                if structural {
                    inject_ignore_bounds(fields, scc);
                } else {
                    strip_field_helper_attrs(fields);
                }
            });
            s.attrs = nat;
            (ep, md)
        }
        _ => (Vec::new(), false),
    };
    (it, engine_paths, engine_md)
}

/// Inject `#[ignore_bounds]` on every field whose type references a cycle type (a recursive child), so
/// the natural `Unparse`/`Spanned` derive drops the per-field `field_ty: Trait` bound that would
/// otherwise form an infinite `where`-clause (E0275). A user-written `#[ignore_bounds]` is left as-is.
pub(crate) fn inject_ignore_bounds(fields: &mut Fields, scc: &HashSet<String>) {
    let go = |f: &mut Field| {
        let mut refs = HashSet::new();
        collect_refs(&f.ty, scc, &mut refs);
        if !refs.is_empty() && !f.attrs.iter().any(|a| a.path().is_ident("ignore_bounds")) {
            f.attrs.push(syn::parse_quote!(#[ignore_bounds]));
        }
    };
    match fields {
        Fields::Named(n) => n.named.iter_mut().for_each(go),
        Fields::Unnamed(u) => u.unnamed.iter_mut().for_each(go),
        Fields::Unit => {}
    }
}

/// The internal **engine** form of a cycle type: `transform_item` (rename `X` → `__XRec`, thread the
/// depth params) then made `pub(crate)` and carrying the engine-routed structural derives
/// (`#[derive(Parse, Unparse, Spanned)]` as the user wrote them). The depth-limited engine is finite, so
/// the normal derives apply. Must be called while the original is still `pub` (transform_item keys on
/// that), then the visibility is narrowed.
pub(crate) fn make_engine_item(item: &Item, ctx: &TransformCtx, engine_paths: &[Path], macro_derive: bool) -> Item {
    let mut eng = transform_item(item.clone(), ctx);
    // Emit the engine's structural derives under the same mechanism the user used — `#[macro_derive]`
    // when the cycle type has `Token!` (type-macro) fields, else plain `#[derive]`.
    let derive_name = Ident::new(if macro_derive { "macro_derive" } else { "derive" }, Span::call_site());
    let derives: Vec<syn::Attribute> = if engine_paths.is_empty() {
        vec![]
    } else {
        vec![syn::parse_quote!(#[#derive_name(#(#engine_paths),*)])]
    };
    // Strip `#[ignore_bounds]` and the visitor view markers `#[seq]`/`#[opt]` from the engine's fields.
    // `#[ignore_bounds]`: the engine's recursive child is the depth param `__Rec` (a *finite* chain), so
    // its derives need the FULL `__Rec: Trait` bound — dropping it would leave the derive body's
    // `__Rec::parse()`/`unparse()` call unsatisfiable. (A user-written `#[ignore_bounds]` is for the
    // natural type, not the engine.) `#[seq]`/`#[opt]` are visitor-only markers the natural type's `Ast`
    // consumes; the engine derives `Parse`/`Unparse`/`Spanned`, which don't declare them, so leaving them
    // would be a "cannot find attribute" error.
    let strip_ib = |fields: &mut Fields| {
        let go = |f: &mut Field| {
            f.attrs.retain(|a| {
                !a.path().is_ident("ignore_bounds")
                    && !a.path().is_ident("seq")
                    && !a.path().is_ident("opt")
            })
        };
        match fields {
            Fields::Named(n) => n.named.iter_mut().for_each(go),
            Fields::Unnamed(u) => u.unnamed.iter_mut().for_each(go),
            Fields::Unit => {}
        }
    };
    match &mut eng {
        Item::Enum(e) => {
            e.attrs = derives;
            e.vis = syn::parse_quote!(pub(crate));
            for v in &mut e.variants {
                strip_ib(&mut v.fields);
            }
        }
        Item::Struct(s) => {
            s.attrs = derives;
            s.vis = syn::parse_quote!(pub(crate));
            strip_ib(&mut s.fields);
        }
        _ => {}
    }
    eng
}

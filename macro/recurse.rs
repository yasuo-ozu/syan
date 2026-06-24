use crate::util::{peel, to_snake, Container};
use proc_macro::TokenStream as TokenStream1;
use proc_macro2::{Ident, Span, TokenStream};
use proc_macro_error::{abort, set_dummy};
use std::collections::{HashMap, HashSet};
use syn::{
    punctuated::Punctuated, AngleBracketedGenericArguments, Fields, FieldsNamed, FieldsUnnamed,
    FnArg, GenericArgument, GenericParam, Generics, ImplItem, Item, ItemMod, PathArguments,
    ReturnType, Token, Type, TypePath, Visibility,
};
use template_quote::quote;

/// Default recursion depth when no `limit` argument is given.
const DEFAULT_RECURSION_DEPTH: usize = 4;

struct RecurseArgs {
    limit: usize,
    /// `#[recurse(visit)]`: also generate a depth-generic visitor over the cycle.
    visit: bool,
}

impl syn::parse::Parse for RecurseArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut limit = DEFAULT_RECURSION_DEPTH;
        let mut visit = false;
        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            if ident == "limit" {
                let _: Token![=] = input.parse()?;
                let lit: syn::LitInt = input.parse()?;
                limit = lit.base10_parse()?;
            } else if ident == "visit" {
                visit = true;
            } else {
                return Err(syn::Error::new(
                    ident.span(),
                    "expected `limit = <integer>` or `visit`",
                ));
            }
            if input.peek(Token![,]) {
                let _: Token![,] = input.parse()?;
            } else {
                break;
            }
        }
        Ok(RecurseArgs { limit, visit })
    }
}

struct TransformCtx {
    cycle_types: HashSet<String>,
    root_types: HashSet<String>,
    internal_names: HashMap<String, Ident>,
    /// Depth parameters, **one per root** of this cycle, in canonical (sorted-root) order: `[__Rec]`
    /// for a single root, `[__RecA, __RecB, …]` for several. Appended in this order to every renamed
    /// cycle type and threaded (all of them) through every cross-edge.
    rec_params: Vec<Ident>,
    /// Root type name → its own depth parameter. A back-edge to root `X` collapses to `root_rec[X]`
    /// (so with several roots each self-edge keeps its own depth dimension, unambiguously).
    root_rec: HashMap<String, Ident>,
    /// The depth parameters as generic-param **declarations with defaults**, appended to a renamed
    /// cycle type: `[__Rec = __XDefault<S, …>]` (single) or one per root (`__RecA = __ADefault<S>`, …).
    rec_decls: Vec<TokenStream>,
    /// Per root type, its own declared generic params as *use*-form normalized token strings (e.g.
    /// `["'a", "S", "N"]`). A back-edge to a root collapses to its depth param, so its generic
    /// arguments must be the *identity* (the root's own params, unchanged) — there is nowhere to
    /// thread a different param like `Expr<Vec<S>>`. `transform_type` checks a root reference's args
    /// against this and aborts on a mismatch instead of silently dropping the param.
    root_ident_args: HashMap<String, Vec<String>>,
}

fn collect_refs(ty: &Type, known: &HashSet<String>, out: &mut HashSet<String>) {
    match ty {
        Type::Path(TypePath { path, .. }) => {
            if let Some(seg) = path.segments.first() {
                let name = seg.ident.to_string();
                if known.contains(&name) {
                    out.insert(name);
                }
            }
            for seg in &path.segments {
                if let PathArguments::AngleBracketed(ab) = &seg.arguments {
                    for arg in &ab.args {
                        if let GenericArgument::Type(t) = arg {
                            collect_refs(t, known, out);
                        }
                    }
                }
            }
        }
        Type::Reference(r) => collect_refs(&r.elem, known, out),
        Type::Slice(s) => collect_refs(&s.elem, known, out),
        Type::Array(a) => collect_refs(&a.elem, known, out),
        Type::Tuple(t) => t.elems.iter().for_each(|e| collect_refs(e, known, out)),
        _ => {}
    }
}

fn collect_refs_fields(fields: &Fields, known: &HashSet<String>, out: &mut HashSet<String>) {
    for field in fields {
        collect_refs(&field.ty, known, out);
    }
}

fn collect_refs_item(item: &Item, known: &HashSet<String>) -> HashSet<String> {
    let mut out = HashSet::new();
    match item {
        Item::Enum(e) => e
            .variants
            .iter()
            .for_each(|v| collect_refs_fields(&v.fields, known, &mut out)),
        Item::Struct(s) => collect_refs_fields(&s.fields, known, &mut out),
        _ => {}
    }
    out
}

// Collect only direct (outermost type constructor) references — used to pick the root type.
fn collect_direct_refs_item(item: &Item, known: &HashSet<String>) -> HashSet<String> {
    let mut out = HashSet::new();
    let check = |ty: &Type, out: &mut HashSet<String>| {
        if let Type::Path(TypePath { path, .. }) = ty {
            if let Some(seg) = path.segments.first() {
                let name = seg.ident.to_string();
                if known.contains(&name) {
                    out.insert(name);
                }
            }
        }
    };
    match item {
        Item::Enum(e) => {
            for v in &e.variants {
                for field in &v.fields {
                    check(&field.ty, &mut out);
                }
            }
        }
        Item::Struct(s) => {
            for field in &s.fields {
                check(&field.ty, &mut out);
            }
        }
        _ => {}
    }
    out
}

/// The *recursive* strongly-connected components of the reference `graph`, via Tarjan's algorithm
/// (`safegraph`). Each returned set is one **independent cycle**: a non-trivial SCC (mutual recursion
/// of size > 1, including longer cycles) or a singleton SCC carrying a self-loop (a directly self-
/// referential type). Non-recursive singletons are omitted. Two types share a set iff they are
/// mutually reachable, so independent cycles in one module come back as *separate* sets — each gets
/// its own recurse machinery. The Vec is sorted by each SCC's least type name for deterministic codegen.
fn find_cycle_sccs(graph: &HashMap<String, HashSet<String>>) -> Vec<HashSet<String>> {
    use safegraph::algo::connectivity::tarjan_scc;
    use safegraph::graph::Graph;
    use safegraph::BTreeGraph;

    // Build the directed reference graph. `safegraph`'s map-backed graph keys nodes by their value
    // (which must be `Copy`), so each type name gets a small `u32` id (its position in `names`);
    // edges carry a bare unique counter (edges are keyed by value too).
    let names: Vec<&String> = graph.keys().collect();
    let id_of: HashMap<&str, u32> = names
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i as u32))
        .collect();

    let mut g = BTreeGraph::<u32, u32>::default();
    let node_ix: Vec<_> = (0..names.len() as u32)
        .map(|i| g.insert_node(i).unwrap())
        .collect();
    let mut edge_id = 0u32;
    for (from, tos) in graph {
        let fi = node_ix[id_of[from.as_str()] as usize];
        for to in tos {
            if let Some(&tid) = id_of.get(to.as_str()) {
                g.push_edge(edge_id, [fi, node_ix[tid as usize]]).unwrap();
                edge_id += 1;
            }
        }
    }

    let mut sccs: Vec<HashSet<String>> = Vec::new();
    for scc in tarjan_scc(&g) {
        if scc.len() > 1 {
            sccs.push(scc.iter().map(|&n| names[*g.node(n) as usize].clone()).collect());
        } else {
            let name = names[*g.node(scc[0]) as usize];
            if graph.get(name).map_or(false, |refs| refs.contains(name)) {
                sccs.push(std::iter::once(name.clone()).collect());
            }
        }
    }
    sccs.sort_by(|a, b| a.iter().min().cmp(&b.iter().min()));
    sccs
}

fn transform_type(ty: &Type, ctx: &TransformCtx) -> Type {
    match ty {
        Type::Path(TypePath { qself: None, path }) => {
            if let Some(seg) = path.segments.first() {
                let name = seg.ident.to_string();
                if ctx.root_types.contains(&name) {
                    // A back-edge to the root collapses to the single opaque depth param `__Rec`, so
                    // any generic arguments it supplies must be the root's own params unchanged. A
                    // non-identity argument (`Expr<Vec<S>>`, `Expr<u8>`, reordered params, …) would be
                    // silently dropped here and miscompile; reject it instead.
                    if let PathArguments::AngleBracketed(ab) = &seg.arguments {
                        if let Some(identity) = ctx.root_ident_args.get(&name) {
                            let actual: Vec<String> =
                                ab.args.iter().map(|a| quote!(#a).to_string()).collect();
                            if &actual != identity {
                                abort!(
                                    seg.ident,
                                    "#[recurse]: the reference to recursion root `{}` carries \
                                     generic arguments `<{}>` that differ from its declared \
                                     parameters `<{}>`. A root reference is the cycle's back-edge \
                                     and collapses to the single depth parameter `__Rec`, so it \
                                     must repeat the root's parameters verbatim; a non-identity \
                                     argument (e.g. `{}<Vec<{}>>`) is unsupported. Move the \
                                     differing part into its own `#[derive(Ast)]` type, or pass the \
                                     root's parameters unchanged.",
                                    name,
                                    actual.join(", "),
                                    identity.join(", "),
                                    name,
                                    identity.first().map(String::as_str).unwrap_or("S"),
                                );
                            }
                        }
                    }
                    let p = &ctx.root_rec[&name];
                    return syn::parse_quote!(#p);
                }
                if let Some(internal) = ctx.internal_names.get(&name) {
                    // A cross-edge to another cycle type: keep its generic args as written (a
                    // back-edge to a root inside them becomes that root's depth param) and append all
                    // of the cycle's depth params.
                    let existing: Vec<GenericArgument> =
                        if let PathArguments::AngleBracketed(ab) = &seg.arguments {
                            ab.args
                                .iter()
                                .map(|arg| match arg {
                                    GenericArgument::Type(t) => {
                                        GenericArgument::Type(transform_type(t, ctx))
                                    }
                                    other => other.clone(),
                                })
                                .collect()
                        } else {
                            vec![]
                        };
                    let recs = &ctx.rec_params;
                    return syn::parse_quote!( #internal < #(#existing,)* #(#recs),* > );
                }
            }
            // Non-cycle path: recurse into generic args
            let mut new_path = path.clone();
            for seg in new_path.segments.iter_mut() {
                if let PathArguments::AngleBracketed(ref mut ab) = seg.arguments {
                    for arg in ab.args.iter_mut() {
                        if let GenericArgument::Type(t) = arg {
                            *t = transform_type(t, ctx);
                        }
                    }
                }
            }
            Type::Path(TypePath {
                qself: None,
                path: new_path,
            })
        }
        Type::Reference(r) => Type::Reference(syn::TypeReference {
            elem: Box::new(transform_type(&r.elem, ctx)),
            ..r.clone()
        }),
        Type::Slice(s) => Type::Slice(syn::TypeSlice {
            elem: Box::new(transform_type(&s.elem, ctx)),
            ..s.clone()
        }),
        Type::Array(a) => Type::Array(syn::TypeArray {
            elem: Box::new(transform_type(&a.elem, ctx)),
            ..a.clone()
        }),
        Type::Tuple(t) => Type::Tuple(syn::TypeTuple {
            elems: t.elems.iter().map(|e| transform_type(e, ctx)).collect(),
            ..t.clone()
        }),
        other => other.clone(),
    }
}

fn transform_fields(fields: &mut Fields, ctx: &TransformCtx) {
    match fields {
        Fields::Named(FieldsNamed { named, .. }) => {
            for field in named.iter_mut() {
                field.ty = transform_type(&field.ty, ctx);
            }
        }
        Fields::Unnamed(FieldsUnnamed { unnamed, .. }) => {
            for field in unnamed.iter_mut() {
                field.ty = transform_type(&field.ty, ctx);
            }
        }
        Fields::Unit => {}
    }
}

fn item_generics(it: &Item) -> Option<&Generics> {
    match it {
        Item::Enum(e) => Some(&e.generics),
        Item::Struct(s) => Some(&s.generics),
        _ => None,
    }
}

/// A stable identity for a generic param (kind + name + const type), used to compare a cycle type's
/// params against the root's.
fn param_key(p: &GenericParam) -> String {
    match p {
        GenericParam::Lifetime(lt) => format!("lifetime {}", lt.lifetime.ident),
        GenericParam::Type(t) => format!("type {}", t.ident),
        GenericParam::Const(c) => {
            let ty = &c.ty;
            format!("const {}: {}", c.ident, quote!(#ty))
        }
    }
}

/// One generic param's `(declaration, use)` token forms. They coincide for lifetimes (`'a`) and type
/// params (`T`) but differ for const params (`const N: usize` vs `N`).
fn param_tokens(p: &GenericParam) -> (TokenStream, TokenStream) {
    match p {
        GenericParam::Lifetime(lt) => {
            let l = &lt.lifetime;
            (quote!(#l), quote!(#l))
        }
        GenericParam::Type(t) => {
            let i = &t.ident;
            (quote!(#i), quote!(#i))
        }
        GenericParam::Const(c) => {
            let (i, ty) = (&c.ident, &c.ty);
            (quote!(const #i: #ty), quote!(#i))
        }
    }
}

/// A type's generic params as `(declaration, use)` token lists, in declaration order. Used both for
/// the root (the depth chain) and per cycle type (its own alias / node type), so a recurse cycle may
/// carry lifetimes / type params / const params alongside the depth `__Rec`.
fn generic_tokens(generics: &Generics) -> (Vec<TokenStream>, Vec<TokenStream>) {
    let mut decl = Vec::new();
    let mut us = Vec::new();
    for p in &generics.params {
        let (d, u) = param_tokens(p);
        decl.push(d);
        us.push(u);
    }
    (decl, us)
}

/// A cycle type's own params in *use* form (to spell its `__*Rec` node) plus the params the root does
/// NOT have, in *declaration* form. In the visitor the trait is keyed on the root's (shared) params;
/// a cycle type's extra params become generics on its `visit_*` method.
fn type_param_tokens(
    generics: &Generics,
    root_keys: &HashSet<String>,
) -> (Vec<TokenStream>, Vec<TokenStream>) {
    let mut own_use = Vec::new();
    let mut extra_decl = Vec::new();
    for p in &generics.params {
        let (decl, us) = param_tokens(p);
        own_use.push(us);
        if !root_keys.contains(&param_key(p)) {
            extra_decl.push(decl);
        }
    }
    (own_use, extra_decl)
}

fn transform_item(item: Item, ctx: &TransformCtx) -> Item {
    match item {
        Item::Enum(mut e)
            if matches!(e.vis, Visibility::Public(_))
                && ctx.cycle_types.contains(&e.ident.to_string()) =>
        {
            let orig_name = e.ident.to_string();
            e.ident = ctx.internal_names[&orig_name].clone();
            // Keep this type's own params and append one depth param per root (each defaulting to
            // that root's depth chain, spelled with the root's params — which every cycle type shares).
            for d in &ctx.rec_decls {
                e.generics.params.push(syn::parse_quote!(#d));
            }
            for variant in &mut e.variants {
                transform_fields(&mut variant.fields, ctx);
            }
            Item::Enum(e)
        }
        Item::Struct(mut s)
            if matches!(s.vis, Visibility::Public(_))
                && ctx.cycle_types.contains(&s.ident.to_string()) =>
        {
            let orig_name = s.ident.to_string();
            s.ident = ctx.internal_names[&orig_name].clone();
            for d in &ctx.rec_decls {
                s.generics.params.push(syn::parse_quote!(#d));
            }
            transform_fields(&mut s.fields, ctx);
            Item::Struct(s)
        }
        // Inherent impl block whose Self type is a cycle type: rename Self type, add __Rec param,
        // and transform all method signature types so they use the internal names.
        Item::Impl(mut impl_block) if impl_block.trait_.is_none() => {
            let cycle_name: Option<String> = (|| {
                let Type::Path(TypePath { qself: None, path }) = impl_block.self_ty.as_ref() else {
                    return None;
                };
                let seg = path.segments.first()?;
                let name = seg.ident.to_string();
                ctx.cycle_types.contains(&name).then_some(name)
            })();

            if let Some(name) = cycle_name {
                let internal = ctx.internal_names[&name].clone();
                let recs = &ctx.rec_params;

                // Rename self_ty (keeping its own generic args) and append one depth type-arg per root.
                if let Type::Path(TypePath { path, .. }) = impl_block.self_ty.as_mut() {
                    if let Some(seg) = path.segments.first_mut() {
                        seg.ident = internal;
                        match &mut seg.arguments {
                            PathArguments::AngleBracketed(ab) => {
                                for arg in ab.args.iter_mut() {
                                    if let GenericArgument::Type(t) = arg {
                                        *t = transform_type(t, ctx);
                                    }
                                }
                                for r in recs {
                                    ab.args.push(syn::parse_quote!(#r));
                                }
                            }
                            PathArguments::None => {
                                let mut args: Punctuated<GenericArgument, Token![,]> =
                                    Punctuated::new();
                                for r in recs {
                                    args.push(syn::parse_quote!(#r));
                                }
                                seg.arguments =
                                    PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                                        colon2_token: None,
                                        lt_token: Default::default(),
                                        args,
                                        gt_token: Default::default(),
                                    });
                            }
                            _ => {}
                        }
                    }
                }

                // Add the depth params to the impl generics (no defaults — not allowed on an impl).
                for r in recs {
                    impl_block.generics.params.push(syn::parse_quote!(#r));
                }

                // Transform types in method signatures (params + return type)
                for item in &mut impl_block.items {
                    if let ImplItem::Fn(method) = item {
                        for input in &mut method.sig.inputs {
                            if let FnArg::Typed(pat_type) = input {
                                pat_type.ty = Box::new(transform_type(&pat_type.ty, ctx));
                            }
                        }
                        if let ReturnType::Type(_, ty) = &mut method.sig.output {
                            *ty = Box::new(transform_type(ty, ctx));
                        }
                    }
                }
            }

            Item::Impl(impl_block)
        }
        other => other,
    }
}

// ---------------------------------------------------------------------------
// `#[recurse(visit)]`: a depth-generic visitor over the cycle.
//
// `#[recurse]` rewrites the cycle's back-edges to the root into the generic `__Rec` param and each
// nesting level into a distinct concrete type, so a fixed-type visitor cannot traverse it. Instead
// we generate visit methods generic over the depth (`__R`) plus a `VisitRec` dispatch trait that the
// root's depth chain (`__RootRec<.., __R>`) and the terminator implement, turning the depth recursion
// into trait dispatch. Single-root cycles only.
// ---------------------------------------------------------------------------

/// See through transparent wrappers to a tuple type's element list (a bare tuple, or one behind
/// `Type::Group`/`Type::Paren`). `None` if `ty` is not a tuple.
fn as_tuple(ty: &Type) -> Option<&Punctuated<Type, Token![,]>> {
    match ty {
        Type::Tuple(t) => Some(&t.elems),
        Type::Group(g) => as_tuple(&g.elem),
        Type::Paren(p) => as_tuple(&p.elem),
        _ => None,
    }
}

/// Dispatch one field of a cycle type: a back-edge to a **root** drives via that root's depth param
/// (`root_depth[head]::visit_rec`); a cross-edge to another cycle type calls that type's visit method;
/// anything else is a leaf (`None`, caller binds `_`). `binding` is the destructured field (`&Field`).
/// `root_depth` maps each root's name to the depth-param ident in scope (one root → `{root: __R}`;
/// several roots → `{A: __R0, B: __R1, …}`).
fn recurse_dispatch_field(
    ty: &Type,
    binding: &TokenStream,
    cycle_types: &std::collections::HashSet<String>,
    root_depth: &HashMap<String, Ident>,
) -> Option<TokenStream> {
    // A tuple field: destructure it and dispatch each element (an element may itself be a cycle
    // ref, a container of one, or a nested tuple). Leaf elements bind `_`; if no element is
    // followed the whole tuple is a leaf.
    if let Some(elems) = as_tuple(ty) {
        let mut pats = Vec::new();
        let mut stmts = Vec::new();
        for (i, elem) in elems.iter().enumerate() {
            let bi = Ident::new(&format!("__t{i}"), Span::call_site());
            if let Some(stmt) = recurse_dispatch_field(elem, &quote!(#bi), cycle_types, root_depth) {
                pats.push(quote!(#bi));
                stmts.push(stmt);
            } else {
                pats.push(quote!(_));
            }
        }
        if stmts.is_empty() {
            return None; // tuple of only leaves → leaf
        }
        return Some(quote!( { let ( #(#pats,)* ) = #binding; #(#stmts)* } ));
    }

    let p = peel(ty, &std::collections::HashSet::new())?;
    // Cycle membership keys on the FIRST path segment: a same-module cycle reference is always a
    // bare single-segment ident, so a foreign multi-segment path (`super::other::Stmt`) whose LAST
    // segment merely equals a cycle type name is correctly a leaf (transform_type, keyed on the
    // first segment, would not rename it). For a real bare cycle ref `head_lead == head`, so the
    // visit method (`visit_<snake(head)>`) is unchanged.
    let hs = p.head_lead.to_string();
    let depth = root_depth.get(&hs);
    if depth.is_none() && !cycle_types.contains(&hs) {
        return None; // leaf
    }
    // Nested containers (e.g. `Vec<Option<_>>`) cannot be traversed — reject cleanly, matching the
    // `visitor!()` builder, rather than emitting mistyped traversal code.
    if p.nested {
        abort!(
            ty,
            "#[recurse(visit)] cannot traverse a nested container (e.g. `Vec<Option<_>>`); wrap the \
             inner part in its own #[derive(Ast)] type"
        );
    }
    // A followed field's head is a bare single-segment cycle ref, so `head_lead == head` here; key
    // the visit-method name on `head_lead` for consistency with the membership decision above.
    let stars: TokenStream = (0..=p.head_box).map(|_| quote!(*)).collect();
    Some(match p.container {
        Container::Direct => recurse_visit_one(depth, &p.head_lead, &stars, binding),
        Container::Seq => {
            // `.iter()` auto-derefs through any `Box` around the sequence (`cont_box`).
            let inner = recurse_visit_one(depth, &p.head_lead, &stars, &quote!(__x));
            quote!( for __x in #binding.iter() { #inner } )
        }
        Container::Opt => {
            // Patterns do not auto-deref `Box`, so deref through any `Box` around the `Option`
            // (`cont_box`) before matching; the leading `*` also derefs the `&Field` binding.
            let cont_stars: TokenStream = (0..=p.cont_box).map(|_| quote!(*)).collect();
            let inner = recurse_visit_one(depth, &p.head_lead, &stars, &quote!(__x));
            quote!( if let ::core::option::Option::Some(__x) = & #cont_stars #binding { #inner } )
        }
    })
}

/// Emit the visit of a single value `acc` (a `&Box^n<head>`): drive via the root's depth param
/// (`depth = Some`) or call `v.visit_<head>` for a cross-edge (`depth = None`); `stars` derefs
/// through any `Box` to a `&head`.
fn recurse_visit_one(
    depth: Option<&Ident>,
    head: &Ident,
    stars: &TokenStream,
    acc: &TokenStream,
) -> TokenStream {
    if let Some(r) = depth {
        quote!( #r::visit_rec(& #stars #acc, v); )
    } else {
        let m = Ident::new(&format!("visit_{}", to_snake(head)), Span::call_site());
        quote!( v.#m(& #stars #acc); )
    }
}

/// `(pattern, statements)` for a cycle type's fields, dispatching followed fields.
fn recurse_visit_fields(
    fields: &Fields,
    cycle_types: &std::collections::HashSet<String>,
    root_depth: &HashMap<String, Ident>,
) -> (TokenStream, TokenStream) {
    match fields {
        Fields::Named(named) => {
            let mut binds = Vec::new();
            let mut stmts = Vec::new();
            for f in &named.named {
                let name = f.ident.clone().unwrap();
                if let Some(stmt) =
                    recurse_dispatch_field(&f.ty, &quote!(#name), cycle_types, root_depth)
                {
                    binds.push(quote!(#name));
                    stmts.push(stmt);
                }
            }
            (quote!( { #(#binds,)* .. } ), quote!( #(#stmts)* ))
        }
        Fields::Unnamed(unnamed) => {
            let mut pats = Vec::new();
            let mut stmts = Vec::new();
            for (i, f) in unnamed.unnamed.iter().enumerate() {
                let b = Ident::new(&format!("__f{i}"), Span::call_site());
                if let Some(stmt) =
                    recurse_dispatch_field(&f.ty, &quote!(#b), cycle_types, root_depth)
                {
                    pats.push(quote!(#b));
                    stmts.push(stmt);
                } else {
                    pats.push(quote!(_));
                }
            }
            (quote!( ( #(#pats),* ) ), quote!( #(#stmts)* ))
        }
        Fields::Unit => (quote!(), quote!()),
    }
}

/// Body of a cycle type's `visit_*` drive fn: destructure `i` (the internal `__XxxRec` type) and
/// dispatch followed fields.
fn recurse_visit_body(
    orig: &Item,
    internal: &Ident,
    cycle_types: &std::collections::HashSet<String>,
    root_depth: &HashMap<String, Ident>,
) -> TokenStream {
    match orig {
        Item::Enum(e) => {
            let arms = e.variants.iter().map(|v| {
                let (pat, stmts) = recurse_visit_fields(&v.fields, cycle_types, root_depth);
                let vid = &v.ident;
                quote!( #internal::#vid #pat => { #stmts } )
            });
            quote!( match i { #(#arms)* } )
        }
        Item::Struct(s) => {
            let (pat, stmts) = recurse_visit_fields(&s.fields, cycle_types, root_depth);
            match &s.fields {
                Fields::Unit => quote!(),
                _ => quote!( let #internal #pat = i; #stmts ),
            }
        }
        _ => quote!(),
    }
}

/// Generate the depth-generic visitor for a single-root cycle. `gen_decl` / `gen_use` are the ROOT's
/// generic params in declaration / use form (they coincide except for const params) — the `Visit` /
/// `VisitRec` traits are keyed on them. A cycle type may carry params beyond the root's: those become
/// generics on its `visit_*` method (`extra_decl`), and its node type is spelled with its *own* full
/// params (`own_use`). `root_keys` identifies which of a type's params are the root's (shared).
#[allow(clippy::too_many_arguments)]
fn generate_recurse_visitor(
    items: &[Item],
    cycle_types: &std::collections::HashSet<String>,
    root_name: &str,
    internal_names: &HashMap<String, Ident>,
    term_ident: &Ident,
    term_args: &TokenStream,
    gen_decl: &[TokenStream],
    gen_use: &[TokenStream],
    root_keys: &HashSet<String>,
    // `None` for a lone cycle → the legacy trait names `Visit` / `VisitRec`. `Some(root)` when the
    // module holds several independent cycles → per-root names `<Root>Visit` / `<Root>VisitRec`, so
    // the cycles' visitors don't collide. The free `visit_*` fns and `XxxNode` aliases are already
    // per-type-unique, so only the two trait names need disambiguating.
    trait_prefix: Option<&str>,
) -> TokenStream {
    let root_internal = &internal_names[root_name];
    let visit_t = Ident::new(
        &format!("{}Visit", trait_prefix.unwrap_or("")),
        Span::call_site(),
    );
    let visit_rec_t = Ident::new(
        &format!("{}VisitRec", trait_prefix.unwrap_or("")),
        Span::call_site(),
    );
    let visit_root = Ident::new(
        &format!(
            "visit_{}",
            to_snake(&Ident::new(root_name, Span::call_site()))
        ),
        Span::call_site(),
    );
    // Single root → its back-edge drives via the lone depth param `__R`.
    let root_depth: HashMap<String, Ident> = std::iter::once((
        root_name.to_string(),
        Ident::new("__R", Span::call_site()),
    ))
    .collect();

    // Per cycle type, the data the generated items need — computed here in Rust, emitted by the
    // `#(for …)` templates below. `own_use` spells the type's `__*Rec` node; `extra_decl` are its
    // params beyond the root's (made generic on its `visit_*`); `body` drives its followed fields.
    struct CycInfo {
        vm: Ident,
        node: Ident,
        internal: Ident,
        own_use: Vec<TokenStream>,
        extra_decl: Vec<TokenStream>,
        body: TokenStream,
    }
    let infos: Vec<CycInfo> = items
        .iter()
        .filter_map(|it| {
            let orig = match it {
                Item::Enum(e) => &e.ident,
                Item::Struct(s) => &s.ident,
                _ => return None,
            };
            if !cycle_types.contains(&orig.to_string()) {
                return None;
            }
            let internal = internal_names[&orig.to_string()].clone();
            let (own_use, extra_decl) =
                type_param_tokens(item_generics(it).expect("cycle item"), root_keys);
            Some(CycInfo {
                vm: Ident::new(&format!("visit_{}", to_snake(orig)), Span::call_site()),
                node: Ident::new(&format!("{orig}Node"), Span::call_site()),
                body: recurse_visit_body(it, &internal, cycle_types, &root_depth),
                internal,
                own_use,
                extra_decl,
            })
        })
        .collect();

    quote! {
        /// Dispatch trait turning the cycle's depth recursion into trait calls: implemented by the
        /// root's depth chain (drives the root visit) and by the terminator (no-op).
        pub trait #visit_rec_t < #(#gen_decl,)* __V > {
            fn visit_rec(&self, v: &mut __V);
        }

        /// Depth-generic visitor over the `#[recurse]` cycle. Implement the `visit_*` methods
        /// (each generic over the remaining depth `__R`); call the free `visit_*` to descend.
        pub trait #visit_t < #(#gen_decl),* > {
            #(for info in &infos) {
                fn #{&info.vm}< #(for e in &info.extra_decl) { #e, } __R: #visit_rec_t < #(#gen_use,)* Self > >(
                    &mut self,
                    i: & #{&info.internal} < #(for u in &info.own_use) { #u, } __R >,
                ) where Self: ::core::marker::Sized {
                    #{&info.vm}(self, i)
                }
            }
        }

        #(for info in &infos) {
            pub fn #{&info.vm}< #(#gen_decl,)* #(for e in &info.extra_decl) { #e, } __V: #visit_t < #(#gen_use),* >, __R: #visit_rec_t < #(#gen_use,)* __V > >(
                v: &mut __V,
                i: & #{&info.internal} < #(for u in &info.own_use) { #u, } __R >,
            ) {
                #{&info.body}
            }
        }

        #(for info in &infos) {
            #[doc = "Depth-generic node type for the visitor (an alias of the internal recurse type)."]
            pub use #{&info.internal} as #{&info.node};
        }

        impl< #(#gen_decl,)* __V: #visit_t < #(#gen_use),* >, __R: #visit_rec_t < #(#gen_use,)* __V > >
            #visit_rec_t < #(#gen_use,)* __V > for #root_internal < #(#gen_use,)* __R >
        {
            fn visit_rec(&self, v: &mut __V) {
                <__V as #visit_t < #(#gen_use),* >>::#visit_root(v, self);
            }
        }
        impl< #(#gen_decl,)* __V: #visit_t < #(#gen_use),* > >
            #visit_rec_t < #(#gen_use,)* __V > for #term_ident #term_args
        {
            fn visit_rec(&self, _v: &mut __V) {}
        }
    }
}

/// Is the subgraph induced by the cycle's **non-root** types cyclic? Used as the multi-root soundness
/// guard: the depth only decrements at a self-referential root, so a cycle running entirely through
/// non-root types would never terminate. Built and tested with `safegraph` (same `u32`-keyed graph as
/// `find_cycle_sccs`, restricted to `scc \ root_types`).
fn subgraph_is_cyclic(
    scc: &HashSet<String>,
    root_types: &HashSet<String>,
    type_refs: &HashMap<String, HashSet<String>>,
) -> bool {
    use safegraph::algo::connectivity::is_cyclic_directed;
    use safegraph::graph::Graph;
    use safegraph::BTreeGraph;

    let nodes: Vec<&String> = scc.iter().filter(|n| !root_types.contains(*n)).collect();
    let id_of: HashMap<&str, u32> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i as u32))
        .collect();
    let mut g = BTreeGraph::<u32, u32>::default();
    let node_ix: Vec<_> = (0..nodes.len() as u32)
        .map(|i| g.insert_node(i).unwrap())
        .collect();
    let mut edge_id = 0u32;
    for n in &nodes {
        let fi = node_ix[id_of[n.as_str()] as usize];
        for to in type_refs.get(n.as_str()).into_iter().flatten() {
            if let Some(&tid) = id_of.get(to.as_str()) {
                g.push_edge(edge_id, [fi, node_ix[tid as usize]]).unwrap();
                edge_id += 1;
            }
        }
    }
    is_cyclic_directed(&g)
}

/// The depth-generic visitor for a cycle with **several roots**. Mirrors `generate_recurse_visitor`
/// but threads one depth parameter per root (`__R0, __R1, …`, in `roots_sorted` order): every visit
/// method is generic over all of them, a back-edge to root `i` drives via `__Ri::visit_rec`, and each
/// root's depth chain plus each terminator implements `VisitRec`. `trait_prefix` root-prefixes the
/// trait names when the module holds other cycles too.
#[allow(clippy::too_many_arguments)]
fn generate_multiroot_visitor(
    items: &[Item],
    cycle_types: &HashSet<String>,
    roots_sorted: &[String],
    internal_names: &HashMap<String, Ident>,
    term_for_root: &HashMap<String, Ident>,
    gen_decl: &[TokenStream],
    gen_use: &[TokenStream],
    root_keys: &HashSet<String>,
    term_args: &TokenStream,
    trait_prefix: Option<&str>,
) -> TokenStream {
    let visit_t = Ident::new(
        &format!("{}Visit", trait_prefix.unwrap_or("")),
        Span::call_site(),
    );
    let visit_rec_t = Ident::new(
        &format!("{}VisitRec", trait_prefix.unwrap_or("")),
        Span::call_site(),
    );
    // One depth param per root, `__R{i}` in canonical order, plus the root → param map for dispatch.
    let dps: Vec<Ident> = (0..roots_sorted.len())
        .map(|i| Ident::new(&format!("__R{i}"), Span::call_site()))
        .collect();
    let root_depth: HashMap<String, Ident> = roots_sorted
        .iter()
        .cloned()
        .zip(dps.iter().cloned())
        .collect();

    struct CycInfo {
        vm: Ident,
        node: Ident,
        internal: Ident,
        own_use: Vec<TokenStream>,
        extra_decl: Vec<TokenStream>,
        body: TokenStream,
    }
    let infos: Vec<CycInfo> = items
        .iter()
        .filter_map(|it| {
            let orig = match it {
                Item::Enum(e) => &e.ident,
                Item::Struct(s) => &s.ident,
                _ => return None,
            };
            if !cycle_types.contains(&orig.to_string()) {
                return None;
            }
            let internal = internal_names[&orig.to_string()].clone();
            let (own_use, extra_decl) =
                type_param_tokens(item_generics(it).expect("cycle item"), root_keys);
            Some(CycInfo {
                vm: Ident::new(&format!("visit_{}", to_snake(orig)), Span::call_site()),
                node: Ident::new(&format!("{orig}Node"), Span::call_site()),
                body: recurse_visit_body(it, &internal, cycle_types, &root_depth),
                internal,
                own_use,
                extra_decl,
            })
        })
        .collect();

    // Per root: its internal node ident + the `visit_<root>` method that its depth chain drives.
    let root_internals: Vec<&Ident> = roots_sorted.iter().map(|r| &internal_names[r]).collect();
    let root_vms: Vec<Ident> = roots_sorted
        .iter()
        .map(|r| Ident::new(&format!("visit_{}", to_snake(&Ident::new(r, Span::call_site()))), Span::call_site()))
        .collect();
    let root_terms: Vec<&Ident> = roots_sorted.iter().map(|r| &term_for_root[r]).collect();

    quote! {
        /// Dispatch trait turning the cycle's depth recursion into trait calls: implemented by every
        /// root's depth chain (each drives its own root visit) and by every terminator (no-op).
        pub trait #visit_rec_t < #(#gen_decl,)* __V > {
            fn visit_rec(&self, v: &mut __V);
        }

        /// Depth-generic visitor over the multi-root `#[recurse]` cycle. Implement the `visit_*`
        /// methods (each generic over the remaining depth of *every* root); call the free `visit_*`.
        pub trait #visit_t < #(#gen_decl),* > {
            #(for info in &infos) {
                fn #{&info.vm}<
                    #(for e in &info.extra_decl) { #e, }
                    #(for d in &dps) { #d: #visit_rec_t < #(#gen_use,)* Self >, }
                >(
                    &mut self,
                    i: & #{&info.internal} < #(for u in &info.own_use) { #u, } #(#dps),* >,
                ) where Self: ::core::marker::Sized {
                    #{&info.vm}(self, i)
                }
            }
        }

        #(for info in &infos) {
            pub fn #{&info.vm}<
                #(#gen_decl,)*
                #(for e in &info.extra_decl) { #e, }
                __V: #visit_t < #(#gen_use),* >,
                #(for d in &dps) { #d: #visit_rec_t < #(#gen_use,)* __V >, }
            >(
                v: &mut __V,
                i: & #{&info.internal} < #(for u in &info.own_use) { #u, } #(#dps),* >,
            ) {
                #{&info.body}
            }
        }

        #(for info in &infos) {
            #[doc = "Depth-generic node type for the visitor (an alias of the internal recurse type)."]
            pub use #{&info.internal} as #{&info.node};
        }

        // Each root's depth chain drives that root's visit method.
        #(for (ri, internal) in root_internals.iter().enumerate()) {
            impl<
                #(#gen_decl,)*
                #(for d in &dps) { #d: #visit_rec_t < #(#gen_use,)* __V >, }
                __V: #visit_t < #(#gen_use),* >
            >
                #visit_rec_t < #(#gen_use,)* __V > for #internal < #(#gen_use,)* #(#dps),* >
            {
                fn visit_rec(&self, v: &mut __V) {
                    <__V as #visit_t < #(#gen_use),* >>::#{&root_vms[ri]}(v, self);
                }
            }
        }
        // Every terminator is a no-op leaf.
        #(for term in &root_terms) {
            impl< #(#gen_decl,)* __V: #visit_t < #(#gen_use),* > >
                #visit_rec_t < #(#gen_use,)* __V > for #term #term_args
            {
                fn visit_rec(&self, _v: &mut __V) {}
            }
        }
    }
}

/// Build the recurse machinery for ONE independent cycle (`scc`): pick its root, rename its types,
/// and produce (a) the `TransformCtx` that rewrites the cycle's items and (b) the *tail* tokens
/// appended to the module (terminator + its `Parse`/`Unparse` impls, the depth-default alias, the
/// public type aliases, and — under `want_visit` — the depth-generic visitor). Each cycle is handled
/// independently, so a module may hold several (see `find_cycle_sccs`); `multi_scc` only controls
/// whether the visitor's trait names are root-prefixed to avoid collisions between cycles.
#[allow(clippy::too_many_arguments)]
fn build_scc(
    scc: &HashSet<String>,
    items: &[Item],
    type_refs: &HashMap<String, HashSet<String>>,
    direct_type_refs: &HashMap<String, HashSet<String>>,
    recursion_depth: usize,
    want_visit: bool,
    multi_scc: bool,
    mod_ident: &Ident,
) -> (TransformCtx, TokenStream) {
    // Root types: this cycle's types that directly reference themselves.
    let root_types: HashSet<String> = scc
        .iter()
        .filter(|name| {
            type_refs
                .get(*name)
                .map_or(false, |refs| refs.contains(*name))
        })
        .cloned()
        .collect();

    // Direct-reference counts within this cycle: how many of its types reference each as a bare field.
    let mut direct_ref_counts: HashMap<&str, usize> = HashMap::new();
    for (from, refs) in direct_type_refs {
        if !scc.contains(from) {
            continue;
        }
        for r in refs {
            if scc.contains(r) {
                *direct_ref_counts.entry(r.as_str()).or_insert(0) += 1;
            }
        }
    }

    // Root type: the one that controls recursion depth.
    // Priority: (1) self-referential types, (2) most directly referenced by other cycle types
    // (so that destructuring a non-root gives back root type directly), (3) most referenced
    // overall, (4) alphabetically smallest for determinism.
    let root_name: String = if !root_types.is_empty() {
        root_types.iter().min().cloned().unwrap()
    } else {
        let mut ref_counts: HashMap<&str, usize> = HashMap::new();
        for (from, refs) in type_refs {
            if !scc.contains(from) {
                continue;
            }
            for r in refs {
                if scc.contains(r) {
                    *ref_counts.entry(r.as_str()).or_insert(0) += 1;
                }
            }
        }
        let mut candidates: Vec<&String> = scc.iter().collect();
        candidates.sort_by(|a, b| {
            let da = direct_ref_counts.get(a.as_str()).copied().unwrap_or(0);
            let db = direct_ref_counts.get(b.as_str()).copied().unwrap_or(0);
            db.cmp(&da)
                .then_with(|| {
                    let ra = ref_counts.get(a.as_str()).copied().unwrap_or(0);
                    let rb = ref_counts.get(b.as_str()).copied().unwrap_or(0);
                    rb.cmp(&ra)
                })
                .then_with(|| a.as_str().cmp(b.as_str()))
        });
        candidates[0].clone()
    };

    let root_ident = Ident::new(&root_name, Span::call_site());
    let term_ident = Ident::new(&format!("{root_name}Term"), Span::call_site());
    let default_alias = Ident::new(&format!("__{root_name}Default"), Span::call_site());

    // Internal (renamed) idents: "Expr" → "__ExprRec"
    let internal_names: HashMap<String, Ident> = scc
        .iter()
        .map(|n| {
            (
                n.clone(),
                Ident::new(&format!("__{n}Rec"), Span::call_site()),
            )
        })
        .collect();

    // The root's full generics drive the depth aliases. (The root is always one of the cycle types,
    // so the fallback is unreachable.)
    let root_generics: Generics = items
        .iter()
        .find_map(|item| match item {
            Item::Enum(e)
                if matches!(e.vis, Visibility::Public(_)) && e.ident.to_string() == root_name =>
            {
                Some(e.generics.clone())
            }
            Item::Struct(s)
                if matches!(s.vis, Visibility::Public(_)) && s.ident.to_string() == root_name =>
            {
                Some(s.generics.clone())
            }
            _ => None,
        })
        .unwrap_or_default();

    // Root generics as (declaration, use) token forms. The `__Rec` default `__RootDefault<root
    // params>` is referenced by every cycle type, so each must declare all of the root's params; a
    // cycle type may additionally carry its own extra params (threaded into its node type, and — for
    // the visitor — made generic on its `visit_*` method).
    let (gen_decl, gen_use) = generic_tokens(&root_generics);
    let gen_decl = &gen_decl;
    let gen_use = &gen_use;
    let root_keys: HashSet<String> = root_generics.params.iter().map(param_key).collect();

    // The terminator struct must be generic over the root's params when the cycle is generic, so the
    // depth-default alias `type __RootDefault<S> = …Term<S>` actually *uses* every param (otherwise an
    // unused-param E0091 fires on the user's own definition — notably at `limit = 1`, where the depth
    // chain bottoms out directly at the terminator). When the cycle has no params the terminator stays
    // the byte-identical unit struct `pub struct RootTerm;`.
    let has_gen = !gen_decl.is_empty();
    // Self-type arguments for the terminator (`RootTerm<S, …>`), empty when non-generic.
    let term_args: TokenStream = if has_gen {
        quote!( < #(#gen_use),* > )
    } else {
        quote!()
    };
    // One PhantomData element per lifetime / type root param: lifetime `'a` -> `&'a ()`; type
    // `T` -> `T`. Const params are *omitted*: only lifetime and type params trigger the unused-
    // parameter error (E0392), so a const param can stay unused in `PhantomData` — which also frees
    // us from the `[(); N]` encoding that only works for `const N: usize` (now any const type, e.g.
    // `const C: char`, is supported).
    let phantom_elems: Vec<TokenStream> = root_generics
        .params
        .iter()
        .filter_map(|p| match p {
            GenericParam::Lifetime(l) => {
                let lt = &l.lifetime;
                Some(quote!(& #lt ()))
            }
            GenericParam::Type(t) => {
                let i = &t.ident;
                Some(quote!(#i))
            }
            GenericParam::Const(_) => None,
        })
        .collect();
    let term_decl: TokenStream = if has_gen {
        quote!( pub struct #term_ident < #(#gen_decl),* > ( ::core::marker::PhantomData<( #(#phantom_elems,)* )> ); )
    } else {
        quote!( pub struct #term_ident; )
    };
    for item in items {
        let (id, generics): (&Ident, &Generics) = match item {
            Item::Enum(e)
                if matches!(e.vis, Visibility::Public(_)) && scc.contains(&e.ident.to_string()) =>
            {
                (&e.ident, &e.generics)
            }
            Item::Struct(s)
                if matches!(s.vis, Visibility::Public(_)) && scc.contains(&s.ident.to_string()) =>
            {
                (&s.ident, &s.generics)
            }
            _ => continue,
        };
        // `param_key` encodes the *kind* alongside the name (`"type S"` / `"lifetime a"` /
        // `"const N: usize"`), so the kinds must match too: a root `type S` is not satisfied by a
        // child's `const S: usize` — they yield different keys and the root's key is reported missing.
        let have: HashSet<String> = generics.params.iter().map(param_key).collect();
        if let Some(missing) = root_keys.iter().find(|k| !have.contains(*k)) {
            abort!(
                id,
                "#[recurse]: cycle type `{}` is missing the recursion root `{}`'s generic parameter \
                 ({}); every cycle type must declare all of the root's parameters (it may add its \
                 own extras) so the depth machinery can thread them through",
                id,
                root_name,
                missing
            );
        }
    }

    // For root detection in the transformer: we treat the ROOT type as "replaced by __Rec"
    // and all other cycle types as "renamed + get __Rec appended".
    // The root type's direct references also become __Rec, so we add it to root_types set.
    let mut effective_roots = root_types.clone();
    effective_roots.insert(root_name.clone());
    // A visitor is only sound for a single effective root (else back-edges collapse ambiguously).
    let single_root = effective_roots.len() == 1;

    // Identity generic arguments per effective root, as use-form normalized token strings: a
    // back-edge to a root must repeat these verbatim (see `TransformCtx::root_ident_args`).
    let root_ident_args: HashMap<String, Vec<String>> = items
        .iter()
        .filter_map(|item| {
            let (id, generics): (&Ident, &Generics) = match item {
                Item::Enum(e)
                    if matches!(e.vis, Visibility::Public(_))
                        && effective_roots.contains(&e.ident.to_string()) =>
                {
                    (&e.ident, &e.generics)
                }
                Item::Struct(s)
                    if matches!(s.vis, Visibility::Public(_))
                        && effective_roots.contains(&s.ident.to_string()) =>
                {
                    (&s.ident, &s.generics)
                }
                _ => return None,
            };
            let (_, us) = generic_tokens(generics);
            Some((id.to_string(), us.iter().map(|t| t.to_string()).collect()))
        })
        .collect();

    // Depth parameters, one per root (canonical sorted order). A lone root keeps the legacy name
    // `__Rec`; several roots get one named `__Rec<Root>` each, so each self-edge stays an independent,
    // unambiguous depth dimension. Each defaults to that root's depth-chain alias `__<Root>Default`.
    let mut roots_sorted: Vec<String> = effective_roots.iter().cloned().collect();
    roots_sorted.sort();
    let rec_for_root: HashMap<String, Ident> = roots_sorted
        .iter()
        .map(|r| {
            let id = if single_root {
                Ident::new("__Rec", Span::call_site())
            } else {
                Ident::new(&format!("__Rec{r}"), Span::call_site())
            };
            (r.clone(), id)
        })
        .collect();
    let default_for_root: HashMap<String, Ident> = roots_sorted
        .iter()
        .map(|r| {
            (
                r.clone(),
                Ident::new(&format!("__{r}Default"), Span::call_site()),
            )
        })
        .collect();
    let rec_params: Vec<Ident> = roots_sorted.iter().map(|r| rec_for_root[r].clone()).collect();
    let rec_decls: Vec<TokenStream> = roots_sorted
        .iter()
        .map(|r| {
            let p = &rec_for_root[r];
            let d = &default_for_root[r];
            quote!(#p = #d<#(#gen_use),*>)
        })
        .collect();

    let ctx = TransformCtx {
        cycle_types: scc.clone(),
        root_types: effective_roots,
        internal_names: internal_names.clone(),
        rec_params,
        root_rec: rec_for_root.clone(),
        rec_decls,
        root_ident_args,
    };

    let tail = if single_root {
        // ── single root: the original depth machinery ───────────────────────────────────────────
        // Inner default: (recursion_depth - 1) levels of __ExprRec<P0, P1, …, depth_ty>. The public
        // Expr<…> alias adds one more layer so that matching Expr::Block { stmts } leaves
        // stmts: Vec<__StmtRec<…, __ExprDefault<…>>> which equals Vec<Stmt<…>>.
        let root_internal = &internal_names[&root_name];
        let mut depth_ty: TokenStream = quote!(#term_ident #term_args);
        for _ in 0..(recursion_depth - 1) {
            depth_ty = quote!(#root_internal<#(#gen_use,)* #depth_ty>);
        }

        let visitor_ts = if want_visit {
            let prefix = if multi_scc { Some(root_name.as_str()) } else { None };
            generate_recurse_visitor(
                items,
                scc,
                root_name.as_str(),
                &internal_names,
                &term_ident,
                &term_args,
                gen_decl,
                gen_use,
                &root_keys,
                prefix,
            )
        } else {
            quote!()
        };

        // Public alias per non-root cycle type, using *its own* generic params (a type may carry
        // extras beyond the root's): `pub type Stmt<S, T> = __StmtRec<S, T, __RootDefault<root>>`.
        let non_root_aliases: Vec<TokenStream> = items
            .iter()
            .filter_map(|item| {
                let (id, generics): (&Ident, &Generics) = match item {
                    Item::Enum(e)
                        if matches!(e.vis, Visibility::Public(_))
                            && scc.contains(&e.ident.to_string())
                            && e.ident.to_string() != root_name =>
                    {
                        (&e.ident, &e.generics)
                    }
                    Item::Struct(s)
                        if matches!(s.vis, Visibility::Public(_))
                            && scc.contains(&s.ident.to_string())
                            && s.ident.to_string() != root_name =>
                    {
                        (&s.ident, &s.generics)
                    }
                    _ => return None,
                };
                let internal = &internal_names[&id.to_string()];
                let (decl, us) = generic_tokens(generics);
                Some(quote! {
                    pub type #id<#(#decl),*> =
                        #internal<#(#us,)* #default_alias<#(#gen_use),*>>;
                })
            })
            .collect();

        quote! {
            #term_decl

            impl< #(#gen_decl,)* __Atom: ::syan::span::Spanned > ::syan::parse::Parse<__Atom> for #term_ident #term_args {
                type Error = ::syan::error::ParseError;
                fn parse(
                    _stream: impl ::syan::parse::IntoParseStream<Atom = __Atom>,
                ) -> ::core::result::Result<Self, Self::Error> {
                    Err(::syan::error::ParseError::new((), "recursion depth limit reached"))
                }
            }

            impl< #(#gen_decl,)* __Atom > ::syan::parse::Unparse<__Atom> for #term_ident #term_args {
                fn unparse<__E: ::syan::parse::unparse::Emitter<__Atom>>(
                    &self,
                    _sink: &mut __E,
                ) -> ::core::result::Result<(), __E::Error> {
                    ::core::panic!("recursion depth limit reached")
                }
            }

            type #default_alias<#(#gen_decl),*> = #depth_ty;
            pub type #root_ident<#(#gen_decl),*> =
                #root_internal<#(#gen_use,)* #default_alias<#(#gen_use),*>>;

            #(#non_root_aliases)*

            #visitor_ts
        }
    } else {
        // ── several self-referential roots in one cycle ─────────────────────────────────────────
        build_multiroot_tail(
            scc,
            items,
            &root_types,
            &roots_sorted,
            &internal_names,
            &default_for_root,
            &root_generics,
            gen_decl,
            gen_use,
            &root_keys,
            &term_args,
            recursion_depth,
            want_visit,
            multi_scc,
            type_refs,
            mod_ident,
        )
    };

    (ctx, tail)
}

/// Emit the tail (terminators, depth-chain aliases, public aliases, and — under `want_visit` — the
/// visitor) for a cycle with **several self-referential roots**. Each root keeps its own depth
/// dimension: every cycle type carries one depth param per root, a back-edge to root `X` is that
/// root's param, and the per-root depth chains are built mutually (level `k` of each root embeds
/// level `k-1` of *all* roots). The depth only decrements at a root back-edge, so every cycle in the
/// SCC must pass through a root — checked via `safegraph` (the SCC minus the roots must be acyclic).
#[allow(clippy::too_many_arguments)]
fn build_multiroot_tail(
    scc: &HashSet<String>,
    items: &[Item],
    root_types: &HashSet<String>,
    roots_sorted: &[String],
    internal_names: &HashMap<String, Ident>,
    default_for_root: &HashMap<String, Ident>,
    root_generics: &Generics,
    gen_decl: &[TokenStream],
    gen_use: &[TokenStream],
    root_keys: &HashSet<String>,
    term_args: &TokenStream,
    recursion_depth: usize,
    want_visit: bool,
    multi_scc: bool,
    type_refs: &HashMap<String, HashSet<String>>,
    mod_ident: &Ident,
) -> TokenStream {
    // The depth decrements only at a root (self-referential) back-edge, so every cycle in the SCC
    // must pass through a root. If the SCC with all roots removed is still cyclic, the generated
    // types would not terminate — reject cleanly rather than emit an infinitely-recursive type.
    if subgraph_is_cyclic(scc, root_types, type_refs) {
        abort!(
            mod_ident,
            "#[recurse]: this multi-root cycle has a sub-cycle running entirely through \
             non-self-referential types, so the depth recursion (which only decrements at a \
             self-referential type) would not terminate. Make one type on that sub-cycle directly \
             self-referential, or split it into its own `#[derive(Ast)]` type."
        );
    }

    // Every root must declare *exactly* the canonical generic params (the depth chain instantiates
    // each root as `__XRec<gen_use, depth args…>`, spelled with the shared params). A root carrying
    // extra params can't be placed in the chain. (Non-root cycle types may still carry extras.)
    for r in roots_sorted {
        let g = items.iter().find_map(|it| match it {
            Item::Enum(e) if &e.ident.to_string() == r => Some(&e.generics),
            Item::Struct(s) if &s.ident.to_string() == r => Some(&s.generics),
            _ => None,
        });
        if let Some(g) = g {
            let keys: HashSet<String> = g.params.iter().map(param_key).collect();
            if keys != *root_keys {
                abort!(
                    g.params,
                    "#[recurse]: in a multi-root cycle every self-referential root must declare \
                     exactly the same generic parameters; `{}` differs. (Non-root cycle types may \
                     carry extra parameters, but roots may not.)",
                    r
                );
            }
        }
    }

    let term_for_root: HashMap<String, Ident> = roots_sorted
        .iter()
        .map(|r| (r.clone(), Ident::new(&format!("{r}Term"), Span::call_site())))
        .collect();

    let has_gen = !gen_decl.is_empty();
    let phantom_elems: Vec<TokenStream> = root_generics
        .params
        .iter()
        .filter_map(|p| match p {
            GenericParam::Lifetime(l) => {
                let lt = &l.lifetime;
                Some(quote!(& #lt ()))
            }
            GenericParam::Type(t) => {
                let i = &t.ident;
                Some(quote!(#i))
            }
            GenericParam::Const(_) => None,
        })
        .collect();

    // One terminator per root (+ its Parse/Unparse impls).
    let term_items: Vec<TokenStream> = roots_sorted
        .iter()
        .map(|r| {
            let term = &term_for_root[r];
            let decl = if has_gen {
                quote!( pub struct #term < #(#gen_decl),* > ( ::core::marker::PhantomData<( #(#phantom_elems,)* )> ); )
            } else {
                quote!( pub struct #term; )
            };
            quote! {
                #decl
                impl< #(#gen_decl,)* __Atom: ::syan::span::Spanned > ::syan::parse::Parse<__Atom> for #term #term_args {
                    type Error = ::syan::error::ParseError;
                    fn parse(
                        _stream: impl ::syan::parse::IntoParseStream<Atom = __Atom>,
                    ) -> ::core::result::Result<Self, Self::Error> {
                        Err(::syan::error::ParseError::new((), "recursion depth limit reached"))
                    }
                }
                impl< #(#gen_decl,)* __Atom > ::syan::parse::Unparse<__Atom> for #term #term_args {
                    fn unparse<__E: ::syan::parse::unparse::Emitter<__Atom>>(
                        &self,
                        _sink: &mut __E,
                    ) -> ::core::result::Result<(), __E::Error> {
                        ::core::panic!("recursion depth limit reached")
                    }
                }
            }
        })
        .collect();

    // Mutual depth chain: level 0 is each root's terminator; level k of every root embeds level k-1
    // of *all* roots. After (recursion_depth - 1) steps, `level[r]` is `__<r>Default`'s body (the
    // public alias adds one final `__rRec` layer, mirroring the single-root case).
    let mut level: HashMap<String, TokenStream> = roots_sorted
        .iter()
        .map(|r| {
            let t = &term_for_root[r];
            (r.clone(), quote!(#t #term_args))
        })
        .collect();
    for _ in 0..(recursion_depth - 1) {
        let args: Vec<TokenStream> = roots_sorted.iter().map(|r| level[r].clone()).collect();
        level = roots_sorted
            .iter()
            .map(|r| {
                let internal = &internal_names[r];
                (r.clone(), quote!( #internal< #(#gen_use,)* #(#args,)* > ))
            })
            .collect();
    }
    let default_decls: Vec<TokenStream> = roots_sorted
        .iter()
        .map(|r| {
            let d = &default_for_root[r];
            let body = &level[r];
            quote!( type #d<#(#gen_decl),*> = #body; )
        })
        .collect();

    // Public alias for EVERY cycle type: `pub type X<own> = __XRec<own, __ADefault<gen>, …>` — its
    // own params, then one root-default per root (in canonical order).
    let default_args: Vec<TokenStream> = roots_sorted
        .iter()
        .map(|r| {
            let d = &default_for_root[r];
            quote!( #d<#(#gen_use),*> )
        })
        .collect();
    let aliases: Vec<TokenStream> = items
        .iter()
        .filter_map(|item| {
            let (id, generics): (&Ident, &Generics) = match item {
                Item::Enum(e)
                    if matches!(e.vis, Visibility::Public(_))
                        && scc.contains(&e.ident.to_string()) =>
                {
                    (&e.ident, &e.generics)
                }
                Item::Struct(s)
                    if matches!(s.vis, Visibility::Public(_))
                        && scc.contains(&s.ident.to_string()) =>
                {
                    (&s.ident, &s.generics)
                }
                _ => return None,
            };
            let internal = &internal_names[&id.to_string()];
            let (decl, us) = generic_tokens(generics);
            Some(quote! {
                pub type #id<#(#decl),*> =
                    #internal< #(#us,)* #(#default_args,)* >;
            })
        })
        .collect();

    let visitor_ts = if want_visit {
        let prefix = if multi_scc {
            Some(roots_sorted[0].as_str())
        } else {
            None
        };
        generate_multiroot_visitor(
            items,
            scc,
            roots_sorted,
            internal_names,
            &term_for_root,
            gen_decl,
            gen_use,
            root_keys,
            term_args,
            prefix,
        )
    } else {
        quote!()
    };

    quote! {
        #(#term_items)*
        #(#default_decls)*
        #(#aliases)*
        #visitor_ts
    }
}

pub fn recurse(attr: TokenStream1, input: TokenStream1) -> TokenStream1 {
    let args: RecurseArgs = match syn::parse(attr) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error().into(),
    };
    let recursion_depth = args.limit;

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
    // `find_cycle_types` lifts it into a `safegraph` graph for the one operation that needs graph
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

    // Build every cycle's `(transform ctx, tail tokens)` up front from the ORIGINAL items (the
    // visitor and aliases read the un-renamed defs), then rewrite the module's items by applying each
    // cycle's transform in turn. A transform only matches the types named in its own cycle, and once
    // a type is renamed to `__XxxRec` no later cycle's transform matches it — so the passes compose.
    // A field referencing another cycle's type is left as-is and resolves to that cycle's public
    // alias. `multi_scc` root-prefixes the visitor trait names so independent cycles don't collide.
    let multi_scc = sccs.len() > 1;
    let plans: Vec<(TransformCtx, TokenStream)> = sccs
        .iter()
        .map(|scc| {
            build_scc(
                scc,
                &items,
                &type_refs,
                &direct_type_refs,
                recursion_depth,
                args.visit,
                multi_scc,
                mod_ident,
            )
        })
        .collect();

    let mut items = items;
    for (ctx, _) in &plans {
        items = items
            .into_iter()
            .map(|item| transform_item(item, ctx))
            .collect();
    }
    let tails: Vec<TokenStream> = plans.into_iter().map(|(_, tail)| tail).collect();

    quote! {
        #(#mod_attrs)* #mod_vis mod #mod_ident {
            #(#items)*

            #(#tails)*
        }
    }
    .into()
}

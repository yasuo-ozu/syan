use crate::util::{peel, to_snake, Container};
use proc_macro::TokenStream as TokenStream1;
use proc_macro2::{Ident, Span, TokenStream};
use proc_macro_error::emit_warning;
use std::collections::{HashMap, HashSet};
use syn::{
    punctuated::Punctuated, AngleBracketedGenericArguments, Fields, FieldsNamed, FieldsUnnamed,
    FnArg, GenericArgument, GenericParam, Generics, ImplItem, Item, ItemMod, Path, PathArguments,
    PathSegment, ReturnType, Token, Type, TypeParam, TypePath, Visibility,
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
    rec_param: Ident,
    default_alias: Ident,
    /// All type parameters of the root type (in order), used for depth-aliases and defaults.
    root_type_params: Vec<Ident>,
    /// Original type-parameter count for each cycle type (before extra root params are added).
    cycle_orig_param_counts: HashMap<String, usize>,
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

fn can_reach(
    from: &str,
    target: &str,
    graph: &HashMap<String, HashSet<String>>,
    visited: &mut HashSet<String>,
) -> bool {
    for next in graph.get(from).into_iter().flatten() {
        if next == target {
            return true;
        }
        if visited.insert(next.clone()) && can_reach(next, target, graph, visited) {
            return true;
        }
    }
    false
}

fn find_cycle_types(graph: &HashMap<String, HashSet<String>>) -> HashSet<String> {
    graph
        .keys()
        .filter(|name| {
            let mut visited = HashSet::new();
            can_reach(name, name, graph, &mut visited)
        })
        .cloned()
        .collect()
}

fn transform_type(ty: &Type, ctx: &TransformCtx) -> Type {
    match ty {
        Type::Path(TypePath { qself: None, path }) => {
            if let Some(seg) = path.segments.first() {
                let name = seg.ident.to_string();
                if ctx.root_types.contains(&name) {
                    let p = &ctx.rec_param;
                    return syn::parse_quote!(#p);
                }
                if let Some(internal) = ctx.internal_names.get(&name) {
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
                    // Insert extra root type params (those beyond this type's original count).
                    let orig_count = ctx
                        .cycle_orig_param_counts
                        .get(&name)
                        .copied()
                        .unwrap_or(existing.len());
                    let rec = &ctx.rec_param;
                    let mut args: Punctuated<GenericArgument, Token![,]> =
                        existing.into_iter().collect();
                    for p in ctx.root_type_params.iter().skip(orig_count) {
                        args.push(syn::parse_quote!(#p));
                    }
                    args.push(syn::parse_quote!(#rec));
                    return Type::Path(TypePath {
                        qself: None,
                        path: Path {
                            leading_colon: None,
                            segments: {
                                let mut s = Punctuated::new();
                                s.push(PathSegment {
                                    ident: internal.clone(),
                                    arguments: PathArguments::AngleBracketed(
                                        AngleBracketedGenericArguments {
                                            colon2_token: None,
                                            lt_token: Default::default(),
                                            args,
                                            gt_token: Default::default(),
                                        },
                                    ),
                                });
                                s
                            },
                        },
                    });
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

fn all_type_params(generics: &Generics) -> Vec<Ident> {
    generics
        .params
        .iter()
        .filter_map(|p| {
            if let GenericParam::Type(TypeParam { ident, .. }) = p {
                Some(ident.clone())
            } else {
                None
            }
        })
        .collect()
}

fn transform_item(item: Item, ctx: &TransformCtx) -> Item {
    match item {
        Item::Enum(mut e)
            if matches!(e.vis, Visibility::Public(_))
                && ctx.cycle_types.contains(&e.ident.to_string()) =>
        {
            let orig_name = e.ident.to_string();
            let orig_count = ctx
                .cycle_orig_param_counts
                .get(&orig_name)
                .copied()
                .unwrap_or(0);
            e.ident = ctx.internal_names[&orig_name].clone();
            let rec = &ctx.rec_param;
            let default_alias = &ctx.default_alias;
            // Append extra root params (those not in this type's original generics).
            for p in ctx.root_type_params.iter().skip(orig_count) {
                e.generics.params.push(syn::parse_quote!(#p));
            }
            let root_params = &ctx.root_type_params;
            e.generics
                .params
                .push(syn::parse_quote!(#rec = #default_alias<#(#root_params),*>));
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
            let orig_count = ctx
                .cycle_orig_param_counts
                .get(&orig_name)
                .copied()
                .unwrap_or(0);
            s.ident = ctx.internal_names[&orig_name].clone();
            let rec = &ctx.rec_param;
            let default_alias = &ctx.default_alias;
            for p in ctx.root_type_params.iter().skip(orig_count) {
                s.generics.params.push(syn::parse_quote!(#p));
            }
            let root_params = &ctx.root_type_params;
            s.generics
                .params
                .push(syn::parse_quote!(#rec = #default_alias<#(#root_params),*>));
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
                let rec = &ctx.rec_param;
                let orig_count = ctx.cycle_orig_param_counts.get(&name).copied().unwrap_or(0);

                // Extra root params to thread into this impl block.
                let extra_root_params: Vec<&Ident> =
                    ctx.root_type_params.iter().skip(orig_count).collect();

                // Rename self_ty, add extra root params, and append __Rec type argument.
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
                                for p in &extra_root_params {
                                    ab.args.push(syn::parse_quote!(#p));
                                }
                                ab.args.push(syn::parse_quote!(#rec));
                            }
                            PathArguments::None => {
                                let mut args: Punctuated<GenericArgument, Token![,]> =
                                    Punctuated::new();
                                for p in &extra_root_params {
                                    args.push(syn::parse_quote!(#p));
                                }
                                args.push(syn::parse_quote!(#rec));
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

                // Add extra root params + __Rec to impl generics (no defaults — not allowed).
                for p in &extra_root_params {
                    impl_block.generics.params.push(syn::parse_quote!(#p));
                }
                impl_block.generics.params.push(syn::parse_quote!(#rec));

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

/// Dispatch one field of a cycle type: a back-edge to the root drives via `__R` (the depth param);
/// a cross-edge to another cycle type calls that type's visit method; anything else is a leaf
/// (`None`, caller binds `_`). `binding` is the destructured field (a `&Field`).
fn recurse_dispatch_field(
    ty: &Type,
    binding: &TokenStream,
    cycle_types: &std::collections::HashSet<String>,
    root_name: &str,
) -> Option<TokenStream> {
    let p = peel(ty, &std::collections::HashSet::new())?;
    let hs = p.head.to_string();
    let is_root = hs == root_name;
    if !is_root && !cycle_types.contains(&hs) {
        return None; // leaf
    }
    let stars: TokenStream = (0..=p.head_box).map(|_| quote!(*)).collect();
    Some(match p.container {
        Container::Direct => recurse_visit_one(is_root, &p.head, &stars, binding),
        Container::Seq => {
            let inner = recurse_visit_one(is_root, &p.head, &stars, &quote!(__x));
            quote!( for __x in #binding.iter() { #inner } )
        }
        Container::Opt => {
            let inner = recurse_visit_one(is_root, &p.head, &stars, &quote!(__x));
            quote!( if let ::core::option::Option::Some(__x) = #binding { #inner } )
        }
    })
}

/// Emit the visit of a single value `acc` (a `&Box^n<head>`): drive via `__R` for the root, or call
/// `v.visit_<head>` for a cross-edge; `stars` derefs through any `Box` to a `&head`.
fn recurse_visit_one(
    is_root: bool,
    head: &Ident,
    stars: &TokenStream,
    acc: &TokenStream,
) -> TokenStream {
    if is_root {
        quote!( __R::visit_rec(& #stars #acc, v); )
    } else {
        let m = Ident::new(&format!("visit_{}", to_snake(head)), Span::call_site());
        quote!( v.#m(& #stars #acc); )
    }
}

/// `(pattern, statements)` for a cycle type's fields, dispatching followed fields.
fn recurse_visit_fields(
    fields: &Fields,
    cycle_types: &std::collections::HashSet<String>,
    root_name: &str,
) -> (TokenStream, TokenStream) {
    match fields {
        Fields::Named(named) => {
            let mut binds = Vec::new();
            let mut stmts = Vec::new();
            for f in &named.named {
                let name = f.ident.clone().unwrap();
                if let Some(stmt) =
                    recurse_dispatch_field(&f.ty, &quote!(#name), cycle_types, root_name)
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
                    recurse_dispatch_field(&f.ty, &quote!(#b), cycle_types, root_name)
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
    root_name: &str,
) -> TokenStream {
    match orig {
        Item::Enum(e) => {
            let arms = e.variants.iter().map(|v| {
                let (pat, stmts) = recurse_visit_fields(&v.fields, cycle_types, root_name);
                let vid = &v.ident;
                quote!( #internal::#vid #pat => { #stmts } )
            });
            quote!( match i { #(#arms)* } )
        }
        Item::Struct(s) => {
            let (pat, stmts) = recurse_visit_fields(&s.fields, cycle_types, root_name);
            match &s.fields {
                Fields::Unit => quote!(),
                _ => quote!( let #internal #pat = i; #stmts ),
            }
        }
        _ => quote!(),
    }
}

/// Generate the depth-generic visitor for a single-root cycle.
fn generate_recurse_visitor(
    items: &[Item],
    cycle_types: &std::collections::HashSet<String>,
    root_name: &str,
    internal_names: &HashMap<String, Ident>,
    term_ident: &Ident,
    root_params: &[Ident],
) -> TokenStream {
    let root_internal = &internal_names[root_name];
    let visit_root = Ident::new(
        &format!("visit_{}", to_snake(&Ident::new(root_name, Span::call_site()))),
        Span::call_site(),
    );

    // Cycle types in declaration order, paired with their internal (renamed) idents.
    let cyc: Vec<(&Ident, &Ident, &Item)> = items
        .iter()
        .filter_map(|it| {
            let id = match it {
                Item::Enum(e) => &e.ident,
                Item::Struct(s) => &s.ident,
                _ => return None,
            };
            if !cycle_types.contains(&id.to_string()) {
                return None;
            }
            Some((id, &internal_names[&id.to_string()], it))
        })
        .collect();

    // Per cycle type: public node alias, the trait method, and the free drive fn.
    let node_aliases = cyc.iter().map(|(orig, internal, _)| {
        let node = Ident::new(&format!("{orig}Node"), Span::call_site());
        quote!(
            #[doc = "Depth-generic node type for the visitor (an alias of the internal recurse type)."]
            pub use #internal as #node;
        )
    });
    let methods = cyc.iter().map(|(orig, internal, _)| {
        let vm = Ident::new(&format!("visit_{}", to_snake(orig)), Span::call_site());
        quote! {
            fn #vm< __R: VisitRec< #(#root_params,)* Self > >(
                &mut self,
                i: & #internal < #(#root_params,)* __R >,
            ) where Self: ::core::marker::Sized {
                #vm(self, i)
            }
        }
    });
    let drives = cyc.iter().map(|(orig, internal, it)| {
        let vm = Ident::new(&format!("visit_{}", to_snake(orig)), Span::call_site());
        let body = recurse_visit_body(it, internal, cycle_types, root_name);
        quote! {
            pub fn #vm< #(#root_params,)* __V: Visit< #(#root_params),* >, __R: VisitRec< #(#root_params,)* __V > >(
                v: &mut __V,
                i: & #internal < #(#root_params,)* __R >,
            ) {
                #body
            }
        }
    });

    quote! {
        /// Dispatch trait turning the cycle's depth recursion into trait calls: implemented by the
        /// root's depth chain (drives the root visit) and by the terminator (no-op).
        pub trait VisitRec< #(#root_params,)* __V > {
            fn visit_rec(&self, v: &mut __V);
        }

        /// Depth-generic visitor over the `#[recurse]` cycle. Implement the `visit_*` methods
        /// (each generic over the remaining depth `__R`); call the free `visit_*` to descend.
        pub trait Visit< #(#root_params),* > {
            #(#methods)*
        }

        #(#drives)*

        #(#node_aliases)*

        impl< #(#root_params,)* __V: Visit< #(#root_params),* >, __R: VisitRec< #(#root_params,)* __V > >
            VisitRec< #(#root_params,)* __V > for #root_internal < #(#root_params,)* __R >
        {
            fn visit_rec(&self, v: &mut __V) {
                <__V as Visit< #(#root_params),* >>::#visit_root(v, self);
            }
        }
        impl< #(#root_params,)* __V: Visit< #(#root_params),* > >
            VisitRec< #(#root_params,)* __V > for #term_ident
        {
            fn visit_rec(&self, _v: &mut __V) {}
        }
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

    // Build reference graph
    let type_refs: HashMap<String, HashSet<String>> = items
        .iter()
        .filter_map(|item| match item {
            Item::Enum(e) if matches!(e.vis, Visibility::Public(_)) => {
                Some((e.ident.to_string(), collect_refs_item(item, &pub_types)))
            }
            Item::Struct(s) if matches!(s.vis, Visibility::Public(_)) => {
                Some((s.ident.to_string(), collect_refs_item(item, &pub_types)))
            }
            _ => None,
        })
        .collect();

    // Find all types in cycles using DFS back-edge detection
    let cycle_types = find_cycle_types(&type_refs);

    if cycle_types.is_empty() {
        return quote!(
            #(#mod_attrs)* #mod_vis mod #mod_ident { #(#items)* }
        )
        .into();
    }

    // Root types: those that directly reference themselves
    let root_types: HashSet<String> = cycle_types
        .iter()
        .filter(|name| {
            type_refs
                .get(*name)
                .map_or(false, |refs| refs.contains(*name))
        })
        .cloned()
        .collect();

    // Build direct-reference counts: how many cycle types reference each type as a bare field
    // (not wrapped in Vec, Box, etc.). This is the primary criterion for root selection.
    let direct_type_refs: HashMap<String, HashSet<String>> = items
        .iter()
        .filter_map(|item| match item {
            Item::Enum(e) if matches!(e.vis, Visibility::Public(_)) => Some((
                e.ident.to_string(),
                collect_direct_refs_item(item, &pub_types),
            )),
            Item::Struct(s) if matches!(s.vis, Visibility::Public(_)) => Some((
                s.ident.to_string(),
                collect_direct_refs_item(item, &pub_types),
            )),
            _ => None,
        })
        .collect();

    let mut direct_ref_counts: HashMap<&str, usize> = HashMap::new();
    for refs in direct_type_refs.values() {
        for r in refs {
            if cycle_types.contains(r) {
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
        for refs in type_refs.values() {
            for r in refs {
                if cycle_types.contains(r) {
                    *ref_counts.entry(r.as_str()).or_insert(0) += 1;
                }
            }
        }
        let mut candidates: Vec<&String> = cycle_types.iter().collect();
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
    let rec_param = Ident::new("__Rec", Span::call_site());

    // Internal (renamed) idents: "Expr" → "__ExprRec"
    let internal_names: HashMap<String, Ident> = cycle_types
        .iter()
        .map(|n| {
            (
                n.clone(),
                Ident::new(&format!("__{n}Rec"), Span::call_site()),
            )
        })
        .collect();

    // Warn if the first type parameter of a cycle type is not named `S` or `Span`.
    // The macro unconditionally uses the first param as the span type in depth aliases;
    // any other name likely means the assumption is wrong.
    for item in &items {
        let (type_ident, generics): (&Ident, &Generics) = match item {
            Item::Enum(e)
                if matches!(e.vis, Visibility::Public(_))
                    && cycle_types.contains(&e.ident.to_string()) =>
            {
                (&e.ident, &e.generics)
            }
            Item::Struct(s)
                if matches!(s.vis, Visibility::Public(_))
                    && cycle_types.contains(&s.ident.to_string()) =>
            {
                (&s.ident, &s.generics)
            }
            _ => continue,
        };
        let first_ty_param = generics.params.iter().find_map(|p| {
            if let GenericParam::Type(tp) = p {
                Some(&tp.ident)
            } else {
                None
            }
        });
        if let Some(param_ident) = first_ty_param {
            let name = param_ident.to_string();
            if name != "S" && name != "Span" {
                emit_warning!(
                    param_ident.span(),
                    "`#[recurse]` uses the first type parameter `{}` of `{}` as the span type \
                     for depth-alias generation; rename it to `S` or `Span` to make this \
                     explicit, or reorder generics so the span type comes first",
                    name,
                    type_ident
                );
            }
        }
    }

    // Collect original type-param counts for every cycle type (before any macro transforms).
    let cycle_orig_param_counts: HashMap<String, usize> = items
        .iter()
        .filter_map(|item| match item {
            Item::Enum(e)
                if matches!(e.vis, Visibility::Public(_))
                    && cycle_types.contains(&e.ident.to_string()) =>
            {
                Some((e.ident.to_string(), all_type_params(&e.generics).len()))
            }
            Item::Struct(s)
                if matches!(s.vis, Visibility::Public(_))
                    && cycle_types.contains(&s.ident.to_string()) =>
            {
                Some((s.ident.to_string(), all_type_params(&s.generics).len()))
            }
            _ => None,
        })
        .collect();

    // All type params of the root type — threaded through depth aliases and all cycle types.
    let root_type_params: Vec<Ident> = items
        .iter()
        .find_map(|item| match item {
            Item::Enum(e)
                if matches!(e.vis, Visibility::Public(_)) && e.ident.to_string() == root_name =>
            {
                Some(all_type_params(&e.generics))
            }
            Item::Struct(s)
                if matches!(s.vis, Visibility::Public(_)) && s.ident.to_string() == root_name =>
            {
                Some(all_type_params(&s.generics))
            }
            _ => None,
        })
        .unwrap_or_else(|| vec![Ident::new("S", Span::call_site())]);

    // For root detection in the transformer: we treat the ROOT type as "replaced by __Rec"
    // and all other cycle types as "renamed + get __Rec appended".
    // The root type's direct references also become __Rec, so we add it to root_types set.
    let mut effective_roots = root_types.clone();
    effective_roots.insert(root_name.clone());
    // A visitor is only sound for a single effective root (else back-edges collapse ambiguously).
    let single_root = effective_roots.len() == 1;

    let ctx = TransformCtx {
        cycle_types: cycle_types.clone(),
        root_types: effective_roots,
        internal_names: internal_names.clone(),
        rec_param: rec_param.clone(),
        default_alias: default_alias.clone(),
        root_type_params: root_type_params.clone(),
        cycle_orig_param_counts: cycle_orig_param_counts.clone(),
    };

    let root_params = &root_type_params;

    // Inner default: (recursion_depth - 1) levels of __ExprRec<P0, P1, …, depth_ty>.
    // The public Expr<…> alias adds one more layer so that matching Expr::Block { stmts }
    // leaves stmts: Vec<__StmtRec<…, __ExprDefault<…>>> which equals Vec<Stmt<…>>.
    let root_internal = &internal_names[&root_name];
    let mut depth_ty: TokenStream = quote!(#term_ident);
    for _ in 0..(recursion_depth - 1) {
        depth_ty = quote!(#root_internal<#(#root_params,)* #depth_ty>);
    }

    // `#[recurse(visit)]`: a depth-generic visitor over the cycle (single-root cycles only — with
    // multiple effective roots every back-edge collapses to one `__Rec`, which is ambiguous).
    let visitor_ts = if args.visit && single_root {
        generate_recurse_visitor(
            &items,
            &cycle_types,
            root_name.as_str(),
            &internal_names,
            &term_ident,
            &root_type_params,
        )
    } else {
        quote!()
    };

    quote! {
        #(#mod_attrs)* #mod_vis mod #mod_ident {
            #(for item in items.into_iter().map(|item| transform_item(item, &ctx))) { #item }

            pub struct #term_ident;

            impl<__Atom: ::syan::span::Spanned> ::syan::parse::Parse<__Atom> for #term_ident {
                type Error = ::syan::error::ParseError;
                fn parse(
                    _stream: impl ::syan::parse::IntoParseStream<Atom = __Atom>,
                ) -> ::core::result::Result<Self, Self::Error> {
                    Err(::syan::error::ParseError::new((), "recursion depth limit reached"))
                }
            }

            impl<__Atom> ::syan::parse::Unparse<__Atom> for #term_ident {
                fn unparse<__E: ::syan::parse::unparse::Emitter<__Atom>>(
                    &self,
                    _sink: &mut __E,
                ) -> ::core::result::Result<(), __E::Error> {
                    ::core::panic!("recursion depth limit reached")
                }
            }

            type #default_alias<#(#root_params),*> = #depth_ty;
            pub type #root_ident<#(#root_params),*> =
                #root_internal<#(#root_params,)* #default_alias<#(#root_params),*>>;

            #(for (orig_name, non_root_internal) in cycle_types.iter().filter(|n| *n != &root_name).map(|n| (n, &internal_names[n]))) {
                pub type #{Ident::new(orig_name, Span::call_site())}<#(#root_params),*> =
                    #non_root_internal<#(#root_params,)* #default_alias<#(#root_params),*>>;
            }

            #visitor_ts
        }
    }
    .into()
}

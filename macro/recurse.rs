use crate::util::{peel, to_snake, Container};
use proc_macro::TokenStream as TokenStream1;
use proc_macro2::{Ident, Span, TokenStream};
use proc_macro_error::{abort, emit_warning};
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
    rec_param: Ident,
    default_alias: Ident,
    /// The root's full generic params in *use* form (lifetimes / type idents / const idents), in
    /// declaration order — used to spell the `__Rec` default `__XxxDefault<'a, S, N>`. Every cycle
    /// type shares the root's params (checked by the caller), so the default is spellable in each.
    root_gen_use: Vec<TokenStream>,
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
                    // A cross-edge to another cycle type: keep its generic args as written (a
                    // back-edge to the root inside them becomes `__Rec`) and append the depth `__Rec`.
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
                    let rec = &ctx.rec_param;
                    return syn::parse_quote!( #internal < #(#existing,)* #rec > );
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
            let rec = &ctx.rec_param;
            let default_alias = &ctx.default_alias;
            // Keep this type's own params and append the depth `__Rec` (its default is the root's
            // depth chain, spelled with the root's params — which every cycle type shares).
            let root_gen_use = &ctx.root_gen_use;
            e.generics
                .params
                .push(syn::parse_quote!(#rec = #default_alias<#(#root_gen_use),*>));
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
            let rec = &ctx.rec_param;
            let default_alias = &ctx.default_alias;
            let root_gen_use = &ctx.root_gen_use;
            s.generics
                .params
                .push(syn::parse_quote!(#rec = #default_alias<#(#root_gen_use),*>));
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

                // Rename self_ty (keeping its own generic args) and append the __Rec type argument.
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
                                ab.args.push(syn::parse_quote!(#rec));
                            }
                            PathArguments::None => {
                                let mut args: Punctuated<GenericArgument, Token![,]> =
                                    Punctuated::new();
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

                // Add __Rec to the impl generics (no default — not allowed on an impl).
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

/// Dispatch one field of a cycle type: a back-edge to the root drives via `__R` (the depth param);
/// a cross-edge to another cycle type calls that type's visit method; anything else is a leaf
/// (`None`, caller binds `_`). `binding` is the destructured field (a `&Field`).
fn recurse_dispatch_field(
    ty: &Type,
    binding: &TokenStream,
    cycle_types: &std::collections::HashSet<String>,
    root_name: &str,
) -> Option<TokenStream> {
    // A tuple field: destructure it and dispatch each element (an element may itself be a cycle
    // ref, a container of one, or a nested tuple). Leaf elements bind `_`; if no element is
    // followed the whole tuple is a leaf.
    if let Some(elems) = as_tuple(ty) {
        let mut pats = Vec::new();
        let mut stmts = Vec::new();
        for (i, elem) in elems.iter().enumerate() {
            let bi = Ident::new(&format!("__t{i}"), Span::call_site());
            if let Some(stmt) = recurse_dispatch_field(elem, &quote!(#bi), cycle_types, root_name) {
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
    let is_root = hs == root_name;
    if !is_root && !cycle_types.contains(&hs) {
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
        Container::Direct => recurse_visit_one(is_root, &p.head_lead, &stars, binding),
        Container::Seq => {
            // `.iter()` auto-derefs through any `Box` around the sequence (`cont_box`).
            let inner = recurse_visit_one(is_root, &p.head_lead, &stars, &quote!(__x));
            quote!( for __x in #binding.iter() { #inner } )
        }
        Container::Opt => {
            // Patterns do not auto-deref `Box`, so deref through any `Box` around the `Option`
            // (`cont_box`) before matching; the leading `*` also derefs the `&Field` binding.
            let cont_stars: TokenStream = (0..=p.cont_box).map(|_| quote!(*)).collect();
            let inner = recurse_visit_one(is_root, &p.head_lead, &stars, &quote!(__x));
            quote!( if let ::core::option::Option::Some(__x) = & #cont_stars #binding { #inner } )
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

/// Generate the depth-generic visitor for a single-root cycle. `gen_decl` / `gen_use` are the ROOT's
/// generic params in declaration / use form (they coincide except for const params) — the `Visit` /
/// `VisitRec` traits are keyed on them. A cycle type may carry params beyond the root's: those become
/// generics on its `visit_*` method (`extra_decl`), and its node type is spelled with its *own* full
/// params (`own_use`). `root_keys` identifies which of a type's params are the root's (shared).
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
) -> TokenStream {
    let root_internal = &internal_names[root_name];
    let visit_root = Ident::new(
        &format!("visit_{}", to_snake(&Ident::new(root_name, Span::call_site()))),
        Span::call_site(),
    );

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
                body: recurse_visit_body(it, &internal, cycle_types, root_name),
                internal,
                own_use,
                extra_decl,
            })
        })
        .collect();

    quote! {
        /// Dispatch trait turning the cycle's depth recursion into trait calls: implemented by the
        /// root's depth chain (drives the root visit) and by the terminator (no-op).
        pub trait VisitRec< #(#gen_decl,)* __V > {
            fn visit_rec(&self, v: &mut __V);
        }

        /// Depth-generic visitor over the `#[recurse]` cycle. Implement the `visit_*` methods
        /// (each generic over the remaining depth `__R`); call the free `visit_*` to descend.
        pub trait Visit< #(#gen_decl),* > {
            #(for info in &infos) {
                fn #{&info.vm}< #(for e in &info.extra_decl) { #e, } __R: VisitRec< #(#gen_use,)* Self > >(
                    &mut self,
                    i: & #{&info.internal} < #(for u in &info.own_use) { #u, } __R >,
                ) where Self: ::core::marker::Sized {
                    #{&info.vm}(self, i)
                }
            }
        }

        #(for info in &infos) {
            pub fn #{&info.vm}< #(#gen_decl,)* #(for e in &info.extra_decl) { #e, } __V: Visit< #(#gen_use),* >, __R: VisitRec< #(#gen_use,)* __V > >(
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

        impl< #(#gen_decl,)* __V: Visit< #(#gen_use),* >, __R: VisitRec< #(#gen_use,)* __V > >
            VisitRec< #(#gen_use,)* __V > for #root_internal < #(#gen_use,)* __R >
        {
            fn visit_rec(&self, v: &mut __V) {
                <__V as Visit< #(#gen_use),* >>::#visit_root(v, self);
            }
        }
        impl< #(#gen_decl,)* __V: Visit< #(#gen_use),* > >
            VisitRec< #(#gen_use,)* __V > for #term_ident #term_args
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
    // One PhantomData element per root param: lifetime `'a` -> `&'a ()`; type `T` -> `T`;
    // const `N` -> `[(); N]`.
    let phantom_elems: Vec<TokenStream> = root_generics
        .params
        .iter()
        .map(|p| match p {
            GenericParam::Lifetime(l) => {
                let lt = &l.lifetime;
                quote!(& #lt ())
            }
            GenericParam::Type(t) => {
                let i = &t.ident;
                quote!(#i)
            }
            GenericParam::Const(c) => {
                let i = &c.ident;
                quote!([(); #i])
            }
        })
        .collect();
    let term_decl: TokenStream = if has_gen {
        quote!( pub struct #term_ident < #(#gen_decl),* > ( ::core::marker::PhantomData<( #(#phantom_elems,)* )> ); )
    } else {
        quote!( pub struct #term_ident; )
    };
    for item in &items {
        let (id, generics): (&Ident, &Generics) = match item {
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

    let ctx = TransformCtx {
        cycle_types: cycle_types.clone(),
        root_types: effective_roots,
        internal_names: internal_names.clone(),
        rec_param: rec_param.clone(),
        default_alias: default_alias.clone(),
        root_gen_use: gen_use.clone(),
    };

    // Inner default: (recursion_depth - 1) levels of __ExprRec<P0, P1, …, depth_ty>.
    // The public Expr<…> alias adds one more layer so that matching Expr::Block { stmts }
    // leaves stmts: Vec<__StmtRec<…, __ExprDefault<…>>> which equals Vec<Stmt<…>>.
    let root_internal = &internal_names[&root_name];
    let mut depth_ty: TokenStream = quote!(#term_ident #term_args);
    for _ in 0..(recursion_depth - 1) {
        depth_ty = quote!(#root_internal<#(#gen_use,)* #depth_ty>);
    }

    // `#[recurse(visit)]`: a depth-generic visitor over the cycle. Single-root cycles only — with
    // multiple effective roots every back-edge collapses to one ambiguous `__Rec`. If the user asked
    // for `visit` on such a cycle, say so clearly instead of silently emitting no visitor.
    let visitor_ts = if args.visit {
        if !single_root {
            let mut roots: Vec<String> = root_types.iter().cloned().collect();
            roots.sort();
            abort!(
                mod_ident,
                "#[recurse(visit)] does not support multi-root cycles (found {} self-referential \
                 cycle types: {}); it generates a depth-generic visitor for a single recursion root",
                roots.len(),
                roots.join(", ")
            );
        }
        generate_recurse_visitor(
            &items,
            &cycle_types,
            root_name.as_str(),
            &internal_names,
            &term_ident,
            &term_args,
            gen_decl,
            gen_use,
            &root_keys,
        )
    } else {
        quote!()
    };

    // Public alias per non-root cycle type, using *its own* generic params (a type may carry extras
    // beyond the root's): `pub type Stmt<S, T> = __StmtRec<S, T, __RootDefault<root params>>`.
    let non_root_aliases: Vec<TokenStream> = items
        .iter()
        .filter_map(|item| {
            let (id, generics): (&Ident, &Generics) = match item {
                Item::Enum(e)
                    if matches!(e.vis, Visibility::Public(_))
                        && cycle_types.contains(&e.ident.to_string())
                        && e.ident.to_string() != root_name =>
                {
                    (&e.ident, &e.generics)
                }
                Item::Struct(s)
                    if matches!(s.vis, Visibility::Public(_))
                        && cycle_types.contains(&s.ident.to_string())
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
        #(#mod_attrs)* #mod_vis mod #mod_ident {
            #(for item in items.into_iter().map(|item| transform_item(item, &ctx))) { #item }

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
    }
    .into()
}

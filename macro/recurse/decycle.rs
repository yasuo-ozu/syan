//! Feature `recurse-decycle`: the cycle-mode `Parse` generator that replaces the internal depth-engine
//! `Parse` with the `decycle` crate's ranked trait-cycle breaking, for the SCC shapes decycle can
//! express. See `docs/recurse-via-decycle-plan.md`.
//!
//! ## What migrates, and why not everything
//!
//! A cycle type's `Parse` is redirected to a per-SCC `#[decycle] trait __ParseDyn_<…>` with a
//! **method-generic** atom (`fn parse_dyn<A: Spanned + Clone, E>(&mut dyn ParseStream<Atom = A,
//! Error = E>)`). Recursive cross-edges are surfaced as bare `Head: __ParseDyn` bounds (decycle ranks
//! them) and dispatched inline to `Head::parse_dyn` (so the backtracking `Dup` stream is re-erased at
//! each recursion boundary and never grows). The whole `#[recurse]` module is wrapped in
//! `#[decycle(recurse_level = N)]`.
//!
//! Two shapes are **structurally out of reach** and stay on the engine (validated in
//! `scratchpad/migrate-phase1`):
//! - **A trait-level atom/span param panics at the decycle re-entry floor** (the "generic-floor
//!   residual"), so the atom must be a *method* generic. But then a leaf whose `Parse` ties the atom's
//!   span to the type param (`WithSpan<_, S>: Parse<A>` ⟹ `A: Spanned<Span = S>`) can't be expressed
//!   (the bound names both a method generic `A` and the impl param `S`; on the shared trait method it's
//!   stricter, on the impl it can't name `A`, and a trait associated `type Sp` fails to normalize
//!   through decycle's rank rewrite). So a cycle with a **span/param-tying leaf** stays on the engine.
//! - **A `#[group]` cross-edge** parses through `<…>::Fill<Substruct>: Parse`, whose obligation routes
//!   back to the cycle via the `Parse` facade — an *indirect* where-cycle decycle can't see or break
//!   (the same wall as the Phase-2 group-ful Unparse NO-GO). So a **group-ful** cycle stays on the
//!   engine.
//!
//! A cycle is therefore **decycle-able** iff it is group-free, every recursive edge is a direct head or
//! `Box`-wrapped head, and every leaf is either S-free or `PhantomData` (no span/param-tying leaf).

use super::*;
use crate::attribute::{Adt, FindAttribute};
use crate::util::first_ty_arg;
use syn::{Data, DataEnum, DataStruct, TypeParamBound, WherePredicate};

/// How one field of a cycle type is lowered in the decycle `parse_dyn` body.
enum FieldKind {
    /// A leaf whose `Parse<A>` bound is S-free (`Integer` etc.) — carried as a `T: Parse<A>` method
    /// bound (the `Type` is the leaf type).
    LeafSFree(Type),
    /// A `PhantomData<…>` leaf — `Parse<A>` holds unconditionally, so no bound is carried.
    LeafPhantom,
    /// A `#[default]` field — `Default::default()`, no bound, no parse.
    LeafDefault,
    /// A recursive head reached through 0+ `Box` layers (`Expr<S>`, `Box<Stmt<S>>`, …): the `Type` is
    /// the (fully-argumented) head (the `Box` depth is recomputed by `recur` at lowering time).
    Recursive(Type),
    /// A leaf whose `Parse<A>` would tie the atom's span to a type param (`WithSpan<_,S>`,
    /// `GroupBrace<(),S>`, …) — NOT expressible with a method-generic atom.
    LeafParamTying,
    /// A recursive edge behind an un-inlineable container (`Vec<Expr>`, `Option<Box<Expr>>`, …) or a
    /// `#[group]` field.
    Unsupported,
}

/// Collect every single-segment path ident appearing anywhere in `ty` (nested through generic args).
fn path_idents(ty: &Type, out: &mut Vec<String>) {
    match ty {
        Type::Path(TypePath { path, qself }) => {
            if let Some(q) = qself {
                path_idents(&q.ty, out);
            }
            for seg in &path.segments {
                out.push(seg.ident.to_string());
                if let PathArguments::AngleBracketed(ab) = &seg.arguments {
                    for a in &ab.args {
                        if let GenericArgument::Type(t) = a {
                            path_idents(t, out);
                        }
                    }
                }
            }
        }
        Type::Reference(r) => path_idents(&r.elem, out),
        Type::Group(g) => path_idents(&g.elem, out),
        Type::Paren(p) => path_idents(&p.elem, out),
        Type::Slice(s) => path_idents(&s.elem, out),
        Type::Array(a) => path_idents(&a.elem, out),
        Type::Tuple(t) => t.elems.iter().for_each(|e| path_idents(e, out)),
        _ => {}
    }
}

/// `true` iff `ty` mentions any of `names` (the containing type's generic params) — a proxy for
/// "this leaf's `Parse` might tie the atom's span to the type param".
fn mentions_any(ty: &Type, names: &HashSet<String>) -> bool {
    let mut ids = Vec::new();
    path_idents(ty, &mut ids);
    ids.iter().any(|i| names.contains(i))
}

/// If `ty` reaches a single-segment SCC head through 0+ `Box<…>` layers, return `(head type, box
/// depth)`. A single-segment ident is required (a foreign `other::Expr` sharing the last segment is a
/// leaf — mirrors `conv_expr`/`collect_refs` first-segment keying).
fn box_chain(ty: &Type, scc: &HashSet<String>) -> Option<(Type, usize)> {
    if let Type::Path(TypePath { qself: None, path }) = ty {
        let seg = path.segments.last()?;
        let name = seg.ident.to_string();
        if path.segments.len() == 1 && scc.contains(&name) {
            return Some((ty.clone(), 0));
        }
        if name == "Box" {
            let (h, d) = box_chain(first_ty_arg(seg)?, scc)?;
            return Some((h, d + 1));
        }
    }
    None
}

/// `true` iff `ty` contains a single-segment SCC head anywhere (nested) — a recursive edge, whether or
/// not it is inlineable.
fn contains_recursive(ty: &Type, scc: &HashSet<String>) -> bool {
    match ty {
        Type::Path(TypePath { qself, path }) => {
            if qself.as_ref().is_some_and(|q| contains_recursive(&q.ty, scc)) {
                return true;
            }
            if path.segments.len() == 1
                && path.segments.last().is_some_and(|s| scc.contains(&s.ident.to_string()))
            {
                return true;
            }
            path.segments.iter().any(|seg| {
                matches!(&seg.arguments, PathArguments::AngleBracketed(ab)
                    if ab.args.iter().any(|a| matches!(a, GenericArgument::Type(t) if contains_recursive(t, scc))))
            })
        }
        Type::Reference(r) => contains_recursive(&r.elem, scc),
        Type::Group(g) => contains_recursive(&g.elem, scc),
        Type::Paren(p) => contains_recursive(&p.elem, scc),
        Type::Slice(s) => contains_recursive(&s.elem, scc),
        Type::Array(a) => contains_recursive(&a.elem, scc),
        Type::Tuple(t) => t.elems.iter().any(|e| contains_recursive(e, scc)),
        _ => false,
    }
}

/// Classify one field for the decycle path.
fn classify_field(field: &Field, scc: &HashSet<String>, params: &HashSet<String>) -> FieldKind {
    if field.attrs.iter().any(|a| a.path().is_ident("group")) {
        return FieldKind::Unsupported;
    }
    if field.has_default() {
        return FieldKind::LeafDefault;
    }
    if let Some((head, depth)) = box_chain(&field.ty, scc) {
        let _ = depth;
        return FieldKind::Recursive(head);
    }
    if contains_recursive(&field.ty, scc) {
        return FieldKind::Unsupported;
    }
    // A leaf.
    if let Type::Path(TypePath { path, .. }) = &field.ty {
        if path.segments.last().is_some_and(|s| s.ident == "PhantomData") {
            return FieldKind::LeafPhantom;
        }
    }
    if mentions_any(&field.ty, params) {
        return FieldKind::LeafParamTying;
    }
    FieldKind::LeafSFree(field.ty.clone())
}

/// `true` iff any path anywhere in `ty` (nested) begins with a relative `super`/`self` segment
/// (no leading `::`). decycle nests the ranked `__ParseDyn` impls in a `shadowing_module`, so a
/// user-written relative path threaded into them shifts and fails to resolve — such a cycle stays on
/// the engine. (`crate::`-rooted and absolute `::`-rooted paths are stable and fine.)
fn ty_has_relative_super(ty: &Type) -> bool {
    fn path_relative(path: &Path) -> bool {
        path.leading_colon.is_none()
            && path
                .segments
                .first()
                .is_some_and(|s| s.ident == "super" || s.ident == "self")
    }
    match ty {
        Type::Path(TypePath { qself, path }) => {
            if qself.as_ref().is_some_and(|q| ty_has_relative_super(&q.ty)) {
                return true;
            }
            if path_relative(path) {
                return true;
            }
            path.segments.iter().any(|seg| {
                matches!(&seg.arguments, PathArguments::AngleBracketed(ab)
                    if ab.args.iter().any(|a| matches!(a, GenericArgument::Type(t) if ty_has_relative_super(t))))
            })
        }
        Type::Reference(r) => ty_has_relative_super(&r.elem),
        Type::Group(g) => ty_has_relative_super(&g.elem),
        Type::Paren(p) => ty_has_relative_super(&p.elem),
        Type::Slice(s) => ty_has_relative_super(&s.elem),
        Type::Array(a) => ty_has_relative_super(&a.elem),
        Type::Tuple(t) => t.elems.iter().any(ty_has_relative_super),
        _ => false,
    }
}

/// `true` iff the item's `where`-clause names a relative `super`/`self` path (bounded type or bound
/// trait) — which decycle's module nesting would break. Such a cycle stays on the engine.
fn where_has_relative_super(generics: &Generics) -> bool {
    let Some(wc) = &generics.where_clause else {
        return false;
    };
    wc.predicates.iter().any(|p| match p {
        WherePredicate::Type(pt) => {
            ty_has_relative_super(&pt.bounded_ty)
                || pt.bounds.iter().any(|b| match b {
                    TypeParamBound::Trait(tb) => {
                        tb.path.leading_colon.is_none()
                            && tb
                                .path
                                .segments
                                .first()
                                .is_some_and(|s| s.ident == "super" || s.ident == "self")
                    }
                    _ => false,
                })
        }
        _ => false,
    })
}

/// The generic type/const param names of an item (lifetimes excluded — they never tie the atom span).
fn type_param_names(generics: &Generics) -> HashSet<String> {
    generics
        .params
        .iter()
        .filter_map(|p| match p {
            GenericParam::Type(t) => Some(t.ident.to_string()),
            GenericParam::Const(c) => Some(c.ident.to_string()),
            GenericParam::Lifetime(_) => None,
        })
        .collect()
}

fn item_fields(item: &Item) -> Vec<&Field> {
    match item {
        Item::Enum(e) => e.variants.iter().flat_map(|v| v.fields.iter()).collect(),
        Item::Struct(s) => s.fields.iter().collect(),
        _ => Vec::new(),
    }
}

/// The `#[decycle] trait` name for an SCC (keyed on its alphabetically-first member + the nonce).
fn parse_dyn_trait_name(scc: &HashSet<String>, nonce: u64) -> Ident {
    let mut names: Vec<&String> = scc.iter().collect();
    names.sort();
    Ident::new(&format!("__ParseDyn_{}_{nonce}", names[0]), Span::call_site())
}

/// Whether an SCC (index `i`) can host the decycle cycle-mode `Parse`: it derives `Parse`, is
/// group-free, and every member's every field classifies as a supported leaf / direct-or-`Box`
/// recursive edge (no span/param-tying leaf, no un-inlineable container). `deleg_parse` is the set of
/// `Parse`-deriving type names.
pub(crate) fn scc_is_decycleable(
    scc: &HashSet<String>,
    items: &[Item],
    deleg_parse: &HashSet<String>,
    group_ful: bool,
) -> bool {
    if group_ful {
        return false;
    }
    let derives_parse = scc.iter().any(|n| deleg_parse.contains(n));
    if !derives_parse {
        return false;
    }
    for item in items {
        let (ident, generics) = match item {
            Item::Enum(e) if scc.contains(&e.ident.to_string()) => (&e.ident, &e.generics),
            Item::Struct(s) if scc.contains(&s.ident.to_string()) => (&s.ident, &s.generics),
            _ => continue,
        };
        let _ = ident;
        // A relative `super`/`self` where-clause path breaks under decycle's module nesting.
        if where_has_relative_super(generics) {
            return false;
        }
        let params = type_param_names(generics);
        for field in item_fields(item) {
            match classify_field(field, scc, &params) {
                FieldKind::LeafSFree(_)
                | FieldKind::LeafPhantom
                | FieldKind::LeafDefault
                | FieldKind::Recursive(_) => {}
                FieldKind::LeafParamTying | FieldKind::Unsupported => return false,
            }
        }
    }
    true
}

/// Emit the decycle module content for ONE decycle-able SCC: the `#[decycle] trait __ParseDyn_<…>`
/// plus, per cycle type, the `impl __ParseDyn for X` (`parse_dyn` body via the derive skeleton) and the
/// public `impl Parse<A> for X` facade. Returns the tokens; the caller splices them into the
/// `#[decycle]`-wrapped module.
pub(crate) fn emit_scc(scc: &HashSet<String>, items: &[Item], nonce: u64) -> TokenStream {
    let syan: Path = syn::parse_quote!(::syan);
    let trait_name = parse_dyn_trait_name(scc, nonce);
    let tp_atom: Ident = syn::parse_quote!(__SyanMacro_Atom);
    let tp_err: Ident = syn::parse_quote!(__SyanMacro_Err);

    // The SCC-wide union of S-free leaf types (deduped by rendered text) → `T: Parse<A>` on the shared
    // trait method + every impl method + facade.
    let mut seen = HashSet::new();
    let mut method_leaf_bounds: Vec<Type> = Vec::new();
    for item in items {
        let generics = match item {
            Item::Enum(e) if scc.contains(&e.ident.to_string()) => &e.generics,
            Item::Struct(s) if scc.contains(&s.ident.to_string()) => &s.generics,
            _ => continue,
        };
        let params = type_param_names(generics);
        for field in item_fields(item) {
            if let FieldKind::LeafSFree(ty) = classify_field(field, scc, &params) {
                if seen.insert(quote!(#ty).to_string()) {
                    method_leaf_bounds.push(ty);
                }
            }
        }
    }

    // The `#[decycle]` trait: a non-generic trait, method-generic atom + error, S-free leaf method
    // bounds. (Method-generic atom is mandatory — a trait-level atom panics at the re-entry floor.)
    // The inner trait carries a BARE `#[decycle]` marker — `process_module` detects it textually (by
    // ident) and strips it before re-emitting; it never resolves as a macro (only the module-level
    // `#[::syan::__decycle::decycle(…)]` does). A qualified path here is NOT recognized.
    let mut out = quote! {
        #[decycle]
        trait #trait_name: ::core::marker::Sized {
            fn parse_dyn<#tp_atom: #syan::span::Spanned + ::core::clone::Clone, #tp_err>(
                __syan_stream: &mut (dyn #syan::parse::parse_stream::ParseStream<Atom = #tp_atom, Error = #tp_err> + '_),
            ) -> ::core::result::Result<Self, #syan::error::ParseError>
            #(if !method_leaf_bounds.is_empty()) {
                where #(#method_leaf_bounds: #syan::parse::parse::Parse<#tp_atom>,)*
            } ;
        }
    };

    // Per cycle type: the ranked `impl __ParseDyn` + the `Parse` facade.
    for item in items {
        let (ident, generics, data) = match item {
            Item::Enum(e) if scc.contains(&e.ident.to_string()) => (
                &e.ident,
                &e.generics,
                Data::Enum(DataEnum {
                    enum_token: e.enum_token,
                    brace_token: e.brace_token,
                    variants: e.variants.clone(),
                }),
            ),
            Item::Struct(s) if scc.contains(&s.ident.to_string()) => (
                &s.ident,
                &s.generics,
                Data::Struct(DataStruct {
                    struct_token: s.struct_token,
                    fields: s.fields.clone(),
                    semi_token: s.semi_token,
                }),
            ),
            _ => continue,
        };
        let params = type_param_names(generics);
        // This type's recursive-head types (deduped) → `H: __ParseDyn` bounds on the impl.
        let mut hseen = HashSet::new();
        let mut cyclic_heads: Vec<Type> = Vec::new();
        for field in item_fields(item) {
            if let FieldKind::Recursive(head) = classify_field(field, scc, &params) {
                if hseen.insert(quote!(#head).to_string()) {
                    cyclic_heads.push(head);
                }
            }
        }
        // The inline recursive-field parser: dispatch to `<Head as __ParseDyn>::parse_dyn`, wrapped in
        // `Box::new` once per `Box` layer.
        let scc = scc.clone();
        let trait_name2 = trait_name.clone();
        let recur = move |ty: &Type, stream: &TokenStream| -> Option<TokenStream> {
            let (head, depth) = box_chain(ty, &scc)?;
            let mut call = quote! {
                <#head as #trait_name2>::parse_dyn::<__SyanMacro_Atom, _>(&mut #stream)?
            };
            for _ in 0..depth {
                call = quote!( ::std::boxed::Box::new(#call) );
            }
            Some(call)
        };

        let body = match &data {
            Data::Enum(e) => e.extract_parse_dyn(&syan, ident, generics, &trait_name, &method_leaf_bounds, &cyclic_heads, &recur),
            Data::Struct(s) => s.extract_parse_dyn(&syan, ident, generics, &trait_name, &method_leaf_bounds, &cyclic_heads, &recur),
            _ => quote!(),
        };
        out.extend(body);
    }
    out
}

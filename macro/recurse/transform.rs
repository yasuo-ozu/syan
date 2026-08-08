use super::*;

pub(crate) struct TransformCtx {
    pub(crate) cycle_types: HashSet<String>,
    pub(crate) root_types: HashSet<String>,
    pub(crate) internal_names: HashMap<String, Ident>,
    /// Depth parameters, **one per root** of this cycle, in canonical (sorted-root) order: `[__Rec]`
    /// for a single root, `[__RecA, __RecB, …]` for several. Appended in this order to every renamed
    /// cycle type and threaded (all of them) through every cross-edge.
    pub(crate) rec_params: Vec<Ident>,
    /// Root type name → its own depth parameter. A back-edge to root `X` collapses to `root_rec[X]`
    /// (so with several roots each self-edge keeps its own depth dimension, unambiguously).
    pub(crate) root_rec: HashMap<String, Ident>,
    /// The depth parameters as generic-param **declarations with defaults**, appended to a renamed
    /// cycle type: `[__Rec = __XDefault<S, …>]` (single) or one per root (`__RecA = __ADefault<S>`, …).
    pub(crate) rec_decls: Vec<TokenStream>,
    /// Per root type, its own declared generic params as *use*-form normalized token strings (e.g.
    /// `["'a", "S", "N"]`). A back-edge to a root collapses to its depth param, so its generic
    /// arguments must be the *identity* (the root's own params, unchanged) — there is nowhere to
    /// thread a different param like `Expr<Vec<S>>`. `transform_type` checks a root reference's args
    /// against this and aborts on a mismatch instead of silently dropping the param.
    pub(crate) root_ident_args: HashMap<String, Vec<String>>,
}

pub(crate) fn transform_type(ty: &Type, ctx: &TransformCtx) -> Type {
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

pub(crate) fn transform_fields(fields: &mut Fields, ctx: &TransformCtx) {
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

/// A stable identity for a generic param (kind + name + const type), used to compare a cycle type's
/// params against the root's.
pub(crate) fn param_key(p: &GenericParam) -> String {
    match p {
        GenericParam::Lifetime(lt) => format!("lifetime {}", lt.lifetime.ident),
        GenericParam::Type(t) => format!("type {}", t.ident),
        GenericParam::Const(c) => {
            let ty = &c.ty;
            format!("const {}: {}", c.ident, quote!(#ty))
        }
    }
}

/// A type's generic params as `(declaration, use)` token lists, in declaration order. Used both for
/// the root (the depth chain) and per cycle type (its own alias / node type), so a recurse cycle may
/// carry lifetimes / type params / const params alongside the depth `__Rec`.
pub(crate) fn generic_tokens(generics: &Generics) -> (Vec<TokenStream>, Vec<TokenStream>) {
    let mut decl = Vec::new();
    let mut us = Vec::new();
    for p in &generics.params {
        let (d, u) = param_tokens(p);
        decl.push(d);
        us.push(u);
    }
    (decl, us)
}

/// A generic param list for an `impl`/trait header that **preserves bounds** (`S: Span`, `const N:
/// usize`, …) — unlike `generic_tokens`, which strips them. Used by the engine→natural conversion +
/// delegation impls so they can name a cycle type carrying a bounded param (e.g. `Spanned`'s
/// `Expr<S: Span>`). A cycle type's own params never carry a default, so none is emitted here.
pub(crate) fn param_decls(generics: &Generics) -> Vec<TokenStream> {
    generics
        .params
        .iter()
        .map(|p| {
            // Drop any default (`= …`) — valid only in a type def, not an impl/trait header.
            let mut p = p.clone();
            match &mut p {
                GenericParam::Type(t) => {
                    t.eq_token = None;
                    t.default = None;
                }
                GenericParam::Const(c) => {
                    c.eq_token = None;
                    c.default = None;
                }
                _ => {}
            }
            quote!(#p)
        })
        .collect()
}

pub(crate) fn transform_item(item: Item, ctx: &TransformCtx) -> Item {
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
        // Inherent impl block whose Self type is a cycle type.
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

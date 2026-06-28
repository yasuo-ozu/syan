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

/// Default recursion depth when no `limit` argument is given.
const DEFAULT_RECURSION_DEPTH: usize = 4;

struct RecurseArgs {
    limit: usize,
}

impl syn::parse::Parse for RecurseArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut limit = DEFAULT_RECURSION_DEPTH;
        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            if ident == "limit" {
                let _: Token![=] = input.parse()?;
                let lit: syn::LitInt = input.parse()?;
                limit = lit.base10_parse()?;
            } else {
                return Err(syn::Error::new(ident.span(), "expected `limit = <integer>`"));
            }
            if input.peek(Token![,]) {
                let _: Token![,] = input.parse()?;
            } else {
                break;
            }
        }
        Ok(RecurseArgs { limit })
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
            if graph.get(name).is_some_and(|refs| refs.contains(name)) {
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

/// A generic param list for an `impl`/trait header that **preserves bounds** (`S: Span`, `const N:
/// usize`, …) — unlike `generic_tokens`, which strips them. Used by the engine→natural conversion +
/// delegation impls so they can name a cycle type carrying a bounded param (e.g. `Spanned`'s
/// `Expr<S: Span>`). A cycle type's own params never carry a default, so none is emitted here.
fn param_decls(generics: &Generics) -> Vec<TokenStream> {
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
// Multi-root soundness guard.
//
// `#[recurse]` only emits the depth-limited *types* + `@recurse` metadata; the depth-generic visitor
// is built by `visitor!(<cycle types>)` (see `macro/visitor.rs`). The depth decrements only at a
// self-referential root, so a multi-root cycle whose roots are not a feedback vertex set would not
// terminate — `subgraph_is_cyclic` rejects that at the type level.
// ---------------------------------------------------------------------------

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

// ───────────────────────────── engine → natural conversion codegen ──────────────────────────────
//
// The public cycle types are emitted as *natural* recursive types; `Parse` is delegated to the
// depth-limited engine (`__XRec`) and the parsed engine value is converted back to the natural type by
// a generated `__ToNat_X` trait. See `docs/recurse-natural-types-plan.md` §4. A field is a *recursive
// child* iff its container-peeled head ∈ the SCC (`child_heads`); the conversion descends children via
// `.__to_nat()` (resolved by receiver type) and moves leaves as-is.

/// Build the conversion *expression* for one engine field value `val` of (original) type `ty`: `None`
/// for a leaf (caller uses `val` unchanged), else the converted natural value. `child_heads` is the set
/// of SCC type names; a peeled head in it is a recursive child (`val.__to_nat()`). Containers
/// (`Box`/`Vec`/`VecDeque`/`Punctuated`/`Option`) and tuples are lowered recursively; anything else is a
/// leaf.
fn conv_expr(ty: &Type, val: TokenStream, child_heads: &HashSet<String>) -> Option<TokenStream> {
    match ty {
        Type::Path(TypePath { qself: None, path }) => {
            let seg = path.segments.last()?;
            let name = seg.ident.to_string();
            // A recursive-child reference is always a same-module *bare* ident (`Stmt`, `Stmt<S>`); a
            // foreign multi-segment path (`other::Stmt`) whose last segment merely collides with a cycle
            // name is a leaf. (Mirrors `transform_type`/`collect_refs` keying on the first segment.)
            if path.segments.len() == 1 && child_heads.contains(&name) {
                return Some(quote!( #val.__to_nat() ));
            }
            match name.as_str() {
                "Box" => conv_expr(first_ty_arg(seg)?, quote!((*#val)), child_heads)
                    .map(|c| quote!( ::std::boxed::Box::new(#c) )),
                "Vec" | "VecDeque" | "Punctuated" => {
                    conv_expr(first_ty_arg(seg)?, quote!(__e), child_heads)
                        .map(|c| quote!( #val.into_iter().map(|__e| #c).collect() ))
                }
                "Option" => conv_expr(first_ty_arg(seg)?, quote!(__e), child_heads)
                    .map(|c| quote!( #val.map(|__e| #c) )),
                _ => None,
            }
        }
        Type::Tuple(t) => {
            let binds: Vec<Ident> = (0..t.elems.len())
                .map(|i| Ident::new(&format!("__t{i}"), Span::call_site()))
                .collect();
            let convs: Vec<TokenStream> = t
                .elems
                .iter()
                .zip(&binds)
                .map(|(e, b)| conv_expr(e, quote!(#b), child_heads).unwrap_or_else(|| quote!(#b)))
                .collect();
            let any = t
                .elems
                .iter()
                .zip(&binds)
                .any(|(e, b)| conv_expr(e, quote!(#b), child_heads).is_some());
            any.then(|| quote!( { let (#(#binds,)*) = #val; (#(#convs,)*) } ))
        }
        _ => None,
    }
}

/// The `match self { … }` body that converts an engine value into the natural type `nat_id`, reusing
/// the *original* item's variant/field names (engine and natural share them). `eng_id` is the engine
/// ident (`__XRec`). Field-level conversion is `conv_expr`; a leaf field is moved unchanged.
fn conv_body(item: &Item, nat_id: &Ident, eng_id: &Ident, child_heads: &HashSet<String>) -> TokenStream {
    let arm_fields = |fields: &Fields| -> (TokenStream, TokenStream) {
        match fields {
            Fields::Named(FieldsNamed { named, .. }) => {
                let names: Vec<&Ident> = named.iter().map(|f| f.ident.as_ref().unwrap()).collect();
                let vals: Vec<TokenStream> = named
                    .iter()
                    .map(|f| {
                        let n = f.ident.as_ref().unwrap();
                        let v = conv_expr(&f.ty, quote!(#n), child_heads).unwrap_or_else(|| quote!(#n));
                        quote!( #n: #v )
                    })
                    .collect();
                (quote!( { #(#names),* } ), quote!( { #(#vals),* } ))
            }
            Fields::Unnamed(FieldsUnnamed { unnamed, .. }) => {
                let binds: Vec<Ident> = (0..unnamed.len())
                    .map(|i| Ident::new(&format!("__f{i}"), Span::call_site()))
                    .collect();
                let vals: Vec<TokenStream> = unnamed
                    .iter()
                    .zip(&binds)
                    .map(|(f, b)| conv_expr(&f.ty, quote!(#b), child_heads).unwrap_or_else(|| quote!(#b)))
                    .collect();
                (quote!( ( #(#binds),* ) ), quote!( ( #(#vals),* ) ))
            }
            Fields::Unit => (quote!(), quote!()),
        }
    };
    match item {
        Item::Enum(e) => {
            let arms: Vec<TokenStream> = e
                .variants
                .iter()
                .map(|v| {
                    let vn = &v.ident;
                    let (pat, ctor) = arm_fields(&v.fields);
                    quote!( #eng_id::#vn #pat => #nat_id::#vn #ctor, )
                })
                .collect();
            quote!( match self { #(#arms)* } )
        }
        Item::Struct(s) => {
            let (pat, ctor) = arm_fields(&s.fields);
            match &s.fields {
                Fields::Unit => quote!( #nat_id ),
                _ => quote!( { let #eng_id #pat = self; #nat_id #ctor } ),
            }
        }
        _ => quote!(),
    }
}

// ── nonce-stamped internal names ────────────────────────────────────────────────────────────────
//
// Every generated, otherwise-private item carries a per-`#[recurse]`-expansion `nonce` so its name
// cannot collide with the user's own items — a user type literally named `ExprTerm` no longer clashes
// with the generated terminator (cf. `ui/audit_recurse_terminator_collision.rs`). The nonce is constant
// across one expansion, so every site that re-derives a name (in `build_scc`, `gen_natural_extras`,
// `build_multiroot_tail`, `from_conv_expr`) agrees on it.

/// Engine (depth-limited) node type for a cycle type: `__<name>Rec_<nonce>`.
fn engine_name(name: &str, nonce: u64) -> Ident {
    Ident::new(&format!("__{name}Rec_{nonce}"), Span::call_site())
}
/// Per-root terminator type: `__<root>Term_<nonce>` (now `__`-prefixed + nonced, so it can't collide).
fn term_name(root: &str, nonce: u64) -> Ident {
    Ident::new(&format!("__{root}Term_{nonce}"), Span::call_site())
}
/// Per-root depth-default chain alias: `__<root>Default_<nonce>`.
fn default_name(root: &str, nonce: u64) -> Ident {
    Ident::new(&format!("__{root}Default_{nonce}"), Span::call_site())
}
/// Engine→natural conversion trait for a cycle type: `__ToNat_<name>_<nonce>`.
fn to_nat_name(name: &str, nonce: u64) -> Ident {
    Ident::new(&format!("__ToNat_{name}_{nonce}"), Span::call_site())
}
/// Natural→engine conversion trait for a cycle type: `__FromNat_<name>_<nonce>`.
fn from_nat_name(name: &str, nonce: u64) -> Ident {
    Ident::new(&format!("__FromNat_{name}_{nonce}"), Span::call_site())
}

/// The reverse of `conv_expr`: build an *engine* field value from a borrowed *natural* field value
/// `val` (a `&NatTy` expression). `None` for a leaf (caller `Clone`s it). A recursive child dispatches
/// on its head's natural type via that type's `__FromNat_<Head>::__from_nat` (Self — the engine field
/// type — is inferred from the surrounding engine constructor). Containers/tuples are lowered by ref.
fn from_conv_expr(
    ty: &Type,
    val: TokenStream,
    child_heads: &HashSet<String>,
    nonce: u64,
) -> Option<TokenStream> {
    match ty {
        Type::Path(TypePath { qself: None, path }) => {
            let seg = path.segments.last()?;
            let name = seg.ident.to_string();
            if path.segments.len() == 1 && child_heads.contains(&name) {
                let tn = from_nat_name(&name, nonce);
                return Some(quote!( #tn::__from_nat(#val) ));
            }
            match name.as_str() {
                "Box" => from_conv_expr(first_ty_arg(seg)?, quote!((&**#val)), child_heads, nonce)
                    .map(|c| quote!( ::std::boxed::Box::new(#c) )),
                "Vec" | "VecDeque" | "Punctuated" => {
                    from_conv_expr(first_ty_arg(seg)?, quote!(__e), child_heads, nonce)
                        .map(|c| quote!( #val.iter().map(|__e| #c).collect() ))
                }
                "Option" => from_conv_expr(first_ty_arg(seg)?, quote!(__e), child_heads, nonce)
                    .map(|c| quote!( #val.as_ref().map(|__e| #c) )),
                _ => None,
            }
        }
        Type::Tuple(t) => {
            let binds: Vec<Ident> = (0..t.elems.len())
                .map(|i| Ident::new(&format!("__t{i}"), Span::call_site()))
                .collect();
            let any = t
                .elems
                .iter()
                .zip(&binds)
                .any(|(e, b)| from_conv_expr(e, quote!(#b), child_heads, nonce).is_some());
            if !any {
                return None;
            }
            let convs: Vec<TokenStream> = t
                .elems
                .iter()
                .zip(&binds)
                .map(|(e, b)| {
                    from_conv_expr(e, quote!(#b), child_heads, nonce)
                        .unwrap_or_else(|| quote!( ::core::clone::Clone::clone(#b) ))
                })
                .collect();
            Some(quote!( { let (#(#binds,)*) = #val; (#(#convs,)*) } ))
        }
        _ => None,
    }
}

/// The `match __nat { … }` body that builds an engine value (`eng_id`, e.g. `__XRec`) from a borrowed
/// natural value (`nat_id`). Recursive children convert via `from_conv_expr`; a leaf field is cloned
/// (`Clone::clone(&field)` — the binding is a reference, so this clones the value, not the reference).
fn from_conv_body(
    item: &Item,
    nat_id: &Ident,
    eng_id: &Ident,
    child_heads: &HashSet<String>,
    nonce: u64,
) -> TokenStream {
    let arm_fields = |fields: &Fields| -> (TokenStream, TokenStream) {
        match fields {
            Fields::Named(FieldsNamed { named, .. }) => {
                let names: Vec<&Ident> = named.iter().map(|f| f.ident.as_ref().unwrap()).collect();
                let vals: Vec<TokenStream> = named
                    .iter()
                    .map(|f| {
                        let n = f.ident.as_ref().unwrap();
                        let v = from_conv_expr(&f.ty, quote!(#n), child_heads, nonce)
                            .unwrap_or_else(|| quote!( ::core::clone::Clone::clone(#n) ));
                        quote!( #n: #v )
                    })
                    .collect();
                (quote!( { #(#names),* } ), quote!( { #(#vals),* } ))
            }
            Fields::Unnamed(FieldsUnnamed { unnamed, .. }) => {
                let binds: Vec<Ident> = (0..unnamed.len())
                    .map(|i| Ident::new(&format!("__f{i}"), Span::call_site()))
                    .collect();
                let vals: Vec<TokenStream> = unnamed
                    .iter()
                    .zip(&binds)
                    .map(|(f, b)| {
                        from_conv_expr(&f.ty, quote!(#b), child_heads, nonce)
                            .unwrap_or_else(|| quote!( ::core::clone::Clone::clone(#b) ))
                    })
                    .collect();
                (quote!( ( #(#binds),* ) ), quote!( ( #(#vals),* ) ))
            }
            Fields::Unit => (quote!(), quote!()),
        }
    };
    match item {
        Item::Enum(e) => {
            let arms: Vec<TokenStream> = e
                .variants
                .iter()
                .map(|v| {
                    let vn = &v.ident;
                    let (pat, ctor) = arm_fields(&v.fields);
                    quote!( #nat_id::#vn #pat => #eng_id::#vn #ctor, )
                })
                .collect();
            quote!( match __nat { #(#arms)* } )
        }
        Item::Struct(s) => {
            let (pat, ctor) = arm_fields(&s.fields);
            match &s.fields {
                Fields::Unit => quote!( #eng_id ),
                _ => quote!( { let #nat_id #pat = __nat; #eng_id #ctor } ),
            }
        }
        _ => quote!(),
    }
}

/// The (whole) types of a cycle type's *leaf* fields — those `from_conv_expr` doesn't convert. Each must
/// be `Clone` for the natural→engine `from_nat` (which clones leaves into the engine). (The `nonce` only
/// satisfies `from_conv_expr`'s signature — leaf-or-not doesn't depend on it.)
fn leaf_field_types(item: &Item, child_heads: &HashSet<String>, nonce: u64) -> Vec<Type> {
    let mut out = Vec::new();
    let mut push = |fields: &Fields| {
        for f in fields.iter() {
            if from_conv_expr(&f.ty, quote!(__x), child_heads, nonce).is_none() {
                out.push(f.ty.clone());
            }
        }
    };
    match item {
        Item::Enum(e) => e.variants.iter().for_each(|v| push(&v.fields)),
        Item::Struct(s) => push(&s.fields),
        _ => {}
    }
    out
}

/// For an SCC whose natural types own the public names, emit the engine→natural bridge: one
/// `__ToNat_X` trait + impl per cycle type, a terminator `__to_nat` (`unreachable!`) per root, and the
/// delegated `impl Parse for X` (parse the depth-limited engine, then `.__to_nat()`). When
/// `Unparse`/`Spanned` are engine-routed (a multi-type or group-ful cycle), also emit the *reverse*
/// `__FromNat_X` bridge (natural→engine, `Clone`ing leaves, terminator `panic!`s past the depth limit)
/// and a delegated `impl Unparse`/`impl Spanned` for the natural type that converts then calls the
/// engine's impl. Replaces the old `@recurse` metadata + public aliases. `default_for_root` maps each
/// root to its `__<root>Default` depth alias; `rec_for_root` to its depth param; `root_generics` are the
/// roots' (shared) params.
#[allow(clippy::too_many_arguments)]
fn gen_natural_extras(
    scc: &HashSet<String>,
    items: &[Item],
    internal_names: &HashMap<String, Ident>,
    roots_sorted: &[String],
    rec_for_root: &HashMap<String, Ident>,
    default_for_root: &HashMap<String, Ident>,
    root_generics: &Generics,
    parse_types: &HashSet<String>,
    unparse_types: &HashSet<String>,
    spanned_types: &HashSet<String>,
    nonce: u64,
) -> TokenStream {
    let child_heads: HashSet<String> = scc.clone();
    // `root_decl`/`xdecl` (below) keep param BOUNDS (for naming a bounded cycle type like `Expr<S: Span>`
    // in the conversion/delegation impls); `*_use` are the bound-free argument forms.
    let root_decl = param_decls(root_generics);
    let root_use = generic_tokens(root_generics).1;
    let rec_params: Vec<&Ident> = roots_sorted.iter().map(|r| &rec_for_root[r]).collect();
    let trait_name = |x: &str| to_nat_name(x, nonce);
    let from_trait_name = |x: &str| from_nat_name(x, nonce);
    // Whether THIS SCC delegates `Unparse`/`Spanned` natural→engine (a multi-type or group-ful cycle
    // whose `Unparse`/`Spanned` derive(s) were routed to the engine). When so, emit the `__FromNat`
    // bridge + the delegated impls. (A single self-recursive group-free cycle keeps them on the natural
    // type directly, so `unparse_types`/`spanned_types` exclude it and this is empty.)
    let delegate_unparse: Vec<&String> = scc.iter().filter(|n| unparse_types.contains(*n)).collect();
    let delegate_spanned: Vec<&String> = scc.iter().filter(|n| spanned_types.contains(*n)).collect();
    let needs_from_nat = !delegate_unparse.is_empty() || !delegate_spanned.is_empty();
    // `R: __FromNat_<root>` per root — the natural→engine bridge's analogue of `root_bounds`.
    let from_root_bounds: Vec<TokenStream> = roots_sorted
        .iter()
        .map(|r| {
            let dp = &rec_for_root[r];
            let tn = from_trait_name(r);
            quote!( #dp: #tn<#(#root_use),*> )
        })
        .collect();

    // Each root depth param must convert to its root's natural type: `__Rec: __ToNat_Root<root gen>`.
    let root_bounds: Vec<TokenStream> = roots_sorted
        .iter()
        .map(|r| {
            let dp = &rec_for_root[r];
            let tn = trait_name(r);
            quote!( #dp: #tn<#(#root_use),*> )
        })
        .collect();

    // `Clone` bounds for the natural→engine `from_nat` impls: the UNION of every SCC member's leaf
    // field types. A member's `from_nat` calls its siblings' `from_nat` (for cross-edge children), so it
    // must carry every sibling's leaf `Clone` bounds too — exactly as the engine-routed delegation
    // requires (the members' leaf types generally differ). Computed once and applied to every impl.
    let from_leaf_clones: Vec<TokenStream> = if needs_from_nat {
        let mut seen = HashSet::new();
        items
            .iter()
            .filter(|it| match it {
                Item::Enum(e) => scc.contains(&e.ident.to_string()),
                Item::Struct(s) => scc.contains(&s.ident.to_string()),
                _ => false,
            })
            .flat_map(|it| leaf_field_types(it, &child_heads, nonce))
            .filter(|t| seen.insert(quote!(#t).to_string()))
            .map(|t| quote!( #t: ::core::clone::Clone ))
            .collect()
    } else {
        Vec::new()
    };

    // Each cycle type's `where`-clause predicates, by name — so the terminator loop (below, which names
    // the root's natural type and impls the conversion traits) can repeat the root's clause too.
    let mut where_preds_of: HashMap<String, Vec<TokenStream>> = HashMap::new();

    let mut out = TokenStream::new();
    for item in items {
        let (id, generics) = match item {
            Item::Enum(e) if matches!(e.vis, Visibility::Public(_)) && scc.contains(&e.ident.to_string()) => {
                (&e.ident, &e.generics)
            }
            Item::Struct(s) if matches!(s.vis, Visibility::Public(_)) && scc.contains(&s.ident.to_string()) => {
                (&s.ident, &s.generics)
            }
            _ => continue,
        };
        let xs = id.to_string();
        let tn = trait_name(&xs);
        let engine = &internal_names[&xs];
        let xdecl = param_decls(generics);
        let xuse = generic_tokens(generics).1;
        let body = conv_body(item, id, engine, &child_heads);
        // The cycle type's own `where`-clause predicates (e.g. `where S: Clone` / `where Expr<S>:
        // Marker`). Every generated item that NAMES the natural type `Expr<S>` (the conversion traits'
        // method signatures, and the conversion/delegated impls) must repeat these — naming `Expr<S>`
        // is only well-formed when its where-clause holds — else the obligation surfaces undischarged
        // (E0277). They reference the cycle's own params, which are in scope on all of these.
        let where_preds: Vec<TokenStream> = generics
            .where_clause
            .as_ref()
            .map(|w| w.predicates.iter().map(|p| quote!(#p)).collect())
            .unwrap_or_default();
        where_preds_of.insert(xs.clone(), where_preds.clone());
        // engine instantiation at the public depth defaults: `__XRec<own…, __RootDefault<root>…>`
        let default_args: Vec<TokenStream> = roots_sorted
            .iter()
            .map(|r| {
                let d = &default_for_root[r];
                quote!( #d<#(#root_use),*> )
            })
            .collect();
        let engine_default = quote!( #engine<#(#xuse,)* #(#default_args),*> );

        // Conversion trait + engine→natural impl (always emitted; used by the delegated `Parse`).
        out.extend(quote! {
            #[doc(hidden)]
            trait #tn<#(#xdecl),*>
            #(if !where_preds.is_empty()) { where #(#where_preds),* }
            {
                fn __to_nat(self) -> #id<#(#xuse),*>;
            }
            impl<#(#xdecl,)* #(#rec_params),*> #tn<#(#xuse),*>
                for #engine<#(#xuse,)* #(#rec_params),*>
            #(if !root_bounds.is_empty() || !where_preds.is_empty()) {
                where #(#root_bounds,)* #(#where_preds,)*
            }
            {
                fn __to_nat(self) -> #id<#(#xuse),*> { #body }
            }
        });
        // Delegated `Parse` on the natural type — only when the user derived `Parse` (else the engine
        // has no `Parse` impl to delegate to).
        if parse_types.contains(&xs) {
            out.extend(quote! {
                impl<#(#xdecl,)* __Atom> ::syan::parse::Parse<__Atom> for #id<#(#xuse),*>
                where
                    __Atom: ::syan::span::Spanned + ::core::clone::Clone,
                    #engine_default: ::syan::parse::Parse<__Atom, Error = ::syan::error::ParseError>,
                    #engine_default: #tn<#(#xuse),*>,
                    #(#where_preds,)*
                {
                    type Error = ::syan::error::ParseError;
                    fn parse(
                        __syan_s: impl ::syan::parse::IntoParseStream<Atom = __Atom>,
                    ) -> ::core::result::Result<Self, Self::Error> {
                        ::core::result::Result::Ok(
                            #tn::__to_nat(
                                <#engine_default as ::syan::parse::Parse<__Atom>>::parse(__syan_s)?,
                            ),
                        )
                    }
                }
            });
        }

        // Natural→engine bridge (`__FromNat_X`) for an engine-delegated `Unparse`/`Spanned` cycle, plus
        // the delegated impls. The bridge `Clone`s leaves into the engine; recursive children recurse
        // through it (terminator `panic!`s past the depth limit — see below). `Unparse`/`Spanned` then
        // convert the (borrowed) natural value to the depth-default engine value and call the engine's
        // own impl.
        if needs_from_nat {
            let ftn = from_trait_name(&xs);
            let from_body = from_conv_body(item, id, engine, &child_heads, nonce);
            out.extend(quote! {
                #[doc(hidden)]
                trait #ftn<#(#xdecl),*>
                #(if !where_preds.is_empty()) { where #(#where_preds),* }
                {
                    fn __from_nat(__nat: &#id<#(#xuse),*>) -> Self;
                }
                impl<#(#xdecl,)* #(#rec_params),*> #ftn<#(#xuse),*>
                    for #engine<#(#xuse,)* #(#rec_params),*>
                where
                    #(#from_root_bounds,)*
                    #(#from_leaf_clones,)*
                    #(#where_preds,)*
                {
                    fn __from_nat(__nat: &#id<#(#xuse),*>) -> Self { #from_body }
                }
            });
            if unparse_types.contains(&xs) {
                out.extend(quote! {
                    impl<#(#xdecl,)* __Atom> ::syan::parse::Unparse<__Atom> for #id<#(#xuse),*>
                    where
                        #engine_default: ::syan::parse::Unparse<__Atom> + #ftn<#(#xuse),*>,
                        #(#where_preds,)*
                    {
                        fn unparse<__E: ::syan::parse::unparse::Emitter<__Atom>>(
                            &self,
                            __sink: &mut __E,
                        ) -> ::core::result::Result<(), __E::Error> {
                            let __e: #engine_default = #ftn::__from_nat(self);
                            <#engine_default as ::syan::parse::Unparse<__Atom>>::unparse(&__e, __sink)
                        }
                    }
                });
            }
            if spanned_types.contains(&xs) {
                // The cycle's span type is its first type param (recurse convention) — and the engine's
                // `Spanned::Span` equals it (the `WithSpan<_, S>` leaves pin it). Use `type Span = S`
                // directly so the *private* engine type doesn't leak into this public assoc type (E0446);
                // the body delegates through the engine (bounded `Spanned<Span = S>`).
                let span_param = generics.params.iter().find_map(|p| match p {
                    GenericParam::Type(t) => Some(&t.ident),
                    _ => None,
                });
                if let Some(sp) = span_param {
                    out.extend(quote! {
                        impl<#(#xdecl),*> ::syan::span::Spanned for #id<#(#xuse),*>
                        where
                            #engine_default: ::syan::span::Spanned<Span = #sp> + #ftn<#(#xuse),*>,
                            #(#where_preds,)*
                        {
                            type Span = #sp;
                            fn span(&self) -> Self::Span {
                                let __e: #engine_default = #ftn::__from_nat(self);
                                <#engine_default as ::syan::span::Spanned>::span(&__e)
                            }
                        }
                    });
                }
            }
        }
    }

    // Terminator → natural: never reached at runtime (the terminator's `Parse` always errors).
    for r in roots_sorted {
        let tn = trait_name(r);
        let term = term_name(r, nonce);
        let root_id = Ident::new(r, Span::call_site());
        let term_args: TokenStream = if root_decl.is_empty() {
            quote!()
        } else {
            quote!( <#(#root_use),*> )
        };
        // The root's own `where`-clause — the terminator names the root's natural type and impls its
        // conversion trait (both carrying the clause), so repeat it here too.
        let rwp: &[TokenStream] = where_preds_of.get(r).map(Vec::as_slice).unwrap_or(&[]);
        out.extend(quote! {
            impl<#(#root_decl),*> #tn<#(#root_use),*> for #term #term_args
            #(if !rwp.is_empty()) { where #(#rwp),* }
            {
                fn __to_nat(self) -> #root_id<#(#root_use),*> {
                    ::core::unreachable!("#[recurse]: depth-limit terminator can never be parsed")
                }
            }
        });
        // Terminator side of the natural→engine bridge: a natural tree deeper than the engine's depth
        // limit reaches the terminator here and cannot be represented — so delegated `Unparse`/`Spanned`
        // panic on a tree deeper than `limit` (within the limit they succeed).
        if needs_from_nat {
            let ftn = from_trait_name(r);
            out.extend(quote! {
                impl<#(#root_decl),*> #ftn<#(#root_use),*> for #term #term_args
                #(if !rwp.is_empty()) { where #(#rwp),* }
                {
                    fn __from_nat(_: &#root_id<#(#root_use),*>) -> Self {
                        ::core::panic!(
                            "#[recurse]: cannot Unparse/Spanned a natural tree deeper than the \
                             recursion limit (delegated through the depth-limited engine)"
                        )
                    }
                }
            });
        }
        // When delegating `Spanned`, the engine's `Spanned` chain bottoms at the terminator, so it too
        // must be `Spanned`. By the recurse convention the cycle's span type is its first type param;
        // the terminator is never constructed (its `Parse` errors), so `span()` is unreachable.
        if !delegate_spanned.is_empty() {
            if let Some(sp) = root_generics.params.iter().find_map(|p| match p {
                GenericParam::Type(t) => Some(&t.ident),
                _ => None,
            }) {
                out.extend(quote! {
                    impl<#(#root_decl),*> ::syan::span::Spanned for #term #term_args
                    where #sp: ::syan::span::Span, #(#rwp,)* {
                        type Span = #sp;
                        fn span(&self) -> Self::Span {
                            ::core::unreachable!("#[recurse]: depth-limit terminator has no span")
                        }
                    }
                });
            }
        }
    }
    out
}

/// Build the recurse machinery for ONE independent cycle (`scc`): pick its root, rename its types,
/// and produce (a) the `TransformCtx` that rewrites the cycle's items and (b) the *tail* tokens
/// appended to the module (terminator + its `Parse`/`Unparse` impls, the depth-default alias, the
/// public type aliases, and the per-cycle-type `@recurse` metadata macros that `visitor!()` consumes
/// to build a depth-generic visitor). Each cycle is handled independently, so a module may hold
/// several (see `find_cycle_sccs`).
#[allow(clippy::too_many_arguments)]
fn build_scc(
    scc: &HashSet<String>,
    items: &[Item],
    type_refs: &HashMap<String, HashSet<String>>,
    direct_type_refs: &HashMap<String, HashSet<String>>,
    recursion_depth: usize,
    mod_ident: &Ident,
    parse_types: &HashSet<String>,
    unparse_types: &HashSet<String>,
    spanned_types: &HashSet<String>,
    nonce: u64,
) -> (TransformCtx, TokenStream) {
    // Root types: this cycle's types that directly reference themselves.
    let root_types: HashSet<String> = scc
        .iter()
        .filter(|name| {
            type_refs
                .get(*name)
                .is_some_and(|refs| refs.contains(*name))
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

    let term_ident = term_name(&root_name, nonce);
    let default_alias = default_name(&root_name, nonce);

    // Internal (renamed) idents: "Expr" → "__ExprRec_<nonce>"
    let internal_names: HashMap<String, Ident> = scc
        .iter()
        .map(|n| (n.clone(), engine_name(n, nonce)))
        .collect();

    // The root's full generics drive the depth aliases. (The root is always one of the cycle types,
    // so the fallback is unreachable.)
    let root_generics: Generics = items
        .iter()
        .find_map(|item| match item {
            Item::Enum(e)
                if matches!(e.vis, Visibility::Public(_)) && e.ident == root_name =>
            {
                Some(e.generics.clone())
            }
            Item::Struct(s)
                if matches!(s.vis, Visibility::Public(_)) && s.ident == root_name =>
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
        .map(|r| (r.clone(), default_name(r, nonce)))
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
        // Soundness: the depth decrements only at the root's back-edge, so a sub-cycle that never
        // touches the root would never terminate. The multi-root path checks this; do it here too
        // (it was previously skipped whenever ≤1 cycle type self-references, silently leaving such a
        // sub-cycle un-depth-limited).
        let root_set: HashSet<String> = ::std::iter::once(root_name.clone()).collect();
        if subgraph_is_cyclic(scc, &root_set, type_refs) {
            abort!(
                mod_ident,
                "#[recurse]: this cycle has a sub-cycle running entirely through non-root types, so \
                 the depth recursion (which only decrements at the root `{}`) would not terminate. \
                 Make a type on that sub-cycle directly self-referential, or split it into its own \
                 `#[derive(Ast)]` type.",
                root_name
            );
        }

        // Inner default: (recursion_depth - 1) levels of __ExprRec<P0, P1, …, depth_ty>. The public
        // Expr<…> alias adds one more layer so that matching Expr::Block { stmts } leaves
        // stmts: Vec<__StmtRec<…, __ExprDefault<…>>> which equals Vec<Stmt<…>>.
        let root_internal = &internal_names[&root_name];
        let mut depth_ty: TokenStream = quote!(#term_ident #term_args);
        for _ in 0..(recursion_depth - 1) {
            depth_ty = quote!(#root_internal<#(#gen_use,)* #depth_ty>);
        }

        // NOTE (natural-type design): the public `pub type Expr = __ExprRec<…>` aliases are *not*
        // emitted — the natural recursive enums/structs own those names. Only the internal depth-chain
        // alias `__ExprDefault` is kept (the delegated `Parse` references `__ExprRec<…, __ExprDefault>`).
        let _ = &default_alias;

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
            type_refs,
            mod_ident,
            nonce,
        )
    };

    // Engine→natural bridge: `__ToNat_X` conversion traits/impls + terminator `unreachable!` +
    // delegated `impl Parse for X` (parse the depth-limited engine, then convert). Replaces the old
    // `@recurse` metadata + public `pub type` aliases.
    let extras = gen_natural_extras(
        scc,
        items,
        &internal_names,
        &roots_sorted,
        &rec_for_root,
        &default_for_root,
        &root_generics,
        parse_types,
        unparse_types,
        spanned_types,
        nonce,
    );

    (ctx, quote!( #tail #extras ))
}

/// Emit the tail (terminators, depth-chain aliases, public aliases) for a cycle with **several
/// self-referential roots**. Each root keeps its own depth dimension: every cycle type carries one
/// depth param per root, a back-edge to root `X` is that root's param, and the per-root depth chains
/// are built mutually (level `k` of each root embeds level `k-1` of *all* roots). The depth only
/// decrements at a root back-edge, so every cycle in the SCC must pass through a root — checked via
/// `safegraph` (the SCC minus the roots must be acyclic).
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
    type_refs: &HashMap<String, HashSet<String>>,
    mod_ident: &Ident,
    nonce: u64,
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
        .map(|r| (r.clone(), term_name(r, nonce)))
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

    // NOTE (natural-type design): no public `pub type X = __XRec<…>` aliases — the natural recursive
    // types own those names. Only the internal terminators + depth-chain aliases are emitted.
    quote! {
        #(#term_items)*
        #(#default_decls)*
    }
}

/// Split a cycle type's `#[derive(...)]`: route the *structural* syan derives that emit per-field
/// `field_ty: Trait` bounds (`Parse`/`Unparse`/`Spanned`) to the depth-limited **engine** (they would
/// form an E0275 where-bound cycle on the natural type; on the engine the recursive child is the depth
/// param, breaking the cycle). Everything else (`Ast`, `Debug`, `Clone`, `#[subast]`, docs, …) stays on
/// the natural type. Returns `(natural attrs, engine-routed derive paths)`. `Parse` on the natural type
/// is re-supplied as a delegating impl (parse engine → convert); `Unparse`/`Spanned` currently live on
/// the engine only (a direct natural impl needs cycle-wide union bounds — see
/// `docs/recurse-natural-types-plan.md` §5).
/// Partition a cycle type's `#[derive(...)]` into (kept-on-natural attrs, engine-routed derive paths),
/// routing the derives in `engine_routed` to the engine. `Parse` is *always* engine-routed (it needs
/// the depth-limited engine — see `make_natural_item`'s doc); `Unparse`/`Spanned` are engine-routed only
/// for a **group-ful** cycle (their natural derive would need cycle-wide union bounds the group `Fill`
/// machinery makes infeasible). Everything else (`Ast`, `Debug`, `#[subast]`, docs, …) stays on natural.
fn split_cycle_derives(
    attrs: &[syn::Attribute],
    engine_routed: &[&str],
) -> (Vec<syn::Attribute>, Vec<Path>) {
    let mut natural = Vec::new();
    let mut engine_paths = Vec::new();
    for attr in attrs {
        if attr.path().is_ident("derive") {
            if let syn::Meta::List(list) = &attr.meta {
                let paths: Punctuated<Path, Token![,]> = list
                    .parse_args_with(Punctuated::parse_terminated)
                    .unwrap_or_default();
                let mut kept: Vec<Path> = Vec::new();
                for p in paths {
                    if p.segments.last().is_some_and(|s| engine_routed.iter().any(|t| s.ident == t)) {
                        engine_paths.push(p);
                    } else {
                        kept.push(p);
                    }
                }
                if !kept.is_empty() {
                    natural.push(syn::parse_quote!( #[derive(#(#kept),*)] ));
                }
                continue;
            }
        }
        natural.push(attr.clone());
    }
    (natural, engine_paths)
}

/// Whether `attrs` contains a `#[derive(...)]` mentioning any of `names`.
fn derives_any(attrs: &[syn::Attribute], names: &[&str]) -> bool {
    attrs.iter().any(|a| {
        a.path().is_ident("derive")
            && matches!(&a.meta, syn::Meta::List(l)
                if l.parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)
                    .map(|ps| ps.iter().any(|p| p.segments.last().is_some_and(|s| names.iter().any(|n| s.ident == n))))
                    .unwrap_or(false))
    })
}

/// Strip the structural-derive field helper attributes from a field set. Used on the natural type when
/// it carries NO structural derive (else the attrs would be unregistered "cannot find attribute").
fn strip_field_helper_attrs(fields: &mut Fields) {
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

/// Inject `#[ignore_bounds]` on every field whose type references a cycle type (a *recursive child*),
/// so the natural type's `Unparse`/`Spanned` derive emits leaf-only bounds (the recursion resolves
/// coinductively at the body's `.unparse()`/`.span()` call sites, not via an E0275 where-bound cycle).
/// A user-written `#[ignore_bounds]` is left as-is (not doubled).
fn inject_ignore_bounds(fields: &mut Fields, scc: &HashSet<String>) {
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

/// The public **natural** form of a cycle type. `Parse` is always routed to the engine (it needs the
/// depth-limited engine: a natural `Parse` overflows both on the per-field where-bound cycle and on the
/// backtracking `Dup<…>` stream-type recursion). `Unparse`/`Spanned` are kept on the natural type when
/// the cycle is **group-free** (`us_natural`) — then `#[ignore_bounds]` is injected on recursive-child
/// fields so their leaf-only-bounded impls compile (the recursion resolves at the body's call sites);
/// for a **group-ful** cycle they too are engine-routed (the group `Fill` bounds would need cycle-wide
/// unioning). When the natural type carries a structural derive its field helper attrs are kept (the
/// derive consumes them); otherwise they are stripped (else unregistered). Returns `(natural item,
/// engine-routed derive paths)`.
fn make_natural_item(item: &Item, scc: &HashSet<String>, us_natural: bool) -> (Item, Vec<Path>) {
    let engine_routed: &[&str] = if us_natural {
        &["Parse"]
    } else {
        &["Parse", "Unparse", "Spanned"]
    };
    let mut it = item.clone();
    let engine_paths = match &mut it {
        Item::Enum(e) => {
            let (nat, ep) = split_cycle_derives(&e.attrs, engine_routed);
            // Does the natural type still carry a structural derive (Unparse/Spanned) that consumes the
            // field helper attrs and needs `#[ignore_bounds]` on recursive children?
            let structural = derives_any(&nat, &["Unparse", "Spanned"]);
            e.attrs = nat;
            for v in &mut e.variants {
                if structural {
                    inject_ignore_bounds(&mut v.fields, scc);
                } else {
                    strip_field_helper_attrs(&mut v.fields);
                }
            }
            ep
        }
        Item::Struct(s) => {
            let (nat, ep) = split_cycle_derives(&s.attrs, engine_routed);
            let structural = derives_any(&nat, &["Unparse", "Spanned"]);
            s.attrs = nat;
            if structural {
                inject_ignore_bounds(&mut s.fields, scc);
            } else {
                strip_field_helper_attrs(&mut s.fields);
            }
            ep
        }
        _ => Vec::new(),
    };
    (it, engine_paths)
}

/// The internal **engine** form of a cycle type: `transform_item` (rename `X` → `__XRec`, thread the
/// depth params) then made `pub(crate)` and carrying the engine-routed structural derives
/// (`#[derive(Parse, Unparse, Spanned)]` as the user wrote them). The depth-limited engine is finite, so
/// the normal derives apply. Must be called while the original is still `pub` (transform_item keys on
/// that), then the visibility is narrowed.
fn make_engine_item(item: &Item, ctx: &TransformCtx, engine_paths: &[Path]) -> Item {
    let mut eng = transform_item(item.clone(), ctx);
    let derives: Vec<syn::Attribute> = if engine_paths.is_empty() {
        vec![]
    } else {
        vec![syn::parse_quote!(#[derive(#(#engine_paths),*)])]
    };
    // Strip any `#[ignore_bounds]` from the engine's fields: the engine's recursive child is the depth
    // param `__Rec` (a *finite* chain), so its derives need the FULL `__Rec: Trait` bound — dropping it
    // would leave the derive body's `__Rec::parse()`/`unparse()` call unsatisfiable. (A user-written
    // `#[ignore_bounds]` is meant for the natural type, not the engine.)
    let strip_ib = |fields: &mut Fields| {
        let go = |f: &mut Field| f.attrs.retain(|a| !a.path().is_ident("ignore_bounds"));
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

pub fn recurse(attr: TokenStream1, input: TokenStream1, nonce: u64) -> TokenStream1 {
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

    // Per SCC: does any member have a `#[group(...)]` field? `Unparse`/`Spanned` are kept on the natural
    // type only for a GROUP-FREE cycle — a group field's `Fill<Substruct>: Unparse` bound would have to
    // be unioned across every cycle member (the substruct is derive-internal), which the per-type derive
    // can't do, so a group-ful cycle keeps `Unparse`/`Spanned` on the engine instead.
    let field_has_group = |f: &Field| f.attrs.iter().any(|a| a.path().is_ident("group"));
    let item_has_group = |item: &Item| match item {
        Item::Enum(e) => e
            .variants
            .iter()
            .any(|v| v.fields.iter().any(&field_has_group)),
        Item::Struct(s) => s.fields.iter().any(&field_has_group),
        _ => false,
    };
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
    let scc_has_group: Vec<bool> = sccs
        .iter()
        .map(|scc| {
            items
                .iter()
                .any(|item| item_in_scc(item, scc) && item_has_group(item))
        })
        .collect();
    // Keep `Unparse`/`Spanned` on the *natural* type only for a **single, self-recursive, group-free**
    // cycle. There the leaf-only-bounded impl (via injected `#[ignore_bounds]`) always type-checks: the
    // body's recursive `.unparse()`/`.span()` call resolves against the *same* impl, so no sibling's
    // bounds need to be in scope. A multi-type cycle would need each member to carry every *other*
    // member's leaf bounds (the members' leaf field types generally differ), and a group-ful cycle would
    // additionally need the derive-internal `Fill<Substruct>` bounds unioned — neither is expressible
    // per-type, so those keep `Unparse`/`Spanned` on the engine (where the depth param breaks the
    // cycle). See `docs/recurse-natural-types-plan.md` §5.
    let scc_us_natural: Vec<bool> = sccs
        .iter()
        .enumerate()
        .map(|(i, scc)| scc.len() == 1 && !scc_has_group[i])
        .collect();
    // The depth-limited engine (+ terminators + `__ToNat_*` conversion) backs `Parse` and any
    // engine-routed `Unparse`/`Spanned`. A cycle that derives no `Parse` and keeps its
    // `Unparse`/`Spanned` on the natural type (or derives none) needs no engine — emit just the natural
    // types (with their direct `Unparse`/`Spanned`/`Ast`).
    let scc_needs_engine: Vec<bool> = sccs
        .iter()
        .enumerate()
        .map(|(i, scc)| {
            let has_parse = items
                .iter()
                .any(|item| item_in_scc(item, scc) && derives_any(item_attrs(item), &["Parse"]));
            let has_us = items.iter().any(|item| {
                item_in_scc(item, scc) && derives_any(item_attrs(item), &["Unparse", "Spanned"])
            });
            has_parse || (!scc_us_natural[i] && has_us)
        })
        .collect();

    // Which cycle types derive `Parse`? Only those get a delegated `impl Parse for X` on the natural
    // type (the engine derives `Parse` only when the user asked for it; emitting the delegation
    // otherwise would reference a non-existent engine `Parse` impl).
    let derives_parse = |attrs: &[syn::Attribute]| -> bool {
        attrs.iter().any(|a| {
            a.path().is_ident("derive")
                && matches!(&a.meta, syn::Meta::List(l)
                    if l.parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)
                        .map(|ps| ps.iter().any(|p| p.segments.last().is_some_and(|s| s.ident == "Parse")))
                        .unwrap_or(false))
        })
    };
    let parse_types: HashSet<String> = items
        .iter()
        .filter_map(|item| match item {
            Item::Enum(e) if derives_parse(&e.attrs) => Some(e.ident.to_string()),
            Item::Struct(s) if derives_parse(&s.attrs) => Some(s.ident.to_string()),
            _ => None,
        })
        .collect();

    // Cycle types that get a *delegated* natural `Unparse`/`Spanned` via the `__FromNat` bridge to the
    // engine: any cycle that derives the trait but does NOT keep it on the natural type directly (i.e.
    // every non-`us_natural` cycle — multi-type, or group-ful). The delegation is structurally identical
    // for a group-ful cycle: `from_nat` clones the leaf `brace` and recurses into `inner`, and the
    // engine's group `Unparse`/`Spanned` (with its `for<'a> Fill<Substruct>: Unparse` HRTB) *does*
    // resolve through the delegated impl's `engine_default: Unparse<__Atom>` bound. (A concrete
    // group-ful `.unparse()`/`.span()` can still hit a **library-level** leaf gap shared with
    // non-`#[recurse]` group types — delimiter symbols only `Unparse` to a `From<String>` atom, not
    // `TokenTree`; a `Group<(),…>` slot needs `(): Spanned` — but that is orthogonal to recurse. See
    // `ui/recurse_group_ful_unparse.rs`.)
    let delegated_trait = |trait_name: &str| -> HashSet<String> {
        items
            .iter()
            .filter_map(|item| {
                let (id, attrs) = match item {
                    Item::Enum(e) => (e.ident.to_string(), &e.attrs),
                    Item::Struct(s) => (s.ident.to_string(), &s.attrs),
                    _ => return None,
                };
                let idx = *type_to_scc.get(&id)?;
                (!scc_us_natural[idx] && derives_any(attrs, &[trait_name]))
                    .then_some(id)
            })
            .collect()
    };
    let unparse_types = delegated_trait("Unparse");
    let spanned_types = delegated_trait("Spanned");

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
                &parse_types,
                &unparse_types,
                &spanned_types,
                nonce,
            )
        })
        .collect();

    // Emit, for each cycle enum/struct: the natural public type (derives split, `#[ignore_bounds]`
    // injected) AND the internal `pub(crate)` engine type. Non-cycle items (incl. user `impl` blocks on
    // cycle types — now plain impls on the natural type) pass through unchanged.
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
                // Keep Unparse/Spanned on the natural type only for a single self-recursive group-free
                // cycle; engine-route them otherwise.
                let us_natural = scc_us_natural[idx];
                let (natural, engine_paths) = make_natural_item(item, scc, us_natural);
                if scc_needs_engine[idx] {
                    let ctx = &plans[idx].0;
                    let engine = make_engine_item(item, ctx, &engine_paths);
                    out_items.push(quote!(#natural #engine));
                } else {
                    // No engine: just the natural type (+ its direct Unparse/Spanned/Ast).
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

    quote! {
        #(#mod_attrs)* #mod_vis mod #mod_ident {
            #(#out_items)*

            #(#tails)*
        }
    }
    .into()
}

use crate::util::{
    angle, gargs, gparams, indicator, innermost_acc, item_generics, item_ident, method_ident_m, mt,
    param_name, param_use, path_may_denote, peel, to_snake, Container, Head, LayerKind,
};
use proc_macro2::{Span, TokenStream};
use proc_macro_error::abort;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use syn::parse::{Parse, ParseStream, Parser};
use syn::punctuated::Punctuated;
use syn::*;
use template_quote::quote;

mod build_input;
mod discover;
mod entry;
mod lower;
mod params;
mod side;
use build_input::*;
use discover::*;
use entry::*;
use lower::*;
use params::*;
use side::*;

// Preserve the public entry points at `crate::visitor::{entry,build}` for `lib.rs`.
pub(crate) use build_input::build;
pub(crate) use entry::entry;
/// Does any visited type's field reference another visited type while filling a **non-shared** generic
/// param position with something other than that param verbatim (e.g. `Box<Stmt<S, u8>>` where `Stmt`'s
/// `T` is non-shared)? Such a *concrete fill* can't be expressed with the union-of-params trait model
/// (the trait would fix `T`, but the cross-edge needs a specific `T`), so the visitor must instead make
/// the non-shared params **per-method generics** (`visit_stmt<T>`). Closures can't be `for<T>` generic,
/// so that mode is struct-only. The common case (no concrete fill) keeps the union model + closures.
fn has_concrete_fill(targets: &[&DoneType], shared: &HashSet<String>) -> bool {
    // Each visited type's own params, in declaration order (lifetimes precede types/consts, matching how
    // generic *arguments* must be ordered — so args zip directly onto params).
    let params_of: HashMap<String, Vec<GenericParam>> = targets
        .iter()
        .filter_map(|d| {
            let id = item_ident(&d.def)?;
            Some((id.to_string(), gparams(item_generics(&d.def)?)))
        })
        .collect();

    fn ty_fills(
        ty: &Type,
        params_of: &HashMap<String, Vec<GenericParam>>,
        shared: &HashSet<String>,
    ) -> bool {
        match ty {
            Type::Path(tp) => {
                for seg in &tp.path.segments {
                    if let PathArguments::AngleBracketed(ab) = &seg.arguments {
                        // Zip the actual args onto the referenced type's declared params (same order).
                        // Only a non-shared TYPE or CONST param filled with a non-identity arg forces
                        // method-generic mode — a lifetime fill (`Stmt<'static, S>`) is fine in the
                        // union model via subtyping, so it does NOT trigger it.
                        if let Some(decl) = params_of.get(&seg.ident.to_string()) {
                            for (param, arg) in decl.iter().zip(ab.args.iter()) {
                                match (param, arg) {
                                    (GenericParam::Type(tp_), GenericArgument::Type(at)) => {
                                        let pname = tp_.ident.to_string();
                                        let bare = matches!(at, Type::Path(p)
                                            if p.qself.is_none() && p.path.is_ident(&pname));
                                        if !shared.contains(&pname) && !bare {
                                            return true;
                                        }
                                    }
                                    (GenericParam::Const(cp), GenericArgument::Const(ce)) => {
                                        let pname = cp.ident.to_string();
                                        let bare = matches!(ce, syn::Expr::Path(p)
                                            if p.qself.is_none() && p.path.is_ident(&pname));
                                        if !shared.contains(&pname) && !bare {
                                            return true;
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        for arg in &ab.args {
                            if let GenericArgument::Type(at) = arg {
                                if ty_fills(at, params_of, shared) {
                                    return true;
                                }
                            }
                        }
                    }
                }
                false
            }
            Type::Reference(r) => ty_fills(&r.elem, params_of, shared),
            Type::Slice(s) => ty_fills(&s.elem, params_of, shared),
            Type::Array(a) => ty_fills(&a.elem, params_of, shared),
            Type::Paren(p) => ty_fills(&p.elem, params_of, shared),
            Type::Group(g) => ty_fills(&g.elem, params_of, shared),
            Type::Tuple(t) => t.elems.iter().any(|e| ty_fills(e, params_of, shared)),
            _ => false,
        }
    }

    let fields_of = |def: &Item| -> Vec<Type> {
        let mut out = Vec::new();
        match def {
            Item::Enum(e) => {
                for v in &e.variants {
                    out.extend(v.fields.iter().map(|f| f.ty.clone()));
                }
            }
            Item::Struct(s) => out.extend(s.fields.iter().map(|f| f.ty.clone())),
            _ => {}
        }
        out
    };
    targets.iter().any(|d| {
        fields_of(&d.def)
            .iter()
            .any(|t| ty_fills(t, &params_of, shared))
    })
}

/// The per-module tag type threaded through [`syan::visit::Walk`] as its first parameter, so two
/// `visitor!` modules over the same node type do not write conflicting impls.
pub(crate) fn walk_tag() -> Ident {
    Ident::new("__SyanWalkTag", Span::call_site())
}

/// The tag's generic params: the union's lifetimes and type params, but **not** its const params — a
/// const of arbitrary type cannot be mentioned in a struct field, and a node that uses one has it
/// constrained by its own self type anyway.
pub(crate) fn walk_tag_params(g_params: &[GenericParam]) -> Vec<GenericParam> {
    g_params
        .iter()
        .filter(|p| !matches!(p, GenericParam::Const(_)))
        .cloned()
        .collect()
}

/// `<'a, S>` for the tag, matching [`walk_tag_params`].
pub(crate) fn walk_tag_use(g_params: &[GenericParam]) -> TokenStream {
    let args: Vec<TokenStream> = walk_tag_params(g_params)
        .iter()
        .map(|p| match p {
            GenericParam::Lifetime(l) => {
                let lt = &l.lifetime;
                quote!(#lt)
            }
            GenericParam::Type(t) => {
                let id = &t.ident;
                quote!(#id)
            }
            GenericParam::Const(c) => {
                let id = &c.ident;
                quote!(#id)
            }
        })
        .collect();
    angle(&args)
}

/// The tag's `PhantomData` payload, so none of its params is left unused.
pub(crate) fn walk_tag_phantom(g_params: &[GenericParam]) -> TokenStream {
    let parts: Vec<TokenStream> = walk_tag_params(g_params)
        .iter()
        .map(|p| match p {
            GenericParam::Lifetime(l) => {
                let lt = &l.lifetime;
                quote!(& #lt ())
            }
            GenericParam::Type(t) => {
                let id = &t.ident;
                quote!(#id)
            }
            GenericParam::Const(_) => unreachable!("const params are filtered out"),
        })
        .collect();
    quote!( ::core::marker::PhantomData<( #(#parts,)* )> )
}

fn generate_module(st: &BuildInput) -> TokenStream {
    // Every generated name (`visit_*`, `*Hook`, inherent methods) derives from a visited type's
    // last-segment ident, so two visited types sharing a last segment would collide. Catch it here
    // with a clear message instead of a downstream cascade of duplicate-definition errors.
    let mut seg_seen: HashMap<String, String> = HashMap::new();
    for p in &st.visited {
        let seg = last_ident(p).to_string();
        let np = norm_path(p);
        if let Some(prev) = seg_seen.insert(seg.clone(), np.clone()) {
            if prev != np {
                abort!(
                    p,
                    "two visited types share the last segment `{}` (`{}` vs `{}`); their generated \
                     `visit_*`/`*Hook` names would collide — give them distinct final idents",
                    seg,
                    prev,
                    np
                );
            }
        }
    }

    // Map each visited type's last-segment ident -> the full path the user wrote, so the generated
    // module names the visited types by that path (portable: no import needed for absolute paths).
    let path_of: HashMap<String, &Path> = st
        .visited
        .iter()
        .map(|p| (last_ident(p).to_string(), p))
        .collect();
    let visited: HashSet<String> = path_of.keys().cloned().collect();
    // Inherited types, rewritten where the base recorded a path that does not mean the same thing here
    // (see `needs_requalify`) — so a base and an extender at different nesting depths, or in different
    // crates, still name the same type.
    let inherited_paths: Vec<(String, Path)> = st
        .inherited
        .iter()
        .map(|e| {
            let p = match &st.base {
                Some(b) if needs_requalify(&e.path, b) => requalify_ancestor(&e.path, b),
                _ => e.path.clone(),
            };
            (e.key.to_string(), p)
        })
        .collect();
    // Every head this visitor can name: its own targets plus what it inherits. Under this rule a field
    // is followed because its type is *visited*, not because the owning node repeated that fact in
    // `#[subast]`; `#[subast]` is left for the unlisted intermediates only it can name.
    let reachable: HashMap<String, Path> = path_of
        .iter()
        .map(|(k, p)| (k.clone(), (*p).clone()))
        .chain(inherited_paths.iter().cloned())
        .collect();
    let reachable_keys: HashSet<String> = reachable.keys().cloned().collect();
    // Heads that recurse via a `visit_*` method (visited here + inherited from a base); every other
    // followed head is an unlisted intermediate that gets drilled through inline.
    let method_set = st.method_set();
    let done_by_path: HashMap<String, &DoneType> =
        st.done.iter().map(|d| (norm_path(&d.path), d)).collect();

    // Types that get visitor methods (named in `visitor!(..)`); inherited/intermediate types don't.
    let targets: Vec<&DoneType> = st
        .done
        .iter()
        .filter(|d| item_ident(&d.def).is_some_and(|id| visited.contains(&id.to_string())))
        .collect();
    if targets.is_empty() {
        let at = st
            .visited
            .first()
            .map_or_else(Span::call_site, |p| last_ident(p).span());
        abort!(at, "no AST definitions resolved for the visitor");
    }

    // The visitor trait is parameterized by the *union* of every visited type's generic params (+ the
    // base's, when inheriting), so one visitor can span e.g. `Expr<S, Tokens>` and `BinOp<S>`; each
    // type is referenced with its own subset, and `base_g_use` (below) names the base's args by the
    // union's idents for every `base::Visit<..>` reference.
    let mut union_params = param_union(&targets, &st.base_generics);
    sort_lifetimes_first(&mut union_params);

    // Params shared by EVERY visited type (∪ the base's, which must stay trait-level to name
    // `base::Visit<base params>`). A non-shared param appears in only some types.
    let mut shared_names: Option<HashSet<String>> = None;
    for d in &targets {
        let own: HashSet<String> = gparams(item_generics(&d.def).unwrap())
            .iter()
            .map(param_name)
            .collect();
        shared_names = Some(match shared_names {
            None => own,
            Some(acc) => acc.intersection(&own).cloned().collect(),
        });
    }
    let mut shared_names = shared_names.unwrap_or_default();
    for bp in &st.base_generics {
        shared_names.insert(param_name(bp));
    }

    // A union param that some visited type does NOT declare. Such a param can stay a trait param (the
    // union) only while it's *unbounded* — a type lacking it is then harmlessly quantified over it. But a
    // `where`-bounded one (`S: Bound`) can't: applied to items over the union, a type lacking `S` carries
    // an undischargeable `S: Bound`. So a bounded unshared param, like a concrete-filled one, must become
    // a per-method generic with the trait keyed on the shared subset (method-mode, below).
    let unshared_names: HashSet<String> = union_params
        .iter()
        .map(param_name)
        .filter(|n| !shared_names.contains(n))
        .collect();
    let has_bounded_unshared = targets.iter().any(|d| {
        item_where_preds(&d.def)
            .iter()
            .any(|p| where_pred_param(p).is_some_and(|id| unshared_names.contains(&id.to_string())))
    });

    // Heterogeneous mode: a non-shared param is either *concrete-filled* in a cross-edge (e.g.
    // `Stmt<S, u8>`) — which the union-of-params trait can't express — or carries a `where`-bound (above).
    // Make non-shared params per-method generics and go struct-only (no closures — a closure can't be
    // `for<T>` generic). Gated to the no-inheritance case (a recurse/heterogeneous base is out of scope)
    // so the common union+closure path is untouched.
    let method_mode =
        st.base.is_none() && (has_concrete_fill(&targets, &shared_names) || has_bounded_unshared);

    // Trait params: the full union normally; only the shared subset in method-mode (non-shared params
    // become method generics instead).
    let mut g_params: Vec<GenericParam> = if method_mode {
        union_params
            .iter()
            .filter(|p| shared_names.contains(&param_name(p)))
            .cloned()
            .collect()
    } else {
        union_params.clone()
    };
    sort_lifetimes_first(&mut g_params);
    let struct_only = method_mode;
    let by_name: HashMap<String, TokenStream> = union_params
        .iter()
        .map(|p| (param_name(p), param_use(p)))
        .collect();
    let by_name_param: HashMap<String, GenericParam> = g_params
        .iter()
        .map(|p| (param_name(p), p.clone()))
        .collect();
    let base_args: Vec<TokenStream> = st
        .base_generics
        .iter()
        .map(|bp| by_name[&param_name(bp)].clone())
        .collect();
    let base_g_use = angle(&base_args);

    // The full transitive ancestor chain (direct base first), so the new visitor's `Driver` can
    // satisfy *every* supertrait obligation — `mid::Visit: base::Visit` means a `mid => new` visitor
    // must impl both `mid::Visit` and `base::Visit` for its `Driver`. Each ancestor's params are a
    // subset of the union (the base's `@bg` transitively carries its own ancestors' params), looked
    // up by name; each impl is quantified over exactly those params (+ the hook) to avoid E0207.
    let mut chain: Vec<AncIn> = Vec::new();
    if let Some(b) = &st.base {
        chain.push(AncIn {
            path: b.clone(),
            names: st
                .base_generics
                .iter()
                .map(|p| Ident::new(&param_name(p), Span::call_site()))
                .collect(),
        });
        // Requalify transitive ancestors that a `crate::`/`super::`/`self::`-relative *upstream*
        // intermediate recorded, resolving them against the direct base's full path (no-op for
        // same-crate / already-concrete chains). This also re-exports them concrete (the chain feeds
        // `anc_export`), so a further extender inherits resolvable ancestor paths too.
        let cross_crate = base_host_crate(b).is_some();
        for a in &st.base_ancestors {
            let path = if cross_crate {
                requalify_ancestor(&a.path, b)
            } else {
                a.path.clone()
            };
            chain.push(AncIn {
                path,
                names: a.names.clone(),
            });
        }
    }
    let ancestors: Vec<Ancestor> = chain
        .iter()
        .map(|a| {
            let g_params: Vec<GenericParam> = a
                .names
                .iter()
                .filter_map(|n| by_name_param.get(&n.to_string()).cloned())
                .collect();
            let args: Vec<TokenStream> = a
                .names
                .iter()
                .filter_map(|n| by_name.get(&n.to_string()).cloned())
                .collect();
            let g_use = angle(&args);
            let path = &a.path;
            Ancestor {
                path: quote!(#path),
                g_params,
                g_use,
            }
        })
        .collect();

    let g_args: Vec<TokenStream> = g_params.iter().map(param_use).collect();
    let g_def = angle(&g_params);
    let g_use = angle(&g_args);

    // Container-edit usage, populated by the mut walk below; consumed by `gen_side(true, ..)` to decide
    // which `visit_<t>_seq` / `visit_<t>_opt` to emit. Shared by both `Lower`s (only the mut one records).
    let seq_used: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
    let opt_used: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
    // The heads this module emits `Walk` impls for. An inherited head is in `method_set` but not here.
    let target_set: HashSet<String> = targets
        .iter()
        .filter_map(|d| item_ident(&d.def).map(|i| i.to_string()))
        .chain(
            st.done
                .iter()
                .filter_map(|d| item_ident(&d.def).map(|i| i.to_string()))
                .filter(|n| !method_set.contains(n)),
        )
        .collect();
    let inherited_heads: RefCell<Vec<(String, TokenStream, Ident)>> = RefCell::new(Vec::new());
    let walk_tag_args = walk_tag_use(&g_params);
    let mk_lower = |mutable: bool| Lower {
        method_set: &method_set,
        done_by_path: &done_by_path,
        mutable,
        seq_used: &seq_used,
        opt_used: &opt_used,
        walk_tag_args: &walk_tag_args,
        target_set: &target_set,
        inherited_heads: &inherited_heads,
        reachable: &reachable,
        reachable_keys: &reachable_keys,
    };
    let lower = mk_lower(false);
    let lower_mut = mk_lower(true);

    let vtypes: Vec<VType> = targets
        .iter()
        .map(|d| {
            let def = &d.def;
            let ident = item_ident(def).unwrap().clone();
            let own_params = gparams(item_generics(def).unwrap());
            let own_use = angle(&gargs(item_generics(def).unwrap()));
            let own_where = item_where_preds(def);
            // In method-mode, this type's non-shared params become method generics; in union mode the
            // trait already carries every param, so none.
            let method_params: Vec<GenericParam> = if method_mode {
                own_params
                    .iter()
                    .filter(|p| !shared_names.contains(&param_name(p)))
                    .cloned()
                    .collect()
            } else {
                Vec::new()
            };
            let scrut_path: &Path = path_of.get(&ident.to_string()).copied().unwrap_or(&d.path);
            let path_tokens = quote!(#scrut_path);
            let mut stack = Vec::new();
            let body = lower.destructure(def, &d.subast, scrut_path, &quote!(i), 0, &mut stack);
            let mut stack = Vec::new();
            let body_mut =
                lower_mut.destructure(def, &d.subast, scrut_path, &quote!(i), 0, &mut stack);
            VType {
                ident,
                path: path_tokens,
                own_params,
                own_use,
                own_where,
                method_params,
                local: path_is_crate_local(scrut_path),
                body,
                body_mut,
            }
        })
        .collect();

    // The union of every visited type's `where`-predicates (deduped by rendered text — identical
    // predicates from two types are harmless but noisy), applied (as `uw`) to each generated item
    // quantified over the param union so a `enum Expr<S> where S: Bound { .. }` stays well-formed there.
    // In method-mode the *non-shared* params are method generics, not trait params, so a bound on one
    // would reference an undeclared param at the trait level — drop it here; `gen_side` re-attaches it
    // to the per-type `visit_*` method + free fn that actually carries the param.
    let mut seen_pred: HashSet<String> = HashSet::new();
    let union_where: Vec<WherePredicate> = vtypes
        .iter()
        .flat_map(|vt| vt.own_where.iter().cloned())
        .filter(|p| {
            !method_mode
                || where_pred_param(p).is_none_or(|id| !unshared_names.contains(&id.to_string()))
        })
        .filter(|p| seen_pred.insert(quote!(#p).to_string()))
        .collect();

    // A drilled intermediate — reached through `#[subast]` but not listed in `visitor!(..)`, so it has
    // no visit method — gets a `Walk` impl of its own instead of being destructured inline at every use
    // site. Unconditional, like a node's, so a `#[subast]` cycle through one terminates.
    let intermediates: Vec<TokenStream> = st
        .done
        .iter()
        .filter(|d| item_ident(&d.def).is_some_and(|id| !method_set.contains(&id.to_string())))
        .flat_map(|d| {
            let own_params = gparams(item_generics(&d.def).unwrap());
            let own_use = angle(&gargs(item_generics(&d.def).unwrap()));
            let own_where = item_where_preds(&d.def);
            let mut preds = union_where.clone();
            preds.extend(own_where);
            let where_cl = where_clause(&preds);
            let mut params = g_params.clone();
            let known: HashSet<String> = g_params.iter().map(param_name).collect();
            params.extend(own_params.iter().filter(|p| !known.contains(&param_name(p))).cloned());
            sort_lifetimes_first(&mut params);
            let path = &d.path;
            let tag = walk_tag();
            [false, true].map(|m| {
                let lw = if m { &lower_mut } else { &lower };
                let mut stack = Vec::new();
                let body = lw.destructure(&d.def, &d.subast, path, &quote!(self), 0, &mut stack);
                let (tr, f, recv) = if m {
                    (quote!(WalkMut), quote!(walk_mut), quote!(&mut self))
                } else {
                    (quote!(Walk), quote!(walk), quote!(&self))
                };
                quote! {
                    impl< #(#params,)* __SyanW: #{if m { quote!(VisitMut) } else { quote!(Visit) }} #g_use + ?Sized >
                        ::syan::visit::#tr< #tag #{walk_tag_use(&g_params)}, ::syan::visit::indicator::Here, __SyanW >
                        for #path #own_use #where_cl
                    {
                        fn #f(#recv, this: &mut __SyanW) {
                            let _ = &this;
                            #body
                        }
                    }
                }
            })
        })
        .collect();

    let inherited_impls: Vec<TokenStream> = inherited_heads
        .borrow()
        .iter()
        .flat_map(|(_, ty, id)| {
            let tag = walk_tag();
            let targs = walk_tag_use(&g_params);
            let uw = where_clause(&union_where);
            [false, true].map(|m| {
                let (tr, f, recv, vt) = if m {
                    (
                        quote!(WalkMut),
                        quote!(walk_mut),
                        quote!(&mut self),
                        quote!(VisitMut),
                    )
                } else {
                    (quote!(Walk), quote!(walk), quote!(&self), quote!(Visit))
                };
                let m_name = method_ident_m(id, m);
                quote! {
                    impl< #(#g_params,)* __SyanW: #vt #g_use + ?Sized >
                        ::syan::visit::#tr< #tag #targs, ::syan::visit::indicator::Here, __SyanW > for #ty #uw
                    {
                        fn #f(#recv, v: &mut __SyanW) {
                            // Plain method syntax: an inherited `visit_*` is reached through the
                            // supertrait, so it is not a member of this module's own trait.
                            v.#m_name(self)
                        }
                    }
                }
            })
        })
        .collect();

    let seq_used = seq_used.into_inner();
    let opt_used = opt_used.into_inner();

    // A `#[seq]`/`#[opt]` field can only view a type this visitor *targets* (its own `visit_*_seq`/`_opt`
    // is emitted only for `visited` types). A marker pointing at an **inherited** base type would make the
    // descent call a `visit_<t>_seq` that lives nowhere — a cryptic E0599 in generated code. Fail clean.
    if let Some(t) = seq_used
        .iter()
        .chain(opt_used.iter())
        .find(|t| !visited.contains(*t))
    {
        abort!(
            Span::call_site(),
            "a `#[seq]`/`#[opt]` field views the inherited type `{}`; container-edit views are not \
             generated for inherited types (the `visit_{}` method would have nowhere to live). Drop the \
             marker — the field is still traversed, calling the inherited per-node visit for each element.",
            t,
            to_snake(&Ident::new(t, Span::call_site()))
        );
    }

    let [shared, mutable] = [false, true].map(|m| {
        gen_side(
            m,
            &vtypes,
            &g_params,
            &g_args,
            &g_def,
            &g_use,
            &base_g_use,
            &ancestors,
            &st.base,
            &union_where,
            struct_only,
            &seq_used,
            &opt_used,
        )
    });

    // Every visitor module exports its full visited-type set (idents), its generic-param union
    // (`@bg`), and its full ancestor chain (`@an`) so another visitor can inherit it (transitively).
    let anc_export = emit_ancestors(&chain);
    let visited_macro = emit_visited_macro(st, &g_params, anc_export);

    // Items are emitted directly into the enclosing module (where `visitor!(...)` was invoked).
    quote! {
        #visited_macro

        // Bring every ancestor's traits in scope so the generated `Driver` impls / method calls
        // resolve (transitive supertraits included).
        #(for a in &ancestors) {
            #[allow(unused_imports)]
            use #{&a.path}::{Visit as _, VisitMut as _};
        }



        #[doc = "Tag identifying this visitor module in `syan::visit::Walk` impls."]
        pub struct #{walk_tag()} #{angle(&walk_tag_params(&g_params))} ( #{walk_tag_phantom(&g_params)} );

        #(for imp in &intermediates) { #imp }
        #(for imp in &inherited_impls) { #imp }

        #shared
        #mutable
    }
}

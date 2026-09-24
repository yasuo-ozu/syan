use super::*;

/// Generate every item for one mutability "side" (`Visit`/`VisitMut`, etc.).
pub(crate) fn gen_side(
    side: Side,
    m: &Model,
    vtypes: &[VType],
    union_where: &[WherePredicate],
    // Which visited types some AST holds Vec-like / Option-like, and so get a `visit_<t>_seq` /
    // `visit_<t>_opt` container-edit view.
    seq_used: &HashSet<String>,
    opt_used: &HashSet<String>,
) -> TokenStream {
    let Model {
        g_params,
        g_args,
        g_def,
        g_use,
        base,
        base_g_use,
        ancestors,
        ..
    } = m;
    // Heterogeneous (method-generic) mode: a non-shared param is concrete-filled in a cross-edge, so
    // each `visit_*` carries its type's non-shared params as method generics. A closure can't be
    // `for<T>` generic, so the closure machinery (`&mut V` blanket / `Driver`/`Hook`/`Chain`/
    // `IntoVisitor`) is omitted and the inherent `.visit()` takes `&mut impl Visit` directly.
    let struct_only = m.method_mode;
    let id = |s: &str| Ident::new(s, Span::call_site());
    let visit_tr = side.visit_trait();
    let into_vis_tr = side.ty("IntoVisitor");
    let into_hook_tr = side.ty("IntoHook");
    let hook_tr = side.ty("Hook");
    let driver = side.ty("Driver");
    let into_vis_fn = side.func("into_visitor");
    let into_hook_fn = side.func("into_hook");
    let visit_method = side.func("visit");
    let amp = side.amp();
    let recv = side.recv();
    let self_ret = if side.mutable {
        quote!(&mut Self)
    } else {
        quote!(&Self)
    };

    // Generated helper type params, named to avoid collision with the visited types' own generic
    // params (which arrive verbatim via `#g_params`). A visited type may thus declare a param
    // literally named `__V`/`__T`/`__H`/`__F`/`__A`/`__B` (or `__F0`…).
    let reserved: HashSet<String> = g_params.iter().map(param_name).collect();
    let p_v = fresh_ident("__V", &reserved);
    let p_t = fresh_ident("__T", &reserved);
    let p_h = fresh_ident("__H", &reserved);
    let p_f = fresh_ident("__F", &reserved);
    let p_a = fresh_ident("__A", &reserved);
    let p_b = fresh_ident("__B", &reserved);
    // Per-method generics for the container-edit views (`visit_*_seq`/`_opt` take `&mut impl SeqView<T>`
    // / `OptView<T>` — the field type itself, no wrapper).
    let p_vw = fresh_ident("__VW", &reserved);
    let p_ow = fresh_ident("__OW", &reserved);

    /// One container-edit view (`#[seq]` or `#[opt]`) generated for a visited type. Replaces the
    /// paired `has_seq`/`has_opt` bools + `seq_method`/`opt_method` + `seq_doc`/`opt_doc` fields on `S`
    /// with a single `Vec` so `trait_def`/`blanket_ref_impl` iterate once instead of two parallel
    /// `#(if ..)` blocks.
    struct ViewSpec {
        /// `visit_<name>_seq` / `visit_<name>_opt`.
        method: Ident,
        /// Doc string for the trait method (was `S::seq_doc`/`S::opt_doc`).
        doc: String,
        /// `::syan::visit::SeqView` / `::syan::visit::OptView` — bound root, sans `<Ty>`.
        view_trait: TokenStream,
        /// The per-method generic naming the view type: `p_vw` for seq, `p_ow` for opt (the *same*
        /// idents `gen_side` already mints once via `fresh_ident` — not reminted per type/kind).
        view_param: Ident,
        /// Trait-method default body: a `for .. in view_iter(v)` loop for a seq, an
        /// `if let Some(..) = get(v)` for an opt.
        default_body: TokenStream,
    }

    impl ViewSpec {
        /// The view a visited type gets for one container shape. Seq and opt differ in the view
        /// trait, the accessor and the wording; everything downstream of here treats them alike.
        fn new(
            kind: Container,
            side: Side,
            ident: &Ident,
            method: &Ident,
            view_param: Ident,
        ) -> Self {
            let (name, mname) = (ident.to_string(), method.to_string());
            let accessor = side.func(match kind {
                Container::Seq => "view_iter",
                Container::Opt => "get",
            });
            let view_trait = kind.view_trait();
            let doc = match (kind, side.mutable) {
                (Container::Seq, true) => format!(
                    "Structurally edit the `{name}` nodes in a `Vec`-like parent slot via a \
                     [`SeqView`](::syan::visit::SeqView) (`push`/`insert`/`remove`/`retain_mut`/\
                     `view_iter_mut`); default descends each via `{mname}`."
                ),
                (Container::Seq, false) => format!(
                    "Observe the `{name}` nodes in a `Vec`-like parent slot via a \
                     [`SeqView`](::syan::visit::SeqView) (`len`/`get`/`view_iter`) — the slot \
                     itself, not just each element; default descends each via `{mname}`."
                ),
                (Container::Opt, true) => format!(
                    "Structurally edit the `{name}` node in an `Option`-like parent slot via an \
                     [`OptView`](::syan::visit::OptView) (`get_mut`/`set`/`take`); default \
                     descends it via `{mname}`."
                ),
                (Container::Opt, false) => format!(
                    "Observe the `{name}` node in an `Option`-like parent slot via an \
                     [`OptView`](::syan::visit::OptView) (`is_some`/`get`) — the slot itself, \
                     not just the element; default descends it via `{mname}`."
                ),
            };
            let default_body = match kind {
                Container::Seq => quote! {
                    for __syan_e in #view_trait::#accessor(v) {
                        self.#method(__syan_e);
                    }
                },
                Container::Opt => quote! {
                    if let ::core::option::Option::Some(__syan_e) = #view_trait::#accessor(v) {
                        self.#method(__syan_e);
                    }
                },
            };
            ViewSpec {
                method: side.func(&format!("visit_{}_{}", to_snake(ident), kind.word())),
                doc,
                view_trait,
                view_param,
                default_body,
            }
        }
    }

    struct S {
        ty: TokenStream,
        /// The visited type's bare name (e.g. `Expr`), for generated doc comments.
        name: String,
        /// Generated doc for the trait method (`fn visit_<name>`) and the free fn (`visit_<name>`).
        tdoc: String,
        fdoc: String,
        method: Ident,
        /// Container-edit views (`visit_<name>_seq`/`_opt`); emitted only where the type is held that way.
        views: Vec<ViewSpec>,
        /// This type's non-shared params (heterogeneous mode), as the trait method's generics —
        /// lifetimes-first; empty in union mode.
        method_params: Vec<GenericParam>,
        /// The free fn's full generic list (trait params ∪ this type's non-shared params), normalized
        /// lifetimes-first so a non-shared lifetime never lands after a type/const param.
        free_params: Vec<GenericParam>,
        /// `where`-clause for the trait method (`where Self: Sized` in struct-only mode, plus any bound
        /// on this type's method-generic params, e.g. `S: Bound`); empty in the common union mode.
        trait_where: TokenStream,
        /// `where`-clause for the free fn — the union predicates (trait-level) plus this type's
        /// method-generic-param bounds; covers naming `Bounded<S>` where `S` is a method generic.
        free_where: TokenStream,
        hook: Ident,
        hook_struct: Ident,
        body: TokenStream,
    }
    let sides: Vec<S> = vtypes
        .iter()
        .map(|t| {
            let ident = t.ident.clone();
            let own = &t.own_use;
            let path = &t.path;
            let ty = quote!( #path #own );
            let mut method_params = t.method_params.clone();
            sort_lifetimes_first(&mut method_params);
            let mut free_params: Vec<GenericParam> =
                g_params.iter().cloned().chain(t.method_params.iter().cloned()).collect();
            sort_lifetimes_first(&mut free_params);
            // This type's `where`-bounds on its method-generic params (e.g. `S: Bound` when `S` is
            // non-shared). They can't live on the trait (it's keyed on the shared params), so they ride
            // the per-type `visit_*` method + free fn that declares the param.
            let mp_names: HashSet<String> = t.method_params.iter().map(param_name).collect();
            let method_where: Vec<WherePredicate> = t
                .own_where
                .iter()
                .filter(|p| where_pred_param(p).is_some_and(|id| mp_names.contains(&id.to_string())))
                .cloned()
                .collect();
            // Trait method: `where Self: Sized` (struct-only) + the method-param bounds.
            let trait_where = if struct_only {
                let mut preds: Vec<WherePredicate> = vec![parse_quote!(Self: ::core::marker::Sized)];
                preds.extend(method_where.iter().cloned());
                where_clause(&preds)
            } else {
                quote!()
            };
            // Free fn: the trait-level union predicates + this type's method-param bounds.
            let free_where = {
                let mut preds = union_where.to_vec();
                preds.extend(method_where.iter().cloned());
                where_clause(&preds)
            };
            let method = side.method(&ident);
            let name = ident.to_string();
            let mname = method.to_string();
            let tdoc = format!(
                "Visit an `{name}` node; the default recurses via [`{mname}`]. Override to act, calling \
                 `{mname}(self, i)` to keep descending."
            );
            let fdoc = format!(
                "Recurse into an `{name}`'s children, dispatching each to `visit_*{mut_sfx}` \
                 ([`{visit_tr}::{mname}`]'s default delegates here).",
                mut_sfx = side.fn_suffix(),
            );
            let mut views = Vec::new();
            // Both sides get the views. The shared one observes the parent slot — its length, its
            // neighbours, an element's index — which `visit_<t>` alone cannot show; the `&mut` one
            // edits it. Named `_seq`/`_opt` on `Visit` and `_seq_mut`/`_opt_mut` on `VisitMut`.
            for (kind, used, view_param) in [
                (Container::Seq, seq_used, &p_vw),
                (Container::Opt, opt_used, &p_ow),
            ] {
                if used.contains(&name) {
                    views.push(ViewSpec::new(kind, side, &ident, &method, view_param.clone()));
                }
            }
            S {
                ty,
                name,
                tdoc,
                fdoc,
                method,
                views,
                method_params,
                free_params,
                trait_where,
                free_where,
                hook: side.func(&format!("hook_{}", to_snake(&ident))),
                hook_struct: side.ty(&format!("{ident}Hook")),
                body: if side.mutable { t.body_mut.clone() } else { t.body.clone() },
            }
        })
        .collect();

    let tup = tuple_impls(8, g_params, g_args, g_use, side, union_where);
    // The union of every visited type's `where`-predicates, repeated on each generated item that is
    // quantified over the full param union (the trait, free fns, the `&mut V` / Driver / closure /
    // Chain impls) so a visited type like `enum Expr<S> where S: Bound { .. }` stays well-formed.
    let uw = where_clause(union_where);

    let visited_list = sides
        .iter()
        .map(|s| format!("`{}`", s.name))
        .collect::<Vec<_>>()
        .join(", ");
    let entry = visit_method.to_string();
    let trait_doc = format!(
        "Visitor over {visited_list} (generated by `visitor!`). Override the `visit_*{mut_sfx}` methods \
         you care about — each default recurses into that node's children; start with \
         `node.{entry}(&mut visitor)`.{base_note}",
        mut_sfx = side.fn_suffix(),
        base_note = if side.mutable { " The by-`&mut` variant of `Visit`." } else { "" },
    );
    // In method-mode the visited set is heterogeneous, so the trait's methods carry their own
    // generics and a closure cannot implement it (a closure is not `for<T>` generic). Say so here:
    // passing one otherwise fails as a bare `expected &mut _, found closure` type mismatch, which
    // names neither the closure machinery nor the reason it is absent.
    let inherent_doc = if struct_only {
        format!(
            "Visit `self` with any `{visit_tr}`, returning `self` to chain. This visitor is \
             heterogeneous — some visited type fills another's parameter concretely, or bounds it — \
             so `{visit_tr}`'s methods are themselves generic and a **closure cannot be used** \
             here; pass `&mut` a type implementing `{visit_tr}`."
        )
    } else {
        format!("Visit `self` with any `{visit_tr}`, returning `self` to chain.")
    };

    // Inherent `visit` / `visit_mut` per type (replaces the Visitable trait). Each type's own
    // params go on the impl; any extra union params go on the method (so a type that doesn't use
    // every union param doesn't leave the impl param unconstrained). The type's own `where`-clause
    // (referencing only its own params) goes on the impl so naming `Expr<S>` stays well-formed.
    let inherent: Vec<TokenStream> = vtypes
        .iter()
        .map(|vt| {
            // A foreign target can't carry an inherent impl (E0116); callers use `Visit::visit_*`.
            if !vt.local {
                return quote!();
            }
            let own_names: HashSet<String> = vt.own_params.iter().map(param_name).collect();
            let extra: Vec<&GenericParam> = g_params
                .iter()
                .filter(|p| !own_names.contains(&param_name(p)))
                .collect();
            let own_def = angle(&vt.own_params);
            let own_w = where_clause(&vt.own_where);
            let path = &vt.path;
            let own_use = &vt.own_use;
            let method = side.method(&vt.ident);
            if struct_only {
                // Direct `&mut impl Visit` (the closure/`IntoVisitor` machinery is off in method-mode).
                quote! {
                    impl #own_def #path #own_use #own_w {
                        #[doc = #inherent_doc]
                        pub fn #visit_method< #(#extra,)* #p_v: #visit_tr #g_use >(
                            #recv,
                            visitor: &mut #p_v,
                        ) -> #self_ret {
                            visitor.#method(self);
                            self
                        }
                    }
                }
            } else {
                quote! {
                    impl #own_def #path #own_use #own_w {
                        #[doc = #inherent_doc]
                        pub fn #visit_method< #(#extra,)* #p_t >(
                            #recv,
                            visitor: impl #into_vis_tr< #(#g_args,)* #p_t >,
                        ) -> #self_ret {
                            let mut visitor = visitor.#into_vis_fn();
                            visitor.#method(self);
                            self
                        }
                    }
                }
            }
        })
        .collect();

    let trait_def = quote! {
        #[doc = #trait_doc]
        pub trait #visit_tr #g_def #(if let Some(b) = base) { : #b::#visit_tr #base_g_use } #uw {
            #(for s in &sides) {
                // In heterogeneous (struct-only) mode the method carries this type's non-shared params
                // as generics, and `where Self: Sized` (the method-generic dispatch needs a sized Self).
                #[doc = #{&s.tdoc}]
                fn #{&s.method}< #(for mp in &s.method_params) { #mp, } >(&mut self, i: #amp #{&s.ty})
                    #{&s.trait_where}
                {
                    #{&s.method}(self, i)
                }
                #(for spec in &s.views) {
                    #[doc = #{&spec.doc}]
                    fn #{&spec.method}< #(for mp in &s.method_params) { #mp, } #{&spec.view_param}: #{&spec.view_trait}< #{&s.ty} > >(
                        &mut self,
                        v: #amp #{&spec.view_param},
                    ) #{&s.trait_where} {
                        #{&spec.default_body}
                    }
                }
            }
        }
    };

    // A visitor can be reached through a wrapper, or several can ride along together; either way every
    // method forwards to the same place. One shape, three headers: `&mut V` and `Box<V>` forward to
    // their single delegate, a tuple to each element in turn, and the container-edit views ride along
    // with the per-node methods.
    let forwarding_impl = |header: TokenStream, delegates: Vec<(TokenStream, TokenStream)>| {
        quote! {
            #(if !struct_only) {
                #header #uw {
                    #(for s in &sides) {
                        fn #{&s.method}(&mut self, i: #amp #{&s.ty}) {
                            #(for (v, recv) in &delegates) {
                                <#v as #visit_tr #g_use>::#{&s.method}(#recv, i);
                            }
                        }
                        #(for spec in &s.views) {
                            fn #{&spec.method}< #{&spec.view_param}: #{&spec.view_trait}< #{&s.ty} > >(&mut self, v: #amp #{&spec.view_param}) {
                                #(for (d, recv) in &delegates) {
                                    <#d as #visit_tr #g_use>::#{&spec.method}(#recv, v);
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    // The receiver a caller passes to `node.visit(&mut pass)`.
    let blanket_ref_impl = forwarding_impl(
        quote!( impl< #(#g_params,)* #p_v: #visit_tr #g_use > #visit_tr #g_use for &mut #p_v ),
        vec![(quote!(#p_v), quote!(self))],
    );

    // A boxed visitor is a visitor, so `node.visit(Box::new(pass))` works. Written for `Box`
    // specifically rather than blanketed over a wrapper trait: such a blanket would overlap the tuple
    // impls below, since this crate is upstream of the generated module and could later implement that
    // trait for a tuple.
    let boxed_visitor_impl = forwarding_impl(
        quote!( impl< #(#g_params,)* #p_v: #visit_tr #g_use + ?Sized > #visit_tr #g_use for ::std::boxed::Box<#p_v> ),
        vec![(quote!(#p_v), quote!(&mut **self))],
    );

    // A tuple of visitors is a visitor: every element sees every node, in one traversal. Arity 2..=8,
    // mirroring the closure-tuple `IntoVisitor` impls.
    let tuple_visit_impls: Vec<TokenStream> = (2..=8usize)
        .map(|n| {
            let ps: Vec<Ident> = (0..n).map(|k| id(&format!("__SyanV{k}"))).collect();
            let delegates = ps
                .iter()
                .enumerate()
                .map(|(k, p)| {
                    let k = syn::Index::from(k);
                    (quote!(#p), quote!(&mut self.#k))
                })
                .collect();
            forwarding_impl(
                quote!( impl< #(#g_params,)* #(for p in &ps) { #p: #visit_tr #g_use, } > #visit_tr #g_use for ( #(#ps,)* ) ),
                delegates,
            )
        })
        .collect();

    // Each visited node implements `Walk`/`WalkMut` by handing itself to the visitor. Unconditional
    // (no `FieldTy: Walk` predicates): those belong on the free fns, and putting them here makes a
    // mutually recursive AST overflow the trait solver instead of terminating.
    let walk_tr = side.walk_trait();
    let walk_fn = side.walk_fn();
    let walk_recv = side.recv();
    let tag = walk_tag();
    let tag_args = walk_tag_use(g_params);
    let walk_impls: Vec<TokenStream> = sides
        .iter()
        .map(|s| {
            quote! {
                impl< #(for gp in &s.free_params) { #gp, } #p_v: #visit_tr #g_use #(if !struct_only) { + ?Sized } >
                    ::syan::visit::#walk_tr< #tag #tag_args, ::syan::visit::indicator::Here, #p_v >
                    for #{&s.ty} #{&s.free_where}
                {
                    fn #walk_fn(#walk_recv, v: &mut #p_v) {
                        <#p_v as #visit_tr #g_use>::#{&s.method}(v, self)
                    }
                }
            }
        })
        .collect();

    let free_fns = quote! {
        #(for s in &sides) {
            // No `?Sized` under struct-only: the body may dispatch through `Self`'s method-generic
            // `visit_*` (which requires `Self: Sized`). `free_params` = trait params ∪ this type's
            // non-shared params, lifetimes-first.
            #[doc = #{&s.fdoc}]
            pub fn #{&s.method}< #(for gp in &s.free_params) { #gp, } #p_v: #visit_tr #g_use #(if !struct_only) { + ?Sized } >(
                this: &mut #p_v,
                i: #amp #{&s.ty},
            ) #{&s.free_where} {
                #{&s.body}
            }
            // The `visit_*_seq`/`_opt` container-edit views have no free fn: their default descent is
            // inlined into the trait-method default (they just iterate the view calling `visit_*_mut`).
        }
    };

    let closure_machinery = quote! {
        #(if !struct_only) {
        pub trait #into_vis_tr< #(#g_params,)* #p_t > #uw {
            fn #into_vis_fn(self) -> impl #visit_tr #g_use;
        }
        impl< #(#g_params,)* #p_v: #visit_tr #g_use > #into_vis_tr< #(#g_args,)* () > for #p_v #uw {
            fn #into_vis_fn(self) -> impl #visit_tr #g_use { self }
        }

        // Closures: shallow Hook + single-pass Driver. Implementation detail of the closure
        // adapters — a user names `IntoVisitor` (via `node.visit(..)`), never these.
        #[doc(hidden)]
        pub trait #hook_tr #g_def #uw {
            #(for s in &sides) {
                fn #{&s.hook}(&mut self, i: #amp #{&s.ty}) { let _ = i; }
            }
        }
        #[doc(hidden)]
        pub trait #into_hook_tr< #(#g_params,)* #p_t > #uw {
            fn #into_hook_fn(self) -> impl #hook_tr #g_use;
        }

        #[doc(hidden)]
        pub struct #driver<#p_h>(pub #p_h);
        impl< #(#g_params,)* #p_h: #hook_tr #g_use > #visit_tr #g_use for #driver<#p_h> #uw {
            #(for s in &sides) {
                fn #{&s.method}(&mut self, i: #amp #{&s.ty}) {
                    self.0.#{&s.hook}(i);
                    #{&s.method}(self, i);
                }
            }
        }
        // The new trait extends the base (transitively), so Driver must satisfy *every* ancestor
        // supertrait (via their defaults). Each empty impl is quantified over only that ancestor's
        // params (+ the wrapped hook) so a wider new-union param is not an unconstrained impl param.
        #(for a in ancestors) {
            impl< #(for p in &a.g_params) { #p, } #p_h >
                #{&a.path}::#visit_tr #{&a.g_use} for #driver<#p_h> {}
        }

        #(for s in &sides) {
            #[doc(hidden)]
            pub struct #{&s.hook_struct}<#p_f>(pub #p_f);
            impl< #(#g_params,)* #p_f: ::core::ops::FnMut( #amp #{&s.ty} ) >
                #hook_tr #g_use for #{&s.hook_struct}<#p_f> #uw
            {
                fn #{&s.hook}(&mut self, i: #amp #{&s.ty}) { (self.0)(i); }
            }
            impl< #(#g_params,)* #p_f: ::core::ops::FnMut( #amp #{&s.ty} ) >
                #into_hook_tr< #(#g_args,)* #{&s.ty} > for #p_f #uw
            {
                fn #into_hook_fn(self) -> impl #hook_tr #g_use { #{&s.hook_struct}(self) }
            }
            impl< #(#g_params,)* #p_f: ::core::ops::FnMut( #amp #{&s.ty} ) >
                #into_vis_tr< #(#g_args,)* #{&s.ty} > for #p_f #uw
            {
                fn #into_vis_fn(self) -> impl #visit_tr #g_use { #driver(#{&s.hook_struct}(self)) }
            }
        }

        // Multiple closures: a 2-tuple of hooks is itself a hook (calls both), so it is the
        // tuple-of-closures combinator directly — no `Chain` newtype. `build_chain` nests them right.
        impl< #(#g_params,)* #p_a: #hook_tr #g_use, #p_b: #hook_tr #g_use >
            #hook_tr #g_use for ( #p_a, #p_b ) #uw
        {
            #(for s in &sides) {
                fn #{&s.hook}(&mut self, i: #amp #{&s.ty}) {
                    self.0.#{&s.hook}(i);
                    self.1.#{&s.hook}(i);
                }
            }
        }
        #(for imp in &tup) { #imp }
        } // end #(if !struct_only) — closure/Driver machinery off for a recurse base
    };

    quote! {
        #trait_def
        #(for imp in &walk_impls) { #imp }
        #blanket_ref_impl
        #boxed_visitor_impl
        #(for imp in &tuple_visit_impls) { #imp }
        #free_fns
        #closure_machinery
        // Inherent entry points (no trait import needed at the call site).
        #(for imp in &inherent) { #imp }
    }
}

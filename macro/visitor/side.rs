use super::*;

/// Generate every item for one mutability "side" (`Visit`/`VisitMut`, etc.).
#[allow(clippy::too_many_arguments)]
pub(crate) fn gen_side(
    mutable: bool,
    vtypes: &[VType],
    g_params: &[GenericParam],
    g_args: &[TokenStream],
    g_def: &TokenStream,
    g_use: &TokenStream,
    base_g_use: &TokenStream,
    ancestors: &[Ancestor],
    base: &Option<Path>,
    union_where: &[WherePredicate],
    // Heterogeneous (method-generic) mode: a non-shared param is concrete-filled in a cross-edge, so
    // each `visit_*` carries its type's non-shared params as method generics. A closure can't be
    // `for<T>` generic, so the closure machinery (`&mut V` blanket / `Driver`/`Hook`/`Chain`/
    // `IntoVisitor`) is omitted and the inherent `.visit()` takes `&mut impl Visit` directly.
    struct_only: bool,
    // (mut side) which visited types are held Vec-like / Option-like by some AST → get a
    // `visit_<t>_seq` / `visit_<t>_opt` container-edit method. Ignored on the immutable side.
    seq_used: &HashSet<String>,
    opt_used: &HashSet<String>,
) -> TokenStream {
    let suffix = if mutable { "Mut" } else { "" };
    let id = |s: &str| Ident::new(s, Span::call_site());
    let visit_tr = id(&format!("Visit{suffix}"));
    let into_vis_tr = id(&format!("IntoVisitor{suffix}"));
    let into_hook_tr = id(&format!("IntoHook{suffix}"));
    let hook_tr = id(&format!("Hook{suffix}"));
    let driver = id(&format!("Driver{suffix}"));
    let into_vis_fn = id(&format!("into_visitor{}", mt(mutable)));
    let into_hook_fn = id(&format!("into_hook{}", mt(mutable)));
    let visit_method = id(&format!("visit{}", mt(mutable)));
    let amp = if mutable { quote!(&mut) } else { quote!(&) };
    let recv = if mutable { quote!(&mut self) } else { quote!(&self) };
    let self_ret = if mutable { quote!(&mut Self) } else { quote!(&Self) };

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

    struct S {
        ty: TokenStream,
        /// The visited type's bare name (e.g. `Expr`), for generated doc comments.
        name: String,
        /// Generated doc for the trait method (`fn visit_<name>`) and the free fn (`visit_<name>`).
        tdoc: String,
        fdoc: String,
        /// Docs for the `visit_mut`-side container-edit methods (`visit_<name>_seq`/`_opt`).
        seq_doc: String,
        opt_doc: String,
        method: Ident,
        /// Container-edit methods (`visit_<name>_seq`/`_opt`); emitted only when `has_seq`/`has_opt`.
        seq_method: Ident,
        opt_method: Ident,
        /// Whether some AST holds this type Vec-like / Option-like (drives emission of `seq`/`opt`).
        has_seq: bool,
        has_opt: bool,
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
            let name = ident.to_string();
            let mname = method_ident_m(&ident, mutable).to_string();
            let tdoc = format!(
                "Visit an `{name}` node; the default recurses via [`{mname}`]. Override to act, calling \
                 `{mname}(self, i)` to keep descending."
            );
            let fdoc = format!(
                "Recurse into an `{name}`'s children, dispatching each to `visit_*{mut_sfx}` \
                 ([`{visit_tr}::{mname}`]'s default delegates here).",
                mut_sfx = mt(mutable),
            );
            let seq_doc = format!(
                "Structurally edit the `{name}` nodes in a `Vec`-like parent slot via a \
                 [`SeqView`](::syan::visit::SeqView) (`push`/`insert`/`remove`/`retain_mut`/`view_iter_mut`); \
                 default descends each via `{mname}`."
            );
            let opt_doc = format!(
                "Structurally edit the `{name}` node in an `Option`-like parent slot via an \
                 [`OptView`](::syan::visit::OptView) (`get_mut`/`set`/`take`); default descends it via `{mname}`."
            );
            let has_seq = mutable && seq_used.contains(&name);
            let has_opt = mutable && opt_used.contains(&name);
            S {
                ty,
                name,
                tdoc,
                fdoc,
                seq_doc,
                opt_doc,
                method: method_ident_m(&ident, mutable),
                seq_method: Ident::new(&format!("visit_{}_seq", to_snake(&ident)), Span::call_site()),
                opt_method: Ident::new(&format!("visit_{}_opt", to_snake(&ident)), Span::call_site()),
                has_seq,
                has_opt,
                method_params,
                free_params,
                trait_where,
                free_where,
                hook: Ident::new(
                    &format!("hook_{}{}", to_snake(&ident), mt(mutable)),
                    Span::call_site(),
                ),
                hook_struct: Ident::new(&format!("{ident}Hook{suffix}"), Span::call_site()),
                body: if mutable { t.body_mut.clone() } else { t.body.clone() },
            }
        })
        .collect();

    let tup = tuple_impls(8, g_params, g_args, g_use, mutable, union_where);
    // The union of every visited type's `where`-predicates, repeated on each generated item that is
    // quantified over the full param union (the trait, free fns, the `&mut V` / Driver / closure /
    // Chain impls) so a visited type like `enum Expr<S> where S: Bound { .. }` stays well-formed.
    let uw = where_clause(union_where);

    // Generated API docs.
    let visited_list = sides.iter().map(|s| format!("`{}`", s.name)).collect::<Vec<_>>().join(", ");
    let entry = visit_method.to_string();
    let trait_doc = format!(
        "Visitor over {visited_list} (generated by `visitor!`). Override the `visit_*{mut_sfx}` methods \
         you care about — each default recurses into that node's children; start with \
         `node.{entry}(&mut visitor)`.{base_note}",
        mut_sfx = mt(mutable),
        base_note = if mutable { " The by-`&mut` variant of `Visit`." } else { "" },
    );
    let inherent_doc = format!("Visit `self` with any `{visit_tr}`, returning `self` to chain.");

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
            let method = method_ident_m(&vt.ident, mutable);
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

    // Assembled from named token-blocks below (all share this fn's locals / `sides`) for readability;
    // the final `quote!` splices them verbatim, so the emitted tokens are identical to one big block.
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
                // Opt-in container-edit hooks (mut side; emitted only where the type is held that way).
                // Default: descend each held node via `visit_*_mut`. Override to restructure the parent.
                #(if s.has_seq) {
                    #[doc = #{&s.seq_doc}]
                    fn #{&s.seq_method}< #(for mp in &s.method_params) { #mp, } #p_vw: ::syan::visit::SeqView< #{&s.ty} > >(
                        &mut self,
                        v: &mut #p_vw,
                    ) #{&s.trait_where} {
                        for __syan_e in ::syan::visit::SeqView::view_iter_mut(v) {
                            self.#{&s.method}(__syan_e);
                        }
                    }
                }
                #(if s.has_opt) {
                    #[doc = #{&s.opt_doc}]
                    fn #{&s.opt_method}< #(for mp in &s.method_params) { #mp, } #p_ow: ::syan::visit::OptView< #{&s.ty} > >(
                        &mut self,
                        v: &mut #p_ow,
                    ) #{&s.trait_where} {
                        if let ::core::option::Option::Some(__syan_e) = ::syan::visit::OptView::get_mut(v) {
                            self.#{&s.method}(__syan_e);
                        }
                    }
                }
            }
        }
    };

    let blanket_ref_impl = quote! {
        #(if !struct_only) {
            impl< #(#g_params,)* #p_v: #visit_tr #g_use > #visit_tr #g_use for &mut #p_v #uw {
                #(for s in &sides) {
                    fn #{&s.method}(&mut self, i: #amp #{&s.ty}) {
                        <#p_v as #visit_tr #g_use>::#{&s.method}(self, i)
                    }
                    #(if s.has_seq) {
                        fn #{&s.seq_method}< #p_vw: ::syan::visit::SeqView< #{&s.ty} > >(&mut self, v: &mut #p_vw) {
                            <#p_v as #visit_tr #g_use>::#{&s.seq_method}(self, v)
                        }
                    }
                    #(if s.has_opt) {
                        fn #{&s.opt_method}< #p_ow: ::syan::visit::OptView< #{&s.ty} > >(&mut self, v: &mut #p_ow) {
                            <#p_v as #visit_tr #g_use>::#{&s.opt_method}(self, v)
                        }
                    }
                }
            }
        }
    };

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
                // Bring the view methods into scope (unnamed) so a `View`-level descent resolves
                // `view_iter[_mut]()` to `SeqView`/`OptView` by the compiler — no container name is named.
                #[allow(unused_imports)]
                use ::syan::visit::{OptView as _, SeqView as _};
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

        // Closures: shallow Hook + single-pass Driver.
        pub trait #hook_tr #g_def #uw {
            #(for s in &sides) {
                fn #{&s.hook}(&mut self, i: #amp #{&s.ty}) { let _ = i; }
            }
        }
        pub trait #into_hook_tr< #(#g_params,)* #p_t > #uw {
            fn #into_hook_fn(self) -> impl #hook_tr #g_use;
        }

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
        #blanket_ref_impl
        #free_fns
        #closure_machinery
        // Inherent entry points (no trait import needed at the call site).
        #(for imp in &inherent) { #imp }
    }
}



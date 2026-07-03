use super::*;

/// Emit, for ONE recursion root, the **owned terminator** + `Parse` re-entry backing unbounded `Parse`:
/// `__XxxTerm(Box<Root<…>>)` and a `Parse` impl that — instead of erroring at the depth floor — looks up a
/// registered fn pointer (`__reentry_X`, the top-level parse monomorphized at the type-erased
/// `&mut dyn ParseStream`) and calls it, so parsing recurses to any runtime depth. The terminator carries
/// NO `Root: Parse` bound — that would re-form the E0275 where-cycle the engine exists to break; re-entry
/// is resolved dynamically. Only emitted when the cycle derives `Parse`. See `core::parse::vtable`.
pub(crate) fn emit_terminator_and_reentry(
    items: &[Item],
    root_name: &str,
    nonce: u64,
    derives_parse: bool,
) -> TokenStream {
    let term = term_name(root_name, nonce);
    let reentry = reentry_name(root_name, nonce);
    let refn = reentry_fn_alias(root_name, nonce);
    let root_id = Ident::new(root_name, Span::call_site());
    let g = item_generics(items, root_name);
    let (gen_bf, gen_use) = generic_tokens(&g);
    let root_decl = param_decls(&g); // bound-preserving (`S: Span`) — the struct names `Root<S>`
    let rwp = where_preds(&g);
    let has_gen = !gen_bf.is_empty();
    let targs: TokenStream = if has_gen { quote!( <#(#gen_use),*> ) } else { quote!() };

    let decl = if has_gen {
        quote! {
            pub struct #term<#(#root_decl),*>(::std::boxed::Box<#root_id<#(#gen_use),*>>)
            #(if !rwp.is_empty()) { where #(#rwp),* } ;
        }
    } else {
        quote!( pub struct #term(::std::boxed::Box<#root_id>); )
    };

    if !derives_parse {
        // U/S-only group-ful cycle: the owned terminator is never constructed (group-ful U/S use the
        // borrow terminator); emit just the struct so the owned depth chain names a real type.
        return decl;
    }

    quote! {
        #decl

        // `+ '_` carries the stream's (non-`'static`) borrow (becomes higher-ranked in the alias).
        #[allow(non_camel_case_types)]
        type #refn<#(#gen_bf,)* __Atom, __E> =
            fn(&mut (dyn ::syan::parse::ParseStream<Atom = __Atom, Error = __E> + '_))
                -> ::core::result::Result<#root_id #targs, ::syan::error::ParseError>;

        // The erased re-entry parser: the top-level parse monomorphized at the dyn stream type (the
        // blanket `&mut dyn ParseStream: IntoParseStream` lets `Root::parse` accept it).
        #[allow(non_snake_case)]
        fn #reentry<#(#root_decl,)* __Atom, __E>(
            __s: &mut (dyn ::syan::parse::ParseStream<Atom = __Atom, Error = __E> + '_),
        ) -> ::core::result::Result<#root_id #targs, ::syan::error::ParseError>
        where
            __Atom: ::syan::span::Spanned + ::core::clone::Clone,
            #root_id #targs: ::syan::parse::Parse<__Atom, Error = ::syan::error::ParseError>,
            #(#rwp,)*
        {
            <#root_id #targs as ::syan::parse::Parse<__Atom>>::parse(__s)
        }

        impl<#(#root_decl,)* __Atom> ::syan::parse::Parse<__Atom> for #term #targs
        where
            __Atom: ::syan::span::Spanned + ::core::clone::Clone,
            #(#rwp,)*
        {
            type Error = ::syan::error::ParseError;
            fn parse(
                __stream: impl ::syan::parse::IntoParseStream<Atom = __Atom>,
            ) -> ::core::result::Result<Self, Self::Error> {
                // Inner `__run` names the concrete stream type `__St`, so we can spell `__St::Error`.
                fn __run<#(#root_decl,)* __Atom, __St>(
                    mut __st: __St,
                ) -> ::core::result::Result<#term #targs, ::syan::error::ParseError>
                where
                    __Atom: ::syan::span::Spanned + ::core::clone::Clone,
                    __St: ::syan::parse::ParseStream<Atom = __Atom>,
                    #(#rwp,)*
                {
                    let __raw = ::syan::parse::vtable::lookup::<
                        ::syan::parse::vtable::ReKey<#term #targs, __Atom, __St::Error>,
                    >();
                    // SAFETY: the (terminator, atom, error) key always stores exactly this concrete fn
                    // type (the delegated `Parse` registered it at this key before descending).
                    let __f: #refn<#(#gen_use,)* __Atom, __St::Error> =
                        unsafe { ::core::mem::transmute::<usize, #refn<#(#gen_use,)* __Atom, __St::Error>>(__raw) };
                    let __dyns: &mut (dyn ::syan::parse::ParseStream<Atom = __Atom, Error = __St::Error> + '_) =
                        &mut __st;
                    ::core::result::Result::Ok(#term(::std::boxed::Box::new(__f(__dyns)?)))
                }
                __run::<#(#gen_use,)* __Atom, _>(__stream.into_parse_stream())
            }
        }
    }
}

/// Emit, for ONE recursion root, the **borrow terminator** backing **unbounded group-ful
/// `Unparse`/`Spanned`**: `__XxxTermRef<'a, …>(&'a Root<…>)` (borrows the natural remainder — no clone, no
/// `Root: Clone`), its `__FromNat` (just wraps the borrow), and its `Unparse`/`Spanned`, which — instead of
/// panicking at the depth floor — **re-enter the top-level natural `Unparse`/`Spanned` at runtime** through
/// a type-erased fn pointer (`core::parse::vtable`). `Unparse` erases the sink to `&mut dyn Emitter` (a
/// `DynSink` re-wraps it for the generic `unparse<E>`); `Spanned` needs no erasure. NO static
/// `Root: Unparse/Spanned` bound here (that would re-form the group where-cycle the engine breaks).
pub(crate) fn emit_borrow_terminator_and_reentry(
    items: &[Item],
    root_name: &str,
    nonce: u64,
    needs_unparse: bool,
    needs_spanned: bool,
) -> TokenStream {
    let term_ref = term_ref_name(root_name, nonce);
    let ftn = from_nat_name(root_name, nonce);
    let root_id = Ident::new(root_name, Span::call_site());
    let g = item_generics(items, root_name);
    let (gen_bf, gen_use) = generic_tokens(&g);
    let root_decl = param_decls(&g);
    let rwp = where_preds(&g);
    let span_param = g.params.iter().find_map(|p| match p {
        GenericParam::Type(t) => Some(t.ident.clone()),
        _ => None,
    });

    // The borrow terminator + its (borrow) `__FromNat` (always, when the cycle delegates U/S). The struct
    // names `Root<…>`, so it carries the root's param bounds (`root_decl`, e.g. `S: Span`) + where-clause.
    let mut out = quote! {
        pub struct #term_ref<'__n, #(#root_decl),*>(&'__n #root_id<#(#gen_use),*>)
        #(if !rwp.is_empty()) { where #(#rwp),* } ;

        impl<'__n, #(#root_decl),*> #ftn<'__n, #(#gen_use),*> for #term_ref<'__n, #(#gen_use),*>
        #(if !rwp.is_empty()) { where #(#rwp),* }
        {
            fn __from_nat(__nat: &'__n #root_id<#(#gen_use),*>) -> Self { #term_ref(__nat) }
        }
    };

    if needs_unparse {
        let re_un = reentry_unparse_name(root_name, nonce);
        out.extend(quote! {
            // The erased re-entry: the top-level natural unparse, monomorphized at the erased emitter.
            #[allow(non_snake_case)]
            fn #re_un<#(#root_decl,)* __Atom, __E>(
                __e: &#root_id<#(#gen_use),*>,
                __sink: &mut (dyn ::syan::parse::unparse::Emitter<__Atom, Error = __E> + '_),
            ) -> ::core::result::Result<(), __E>
            where
                #root_id<#(#gen_use),*>: ::syan::parse::Unparse<__Atom>,
                #(#rwp,)*
            {
                <#root_id<#(#gen_use),*> as ::syan::parse::Unparse<__Atom>>::unparse(
                    __e,
                    &mut ::syan::parse::vtable::DynSink(__sink),
                )
            }

            // Borrow terminator `Unparse`: re-enter at runtime via the registry (no static `Root: Unparse`).
            impl<'__n, #(#root_decl,)* __Atom> ::syan::parse::Unparse<__Atom> for #term_ref<'__n, #(#gen_use),*>
            #(if !rwp.is_empty()) { where #(#rwp),* }
            {
                fn unparse<__E: ::syan::parse::unparse::Emitter<__Atom>>(
                    &self,
                    __sink: &mut __E,
                ) -> ::core::result::Result<(), __E::Error> {
                    let __raw = ::syan::parse::vtable::lookup::<
                        ::syan::parse::vtable::ReKey<#root_id<#(#gen_use),*>, __Atom, __E::Error>,
                    >();
                    type __ReUnFn<#(#gen_bf,)* __Atom, __E> = fn(
                        &#root_id<#(#gen_use),*>,
                        &mut (dyn ::syan::parse::unparse::Emitter<__Atom, Error = __E> + '_),
                    ) -> ::core::result::Result<(), __E>;
                    // SAFETY: the (root, atom, error) key always stores exactly this concrete fn type.
                    let __f: __ReUnFn<#(#gen_use,)* __Atom, __E::Error> =
                        unsafe { ::core::mem::transmute::<usize, __ReUnFn<#(#gen_use,)* __Atom, __E::Error>>(__raw) };
                    let __dyns: &mut (dyn ::syan::parse::unparse::Emitter<__Atom, Error = __E::Error> + '_) = __sink;
                    __f(self.0, __dyns)
                }
            }
        });
    }

    if needs_spanned {
        if let Some(sp) = &span_param {
            let re_sp = reentry_span_name(root_name, nonce);
            out.extend(quote! {
                #[allow(non_snake_case)]
                fn #re_sp<#(#root_decl),*>(__e: &#root_id<#(#gen_use),*>) -> #sp
                where
                    #root_id<#(#gen_use),*>: ::syan::span::Spanned<Span = #sp>,
                    #(#rwp,)*
                {
                    <#root_id<#(#gen_use),*> as ::syan::span::Spanned>::span(__e)
                }

                impl<'__n, #(#root_decl),*> ::syan::span::Spanned for #term_ref<'__n, #(#gen_use),*>
                where #sp: ::syan::span::Span, #(#rwp,)*
                {
                    type Span = #sp;
                    fn span(&self) -> Self::Span {
                        let __raw = ::syan::parse::vtable::lookup::<
                            ::syan::parse::vtable::ReKey<#root_id<#(#gen_use),*>, ::syan::parse::vtable::SpanReentry, #sp>,
                        >();
                        type __ReSpFn<#(#gen_bf,)* __Sp> = fn(&#root_id<#(#gen_use),*>) -> __Sp;
                        // SAFETY: the (root, SpanReentry, span) key always stores exactly this fn type.
                        let __f: __ReSpFn<#(#gen_use,)* #sp> =
                            unsafe { ::core::mem::transmute::<usize, __ReSpFn<#(#gen_use,)* #sp>>(__raw) };
                        __f(self.0)
                    }
                }
            });
        }
    }
    out
}

/// Emit the delegated `impl Unparse for X` — the **unbounded** group-ful variant. Registers each root's
/// erased re-entry unparse fn into `core::parse::vtable`, builds the DEPTH-1 **borrow** engine
/// (`__XRec<…, __XxxTermRef<'_, …>>` — leaves cloned, recursive children borrowed), then unparses it. The
/// borrow engine's terminator re-enters at runtime, giving unbounded depth with no `Root: Clone`.
pub(crate) fn emit_delegated_unparse(
    tg: &DelegTarget,
    roots: &[RootReentry],
    root_use: &[TokenStream],
    root_targs: &TokenStream,
    self_name: &str,
    nonce: u64,
) -> TokenStream {
    let DelegTarget { id, xdecl, xuse, where_preds, .. } = *tg;
    let engine = engine_name(self_name, nonce);
    let ftn = from_nat_name(self_name, nonce);
    // The borrow engine instantiation: `__XRec<xuse, __<root>TermRef<'_, root_use> …>` (one per root). `'_b`
    // forms the HRTB bound; `'_` is inferred (to the `&self` borrow) in the body.
    let tr_args = |lt: TokenStream| -> Vec<TokenStream> {
        roots
            .iter()
            .map(|r| {
                let tr = term_ref_name(&r.name, nonce);
                quote!( #tr<#lt, #(#root_use),*> )
            })
            .collect()
    };
    let tr_b = tr_args(quote!('__b));
    let tr_anon = tr_args(quote!('_));
    // `Root: Unparse<__Atom>` for every OTHER root (so the `reentry_unparse::<…> as usize` cast type-checks);
    // the self root's bound is assumed inside its own impl body.
    let from_bounds: Vec<TokenStream> = roots
        .iter()
        .filter(|r| r.name != self_name)
        .map(|r| {
            let rid = &r.root_id;
            quote!( #rid #root_targs: ::syan::parse::Unparse<__Atom> )
        })
        .collect();
    let registrations: Vec<TokenStream> = roots
        .iter()
        .map(|r| {
            let RootReentry { root_id, name, .. } = r;
            let re_un = reentry_unparse_name(name, nonce);
            quote! {
                ::syan::parse::vtable::register::<
                    ::syan::parse::vtable::ReKey<#root_id #root_targs, __Atom, __E::Error>,
                >(#re_un::<#(#root_use,)* __Atom, __E::Error> as usize);
            }
        })
        .collect();
    quote! {
        impl<#(#xdecl,)* __Atom> ::syan::parse::Unparse<__Atom> for #id<#(#xuse),*>
        where
            for<'__b> #engine<#(#xuse,)* #(#tr_b),*>:
                ::syan::parse::Unparse<__Atom> + #ftn<'__b, #(#xuse),*>,
            #(#from_bounds,)*
            #(#where_preds,)*
        {
            fn unparse<__E: ::syan::parse::unparse::Emitter<__Atom>>(
                &self,
                __sink: &mut __E,
            ) -> ::core::result::Result<(), __E::Error> {
                #(#registrations)*
                let __e: #engine<#(#xuse,)* #(#tr_anon),*> =
                    <#engine<#(#xuse,)* #(#tr_anon),*> as #ftn<'_, #(#xuse),*>>::__from_nat(self);
                <#engine<#(#xuse,)* #(#tr_anon),*> as ::syan::parse::Unparse<__Atom>>::unparse(&__e, __sink)
            }
        }
    }
}

/// Emit the delegated `impl Spanned for X` — the **unbounded** group-ful variant (analogue of
/// `emit_delegated_unparse`, no emitter to erase). Registers each root's erased re-entry span fn, builds
/// the depth-1 borrow engine, and folds its span.
pub(crate) fn emit_delegated_spanned(
    tg: &DelegTarget,
    roots: &[RootReentry],
    root_use: &[TokenStream],
    root_targs: &TokenStream,
    self_name: &str,
    nonce: u64,
) -> TokenStream {
    let DelegTarget { id, xdecl, xuse, span_param, where_preds, .. } = *tg;
    let sp = span_param.expect("Spanned delegation requires the cycle's span type param");
    let engine = engine_name(self_name, nonce);
    let ftn = from_nat_name(self_name, nonce);
    let tr_args = |lt: TokenStream| -> Vec<TokenStream> {
        roots
            .iter()
            .map(|r| {
                let tr = term_ref_name(&r.name, nonce);
                quote!( #tr<#lt, #(#root_use),*> )
            })
            .collect()
    };
    let tr_b = tr_args(quote!('__b));
    let tr_anon = tr_args(quote!('_));
    // `Root: Spanned<Span=#sp>` for every OTHER root (the self root's is assumed in its own impl body).
    let span_bounds: Vec<TokenStream> = roots
        .iter()
        .filter(|r| r.name != self_name)
        .map(|r| {
            let rid = &r.root_id;
            quote!( #rid #root_targs: ::syan::span::Spanned<Span = #sp> )
        })
        .collect();
    let registrations: Vec<TokenStream> = roots
        .iter()
        .map(|r| {
            let RootReentry { root_id, name, .. } = r;
            let re_sp = reentry_span_name(name, nonce);
            quote! {
                ::syan::parse::vtable::register::<
                    ::syan::parse::vtable::ReKey<#root_id #root_targs, ::syan::parse::vtable::SpanReentry, #sp>,
                >(#re_sp::<#(#root_use),*> as usize);
            }
        })
        .collect();
    quote! {
        impl<#(#xdecl),*> ::syan::span::Spanned for #id<#(#xuse),*>
        where
            for<'__b> #engine<#(#xuse,)* #(#tr_b),*>:
                ::syan::span::Spanned<Span = #sp> + #ftn<'__b, #(#xuse),*>,
            #(#span_bounds,)*
            #sp: ::syan::span::Span,
            #(#where_preds,)*
        {
            type Span = #sp;
            fn span(&self) -> Self::Span {
                #(#registrations)*
                let __e: #engine<#(#xuse,)* #(#tr_anon),*> =
                    <#engine<#(#xuse,)* #(#tr_anon),*> as #ftn<'_, #(#xuse),*>>::__from_nat(self);
                <#engine<#(#xuse,)* #(#tr_anon),*> as ::syan::span::Spanned>::span(&__e)
            }
        }
    }
}

/// The (whole) types of a cycle type's *leaf* fields — those `conv_expr` doesn't convert. Each must be
/// `Clone` for the natural→engine `__FromNat` (which clones leaves into the engine). Leaf-ness is
/// direction-independent, so this probes with `ConvDir::ToNat`.
pub(crate) fn leaf_field_types(item: &Item, child_heads: &HashSet<String>) -> Vec<Type> {
    let mut out = Vec::new();
    let mut push = |fields: &Fields| {
        for f in fields.iter() {
            if conv_expr(&f.ty, quote!(__x), child_heads, ConvDir::ToNat).is_none() {
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

/// The per-cycle-type context a delegated impl is built against — the natural type, its generics
/// (`xdecl` bound-preserving / `xuse` bare), the engine instantiated at the Parse depth-default, the
/// engine→natural conversion-trait ident (`__ToNat`, for `Parse`), the cycle's span type param (only
/// `Spanned` needs it), and the cycle type's own `where`-clause. Built once per type in
/// `gen_natural_extras` and shared by `emit_delegated_parse`/`_unparse`/`_spanned`.
pub(crate) struct DelegTarget<'a> {
    pub(crate) id: &'a Ident,
    pub(crate) xdecl: &'a [TokenStream],
    pub(crate) xuse: &'a [TokenStream],
    pub(crate) engine_default: &'a TokenStream,
    pub(crate) to_nat: &'a Ident,
    pub(crate) span_param: Option<&'a Ident>,
    pub(crate) where_preds: &'a [TokenStream],
}

/// Per-root data the delegated `Parse` needs to register its re-entry parser: the terminator type, the
/// re-entry fn, the root's natural type name, and the root's name (to skip a self-bound).
pub(crate) struct RootReentry {
    pub(crate) term: Ident,
    pub(crate) reentry: Ident,
    pub(crate) root_id: Ident,
    pub(crate) name: String,
}

/// Emit the delegated `impl Parse for <natural type>` — the **unbounded** variant. Before running
/// the engine it **registers** each root's erased re-entry parser into `core::parse::vtable` so the engine's
/// terminator can re-enter at runtime (giving unbounded depth). An inner `__run` fn names the concrete
/// stream type `__St` so the registry key/erasure can spell `__St::Error` (the `Parse` trait method's
/// `impl IntoParseStream` is anonymous). Registering ALL roots (not just self) is required because parsing
/// *any* cycle type descends through the root terminator(s).
pub(crate) fn emit_delegated_parse(
    tg: &DelegTarget,
    roots: &[RootReentry],
    root_use: &[TokenStream],
    root_targs: &TokenStream,
    self_name: &str,
) -> TokenStream {
    let DelegTarget { id, xdecl, xuse, engine_default, to_nat, where_preds, .. } = *tg;
    let engine_bound = quote! {
        #engine_default: ::syan::parse::Parse<__Atom, Error = ::syan::error::ParseError> + #to_nat<#(#xuse),*>
    };
    // `Root: Parse` for every root (needed so the `reentry::<…> as usize` cast type-checks). On the inner
    // `__run` we list ALL roots; on the impl header we omit `self` (a root's own impl assumes `Self: Parse`).
    let run_root_bounds: Vec<TokenStream> = roots
        .iter()
        .map(|r| {
            let rid = &r.root_id;
            quote!( #rid #root_targs: ::syan::parse::Parse<__Atom, Error = ::syan::error::ParseError> )
        })
        .collect();
    let impl_root_bounds: Vec<TokenStream> = roots
        .iter()
        .filter(|r| r.name != self_name)
        .map(|r| {
            let rid = &r.root_id;
            quote!( #rid #root_targs: ::syan::parse::Parse<__Atom, Error = ::syan::error::ParseError> )
        })
        .collect();
    let registrations: Vec<TokenStream> = roots
        .iter()
        .map(|r| {
            let RootReentry { term, reentry, .. } = r;
            quote! {
                ::syan::parse::vtable::register::<
                    ::syan::parse::vtable::ReKey<#term #root_targs, __Atom, __St::Error>,
                >(#reentry::<#(#root_use,)* __Atom, __St::Error> as usize);
            }
        })
        .collect();
    quote! {
        impl<#(#xdecl,)* __Atom> ::syan::parse::Parse<__Atom> for #id<#(#xuse),*>
        where
            __Atom: ::syan::span::Spanned + ::core::clone::Clone,
            #engine_bound,
            #(#impl_root_bounds,)*
            #(#where_preds,)*
        {
            type Error = ::syan::error::ParseError;
            fn parse(
                __syan_s: impl ::syan::parse::IntoParseStream<Atom = __Atom>,
            ) -> ::core::result::Result<Self, Self::Error> {
                fn __run<#(#xdecl,)* __Atom, __St>(
                    mut __st: __St,
                ) -> ::core::result::Result<#id<#(#xuse),*>, ::syan::error::ParseError>
                where
                    __Atom: ::syan::span::Spanned + ::core::clone::Clone,
                    __St: ::syan::parse::ParseStream<Atom = __Atom>,
                    #engine_bound,
                    #(#run_root_bounds,)*
                    #(#where_preds,)*
                {
                    #(#registrations)*
                    let __e: #engine_default =
                        <#engine_default as ::syan::parse::Parse<__Atom>>::parse(&mut __st)?;
                    ::core::result::Result::Ok(#to_nat::__to_nat(__e))
                }
                __run::<#(#xuse,)* __Atom, _>(__syan_s.into_parse_stream())
            }
        }
    }
}

/// Which cycle types delegate which trait through the engine, computed once per `#[recurse]` expansion. A
/// type name is in `parse`/`unparse`/`spanned` iff it `#[derive]`s that trait; `gen_natural_extras` emits
/// the matching delegated impl (+ `__FromNat` bridge for `unparse`/`spanned`) for it.
pub(crate) struct DelegSets {
    pub(crate) parse: HashSet<String>,
    pub(crate) unparse: HashSet<String>,
    pub(crate) spanned: HashSet<String>,
}

/// The per-SCC engine/root data `build_scc` computes and hands to `gen_natural_extras`: the renamed
/// engine idents (`X` → `__XRec`), the sorted roots with their depth params / default aliases, and the
/// roots' shared generics.
pub(crate) struct RootData<'a> {
    pub(crate) internal_names: &'a HashMap<String, Ident>,
    pub(crate) roots_sorted: &'a [String],
    pub(crate) rec_for_root: &'a HashMap<String, Ident>,
    pub(crate) default_for_root: &'a HashMap<String, Ident>,
    pub(crate) root_generics: &'a Generics,
}

/// For an SCC whose natural types own the public names, emit the engine→natural bridge: one
/// `__ToNat_X` trait + impl per cycle type, a terminator `__to_nat` (unwraps its `Box`) per root, and the
/// delegated `impl Parse for X` (parse the depth-limited engine, then `.__to_nat()`). Group-ful
/// `Unparse`/`Spanned` are likewise re-supplied on the natural type by **delegation**: the *reverse*
/// `__FromNat_X` bridge (natural→borrow engine, `Clone`ing leaves, borrowing recursive children) + a
/// delegated `impl Unparse`/`impl Spanned` (`emit_delegated_unparse`/`_spanned`) that converts then calls
/// the engine's impl. `default_for_root` maps each root to its `__<root>Default` depth alias; `rec_for_root`
/// to its depth param; `root_generics` are the roots' (shared) params.
pub(crate) fn gen_natural_extras(
    scc: &HashSet<String>,
    items: &[Item],
    rd: &RootData,
    deleg: &DelegSets,
    nonce: u64,
) -> TokenStream {
    let RootData { internal_names, roots_sorted, rec_for_root, default_for_root, root_generics } = *rd;
    let child_heads: HashSet<String> = scc.clone();
    // `root_decl`/`xdecl` (below) keep param BOUNDS (for naming a bounded cycle type like `Expr<S: Span>`
    // in the conversion/delegation impls); `*_use` are the bound-free argument forms.
    let root_decl = param_decls(root_generics);
    let root_use = generic_tokens(root_generics).1;
    // Per-root re-entry data + the shared `<root params>` arg list, for the unbounded delegated `Parse`.
    let root_targs: TokenStream = if root_use.is_empty() { quote!() } else { quote!( <#(#root_use),*> ) };
    let roots: Vec<RootReentry> = roots_sorted
        .iter()
        .map(|r| RootReentry {
            term: term_name(r, nonce),
            reentry: reentry_name(r, nonce),
            root_id: Ident::new(r, Span::call_site()),
            name: r.clone(),
        })
        .collect();
    let rec_params: Vec<&Ident> = roots_sorted.iter().map(|r| &rec_for_root[r]).collect();
    let trait_name = |x: &str| to_nat_name(x, nonce);
    let from_trait_name = |x: &str| from_nat_name(x, nonce);
    // Whether THIS SCC delegates `Unparse`/`Spanned` natural→engine. When so, emit the `__FromNat` bridge
    // + the delegated impls (a cycle deriving neither has no `__FromNat`).
    let delegate_unparse: Vec<&String> = scc.iter().filter(|n| deleg.unparse.contains(*n)).collect();
    let delegate_spanned: Vec<&String> = scc.iter().filter(|n| deleg.spanned.contains(*n)).collect();
    let needs_from_nat = !delegate_unparse.is_empty() || !delegate_spanned.is_empty();
    // `R: __FromNat_<root><'__n>` per root — the natural→engine bridge's analogue of `root_bounds`. The
    // `'__n` is the borrow of the natural tree the (borrow) engine holds (see `__FromNat` below).
    let from_root_bounds: Vec<TokenStream> = roots_sorted
        .iter()
        .map(|r| {
            let dp = &rec_for_root[r];
            let tn = from_trait_name(r);
            quote!( #dp: #tn<'__n, #(#root_use),*> )
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
            .flat_map(|it| leaf_field_types(it, &child_heads))
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
        let ftn = from_trait_name(&xs);
        let engine = &internal_names[&xs];
        let xdecl = param_decls(generics);
        let xuse = generic_tokens(generics).1;
        // engine → natural (`__to_nat(self)`): src = engine, tgt = natural.
        let body = conv_body(item, engine, id, quote!(self), &child_heads, ConvDir::ToNat);
        // The cycle type's own `where`-clause predicates (e.g. `where S: Clone` / `where Expr<S>:
        // Marker`). Every generated item that NAMES the natural type `Expr<S>` (the conversion traits'
        // method signatures, and the conversion/delegated impls) must repeat these — naming `Expr<S>`
        // is only well-formed when its where-clause holds — else the obligation surfaces undischarged
        // (E0277). They reference the cycle's own params, which are in scope on all of these.
        let where_preds = where_preds(generics);
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
        // The cycle's span type is its first type param (recurse convention) — and the engine's
        // `Spanned::Span` equals it (the `WithSpan<_, S>` leaves pin it). Used by the delegated `Spanned`
        // (so the *private* engine type doesn't leak into the public assoc type — E0446).
        let span_param = generics.params.iter().find_map(|p| match p {
            GenericParam::Type(t) => Some(&t.ident),
            _ => None,
        });
        let target = DelegTarget {
            id,
            xdecl: &xdecl,
            xuse: &xuse,
            engine_default: &engine_default,
            to_nat: &tn,
            span_param,
            where_preds: &where_preds,
        };
        // Delegated `Parse` on the natural type — only when the user derived `Parse` (else the engine
        // has no `Parse` impl to delegate to). The unbounded variant: register each root's erased
        // re-entry parser, run the engine, then `__to_nat`.
        if deleg.parse.contains(&xs) {
            out.extend(emit_delegated_parse(&target, &roots, &root_use, &root_targs, &xs));
        }

        // Natural→engine bridge (`__FromNat_X`) for a delegated `Unparse`/`Spanned` cycle, plus the
        // delegated impls. The bridge `Clone`s leaves into the engine; recursive children recurse through
        // it, bottoming at the borrow terminator (which re-enters at runtime — see below).
        // `Unparse`/`Spanned` then convert the (borrowed) natural value to the depth-1 borrow engine and
        // call the engine's own impl (`emit_delegated_unparse`/`_spanned`).
        if needs_from_nat {
            // natural → borrow engine: clone leaves, recurse into children, bottoming at the borrow
            // terminator `__XxxTermRef<'__n>` (just `&'__n child`) — so only leaves copy (no `Root: Clone`).
            let from_body =
                conv_body(item, id, engine, quote!(__nat), &child_heads, ConvDir::FromNat { nonce });
            out.extend(quote! {
                #[doc(hidden)]
                trait #ftn<'__n, #(#xdecl),*>
                #(if !where_preds.is_empty()) { where #(#where_preds),* }
                {
                    fn __from_nat(__nat: &'__n #id<#(#xuse),*>) -> Self;
                }
                impl<'__n, #(#xdecl,)* #(#rec_params),*> #ftn<'__n, #(#xuse),*>
                    for #engine<#(#xuse,)* #(#rec_params),*>
                where
                    #(#from_root_bounds,)*
                    #(#from_leaf_clones,)*
                    #(#where_preds,)*
                {
                    fn __from_nat(__nat: &'__n #id<#(#xuse),*>) -> Self { #from_body }
                }
            });
            if deleg.unparse.contains(&xs) {
                out.extend(emit_delegated_unparse(&target, &roots, &root_use, &root_targs, &xs, nonce));
            }
            if deleg.spanned.contains(&xs) && span_param.is_some() {
                out.extend(emit_delegated_spanned(&target, &roots, &root_use, &root_targs, &xs, nonce));
            }
        }
    }

    // Terminator → natural: the inhabited terminator simply *wraps* the natural root (it was filled by
    // the re-entry parser), so `__to_nat` unwraps the `Box`.
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
                    *self.0
                }
            }
        });
        // Group-ful `Unparse`/`Spanned` go through the borrow terminator (re-enters at runtime; unbounded).
        if needs_from_nat {
            out.extend(emit_borrow_terminator_and_reentry(
                items,
                r,
                nonce,
                !delegate_unparse.is_empty(),
                !delegate_spanned.is_empty(),
            ));
        }
    }
    out
}

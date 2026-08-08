use super::*;

/// The `__SyanMacro_Atom`-generic impl header shared by `extract_parse`/`extract_unparse`: the atom
/// type param, the fully-applied trait path, the impl's generic params (with the atom appended), and
/// the type generics to instantiate `ident` with.
fn atom_impl_header<'g>(
    generics: &'g Generics,
    trait_path: &Path,
) -> (
    Ident,
    Path,
    Punctuated<GenericParam, Token![,]>,
    syn::TypeGenerics<'g>,
) {
    let tp_atom: Ident = parse_quote!(__SyanMacro_Atom);
    let trait_fullpath: Path = parse_quote!(#trait_path<#tp_atom>);
    let mut generic_params = generics.params.clone();
    strip_param_defaults(&mut generic_params);
    generic_params.push(parse_quote!(#tp_atom));
    let ty_generics = generics.split_for_impl().1;
    (tp_atom, trait_fullpath, generic_params, ty_generics)
}

/// Emits a `#[group]` substruct's own item definition (stripped of derive-helper attrs) plus its
/// derived impl, for every substruct collected while walking a struct/enum's fields.
fn substruct_items(
    substructs: &[ItemStruct],
    mut derive: impl FnMut(&DataStruct, &Generics, &Ident) -> TokenStream,
) -> TokenStream {
    let substructs_for_emit: Vec<ItemStruct> =
        substructs.iter().map(strip_derive_helper_attrs).collect();
    let substruct_impls: Vec<TokenStream> = substructs
        .iter()
        .map(|substruct| {
            let data_struct = DataStruct {
                struct_token: Default::default(),
                fields: substruct.fields.clone(),
                semi_token: substruct.semi_token,
            };
            derive(&data_struct, &substruct.generics, &substruct.ident)
        })
        .collect();
    quote! {
        #(for substruct in &substructs_for_emit) { #substruct }
        #(for substruct_impl in &substruct_impls) { #substruct_impl }
    }
}

pub(crate) trait Adt {
    fn all_fields(&self) -> Vec<&Field>;

    fn extract_parse_inner(
        &self,
        syan: &Path,
        ident: &Ident,
        tp_error_final: &Type,
        f: impl FnMut(&[(Member, Ident, &Field)]) -> TokenStream,
    ) -> TokenStream;

    fn extract_inner(
        &self,
        ident: &Ident,
        v_self: &TokenStream,
        f: impl FnMut(&[(Member, Ident, &Field)]) -> TokenStream,
    ) -> TokenStream;

    fn extract_parse(
        &self,
        syan: &Path,
        generics: &Generics,
        ident: &Ident,
        nonce: u64,
        trait_path: &Path,
    ) -> TokenStream {
        let (tp_atom, trait_fullpath, generic_params, ty_generics) =
            atom_impl_header(generics, trait_path);
        proc_macro_error::append_dummy(quote! {
            impl< #generic_params > #trait_fullpath for #ident #ty_generics {
                type Error = ::core::convert::Infallible;
                fn parse(
                    _stream: impl #syan::parse::into_parse_stream::IntoParseStream<Atom = #tp_atom>
                ) -> ::core::result::Result<Self, Self::Error> {
                    ::core::unimplemented!()
                }
            }
        });
        let mut where_predicates: Punctuated<WherePredicate, token::Comma> = Punctuated::new();
        let v_stream: Ident = parse_quote!(__syan_stream);

        where_predicates.push(parse_quote!(#tp_atom: #syan::span::Spanned));
        where_predicates.push(parse_quote!(#tp_atom: ::core::clone::Clone));
        let tp_error_final: Type = parse_quote!(#syan::error::ParseError);
        let mut substructs: Vec<ItemStruct> = Vec::new();

        let field_phantom: Ident = parse_quote!(_syan_phantom);
        let inner = self.extract_parse_inner(syan, ident,&tp_error_final, |fields| {
            let mut ret = quote!();

            let mut fields: VecDeque<_> = fields.iter().cloned().collect();
            while let Some((member, field_ident, field)) = fields.pop_front() {
                if field.has_default() {
                    ret.extend(quote!(
                        let #field_ident = ::core::default::Default::default();
                    ));
                    continue;
                }

                if !field.has_ignore_bounds() {
                    let field_ty = & field.ty;
                    where_predicates.push(parse_quote!{
                        #field_ty: #trait_fullpath
                    });
                }

                if let Some(group_member) = field.find_group() {
                    abort!(
                        &group_member,
                        "Cannot find member {} in struct {ident}",
                        quote!(#group_member)
                    );
                }

                let spacing = match (field.find_attribute("joint"), field.find_attribute("alone")) {
                    (None, None) => None,
                    (Some(_),None) => Some(Spacing::Joint),
                    (None, Some(_)) => Some(Spacing::Alone),
                    (Some(o1), Some(o2)) => abort!(quote!{#o1, #o2}, "Cannot implement both #[joint] and #[alone] to field `{}`", quote!{#{&field.ident}}),
                };

                let substruct = generate_substruct(&member, generics, ident, &field_ident, &field_phantom, &mut fields, nonce, false);
                    let field_ty = &field.ty;

                if let Some((substruct, subfields)) = substruct {
                    if spacing.is_some() {
                        abort!(&field, "Cannot specify #[joint] or #[alonw] to field {}", quote!(#{&field.ident}));
                    }
                    let substruct_ident = &substruct.ident;
                    let to_parse_ty: Type = parse_quote! {<#field_ty as #syan::nested::group::EmptyGroup>::Fill<
                        #substruct_ident  #ty_generics
                    >};
                    ret.extend(quote!(
                        let #field_ident: #to_parse_ty = ::core::result::Result::map_err(
                            <#to_parse_ty as #trait_fullpath>::parse(&mut #v_stream),
                            |err| <_ as #syan::error::Error>::into_parse_error(err)
                        )?;
                        let (#{ &substruct.ident } {
                            #(for subfield in subfields) { #{&subfield.ident.as_ref().unwrap()}, }
                            #field_phantom: _
                        }, #field_ident) = #syan::nested::group::EmptyGroup::unfill(#field_ident);
                    ));

                    substructs.push(substruct);
                    where_predicates.push(parse_quote!(#field_ty: #syan::nested::group::EmptyGroup));
                    where_predicates.push(parse_quote!(#to_parse_ty: #trait_fullpath));
                } else {
                    let to_parse_ty = field.ty.clone();
                    ret.extend(quote!(
                        #(if let Some(spacing) = spacing) {
                            let #field_ident = #syan::parse::parse_stream::ParseStream::validate_spacing(
                                &mut #v_stream,
                                #{spacing == Spacing::Joint}
                            )?;
                        }
                        let #field_ident = ::core::result::Result::map_err(
                            <#to_parse_ty as #trait_fullpath>::parse(&mut #v_stream),
                            |err| <_ as #syan::error::Error>::into_parse_error(err)
                        )?;
                    ));
                }
            }
            ret
        });
        let substruct_defs = substruct_items(&substructs, |data_struct, generics, ident| {
            data_struct.extract_parse(syan, generics, ident, nonce, trait_path)
        });
        append_user_where_predicates(&mut where_predicates, generics);
        quote! {
            #substruct_defs
            #[automatically_derived]
            impl< #generic_params > #trait_fullpath for #ident #ty_generics
            #(if !where_predicates.is_empty()) { where #where_predicates}
            {
                type Error = #tp_error_final;
                fn parse(
                    #v_stream: impl #syan::parse::into_parse_stream::IntoParseStream<Atom = #tp_atom>
                ) -> ::core::result::Result<Self, Self::Error> {
                    let mut #v_stream = #v_stream.into_parse_stream();
                    #inner
                }
            }
        }
    }

    fn extract_unparse(
        &self,
        syan: &Path,
        generics: &Generics,
        ident: &Ident,
        attrs: &[Attribute],
        nonce: u64,
        trait_path: &Path,
    ) -> TokenStream {
        let (tp_atom, trait_fullpath, generic_params, ty_generics) =
            atom_impl_header(generics, trait_path);
        proc_macro_error::append_dummy(quote! {
            impl< #generic_params > #trait_fullpath for #ident #ty_generics {
                fn unparse<__Syan_Emitter: #syan::parse::unparse::Emitter<#tp_atom>>(&self, _: &mut __Syan_Emitter) -> ::core::result::Result<(), __Syan_Emitter::Error> {
                    ::core::unimplemented!()
                }
            }
        });
        let mut where_predicates: Punctuated<WherePredicate, Token![,]> = Punctuated::new();

        let v_sink: Ident = parse_quote!(__syan_sink);
        let v_self: TokenStream = quote!(self);
        let mut substructs = Vec::new();
        let field_phantom: Ident = parse_quote!(_syan_phantom);
        let inner = self.extract_inner(ident, &v_self, |fields| {
            let mut ret = quote!();
            let mut fields: VecDeque<_> = fields.iter().cloned().collect();

            while let Some((member, field_ident, field)) = fields.pop_front() {
                if field.has_default() {
                    continue;
                }

                if !field.has_ignore_bounds() {
                    let field_ty = &field.ty;
                    where_predicates.push(parse_quote!{
                        #field_ty: #trait_fullpath
                    });
                }

                let field_ty = &field.ty;
                if let Some((substruct, subfields)) = generate_substruct(
                    &member,
                    generics,
                    ident,
                    &field_ident,
                    &field_phantom,
                    &mut fields,
                    nonce,
                    true,
                ) {
                    ret.extend(quote! {
                        let #field_ident = <#field_ty as #syan::nested::group::EmptyGroup>::fill(
                            ::core::clone::Clone::clone(#field_ident),
                            #{&substruct.ident} {
                                #(for subfield in &subfields) { #{&subfield.ident}, }
                                #field_phantom: ::core::marker::PhantomData
                            }
                        );
                    });

                    let mut fill_ty_generics = generics.clone();
                    fill_ty_generics.params.insert(0, parse_quote!('syan_substruct_ref));
                    let fill_ty = quote! {
                        #{&substruct.ident}
                        #{fill_ty_generics.split_for_impl().1}
                    };
                    where_predicates.push(parse_quote!(#field_ty: #syan::nested::group::EmptyGroup + ::core::clone::Clone));
                    where_predicates.push(parse_quote!(for<'syan_substruct_ref> <#field_ty as #syan::nested::group::EmptyGroup>::Fill<#fill_ty>: #trait_fullpath));
                    substructs.push(substruct);
                ret.extend(quote!(
                    <_ as #trait_fullpath>::unparse(&#field_ident, #v_sink)?;
                ));
                } else  {
                ret.extend(quote!(
                    <_ as #trait_fullpath>::unparse(#field_ident, #v_sink)?;
                ));
                }
            }
            quote! {
                #ret
                ::core::result::Result::Ok(())
            }
        });

        let substruct_defs = substruct_items(&substructs, |data_struct, generics, ident| {
            data_struct.extract_unparse(syan, generics, ident, &[], nonce, trait_path)
        });
        for ty in predicate_tys(attrs, "predicate_unparse") {
            where_predicates.push(parse_quote!(#ty: #trait_fullpath));
        }
        append_user_where_predicates(&mut where_predicates, generics);
        quote! {
            #substruct_defs
            #[automatically_derived]
            impl< #generic_params > #trait_fullpath for #ident #ty_generics
            #(if !where_predicates.is_empty()) { where #where_predicates}
            {
                fn unparse<__Syan_Emitter: #syan::parse::unparse::Emitter<#tp_atom>>(&self, #v_sink: &mut __Syan_Emitter) -> ::core::result::Result<(), __Syan_Emitter::Error> {
                    #inner
                }
            }
        }
    }

    fn extract_spanned(
        &self,
        syan: &Path,
        generics: &Generics,
        ident: &Ident,
        attrs: &[Attribute],
        trait_path: &Path,
    ) -> TokenStream {
        let trait_fullpath: Path = trait_path.clone();
        let ty_generics = generics.split_for_impl().1;
        let mut generic_params = generics.params.clone();
        strip_param_defaults(&mut generic_params);
        let mut where_predicates: Punctuated<WherePredicate, token::Comma> = Punctuated::new();

        let tp_span: Ident = parse_quote!(__Syan_Span);

        add_spanned_param_predicates(&mut where_predicates, generics, syan, &tp_span);
        generic_params.push(parse_quote!(#tp_span: #syan::span::Span));
        proc_macro_error::append_dummy(quote! {
            impl< #generic_params > #trait_fullpath for #ident #ty_generics {
                type Span = #tp_span;

                fn span(&self) -> Self::Span {
                    ::core::unimplemented!()
                }
            }
        });

        let fields = self.all_fields();

        if fields.is_empty() {
            abort!(Span::call_site(), "no field exists");
        }
        let v_self: TokenStream = quote!(self);

        let span_impl = self.extract_inner(ident, &v_self, |fields| {
            for (_, _, field) in fields {
                if field.has_default() {
                    continue;
                }
                // `#[ignore_bounds]` suppresses the synthesized predicate (for a naturally-recursive
                // child whose bound would otherwise cycle). NOTE: `Spanned` carries an associated `Span`
                // type, and the dropped predicate is what pins it to `__Syan_Span`; without it the
                // child's `Span` is unconstrained in the `migrate` fold below, so `#[ignore_bounds]` on
                // a `Spanned` field only type-checks when the field's `Span` is otherwise inferable.
                if field.has_ignore_bounds() {
                    continue;
                }
                // Every folded field must report the impl's span type `__Syan_Span`. Constraining it
                // here pins the invented span param (fixes E0207 "unconstrained") and makes the
                // `Span::migrate(acc, Spanned::span(field))` fold type-check (fixes E0308) for
                // composite / bounded field types like `WithSpan<_, S>`. (For a bare unbounded type
                // param, add_spanned_param_predicates already adds the matching bound — harmless dup.)
                let field_ty = &field.ty;
                where_predicates
                    .push(parse_quote!(#field_ty: #syan::span::Spanned<Span = #tp_span>));
            }
            let ret = quote! {
                let __syan_span = <#tp_span as ::core::default::Default>::default();
                #(for (_, field, Field{attrs, ..}) in fields){
                    #(if !attrs.has_default()) {
                        let __syan_span = #syan::span::Span::migrate(
                            __syan_span,
                            #syan::span::Spanned::span(#field)
                        );
                    }
                }
                __syan_span
            };
            ret
        });

        for ty in predicate_tys(attrs, "predicate_spanned") {
            where_predicates.push(parse_quote!(#ty: #syan::span::Spanned<Span = #tp_span>));
        }
        append_user_where_predicates(&mut where_predicates, generics);
        quote! {
            #[automatically_derived]
            impl <#generic_params> #trait_fullpath for #ident #ty_generics
            #(if !where_predicates.is_empty()){where #where_predicates}
            {
                type Span = #tp_span;

                fn span(&self) -> Self::Span {
                    #span_impl
                }
            }
        }
    }
}

fn map_fields_to_idents<'a>(
    fields: impl IntoIterator<Item = &'a Field>,
) -> Vec<(Member, Ident, &'a Field)> {
    fields
        .into_iter()
        .enumerate()
        .map(|(n, field)| {
            let ident = field
                .ident
                .clone()
                .unwrap_or_else(|| Ident::new(&format!("__syan_a{n}"), field.span()));
            let member = field.ident.clone().map(Member::Named).unwrap_or_else(|| {
                Member::Unnamed(Index {
                    index: n as u32,
                    span: field.span(),
                })
            });
            (member, ident, field)
        })
        .collect::<Vec<_>>()
}

/// One mapped field: its `Member` (for construction), the binding `Ident`, and the source `Field`.
type MappedField<'a> = (Member, Ident, &'a Field);
/// A variant paired with its mapped fields.
type VariantFields<'a> = (&'a Variant, Vec<MappedField<'a>>);

/// `(a, b,)` for tuple fields, `{a, b,}` for named fields, or nothing for a unit shape.
fn fields_pattern(shape: &Fields, fields: &[MappedField]) -> TokenStream {
    quote! {
        #(if let Fields::Unnamed(_) = shape) {
            (#(for (_, id, _) in fields) {#id,})
        }
        #(if let Fields::Named(_) = shape) {
            {#(for (_, id, _) in fields) {#id,}}
        }
    }
}

/// The number of leading fields that EVERY variant shares with identical parse behaviour — same member
/// (so the binding ident aligns for construction), type, and attributes. Drives the enum `Parse`
/// prefix-dedup. Zero when there are no variants, any variant is fieldless, or the first fields already
/// differ — in which case the derive keeps the per-variant backtracking scheme unchanged.
fn common_field_prefix_len(variants: &[VariantFields]) -> usize {
    let sig = |(m, _, f): &(Member, Ident, &Field)| -> String {
        let ty = &f.ty;
        let attrs: String = f
            .attrs
            .iter()
            .map(|a| quote!(#a).to_string())
            .collect::<Vec<_>>()
            .join(",");
        format!("{}|{}|{}", quote!(#m), quote!(#ty), attrs)
    };
    let Some(min_len) = variants.iter().map(|(_, fs)| fs.len()).min() else {
        return 0;
    };
    let mut lcp = 0;
    for i in 0..min_len {
        let s0 = sig(&variants[0].1[i]);
        if variants[1..].iter().any(|(_, fs)| sig(&fs[i]) != s0) {
            break;
        }
        lcp += 1;
    }
    lcp
}

impl Adt for DataStruct {
    fn all_fields(&self) -> Vec<&Field> {
        self.fields.iter().collect()
    }

    fn extract_parse_inner(
        &self,
        _syan: &Path,
        ident: &Ident,
        _tp_error_final: &Type,
        mut f: impl FnMut(&[(Member, Ident, &Field)]) -> TokenStream,
    ) -> TokenStream {
        let fields = map_fields_to_idents(&self.fields);
        let inner = f(&fields[..]);
        quote! {
            #inner

            ::core::result::Result::Ok(
                #ident #{fields_pattern(&self.fields, &fields)}
            )
        }
    }

    fn extract_inner(
        &self,
        ident: &Ident,
        v_self: &TokenStream,
        mut f: impl FnMut(&[(Member, Ident, &Field)]) -> TokenStream,
    ) -> TokenStream {
        let fields = map_fields_to_idents(&self.fields);
        let inner = f(&fields[..]);
        quote! {
            let #ident #{fields_pattern(&self.fields, &fields)} = #v_self;
            #inner
        }
    }
}

impl Adt for DataEnum {
    fn all_fields(&self) -> Vec<&Field> {
        self.variants.iter().flat_map(|v| &v.fields).collect()
    }

    fn extract_parse_inner(
        &self,
        syan: &Path,
        ident: &Ident,
        tp_error_final: &Type,
        mut f: impl FnMut(&[MappedField]) -> TokenStream,
    ) -> TokenStream {
        let variants: Vec<VariantFields> = self
            .variants
            .iter()
            .map(|v| (v, map_fields_to_idents(&v.fields)))
            .collect();

        let construct_of = |variant: &Variant, fields: &[MappedField]| -> TokenStream {
            quote! {
                #ident :: #{ &variant.ident } #{fields_pattern(&variant.fields, fields)}
            }
        };

        // **Prefix-dedup**: the length of the leading run of fields that EVERY variant shares (same
        // member + type + attrs, hence same parse + binding). When non-zero, those fields are parsed ONCE
        // up front instead of being re-parsed inside each variant's backtracking attempt (`E | E!`). When
        // zero (or <2 variants) — the common case, e.g. variants distinguished by their first token, incl.
        // every recurse-engine enum — codegen is the per-variant-`dup` scheme, unchanged.
        let lcp = common_field_prefix_len(&variants);

        if lcp == 0 || variants.len() < 2 {
            let blocks: Vec<TokenStream> = variants
                .iter()
                .map(|(variant, fields)| {
                    let inner = f(&fields[..]);
                    let construct = construct_of(variant, fields);
                    quote! {
                        match #syan::parse::ParseStream::dup(&mut __syan_stream, |mut __syan_stream| {
                            #inner
                            ::core::result::Result::Ok(#construct)
                        }) {
                            ::core::result::Result::Err(err) => { __syan_errors.push(err); }
                            ok => { return ok; }
                        }
                    }
                })
                .collect();
            return quote! {
                let mut __syan_errors = ::std::vec::Vec::new();
                #(#blocks)*
                ::core::result::Result::Err(
                    <#tp_error_final as #syan::error::Error>::from_cause(__syan_errors)
                )
            };
        }

        // ── factored: parse the shared prefix once, branch on the per-variant suffix ──
        // The whole thing runs in one outer `dup` so a total failure still rewinds the stream (preserving
        // the "enum parse rewinds on failure" property of the per-variant scheme); the prefix is parsed
        // once inside it, and each non-empty suffix gets its own inner `dup` so a failed variant rewinds
        // only the suffix and the next variant is tried from the post-prefix position.
        let prefix_parse = f(&variants[0].1[..lcp]);

        // A variant whose fields are exactly the prefix is an unconditional fallback (empty suffix); the
        // FIRST such variant ends the chain (later variants are unreachable). It becomes the closure's tail
        // `Ok(..)`; the variants before it each get a suffix `dup` and `return` on success. With a fallback
        // a failed suffix is discarded; without one the suffix errors are collected for `from_cause`.
        let fallback = variants.iter().position(|(_, fs)| fs.len() == lcp);
        let tried = &variants[..fallback.unwrap_or(variants.len())];
        let has_fallback = fallback.is_some();

        let branches: Vec<TokenStream> = tried
            .iter()
            .map(|(variant, fields)| {
                let construct = construct_of(variant, fields);
                let suffix = &fields[lcp..]; // non-empty (the first empty one is the fallback, excluded)
                let suffix_parse = f(suffix);
                let suffix_ids: Vec<&Ident> = suffix.iter().map(|(_, id, _)| id).collect();
                let on_err = if has_fallback {
                    quote!( ::core::result::Result::Err(_) => {} )
                } else {
                    quote!( ::core::result::Result::Err(err) => { __syan_errors.push(err); } )
                };
                quote! {
                    // Turbofish pins the suffix `dup`'s error type (a discarded `Err(_)` arm wouldn't).
                    match #syan::parse::ParseStream::dup::<_, #tp_error_final, _>(
                        &mut __syan_stream,
                        |mut __syan_stream| {
                            #suffix_parse
                            ::core::result::Result::Ok((#(#suffix_ids,)*))
                        },
                    ) {
                        ::core::result::Result::Ok((#(#suffix_ids,)*)) => {
                            return ::core::result::Result::Ok(#construct);
                        }
                        #on_err
                    }
                }
            })
            .collect();

        let tail = match fallback {
            Some(i) => {
                let (variant, fields) = &variants[i];
                let construct = construct_of(variant, fields);
                quote!( ::core::result::Result::Ok(#construct) )
            }
            None => quote! {
                ::core::result::Result::Err(
                    <#tp_error_final as #syan::error::Error>::from_cause(__syan_errors)
                )
            },
        };
        let errors_decl =
            (!has_fallback).then(|| quote!( let mut __syan_errors = ::std::vec::Vec::new(); ));
        quote! {
            #syan::parse::ParseStream::dup(&mut __syan_stream, |mut __syan_stream| {
                #prefix_parse
                #errors_decl
                #(#branches)*
                #tail
            })
        }
    }

    fn extract_inner(
        &self,
        ident: &Ident,
        v_self: &TokenStream,
        mut f: impl FnMut(&[(Member, Ident, &Field)]) -> TokenStream,
    ) -> TokenStream {
        let variants = self.variants.iter().map(|v| {
            let fields = map_fields_to_idents(&v.fields);
            let inner = f(&fields[..]);
            (v, fields, inner)
        });
        quote! {
            match #v_self {
                #(for (variant, fields, inner) in variants) {
                    #ident :: #{ &variant.ident } #{fields_pattern(&variant.fields, &fields)} => {
                        #inner
                    }
                }
            }
        }
    }
}

use proc_macro2::{Spacing, Span, TokenStream};
use proc_macro_error::abort;
use std::collections::VecDeque;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::*;
use template_quote::quote;

pub(crate) trait FindAttribute {
    fn find_attribute<I: ?Sized>(&self, name: &I) -> Option<&Attribute>
    where
        Ident: PartialEq<I>;

    fn get_syan(&self) -> Path {
        match self.find_attribute("syan") {
            Some(attr) => {
                if let Meta::List(MetaList { tokens, .. }) = &attr.meta {
                    if let Ok(path) = parse2::<Path>(tokens.clone()) {
                        return path;
                    }
                }
                abort!(attr, "should be formatted as #[syan(<path>)]")
            }
            None => parse_quote!(::syan),
        }
    }

    fn find_group(&self) -> Option<Member> {
        match &self.find_attribute("group")?.meta {
            Meta::List(MetaList { tokens, .. }) => match parse2::<ExprField>(tokens.clone()) {
                Ok(ExprField { base, member, .. }) if &quote!(#base).to_string() == "self" => {
                    Some(member)
                }
                _ => abort!(
                    tokens,
                    "the content of #[group(..)] should be formatted as `self.???`"
                ),
            },
            _ => abort!(
                self.find_attribute("group").unwrap(),
                "#[group(..)] format error"
            ),
        }
    }

    fn has_default(&self) -> bool {
        self.find_attribute("default").is_some()
    }

    /// `#[ignore_bounds]` on a field suppresses the synthesized `field_ty: Trait` where-predicate in
    /// the `Parse`/`Unparse`/`Spanned` derives. This lets a *naturally* mutually-recursive type carry
    /// leaf-only bounds (the recursive children are resolved coinductively via their sibling impls'
    /// call sites, not via a where-bound cycle that would overflow with E0275). `#[recurse]` injects it
    /// on every recursive-child field of a natural cycle type.
    fn has_ignore_bounds(&self) -> bool {
        self.find_attribute("ignore_bounds").is_some()
    }
}

#[allow(dead_code)]
fn collect_primitive_tys(ty: &Type) -> impl Iterator<Item = Type> {
    #[derive(Default)]
    struct TypeCollector {
        types: Vec<Type>,
    }

    impl<'ast> Visit<'ast> for TypeCollector {
        fn visit_type(&mut self, ty: &'ast Type) {
            if let Type::Macro(_type_macro) = ty {
                self.types.push(ty.clone());
            } else {
                syn::visit::visit_type(self, ty);
            }
        }
    }

    let mut collector = TypeCollector::default();
    collector.visit_type(ty);
    collector.types.into_iter()
}

fn is_derive_helper_attr(attr: &Attribute) -> bool {
    attr.path().is_ident("group")
        || attr.path().is_ident("syan")
        || attr.path().is_ident("joint")
        || attr.path().is_ident("alone")
        || attr.path().is_ident("ignore_bounds")
        || attr.path().is_ident("default")
        || attr.path().is_ident("fundamental_tys")
        || attr.path().is_ident("predicate")
        || attr.path().is_ident("predicate_parse")
        || attr.path().is_ident("predicate_unparse")
        // `#[derive(Ast)]`'s view markers: strip them off a `#[group]`-cloned substruct (which carries no
        // `Ast` derive to register them), else `#[group] #[seq] Punctuated<..>` fails with "cannot find
        // attribute `seq`".
        || attr.path().is_ident("seq")
        || attr.path().is_ident("opt")
}

fn strip_derive_helper_attrs(substruct: &ItemStruct) -> ItemStruct {
    let mut substruct = substruct.clone();
    match &mut substruct.fields {
        Fields::Named(fields) => {
            for field in fields.named.iter_mut() {
                field.attrs.retain(|attr| !is_derive_helper_attr(attr));
            }
        }
        Fields::Unnamed(fields) => {
            for field in fields.unnamed.iter_mut() {
                field.attrs.retain(|attr| !is_derive_helper_attr(attr));
            }
        }
        Fields::Unit => {}
    }
    substruct
}

/// For each **unbounded** type param of `generics`, add `T: Spanned<Span = #tp_span>` to
/// `where_predicates` — the `Spanned` derive's per-param bound (pinning every param's span to the impl's
/// invented span type). A param that already carries bounds is left to the user. Only the `Spanned`
/// derive needs this (the `Parse`/`Unparse` derives synthesize their per-field bounds directly).
fn add_spanned_param_predicates(
    where_predicates: &mut Punctuated<WherePredicate, Token![,]>,
    generics: &Generics,
    syan: &Path,
    tp_span: &Ident,
) {
    for param in &generics.params {
        if let GenericParam::Type(type_param) = param {
            if type_param.bounds.is_empty() {
                let ty = &type_param.ident;
                where_predicates.push(parse_quote!(#ty: #syan::span::Spanned<Span = #tp_span>));
            }
        }
    }
}

/// Append the user-written where-clause predicates (if any) onto the macro-synthesized
/// `where_predicates`, so the generated impl carries both the synthesized bounds and the user's
/// own bounds (otherwise a `where`-clause is dropped and the Self type fails well-formedness).
fn append_user_where_predicates(
    where_predicates: &mut Punctuated<WherePredicate, Token![,]>,
    generics: &Generics,
) {
    if let Some(where_clause) = &generics.where_clause {
        for predicate in &where_clause.predicates {
            where_predicates.push(predicate.clone());
        }
    }
}

/// Collect the types listed in every `#[<name>(Ty0, Ty1, …)]` item attribute (`predicate_unparse` /
/// `predicate_spanned`). The caller turns each `Ty` into a trait bound (`Ty: Unparse<atom>` /
/// `Ty: Spanned<Span = span>`) and adds it to the impl. `#[recurse]` uses this to inject the cross-cycle
/// *union* of leaf bounds onto a member's natural `Unparse`/`Spanned` impl, so the body's calls into
/// sibling cycle types resolve — the per-field bounds for the recursive children are suppressed by
/// `#[ignore_bounds]`, which is what avoids the E0275 where-cycle.
fn predicate_tys(attrs: &[Attribute], name: &str) -> Vec<Type> {
    let mut out = Vec::new();
    for a in attrs {
        if a.path().is_ident(name) {
            if let Meta::List(ml) = &a.meta {
                if let Ok(tys) =
                    ml.parse_args_with(Punctuated::<Type, Token![,]>::parse_terminated)
                {
                    out.extend(tys);
                }
            }
        }
    }
    out
}

impl FindAttribute for Field {
    fn find_attribute<I: ?Sized>(&self, name: &I) -> Option<&Attribute>
    where
        Ident: PartialEq<I>,
    {
        self.attrs[..].find_attribute(name)
    }
}

impl FindAttribute for [Attribute] {
    fn find_attribute<I: ?Sized>(&self, name: &I) -> Option<&Attribute>
    where
        Ident: PartialEq<I>,
    {
        self.iter().find_map(|field| field.find_attribute(name))
    }
}

impl FindAttribute for Attribute {
    fn find_attribute<I: ?Sized>(&self, name: &I) -> Option<&Attribute>
    where
        Ident: PartialEq<I>,
    {
        match &self.meta {
            Meta::List(MetaList { path, .. })
            | Meta::Path(path)
            | Meta::NameValue(MetaNameValue { path, .. }) => {
                if path.is_ident(name) {
                    Some(self)
                } else {
                    None
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn generate_substruct(
    member: &Member,
    generics: &Generics,
    ident: &Ident,
    field_ident: &Ident,
    field_phantom: &Ident,
    fields: &mut VecDeque<(Member, Ident, &Field)>,
    nonce: u64,
    by_ref: bool,
) -> Option<(ItemStruct, Vec<Field>)> {
    let mut subfields = Vec::new();
    let lt = Lifetime::new("'syan_substruct_ref", Span::call_site());
    while let Some((submember, subident, subfield)) = fields.pop_front() {
        if let Some(group_member) = subfield.find_group() {
            let mut subfield = subfield.clone();
            subfield.ident = Some(subident);
            if &group_member == member {
                // remove #[group(..)] attribute if is toplevel field of substruct.
                let _ = subfield
                    .attrs
                    .extract_if(.., |attr| {
                        attr.find_group().map(|g| &g == member).unwrap_or(false)
                    })
                    .collect::<Vec<_>>();
            }
            subfield.vis = Visibility::Inherited;
            if by_ref {
                let ty = &subfield.ty;
                subfield.ty = parse_quote!(& #lt #ty);
            }
            subfields.push(subfield);
        } else {
            fields.push_front((submember, subident, subfield));
            break;
        }
    }
    if !subfields.is_empty() {
        let substruct_ident = Ident::new(
            &format!("__SyanSubstructOf_{field_ident}_{ident}_{nonce}"),
            member.span(),
        );
        let phantom_args: Punctuated<Type, Token![,]> = generics
            .params
            .iter()
            .filter_map(|param| -> Option<Type> {
                match param {
                    GenericParam::Lifetime(LifetimeParam { lifetime, .. }) => {
                        Some(parse_quote!(&#lifetime ()))
                    }
                    GenericParam::Type(TypeParam { ident, .. }) => Some(parse_quote!(#ident)),
                    GenericParam::Const(_) => None,
                }
            })
            .collect();
        let phantom_ty: Type = if phantom_args.is_empty() {
            parse_quote!(())
        } else {
            parse_quote!((#phantom_args,))
        };
        let mut generics = generics.clone();
        if by_ref {
            generics.params.insert(0, parse_quote!(#lt))
        }
        let substruct = ItemStruct {
            attrs: vec![parse_quote!(#[allow(non_camel_case_types)])],
            vis: Visibility::Inherited,
            struct_token: Default::default(),
            ident: substruct_ident.clone(),
            generics: generics.clone(),
            fields: Fields::Named(FieldsNamed {
                brace_token: Default::default(),
                named: subfields
                    .iter()
                    .cloned()
                    .chain(core::iter::once(parse_quote!(
                        #[default]
                        #[ignore_bounds]
                        #field_phantom: ::core::marker::PhantomData<#phantom_ty>
                    )))
                    .collect(),
            }),
            semi_token: None,
        };
        Some((substruct, subfields))
    } else {
        None
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
        let tp_atom: Ident = parse_quote!(__SyanMacro_Atom);
        let trait_path_owned: Path = trait_path.clone();
        let trait_fullpath: Path = parse_quote!(#trait_path_owned<#tp_atom>);
        let mut generic_params = generics.params.clone();
        for param in &mut generic_params {
            match param {
                GenericParam::Type(type_param) => {
                    type_param.eq_token = None;
                    type_param.default = None;
                }
                GenericParam::Const(const_param) => {
                    const_param.eq_token = None;
                    const_param.default = None;
                }
                _ => (),
            }
        }
        generic_params.push(parse_quote!(#tp_atom));
        let ty_generics = generics.split_for_impl().1;
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
        let mut wrapper_counter = 0usize;

        where_predicates.push(parse_quote!(#tp_atom: #syan::span::Spanned));
        where_predicates.push(parse_quote!(#tp_atom: ::core::clone::Clone));
        let tp_error_final: Type = parse_quote!(#syan::error::ParseError);
        let mut substructs: Vec<ItemStruct> = Vec::new();

        let field_phantom: Ident = parse_quote!(_syan_phantom);
        let inner = self.extract_parse_inner(syan, ident,&tp_error_final, |fields| {
            let mut ret = quote!();

            let mut fields: VecDeque<_> = fields.iter().cloned().collect();
            while let Some((member, field_ident, field)) = fields.pop_front() {
                // Skip fields with #[default] attribute - they use Default::default()
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

                // check if the toplevel field has no `#[group(..)]` attr
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
                    wrapper_counter += 1;

            }
            ret
        });
        let substruct_impls: Vec<TokenStream> = substructs
            .iter()
            .map(|substruct| {
                let data_struct = DataStruct {
                    struct_token: Default::default(),
                    fields: substruct.fields.clone(),
                    semi_token: substruct.semi_token,
                };
                data_struct.extract_parse(
                    syan,
                    &substruct.generics,
                    &substruct.ident,
                    nonce,
                    trait_path,
                )
            })
            .collect();
        let substructs_for_emit: Vec<ItemStruct> =
            substructs.iter().map(strip_derive_helper_attrs).collect();
        // Thread the user's where-clause predicates into the generated impl (merged with the
        // synthesized bounds) so a Parse-derived type may carry a where-clause.
        append_user_where_predicates(&mut where_predicates, generics);
        quote! {
            #(for substruct in &substructs_for_emit) {
                #substruct
            }
            #(for substruct_impl in &substruct_impls) {
                #substruct_impl
            }
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
        let tp_atom: Ident = parse_quote!(__SyanMacro_Atom);
        let trait_path_owned: Path = trait_path.clone();
        let trait_fullpath: Path = parse_quote!(#trait_path_owned<#tp_atom>);
        let mut generic_params = generics.params.clone();
        for param in &mut generic_params {
            match param {
                GenericParam::Type(type_param) => {
                    type_param.eq_token = None;
                    type_param.default = None;
                }
                GenericParam::Const(const_param) => {
                    const_param.eq_token = None;
                    const_param.default = None;
                }
                _ => (),
            }
        }
        generic_params.push(parse_quote!(#tp_atom));
        let ty_generics = generics.split_for_impl().1;
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
                // Skip fields with #[default] attribute - they are not unparsed
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
                // Check if this field has grouped subfields (though for unparse we don't generate substructs)
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

        let substruct_impls: Vec<TokenStream> = substructs
            .iter()
            .map(|substruct| {
                let data_struct = DataStruct {
                    struct_token: Default::default(),
                    fields: substruct.fields.clone(),
                    semi_token: substruct.semi_token,
                };
                data_struct.extract_unparse(
                    syan,
                    &substruct.generics,
                    &substruct.ident,
                    &[],
                    nonce,
                    &trait_path_owned,
                )
            })
            .collect();
        let substructs_for_emit: Vec<ItemStruct> =
            substructs.iter().map(strip_derive_helper_attrs).collect();
        // `#[predicate_unparse(Ty, …)]` → `Ty: Unparse<__SyanMacro_Atom>` (the cross-cycle leaf union).
        for ty in predicate_tys(attrs, "predicate_unparse") {
            where_predicates.push(parse_quote!(#ty: #trait_fullpath));
        }
        // Thread the user's where-clause predicates into the generated impl (merged with the
        // synthesized bounds); otherwise the Self type fails WF with "required by a bound in <T>".
        append_user_where_predicates(&mut where_predicates, generics);
        quote! {
            #(for substruct in &substructs_for_emit) {
                #substruct
            }
            #(for substruct_impl in &substruct_impls) {
                #substruct_impl
            }
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
        // A generic param default (e.g. the engine's `__Rec = __ExprDefault<S>`) is only valid in the
        // type *definition*; carried onto an `impl` header it is an error (and a non-trailing one once
        // `__Syan_Span` is appended). Strip defaults here, mirroring the `Parse`/`Unparse` derives.
        for param in &mut generic_params {
            match param {
                GenericParam::Type(type_param) => {
                    type_param.eq_token = None;
                    type_param.default = None;
                }
                GenericParam::Const(const_param) => {
                    const_param.eq_token = None;
                    const_param.default = None;
                }
                _ => {}
            }
        }
        let mut where_predicates: Punctuated<WherePredicate, token::Comma> = Punctuated::new();

        let tp_span: Ident = parse_quote!(__Syan_Span);

        // Add `T: Spanned<Span = __Syan_Span>` for each unbounded type parameter.
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
        let mut wrapper_counter = 0usize;

        let span_impl = self.extract_inner(ident, &v_self, |fields| {
            for (_, _, field) in fields {
                // Skip fields with #[default] attribute - they don't contribute to span
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
                where_predicates.push(parse_quote!(#field_ty: #syan::span::Spanned<Span = #tp_span>));
            }
            let ret = quote! {
                let __syan_span = <#tp_span as ::core::default::Default>::default();
                #(for (_, field, Field{attrs, ..}) in fields){
                    // Skip fields with #[default] attribute
                    #(if !attrs.has_default()) {
                        let __syan_span = #syan::span::Span::migrate(
                            __syan_span,
                            #syan::span::Spanned::span(#field)
                        );
                    }
                }
                __syan_span
            };
            wrapper_counter += 1;
            ret
        });

        // `#[predicate_spanned(Ty, …)]` → `Ty: Spanned<Span = __Syan_Span>` (the cross-cycle leaf union).
        for ty in predicate_tys(attrs, "predicate_spanned") {
            where_predicates.push(parse_quote!(#ty: #syan::span::Spanned<Span = #tp_span>));
        }
        // Thread the user's where-clause predicates into the generated impl (merged with the
        // synthesized bounds); otherwise the Self type fails WF with "required by a bound in <T>".
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

/// The number of leading fields that EVERY variant shares with identical parse behaviour — same member
/// (so the binding ident aligns for construction), type, and attributes. Drives the enum `Parse`
/// prefix-dedup. Zero when there are no variants, any variant is fieldless, or the first fields already
/// differ — in which case the derive keeps the per-variant backtracking scheme unchanged.
fn common_field_prefix_len(variants: &[VariantFields]) -> usize {
    let sig = |(m, _, f): &(Member, Ident, &Field)| -> String {
        let ty = &f.ty;
        let attrs: String =
            f.attrs.iter().map(|a| quote!(#a).to_string()).collect::<Vec<_>>().join(",");
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
                #ident
                #(if let Fields::Unnamed(_) = &self.fields) {
                    (#(for (_, field_ident, _) in &fields) {#field_ident,})
                }
                #(if let Fields::Named(_) = &self.fields) {
                    {#(for (_, field_ident, _) in &fields) {#field_ident,}}
                }
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
            let #ident
            #(if let Fields::Unnamed(_) = &self.fields) {
                (#(for (_, field_ident, _) in &fields) {#field_ident,})
            }
            #(if let Fields::Named(_) = &self.fields) {
                {#(for (_, field_ident, _) in &fields) {#field_ident,}}
            } = #v_self;
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

        // Construct `Ident::Variant { fields… }` / `Ident::Variant(fields…)` from the field bindings.
        let construct_of = |variant: &Variant, fields: &[MappedField]| -> TokenStream {
            quote! {
                #ident :: #{ &variant.ident }
                #(if let Fields::Unnamed(_) = &variant.fields) {
                    (#(for (_, id, _) in fields) {#id,})
                }
                #(if let Fields::Named(_) = &variant.fields) {
                    {#(for (_, id, _) in fields) {#id,}}
                }
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
                    #ident :: #{ &variant.ident }
                    #(if let Fields::Unnamed(_) = &variant.fields) {
                        (#(for (_, field_ident, _) in &fields) {#field_ident,})
                    }
                    #(if let Fields::Named(_) = &variant.fields) {
                        {#(for (_, field_ident, _) in &fields) {#field_ident,}}
                    } => {
                        #inner
                    }
                }
            }
        }
    }
}

pub fn parse(
    ident: &Ident,
    generics: &Generics,
    input: &Data,
    nonce: u64,
    syan: &Path,
    trait_path: &Path,
) -> TokenStream {
    match input {
        Data::Struct(data_struct) => {
            data_struct.extract_parse(syan, generics, ident, nonce, trait_path)
        }
        Data::Enum(data_enum) => data_enum.extract_parse(syan, generics, ident, nonce, trait_path),
        _ => abort!(ident, "Bad data"),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn unparse(
    ident: &Ident,
    generics: &Generics,
    input: &Data,
    attrs: &[Attribute],
    nonce: u64,
    syan: &Path,
    trait_path: &Path,
) -> TokenStream {
    match &input {
        Data::Struct(data_struct) => {
            data_struct.extract_unparse(syan, generics, ident, attrs, nonce, trait_path)
        }
        Data::Enum(data_enum) => {
            data_enum.extract_unparse(syan, generics, ident, attrs, nonce, trait_path)
        }
        _ => abort!(ident, "Bad data"),
    }
}

pub fn spanned(input: &DeriveInput, trait_path: Path) -> TokenStream {
    let syan = input.attrs.get_syan();
    match &input.data {
        Data::Struct(data_struct) => {
            data_struct.extract_spanned(&syan, &input.generics, &input.ident, &input.attrs, &trait_path)
        }
        Data::Enum(data_enum) => {
            data_enum.extract_spanned(&syan, &input.generics, &input.ident, &input.attrs, &trait_path)
        }
        _ => abort!(input, "Bad data"),
    }
}

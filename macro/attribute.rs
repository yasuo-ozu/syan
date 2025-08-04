use proc_macro2::{Spacing, Span, TokenStream};
use proc_macro_error::abort;
use std::collections::{HashSet, VecDeque};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::*;
use template_quote::quote;

trait FindAttribute {
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

fn generate_substruct(
    member: &Member,
    generics: &Generics,
    ident: &Ident,
    field_ident: &Ident,
    fields: &mut VecDeque<(Member, Ident, &Field)>,
    nonce: u64,
    by_ref: bool,
) -> Option<(ItemStruct, Vec<Field>)> {
    // iterate over subfields which has attribute `#[group(...)]`
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
        // make substruct
        let substruct_ident = Ident::new(
            &format!("__SyanSubstructOf_{field_ident}_{ident}_{nonce}"),
            member.span(),
        );
        let mut generics = generics.clone();
        if by_ref {
            generics.params.insert(0, parse_quote!(#lt))
        }
        let substruct = ItemStruct {
            attrs: Vec::new(),
            vis: Visibility::Inherited,
            struct_token: Default::default(),
            ident: substruct_ident.clone(),
            generics: generics.clone(),
            fields: Fields::Named(FieldsNamed {
                brace_token: Default::default(),
                named: subfields.iter().cloned().collect(),
            }),
            semi_token: None,
        };
        Some((substruct, subfields))
    } else {
        None
    }
}

trait Adt {
    fn all_fields(&self) -> Vec<&Field>;

    fn extract_parse_inner(
        &self,
        syan: &Path,
        ident: &Ident,
        tp_error_final: &Type,
        f: impl FnMut(&[(Member, Ident, &Field)]) -> TokenStream,
    ) -> TokenStream;

    fn extract_unparse_inner(
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
    ) -> TokenStream {
        assert!(generics.where_clause.is_none());
        let tp_atom: Ident = parse_quote!(__SyanMacro_Atom);
        let trait_fullpath: Path = parse_quote!(#syan::parse::parse::Parse<#tp_atom>);
        let mut generic_params = generics.params.clone();
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
        let tp_error_final: Ident = parse_quote!(__SyanError);
        let mut tp_error_combi_generator = (0..).map(|n| {
            (
                Ident::new(&format!("__SyanError_{n}"), Span::call_site()),
                Ident::new(&format!("__SyanErrorMerged_{n}"), Span::call_site()),
            )
        });

        let error_fixed = self
            .all_fields()
            .iter()
            .any(|f| f.find_attribute("joint").is_some() || f.find_attribute("alone").is_some());

        let mut tp_error_merged_last = tp_error_final.clone();

        let tp_error_final = if error_fixed {
            where_predicates.push(parse_quote!(#tp_atom: #syan::span::Spanned));
            parse_quote!(#syan::error::ParseError<<#tp_atom as #syan::span::Spanned>::Span>)
        } else {
            generic_params.push(parse_quote!(#tp_error_final));
            parse_quote!(#tp_error_final)
        };
        let mut tp_error_hist = Vec::new();
        let mut substructs: Vec<ItemStruct> = Vec::new();

        fn generate_error_mapper(
            syan: &Path,
            tp_error_hist: &[Ident],
            arg: &TokenStream,
        ) -> TokenStream {
            assert!(!tp_error_hist.is_empty());
            if tp_error_hist.len() == 1 {
                quote! { <#{&tp_error_hist[0]} as #syan::error::UnionWith<_>>::from_left(#arg) }
            } else {
                quote! { <#{&tp_error_hist[0]} as #syan::error::UnionWith<_>>::from_right(#{
                    generate_error_mapper(syan, &tp_error_hist[1..], arg)
                }) }
            }
        }

        let inner = self.extract_parse_inner(syan, ident,&tp_error_final, |fields| {
            let mut ret = quote!();

            let mut fields: VecDeque<_> = fields.iter().cloned().collect();
            while let Some((member, field_ident, field)) = fields.pop_front() {
                // check if the toplevel field has no `#[group(..)]` attr
                if let Some(group_member) = field.find_group() {
                    abort!(
                        &group_member,
                        "Cannot find member {} in struct {ident}",
                        quote!(#group_member)
                    );
                }

                let (tp_error, tp_error_merged) = tp_error_combi_generator.next().unwrap();
                tp_error_hist.push(tp_error.clone());

                let v_error = quote!(e);
                let err_mapper = if error_fixed {
                    quote!(#syan::error::UnionWith::<::core::core::convert::Infallible>::from_left(#v_stream))
                } else {
                    generate_error_mapper(syan, &tp_error_hist, &v_error)
                };

                let spacing = match (field.find_attribute("joint"), field.find_attribute("alone")) {
                    (None, None) => None,
                    (Some(_),None) => Some(Spacing::Joint),
                    (None, Some(_)) => Some(Spacing::Alone),
                    (Some(o1), Some(o2)) => abort!(quote!{#o1, #o2}, "Cannot implement both #[joint] and #[alone] to field `{}`", quote!{#{&field.ident}}),
                };

                let to_parse_ty = if let Some((substruct, subfields)) =
                    generate_substruct(&member, generics, ident, &field_ident, &mut fields, nonce, false)
                {
                    if spacing.is_some() {
                        abort!(&field, "Cannot specify #[joint] or #[alonw] to field {}", quote!(#{&field.ident}));
                    }
                    let field_ty = &field.ty;
                    let substruct_ident = &substruct.ident;
                    let field_ty_to_parse = parse_quote! {<#field_ty as #syan::nested::group::EmptyGroup>::Fill<
                        #substruct_ident  #ty_generics
                    >};
                    ret.extend(quote!(
                        let #field_ident: #field_ty_to_parse = ::core::result::Result::map_err(
                            #syan::parse::parse::Parse::parse(&mut #v_stream),
                            |#v_error| #err_mapper
                        )?;
                        let (#{ &substruct.ident } {
                            #(for subfield in &subfields) { #{&subfield.ident.as_ref().unwrap()}, }
                        }, #field_ident) = #syan::nested::group::EmptyGroup::unfill(#field_ident);
                    ));

                    substructs.push(substruct);
                    where_predicates.push(parse_quote!(#field_ty: #syan::nested::group::EmptyGroup));
                    field_ty_to_parse
                } else {
                    ret.extend(quote!(
                        #(if let Some(spacing) = spacing) {
                            let #field_ident = #syan::parse::parse_stream::ParseStream::validate_spacing(
                                &mut #v_stream,
                                #{spacing == Spacing::Joint}
                            )?;
                        }
                        let #field_ident = ::core::result::Result::map_err(
                            #syan::parse::parse::Parse::parse(&mut #v_stream),
                            |#v_error| #err_mapper
                        )?;
                    ));
                    field.ty.clone()
                };

                if !error_fixed {
                    where_predicates.push(parse_quote!(#to_parse_ty: #syan::parse::parse::Parse<#tp_atom, Error = #tp_error>));
                    generic_params.push(parse_quote!(#tp_error));
                    where_predicates.push(parse_quote!(#tp_error: #syan::error::UnionWith<#tp_error_merged, Output = #tp_error_merged_last>));
                    generic_params.push(parse_quote!(#tp_error_merged));
                    tp_error_merged_last = tp_error_merged;
                } else {
                    where_predicates.push(parse_quote!(#to_parse_ty: #syan::parse::parse::Parse<#tp_atom>));
                }
            }
            ret
        });
        if !error_fixed {
            where_predicates.push(parse_quote!(::core::convert::Infallible: #syan::error::UnionWith<::core::convert::Infallible, Output = #tp_error_merged_last>));
            where_predicates.push(parse_quote!(#tp_error_final: #syan::error::Error));
        }
        quote! {
            #(for substruct in &substructs) {
                #[derive(#syan::parse::parse::Parse)]
                #[syan(#syan)]
                #substruct
            }
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
        nonce: u64,
    ) -> TokenStream {
        let tp_atom: Ident = parse_quote!(__SyanMacro_Atom);
        let trait_fullpath: Path = parse_quote!(#syan::parse::unparse::Unparse<#tp_atom>);
        let mut generic_params = generics.params.clone();
        generic_params.push(parse_quote!(#tp_atom));
        let ty_generics = generics.split_for_impl().1;
        proc_macro_error::append_dummy(quote! {
            impl< #generic_params > #trait_fullpath for #ident #ty_generics {
                fn unparse<S: #syan::parse::unparse::Emitter<#tp_atom>>(&self, _: &mut S) -> ::core::result::Result<(), S::Error> {
                    ::core::unimplemented!()
                }
            }
        });
        let field_tys = self
            .all_fields()
            .into_iter()
            .map(|field| field.ty.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let where_predicates = field_tys
            .iter()
            .map(|ty| -> WherePredicate {
                parse_quote!(#ty: #syan::parse::unparse::Unparse<#tp_atom>)
            })
            .collect::<Punctuated<WherePredicate, Token![,]>>();
        let v_sink: Ident = parse_quote!(__syan_sink);
        let v_self: TokenStream = quote!(self);
        let mut substructs = Vec::new();
        let inner = self.extract_unparse_inner(ident, &v_self, |fields| {
            let mut ret = quote!();
            let mut fields: VecDeque<_> = fields.iter().cloned().collect();

            while let Some((member, field_ident, field)) = fields.pop_front() {

                // Check if this field has grouped subfields (though for unparse we don't generate substructs)
                if let Some((substruct, subfields)) =
                    generate_substruct(&member, generics, ident, &field_ident, &mut fields, nonce, true)
                {
                    ret.extend(quote! {
                        use #syan::nested::group::EmptyGroup as _;
                        let #field_ident = <#{&field.ty} as #syan::nested::group::EmptyGroup>::Fill::fill(
                            #field_ident.clone(),
                            #{&substruct.ident} {
                                #(for subfield in &subfields) { #{&subfield.ident}, }
                            }
                        );
                    });
                    substructs.push(substruct);
                }
                ret.extend(quote!(
                    #syan::parse::unparse::Unparse::unparse(&#field_ident, #v_sink)?;
                ));
            }
            ret
        });

        quote! {
            #(for substruct in &substructs) {
                #[derive(#syan::parse::unparse::Unparse)]
                #[syan(#syan)]
                #substruct
            }
            impl< #generic_params > #trait_fullpath for #ident #ty_generics
            #(if !where_predicates.is_empty()) { where #where_predicates}
            {
                fn unparse<S: #syan::parse::unparse::Emitter<#tp_atom>>(&self, #v_sink: &mut S) -> ::core::result::Result<(), S::Error> {
                    #inner
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

    fn extract_unparse_inner(
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
            ::core::result::Result::Ok(())
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
        mut f: impl FnMut(&[(Member, Ident, &Field)]) -> TokenStream,
    ) -> TokenStream {
        let variants = self.variants.iter().map(|v| {
            let fields = map_fields_to_idents(&v.fields);
            let inner = f(&fields[..]);
            (v, fields, inner)
        });
        quote! {
            let mut __syan_errors = ::std::vec::Vec::new();
            #(for (variant, fields, inner) in variants) {
                match (|| {
                    #inner
                    ::core::result::Result::Ok(
                        #ident :: #{ &variant.ident }
                        #(if let Fields::Unnamed(_) = &variant.fields) {
                            (#(for (_, field_ident, _) in &fields) {#field_ident,})
                        }
                        #(if let Fields::Named(_) = &variant.fields) {
                            {#(for (_, field_ident, _) in &fields) {#field_ident,}}
                        }
                    )
                })() {
                    ::core::result::Result::Err(err) => {
                        __syan_errors.push(err);
                    }
                    ok => { return ok; }
                }
            }
            ::core::result::Result::Err(
                <#tp_error_final as #syan::error::Error>::from_cause(__syan_errors)
            )
        }
    }

    fn extract_unparse_inner(
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
                        {
                            #inner
                        };
                        ::core::result::Result::Ok(())
                    }
                }
            }
        }
    }
}

pub fn parse(input: &DeriveInput, nonce: u64) -> TokenStream {
    let syan = input.attrs.get_syan();
    match &input.data {
        Data::Struct(data_struct) => {
            data_struct.extract_parse(&syan, &input.generics, &input.ident, nonce)
        }
        Data::Enum(data_enum) => {
            data_enum.extract_parse(&syan, &input.generics, &input.ident, nonce)
        }
        _ => abort!(input, "Bad data"),
    }
}

pub fn unparse(input: &DeriveInput, nonce: u64) -> TokenStream {
    let syan = input.attrs.get_syan();
    match &input.data {
        Data::Struct(data_struct) => {
            data_struct.extract_unparse(&syan, &input.generics, &input.ident, nonce)
        }
        Data::Enum(data_enum) => {
            data_enum.extract_unparse(&syan, &input.generics, &input.ident, nonce)
        }
        _ => abort!(input, "Bad data"),
    }
}

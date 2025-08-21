use proc_macro2::{Spacing, Span, TokenStream};
use proc_macro_error::abort;
use std::collections::VecDeque;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit::Visit;
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

    fn has_default(&self) -> bool {
        self.find_attribute("default").is_some()
    }
}

fn extract_symbol_token_type_params(ty: &Type, collected: &mut std::collections::HashSet<Ident>) {
    match ty {
        Type::Path(type_path) => {
            if let Some(segment) = type_path.path.segments.last() {
                // Check if this looks like a Symbol or Token macro invocation
                if segment.ident == "Symbol" || segment.ident == "Token" {
                    if let PathArguments::AngleBracketed(args) = &segment.arguments {
                        for arg in &args.args {
                            if let GenericArgument::Type(Type::Path(inner_path)) = arg {
                                if let Some(ident) = inner_path.path.get_ident() {
                                    collected.insert(ident.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
        Type::Array(array) => extract_symbol_token_type_params(&array.elem, collected),
        Type::Slice(slice) => extract_symbol_token_type_params(&slice.elem, collected),
        Type::Tuple(tuple) => {
            for elem in &tuple.elems {
                extract_symbol_token_type_params(elem, collected);
            }
        }
        Type::Group(group) => extract_symbol_token_type_params(&group.elem, collected),
        Type::Paren(paren) => extract_symbol_token_type_params(&paren.elem, collected),
        Type::Ptr(ptr) => extract_symbol_token_type_params(&ptr.elem, collected),
        Type::Reference(reference) => extract_symbol_token_type_params(&reference.elem, collected),
        _ => {}
    }
}

fn collect_primitive_tys(ty: &Type) -> impl Iterator<Item = Type> {
    #[derive(Default)]
    struct TypeCollector {
        types: Vec<Type>,
    }

    impl<'ast> Visit<'ast> for TypeCollector {
        fn visit_type(&mut self, ty: &'ast Type) {
            if let Type::Macro(type_macro) = ty {
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

fn add_type_param_predicates(
    where_predicates: &mut Punctuated<WherePredicate, Token![,]>,
    generics: &Generics,
    syan: &Path,
    tp_atom: &Ident,
    for_parse: bool,
    for_unparse: bool,
    for_spanned: bool,
    tp_span: Option<&Ident>,
) {
    for param in &generics.params {
        if let GenericParam::Type(type_param) = param {
            // Check if the type parameter has any bounds
            if type_param.bounds.is_empty() {
                let ty = &type_param.ident;
                if for_parse {
                    where_predicates.push(parse_quote!(#ty: #syan::parse::parse::Parse<#tp_atom>));
                }
                if for_unparse {
                    where_predicates
                        .push(parse_quote!(#ty: #syan::parse::unparse::Unparse<#tp_atom>));
                }
                if for_spanned {
                    if let Some(span) = tp_span {
                        where_predicates
                            .push(parse_quote!(#ty: #syan::span::Spanned<Span = #span>));
                    } else {
                        where_predicates.push(parse_quote!(#ty: #syan::span::Spanned));
                    }
                }
            }
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
                        #field_phantom: ::core::marker::PhantomData<(#phantom_args,)>
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

trait Adt {
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
        _input_attrs: &[Attribute],
    ) -> TokenStream {
        assert!(generics.where_clause.is_none());
        let tp_atom: Ident = parse_quote!(__SyanMacro_Atom);
        let trait_fullpath: Path = parse_quote!(#syan::parse::parse::Parse<#tp_atom>);
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
                fn convert_error(_: Self::Error) -> #syan::error::ParseError<<#tp_atom as #syan::span::Spanned>::Span>
                where
                    #tp_atom: #syan::span::Spanned,
                {
                    ::core::unimplemented!()
                }
            }
        });
        let mut where_predicates: Punctuated<WherePredicate, token::Comma> = Punctuated::new();
        let v_stream: Ident = parse_quote!(__syan_stream);
        let mut wrapper_counter = 0usize;

        where_predicates.push(parse_quote!(#tp_atom: #syan::span::Spanned));
        let tp_error_final: Type =
            parse_quote!(#syan::error::ParseError<<#tp_atom as #syan::span::Spanned>::Span>);
        let mut substructs: Vec<ItemStruct> = Vec::new();

        // Add where predicates for unbounded type parameters
        add_type_param_predicates(
            &mut where_predicates,
            generics,
            syan,
            &tp_atom,
            true,
            false,
            false,
            None,
        );

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

                for ty in collect_primitive_tys(&field.ty) {
                    where_predicates.push(parse_quote!{
                        #ty: #trait_fullpath
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

                let to_parse_ty = if let Some((substruct, _)) = &substruct {
                    if spacing.is_some() {
                        abort!(&field, "Cannot specify #[joint] or #[alonw] to field {}", quote!(#{&field.ident}));
                    }
                    let substruct_ident = &substruct.ident;
                    let field_ty_to_parse = parse_quote! {<#field_ty as #syan::nested::group::EmptyGroup>::Fill<
                        #substruct_ident  #ty_generics
                    >};
                    field_ty_to_parse
                } else {
                    field.ty.clone()
                };

                let v_error = quote!(e);
                let err_mapper = quote!(<#to_parse_ty as #syan::parse::parse::Parse<#tp_atom>>::convert_error(#v_error));

                if let Some((substruct, subfields)) = substruct {
                    ret.extend(quote!(
                        let #field_ident: #to_parse_ty = ::core::result::Result::map_err(
                            <#to_parse_ty as #syan::parse::parse::Parse<#tp_atom>>::parse(&mut #v_stream),
                            |#v_error| #err_mapper
                        )?;
                        let (#{ &substruct.ident } {
                            #(for subfield in subfields) { #{&subfield.ident.as_ref().unwrap()}, }
                            #field_phantom: _
                        }, #field_ident) = #syan::nested::group::EmptyGroup::unfill(#field_ident);
                    ));

                    substructs.push(substruct);
                    where_predicates.push(parse_quote!(#field_ty: #syan::nested::group::EmptyGroup));
                } else {
                    ret.extend(quote!(
                        #(if let Some(spacing) = spacing) {
                            let #field_ident = #syan::parse::parse_stream::ParseStream::validate_spacing(
                                &mut #v_stream,
                                #{spacing == Spacing::Joint}
                            )?;
                        }
                        let #field_ident = ::core::result::Result::map_err(
                            <#to_parse_ty as #syan::parse::parse::Parse<#tp_atom>>::parse(&mut #v_stream),
                            |#v_error| #err_mapper
                        )?;
                    ));
                }
                    wrapper_counter += 1;

            }
            ret
        });
        quote! {
            #(for substruct in &substructs) {
                #[derive(#syan::parse::parse::Parse)]
                #[syan(#syan)]
                #substruct
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
                fn convert_error(_: Self::Error) -> #syan::error::ParseError<<#tp_atom as #syan::span::Spanned>::Span>
                where
                    #tp_atom: #syan::span::Spanned,
                {
                    ::core::unimplemented!()
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
        _input_attrs: &[Attribute],
    ) -> TokenStream {
        let tp_atom: Ident = parse_quote!(__SyanMacro_Atom);
        let trait_fullpath: Path = parse_quote!(#syan::parse::unparse::Unparse<#tp_atom>);
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

        // Add where predicates for unbounded type parameters
        add_type_param_predicates(
            &mut where_predicates,
            generics,
            syan,
            &tp_atom,
            false,
            true,
            false,
            None,
        );

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

                for ty in collect_primitive_tys(&field.ty) {
                    where_predicates.push(parse_quote!{
                        #ty: #trait_fullpath
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
                        use #syan::nested::group::EmptyGroup as _;
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
                    where_predicates.push(parse_quote!(for<'syan_substruct_ref> <#field_ty as #syan::nested::group::EmptyGroup>::Fill<#fill_ty>: #syan::parse::unparse::Unparse<#tp_atom>));
                    substructs.push(substruct);
                }
                ret.extend(quote!(
                    #syan::parse::unparse::Unparse::unparse(&#field_ident, #v_sink)?;
                ));
            }
            quote! {
                #ret
                ::core::result::Result::Ok(())
            }
        });

        quote! {
            #(for substruct in &substructs) {
                #[derive(#syan::parse::unparse::Unparse)]
                #[syan(#syan)]
                #substruct
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
        _input_attrs: &[Attribute],
    ) -> TokenStream {
        let trait_fullpath: Path = parse_quote!(#syan::span::Spanned);
        let ty_generics = generics.split_for_impl().1;
        let mut generic_params = generics.params.clone();
        let mut where_predicates: Punctuated<WherePredicate, token::Comma> = Punctuated::new();

        let tp_span: Ident = parse_quote!(__Syan_Span);

        // Add where predicates for unbounded type parameters
        add_type_param_predicates(
            &mut where_predicates,
            generics,
            syan,
            &tp_span,
            false,
            false,
            true,
            Some(&tp_span),
        );
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
            for (_, _, Field { attrs, .. }) in fields {
                // Skip fields with #[default] attribute - they don't contribute to span
                if attrs.has_default() {
                    continue;
                }
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

pub fn parse(input: &DeriveInput, nonce: u64) -> TokenStream {
    let syan = input.attrs.get_syan();
    match &input.data {
        Data::Struct(data_struct) => {
            data_struct.extract_parse(&syan, &input.generics, &input.ident, nonce, &input.attrs)
        }
        Data::Enum(data_enum) => {
            data_enum.extract_parse(&syan, &input.generics, &input.ident, nonce, &input.attrs)
        }
        _ => abort!(input, "Bad data"),
    }
}

pub fn unparse(input: &DeriveInput, nonce: u64) -> TokenStream {
    let syan = input.attrs.get_syan();
    match &input.data {
        Data::Struct(data_struct) => {
            data_struct.extract_unparse(&syan, &input.generics, &input.ident, nonce, &input.attrs)
        }
        Data::Enum(data_enum) => {
            data_enum.extract_unparse(&syan, &input.generics, &input.ident, nonce, &input.attrs)
        }
        _ => abort!(input, "Bad data"),
    }
}

pub fn spanned(input: &DeriveInput) -> TokenStream {
    let syan = input.attrs.get_syan();
    match &input.data {
        Data::Struct(data_struct) => {
            data_struct.extract_spanned(&syan, &input.generics, &input.ident, &input.attrs)
        }
        Data::Enum(data_enum) => {
            data_enum.extract_spanned(&syan, &input.generics, &input.ident, &input.attrs)
        }
        _ => abort!(input, "Bad data"),
    }
}

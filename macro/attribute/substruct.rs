use super::*;

#[allow(clippy::too_many_arguments)]
pub(crate) fn generate_substruct(
    member: &Member,
    generics: &Generics,
    ident: &Ident,
    field_ident: &Ident,
    field_phantom: &Ident,
    fields: &mut VecDeque<(Member, Ident, &Field)>,
    nonce: u64,
    by_ref: bool,
    // The enclosing enum variant, when the field lives inside one — mixed into the substruct name so
    // that two variants with a same-named, same-shaped `#[group(..)]` field don't collide (E0428): the
    // name would otherwise only depend on the field name + owning type + derive nonce, all shared
    // across variants.
    variant_ident: Option<&Ident>,
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
        // Field index (the `Member`'s tuple position, or 0 for a named field, where the name already
        // disambiguates) alongside the variant, per the two collision axes above.
        let member_index = match member {
            Member::Named(_) => 0,
            Member::Unnamed(index) => index.index,
        };
        let variant_key = variant_ident.map(|v| format!("{v}_")).unwrap_or_default();
        let substruct_ident = Ident::new(
            &format!("__SyanSubstructOf_{variant_key}{field_ident}_{member_index}_{ident}_{nonce}"),
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

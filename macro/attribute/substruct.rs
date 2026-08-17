use super::*;

#[allow(clippy::too_many_arguments)]
pub(crate) fn generate_substruct(
    member: &Member,
    generics: &Generics,
    ident: &Ident,
    // The enum variant these fields came from, or `None` for a struct — and for an enum's
    // prefix-dedup prefix, which is parsed once for all variants. See the naming note below.
    variant: Option<&Ident>,
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
                subfield
                    .attrs
                    .retain(|attr| !attr.find_group().map(|g| &g == member).unwrap_or(false));
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
        // `by_ref` is part of the NAME, not just of the shape. The two substructs generated for one
        // `#[group]` field are genuinely different types — `Parse` needs its fields owned (it builds
        // the value while parsing), `Unparse` needs them borrowed and carries an extra lifetime (it
        // emits from `&self` without cloning) — so they cannot share a definition and must not share
        // an ident. Encoding it here is what lets a caller driving BOTH derives from one expansion
        // (`#[recurse]`) use a single nonce; the distinction is semantic, so it belongs in the name
        // rather than hidden in a per-trait nonce perturbation.
        // The VARIANT is part of the name too. Without it the name was a function of
        // `(shape, group-field name, enum name)` only, so two variants of one enum whose `#[group]`
        // holders share a field name were indistinguishable: the second definition redefined the
        // first (E0428), their impls collided (E0119), and the survivor carried the other variant's
        // fields — surfacing as `struct … does not have a field named 'x'` on a line that is
        // perfectly correct. Worse, when the two variants' subfield lists happened to be identical
        // NOTHING errored and one variant silently drove the other's generated code.
        let shape = if by_ref { "Ref" } else { "Own" };
        let scope = match variant {
            Some(v) => format!("{ident}_{v}"),
            None => format!("{ident}"),
        };
        let substruct_ident = Ident::new(
            &format!("__SyanSubstructOf{shape}_{field_ident}_{scope}_{nonce}"),
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

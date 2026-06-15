use proc_macro2::{Literal, Span, TokenStream};
use syn::*;
use template_quote::quote;
use type_leak::Leaker;

/// Convert a CamelCase / PascalCase identifier to snake_case (for the hidden macro name).
pub(crate) fn to_snake(ident: &Ident) -> String {
    let s = ident.to_string();
    let mut out = String::with_capacity(s.len() + 4);
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() {
            if i != 0 {
                out.push('_');
            }
            out.extend(ch.to_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// Produce a cleaned copy of the input definition (all attributes stripped) so it can be embedded
/// verbatim inside the metadata `macro_rules!` and re-parsed as a `syn::Item` by `__visitor_build`.
fn cleaned_definition(input: &DeriveInput) -> DeriveInput {
    let mut di = input.clone();
    di.attrs.clear();
    di.vis = Visibility::Public(Default::default());
    match &mut di.data {
        Data::Enum(e) => {
            for v in &mut e.variants {
                v.attrs.clear();
                for f in &mut v.fields {
                    f.attrs.clear();
                    f.vis = Visibility::Inherited;
                }
            }
        }
        Data::Struct(s) => {
            for f in &mut s.fields {
                f.attrs.clear();
                f.vis = Visibility::Inherited;
            }
        }
        Data::Union(u) => {
            for f in &mut u.fields.named {
                f.attrs.clear();
                f.vis = Visibility::Inherited;
            }
        }
    }
    di
}

/// Generic params with defaults stripped (for `impl<...>` / `struct <...>` headers).
fn gparams(g: &Generics) -> Vec<GenericParam> {
    g.params
        .iter()
        .cloned()
        .map(|mut p| {
            match &mut p {
                GenericParam::Type(t) => {
                    t.eq_token = None;
                    t.default = None;
                }
                GenericParam::Const(c) => {
                    c.eq_token = None;
                    c.default = None;
                }
                _ => {}
            }
            p
        })
        .collect()
}

/// Use-side generic args (idents / lifetimes).
fn gargs(g: &Generics) -> Vec<TokenStream> {
    g.params
        .iter()
        .map(|p| match p {
            GenericParam::Lifetime(l) => {
                let lt = &l.lifetime;
                quote!(#lt)
            }
            GenericParam::Type(t) => {
                let i = &t.ident;
                quote!(#i)
            }
            GenericParam::Const(c) => {
                let i = &c.ident;
                quote!(#i)
            }
        })
        .collect()
}

/// A `PhantomData` payload that mentions every generic param (so the leaker marker has no
/// unconstrained parameters).
fn phantom_payload(g: &Generics) -> TokenStream {
    let elems = g.params.iter().map(|p| match p {
        GenericParam::Lifetime(l) => {
            let lt = &l.lifetime;
            quote!(& #lt ())
        }
        GenericParam::Type(t) => {
            let i = &t.ident;
            quote!(#i)
        }
        GenericParam::Const(c) => {
            let i = &c.ident;
            quote!([(); #i])
        }
    });
    quote!( ::core::marker::PhantomData<( #(#elems,)* )> )
}

/// Build a `type_leak::Referrer` for the definition (the ordered list of field types that depend on
/// the definition's type context). `None` if type-leak can't analyze it (e.g. a union, or a
/// not-internable contradiction); the derive then simply omits the leaker.
fn build_referrer(input: &DeriveInput) -> Option<type_leak::Referrer> {
    let mut leaker = match &input.data {
        Data::Struct(ds) => {
            let item = ItemStruct {
                attrs: vec![],
                vis: Visibility::Inherited,
                struct_token: ds.struct_token,
                ident: input.ident.clone(),
                generics: input.generics.clone(),
                fields: ds.fields.clone(),
                semi_token: ds.semi_token,
            };
            Leaker::from_struct(&item).ok()?
        }
        Data::Enum(de) => {
            let item = ItemEnum {
                attrs: vec![],
                vis: Visibility::Inherited,
                enum_token: de.enum_token,
                ident: input.ident.clone(),
                generics: input.generics.clone(),
                brace_token: de.brace_token,
                variants: de.variants.clone(),
            };
            Leaker::from_enum(&item).ok()?
        }
        Data::Union(_) => return None,
    };
    leaker.reduce_roots();
    Some(leaker.finish())
}

/// `#[derive(Ast)]` expansion.
///
/// Emits:
/// * `impl Ast for T<..> {}` (the empty marker trait from `syan::visit`),
/// * a `type-leak` leaker marker + `Repeater<N>` impls carrying each context-dependent field type
///   out of the definition's type context (so it can be named portably as
///   `<leaker as Repeater<N>>::Type`),
/// * a `#[macro_export]` callback metadata `macro_rules!` carrying a cleaned copy of the
///   definition, and
/// * a macro-namespace re-export under the type's own name so a generated visitor can reach it as
///   `path::to::T! { .. }`.
pub fn derive_ast(input: &DeriveInput, nonce: u64, syan: &Path) -> TokenStream {
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let cleaned = cleaned_definition(input);
    let macro_name = Ident::new(&format!("__{}_ast_{}", to_snake(ident), nonce), Span::call_site());
    let leaker_ident = Ident::new(
        &format!("__{}_leaker_{}", to_snake(ident), nonce),
        Span::call_site(),
    );

    // type-leak: leaker marker + one `Repeater<N>` impl per leaked field type.
    let referrer = build_referrer(input);
    let leaker_items: TokenStream = if let Some(referrer) = &referrer {
        let g_params = gparams(&input.generics);
        let g_args = gargs(&input.generics);
        let phantom = phantom_payload(&input.generics);
        let leak_tys: Vec<&Type> = referrer.iter().collect();
        quote! {
            #[doc(hidden)]
            #[allow(non_camel_case_types, dead_code)]
            pub struct #leaker_ident < #(#g_params),* > ( #phantom );

            #(for (n, ty) in leak_tys.iter().enumerate()) {
                #[automatically_derived]
                impl < #(#g_params),* > #syan::visit::Repeater< #{Literal::usize_unsuffixed(n)} >
                    for #leaker_ident < #(#g_args),* > #where_clause
                {
                    type Type = #ty;
                }
            }
        }
    } else {
        quote!()
    };

    quote! {
        #[automatically_derived]
        impl #impl_generics #syan::visit::Ast for #ident #ty_generics #where_clause {}

        #leaker_items

        #[macro_export]
        #[doc(hidden)]
        macro_rules! #macro_name {
            // Callback muncher: append this type's metadata, then re-invoke the continuation `$cb`.
            (@ast $cb:path { $($pre:tt)* }) => {
                $cb ! {
                    $($pre)*
                    @ast { #cleaned }
                }
            };
        }

        #[doc(hidden)]
        #{ &input.vis } use #macro_name as #ident;
    }
}

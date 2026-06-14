use proc_macro2::{Span, TokenStream};
use syn::*;
use template_quote::quote;

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

/// `#[derive(Ast)]` expansion (Stage 2).
///
/// Emits:
/// * `impl Ast for T<..> {}` (the empty marker trait from `syan::visit`),
/// * a `#[macro_export]` callback metadata `macro_rules!` carrying a cleaned copy of the
///   definition, and
/// * a macro-namespace re-export under the type's own name so a generated visitor can reach it as
///   `path::to::T! { .. }`.
pub fn derive_ast(input: &DeriveInput, nonce: u64, syan: &Path) -> TokenStream {
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let cleaned = cleaned_definition(input);
    let macro_name = Ident::new(&format!("__{}_ast_{}", to_snake(ident), nonce), Span::call_site());

    quote! {
        #[automatically_derived]
        impl #impl_generics #syan::visit::Ast for #ident #ty_generics #where_clause {}

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

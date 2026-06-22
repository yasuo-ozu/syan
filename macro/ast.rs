use proc_macro2::{Literal, Span, TokenStream};
use proc_macro_error::{abort, emit_warning};
use std::collections::HashMap;
use syn::parse::{Parse, ParseStream, Parser};
use syn::punctuated::Punctuated;
use syn::*;
use template_quote::quote;
use type_leak::Leaker;

/// One `#[subast(..)]` entry: a path to a sub-AST type, optionally aliased (`b::Foo as BFoo`). The
/// `matchkey` (the alias, or the path's last segment) is the ident a field head is matched against;
/// `path` is the resolvable path used to fetch that sub-AST's metadata macro / as a drill scrutinee.
struct SubastEntry {
    path: Path,
    alias: Option<Ident>,
}

impl Parse for SubastEntry {
    fn parse(input: ParseStream) -> Result<Self> {
        let path: Path = input.parse()?;
        let alias = if input.peek(Token![as]) {
            input.parse::<Token![as]>()?;
            Some(input.parse()?)
        } else {
            None
        };
        Ok(SubastEntry { path, alias })
    }
}

impl SubastEntry {
    /// The ident a (container-peeled) field head is matched against.
    fn matchkey(&self) -> Ident {
        self.alias
            .clone()
            .unwrap_or_else(|| self.path.segments.last().unwrap().ident.clone())
    }
}

/// Collect the `#[subast(..)]` allowlist from the type's attributes. Two entries resolving to the
/// same `matchkey` is an error (a bare field head can't disambiguate them — alias one).
fn parse_subast(attrs: &[Attribute]) -> Vec<SubastEntry> {
    let mut entries = Vec::new();
    for attr in attrs {
        if !attr.path().is_ident("subast") {
            continue;
        }
        let list = match &attr.meta {
            Meta::List(ml) => ml.tokens.clone(),
            _ => abort!(attr, "`#[subast(..)]` takes a parenthesized list of paths"),
        };
        match Punctuated::<SubastEntry, Token![,]>::parse_terminated.parse2(list) {
            Ok(parsed) => entries.extend(parsed),
            Err(e) => abort!(e.span(), "invalid `#[subast(..)]`: {}", e),
        }
    }
    let mut seen: HashMap<String, ()> = HashMap::new();
    for e in &entries {
        let key = e.matchkey().to_string();
        if seen.insert(key.clone(), ()).is_some() {
            abort!(
                e.path,
                "two `#[subast(..)]` entries share the last segment `{}`; alias one (`path as Alias`)",
                key
            );
        }
    }
    entries
}

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
/// Collect every ident that appears as a path-segment head anywhere inside a field type (so
/// `Vec<Box<Stmt<S>>>` contributes `Vec`, `Box`, `Stmt`, `S`). Used only to warn about `#[subast]`
/// entries that match no field — an over-approximation, so it never false-warns.
fn collect_type_idents(ty: &Type, out: &mut std::collections::HashSet<String>) {
    match ty {
        Type::Path(tp) => {
            for seg in &tp.path.segments {
                out.insert(seg.ident.to_string());
                if let PathArguments::AngleBracketed(ab) = &seg.arguments {
                    for arg in &ab.args {
                        if let GenericArgument::Type(t) = arg {
                            collect_type_idents(t, out);
                        }
                    }
                }
            }
        }
        Type::Reference(r) => collect_type_idents(&r.elem, out),
        Type::Slice(s) => collect_type_idents(&s.elem, out),
        Type::Array(a) => collect_type_idents(&a.elem, out),
        Type::Paren(p) => collect_type_idents(&p.elem, out),
        Type::Group(g) => collect_type_idents(&g.elem, out),
        Type::Tuple(t) => {
            for e in &t.elems {
                collect_type_idents(e, out);
            }
        }
        _ => {}
    }
}

fn field_head_idents(input: &DeriveInput) -> std::collections::HashSet<String> {
    let mut out = std::collections::HashSet::new();
    let visit_fields = |fields: &Fields, out: &mut std::collections::HashSet<String>| {
        for f in fields {
            collect_type_idents(&f.ty, out);
        }
    };
    match &input.data {
        Data::Struct(s) => visit_fields(&s.fields, &mut out),
        Data::Enum(e) => {
            for v in &e.variants {
                visit_fields(&v.fields, &mut out);
            }
        }
        Data::Union(u) => {
            for f in &u.fields.named {
                collect_type_idents(&f.ty, &mut out);
            }
        }
    }
    out
}

pub fn derive_ast(input: &DeriveInput, nonce: u64, syan: &Path) -> TokenStream {
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // `#[subast(..)]` allowlist of this type's sub-AST children (+ their resolvable paths). Carried
    // verbatim in the metadata macro; the visitor matches field heads against it.
    let subast = parse_subast(&input.attrs);
    let field_heads = field_head_idents(input);
    for e in &subast {
        let key = e.matchkey();
        if !field_heads.contains(&key.to_string()) {
            emit_warning!(
                e.path,
                "`#[subast]` entry `{}` matches no field of `{}`",
                key,
                ident
            );
        }
    }
    let subast_tokens: Vec<TokenStream> = subast
        .iter()
        .map(|e| {
            let path = &e.path;
            let key = e.matchkey();
            quote!( #path as #key )
        })
        .collect();

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
                    @subast { #(#subast_tokens),* }
                }
            };
        }

        #[doc(hidden)]
        #{ &input.vis } use #macro_name as #ident;
    }
}

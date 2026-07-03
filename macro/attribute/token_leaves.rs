use super::*;

/// The parsed `#[token_leaf(atom = "..", span = "..")]` item attribute: the atom type and the
/// closure that reads a leaf's span from an `&atom`.
struct TokenLeafCfg {
    atom: Type,
    span: Expr,
}

/// The parsed `#[leaf(name = "..", expect = "..", [field = ".."])]` variant attribute.
struct LeafCfg {
    name: Ident,
    expect: LitStr,
    field: Option<Ident>,
}

/// Parse a `#[name(k = "v", ..)]` list attribute into its `MetaNameValue` entries.
fn name_value_list(attr: &Attribute) -> Punctuated<MetaNameValue, Token![,]> {
    match &attr.meta {
        Meta::List(list) => {
            match list.parse_args_with(Punctuated::<MetaNameValue, Token![,]>::parse_terminated) {
                Ok(nvs) => nvs,
                Err(e) => abort!(attr, "malformed attribute: {}", e),
            }
        }
        _ => abort!(attr, "expected `key = \"value\"` entries in parentheses"),
    }
}

/// The string content of a `key = "value"` entry (every value is a string literal, re-parsed per key).
fn str_value(nv: &MetaNameValue) -> LitStr {
    match &nv.value {
        Expr::Lit(ExprLit {
            lit: Lit::Str(s), ..
        }) => s.clone(),
        _ => abort!(&nv.value, "expected a string literal"),
    }
}

fn ident_value(lit: &LitStr) -> Ident {
    match lit.parse::<Ident>() {
        Ok(ident) => ident,
        Err(e) => abort!(lit, "not a valid identifier: {}", e),
    }
}

fn token_leaf_cfg(attrs: &[Attribute]) -> TokenLeafCfg {
    let attr = match attrs.find_attribute("token_leaf") {
        Some(a) => a,
        None => proc_macro_error::abort_call_site!(
            "`#[derive(TokenLeaves)]` requires `#[token_leaf(atom = \"..\", span = \"..\")]`"
        ),
    };
    let mut atom: Option<Type> = None;
    let mut span: Option<Expr> = None;
    for nv in name_value_list(attr) {
        let value = str_value(&nv);
        if nv.path.is_ident("atom") {
            atom = Some(match value.parse::<Type>() {
                Ok(t) => t,
                Err(e) => abort!(value, "`atom` is not a type: {}", e),
            });
        } else if nv.path.is_ident("span") {
            span = Some(match value.parse::<Expr>() {
                Ok(e) => e,
                Err(e) => abort!(value, "`span` is not an expression: {}", e),
            });
        } else {
            abort!(&nv.path, "unknown `token_leaf` key (expected `atom` or `span`)");
        }
    }
    match (atom, span) {
        (Some(atom), Some(span)) => TokenLeafCfg { atom, span },
        _ => abort!(attr, "`#[token_leaf(..)]` needs both `atom` and `span`"),
    }
}

fn leaf_cfg(variant: &Variant) -> Option<LeafCfg> {
    let attr = variant.attrs.find_attribute("leaf")?;
    let mut name: Option<Ident> = None;
    let mut expect: Option<LitStr> = None;
    let mut field: Option<Ident> = None;
    for nv in name_value_list(attr) {
        let value = str_value(&nv);
        if nv.path.is_ident("name") {
            name = Some(ident_value(&value));
        } else if nv.path.is_ident("expect") {
            expect = Some(value);
        } else if nv.path.is_ident("field") {
            field = Some(ident_value(&value));
        } else {
            abort!(&nv.path, "unknown `leaf` key (expected `name`, `expect`, or `field`)");
        }
    }
    match (name, expect) {
        (Some(name), Some(expect)) => Some(LeafCfg {
            name,
            expect,
            field,
        }),
        _ => abort!(attr, "`#[leaf(..)]` needs both `name` and `expect`"),
    }
}

/// The token-payload shape of an annotated variant. A unit variant becomes a span-only tuple leaf; a
/// single-field variant becomes a struct leaf carrying the cloned field plus a span.
enum Payload {
    Unit,
    Unnamed { field: Ident, ty: Type },
    Named { leaf_field: Ident, src: Ident, ty: Type },
}

fn payload_of(variant: &Variant, cfg: &LeafCfg) -> Payload {
    match &variant.fields {
        Fields::Unit => Payload::Unit,
        Fields::Unnamed(fields) if fields.unnamed.len() == 1 => Payload::Unnamed {
            field: cfg.field.clone().unwrap_or_else(|| parse_quote!(value)),
            ty: fields.unnamed[0].ty.clone(),
        },
        Fields::Named(fields) if fields.named.len() == 1 => {
            let src = fields.named[0].ident.clone().unwrap();
            Payload::Named {
                leaf_field: cfg.field.clone().unwrap_or_else(|| src.clone()),
                src,
                ty: fields.named[0].ty.clone(),
            }
        }
        _ => abort!(
            &variant.fields,
            "`#[leaf]` supports only unit variants and single-field variants"
        ),
    }
}

pub fn token_leaves(input: &DeriveInput, nonce: u64, syan: &Path) -> TokenStream {
    let data = match &input.data {
        Data::Enum(data) => data,
        _ => abort!(input, "`#[derive(TokenLeaves)]` only applies to enums"),
    };
    if !input.generics.params.is_empty() {
        abort!(
            &input.generics,
            "`#[derive(TokenLeaves)]` does not support generic token enums"
        );
    }
    let cfg = token_leaf_cfg(&input.attrs);
    let enum_ident = &input.ident;
    let vis = &input.vis;
    let atom = &cfg.atom;
    let read_span = &cfg.span;
    let span_ty: Type = parse_quote!(<#atom as #syan::span::Spanned>::Span);
    // A private alias so the atom can be rebuilt by struct-literal (`Ctor { slot, span }`) regardless of
    // whether `#atom` carries generic arguments — a bare generic type path in expression position would
    // need a turbofish. The atom is `WithSpan<Self, S>`-shaped (public `slot: Self`, `span: S`).
    let atom_ctor = Ident::new(&format!("__SyanTokenLeafAtom_{nonce}"), Span::call_site());

    let mut items = TokenStream::new();
    for variant in &data.variants {
        let Some(cfg) = leaf_cfg(variant) else {
            continue;
        };
        let variant_ident = &variant.ident;
        let leaf = &cfg.name;
        let expect = &cfg.expect;

        // Per-variant: the leaf's own body + terminator, the pattern matching this variant's atom slot
        // (binding the payload as `__payload`), the `Ok` value built on a match, the `Spanned` body, and
        // the token value rebuilt for `Unparse`.
        let (body, pat, ok_value, span_body, token_expr): (
            TokenStream,
            TokenStream,
            TokenStream,
            TokenStream,
            TokenStream,
        ) = match payload_of(variant, &cfg) {
            Payload::Unit => (
                quote!((#vis #span_ty);),
                quote!(#enum_ident::#variant_ident),
                quote!(#leaf(__span)),
                quote!(::core::clone::Clone::clone(&self.0)),
                quote!(#enum_ident::#variant_ident),
            ),
            Payload::Unnamed { field, ty } => (
                quote!({ #vis #field: #ty, #vis span: #span_ty }),
                quote!(#enum_ident::#variant_ident(__payload)),
                quote!(#leaf { #field: ::core::clone::Clone::clone(__payload), span: __span }),
                quote!(::core::clone::Clone::clone(&self.span)),
                quote!(#enum_ident::#variant_ident(::core::clone::Clone::clone(&self.#field))),
            ),
            Payload::Named {
                leaf_field,
                src,
                ty,
            } => (
                quote!({ #vis #leaf_field: #ty, #vis span: #span_ty }),
                quote!(#enum_ident::#variant_ident { #src: __payload }),
                quote!(#leaf { #leaf_field: ::core::clone::Clone::clone(__payload), span: __span }),
                quote!(::core::clone::Clone::clone(&self.span)),
                quote!(#enum_ident::#variant_ident {
                    #src: ::core::clone::Clone::clone(&self.#leaf_field)
                }),
            ),
        };

        items.extend(quote! {
            #[derive(::core::clone::Clone, ::core::fmt::Debug)]
            #vis struct #leaf #body

            impl #syan::parse::parse::Parse<#atom> for #leaf {
                type Error = #syan::error::ParseError;
                fn parse(
                    __source: impl #syan::parse::into_parse_stream::IntoParseStream<Atom = #atom>,
                ) -> ::core::result::Result<Self, Self::Error> {
                    let mut __stream =
                        #syan::parse::into_parse_stream::IntoParseStream::into_parse_stream(__source);
                    // Bind the `span` reader to a `fn` pointer so its `&atom` parameter type is fixed
                    // (a bare `|a| ..` closure can't infer it from an immediate call).
                    let __read_span: fn(&#atom) -> #span_ty = #read_span;
                    match #syan::parse::parse_stream::ParseStream::next(&mut __stream) {
                        ::core::option::Option::Some(__atom) => match &__atom.slot {
                            #pat => {
                                let __span = __read_span(&__atom);
                                ::core::result::Result::Ok(#ok_value)
                            }
                            _ => {
                                let __span = __read_span(&__atom);
                                #syan::parse::parse_stream::ParseStream::push(&mut __stream, __atom);
                                ::core::result::Result::Err(#syan::error::ParseError::new(
                                    __span,
                                    ::core::concat!("expected ", #expect),
                                ))
                            }
                        },
                        ::core::option::Option::None => ::core::result::Result::Err(
                            #syan::error::ParseError::new(
                                <#span_ty as ::core::default::Default>::default(),
                                ::core::concat!("unexpected end of input, expected ", #expect),
                            ),
                        ),
                    }
                }
            }

            impl #syan::parse::unparse::Unparse<#atom> for #leaf {
                fn unparse<__E: #syan::parse::unparse::Emitter<#atom>>(
                    &self,
                    __sink: &mut __E,
                ) -> ::core::result::Result<(), __E::Error> {
                    type #atom_ctor = #atom;
                    #syan::parse::unparse::Emitter::write_one(
                        __sink,
                        #atom_ctor { slot: #token_expr, span: #span_body },
                    )
                }
            }

            impl #syan::span::Spanned for #leaf {
                type Span = #span_ty;
                fn span(&self) -> Self::Span {
                    #span_body
                }
            }
        });
    }

    items
}

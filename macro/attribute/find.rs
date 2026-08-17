use super::*;

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
}

fn is_derive_helper_attr(attr: &Attribute) -> bool {
    [
        "group", "syan", "joint", "alone", "default",
        // `#[derive(Ast)]`'s view markers: strip them off a `#[group]`-cloned substruct (which carries no
        // `Ast` derive to register them), else `#[group] #[seq] Punctuated<..>` fails with "cannot find
        // attribute `seq`".
        "seq", "opt",
    ]
    .iter()
    .any(|n| attr.path().is_ident(n))
}

pub(crate) fn strip_derive_helper_attrs(substruct: &ItemStruct) -> ItemStruct {
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

/// Strip any type/const generic-param defaults. A default (e.g. the engine's
/// `__Rec = __ExprDefault<S>`) is only valid in the type *definition*; carried onto an `impl` header it
/// is an error (and a non-trailing one once an invented param like `__Syan_Span` is appended).
pub(crate) fn strip_param_defaults(params: &mut Punctuated<GenericParam, Token![,]>) {
    for param in params {
        match param {
            GenericParam::Type(type_param) => {
                type_param.eq_token = None;
                type_param.default = None;
            }
            GenericParam::Const(const_param) => {
                const_param.eq_token = None;
                const_param.default = None;
            }
            GenericParam::Lifetime(_) => {}
        }
    }
}

/// For each **unbounded** type param of `generics`, add `T: Spanned<Span = #tp_span>` to
/// `where_predicates` — the `Spanned` derive's per-param bound (pinning every param's span to the impl's
/// invented span type). A param that already carries bounds is left to the user. Only the `Spanned`
/// derive needs this (the `Parse`/`Unparse` derives synthesize their per-field bounds directly).
pub(crate) fn add_spanned_param_predicates(
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
pub(crate) fn append_user_where_predicates(
    where_predicates: &mut Punctuated<WherePredicate, Token![,]>,
    generics: &Generics,
) {
    if let Some(where_clause) = &generics.where_clause {
        for predicate in &where_clause.predicates {
            where_predicates.push(predicate.clone());
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

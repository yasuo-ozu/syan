use super::*;

// Every generated, otherwise-private item carries a per-`#[recurse]`-expansion `nonce` so its name
// cannot collide with the user's own items — a user type literally named `ExprTerm` no longer clashes
// with the generated terminator (cf. `ui/audit_recurse_terminator_collision.rs`). The nonce is constant
// across one expansion, so every site that re-derives a name (in `build_scc`, `gen_natural_extras`,
// `build_multiroot_tail`, `ConvDir::FromNat`) agrees on it.

/// Engine (depth-limited) node type for a cycle type: `__<name>Rec_<nonce>`.
pub(crate) fn engine_name(name: &str, nonce: u64) -> Ident {
    Ident::new(&format!("__{name}Rec_{nonce}"), Span::call_site())
}
/// Per-root terminator type: `__<root>Term_<nonce>`.
pub(crate) fn term_name(root: &str, nonce: u64) -> Ident {
    Ident::new(&format!("__{root}Term_{nonce}"), Span::call_site())
}
/// Per-root depth-default chain alias: `__<root>Default_<nonce>`.
pub(crate) fn default_name(root: &str, nonce: u64) -> Ident {
    Ident::new(&format!("__{root}Default_{nonce}"), Span::call_site())
}
/// Engine→natural conversion trait for a cycle type: `__ToNat_<name>_<nonce>`.
pub(crate) fn to_nat_name(name: &str, nonce: u64) -> Ident {
    Ident::new(&format!("__ToNat_{name}_{nonce}"), Span::call_site())
}
/// Natural→engine conversion trait for a cycle type: `__FromNat_<name>_<nonce>`.
pub(crate) fn from_nat_name(name: &str, nonce: u64) -> Ident {
    Ident::new(&format!("__FromNat_{name}_{nonce}"), Span::call_site())
}
/// Per-root erased re-entry parser fn: `__reentry_<root>_<nonce>`.
pub(crate) fn reentry_name(root: &str, nonce: u64) -> Ident {
    Ident::new(&format!("__reentry_{root}_{nonce}"), Span::call_site())
}
/// Per-root re-entry fn-pointer type alias: `__ReFn_<root>_<nonce>`.
pub(crate) fn reentry_fn_alias(root: &str, nonce: u64) -> Ident {
    Ident::new(&format!("__ReFn_{root}_{nonce}"), Span::call_site())
}
/// Per-root **borrow** terminator (for unbounded group-ful `Unparse`/`Spanned`): `__<root>TermRef_<nonce>`.
pub(crate) fn term_ref_name(root: &str, nonce: u64) -> Ident {
    Ident::new(&format!("__{root}TermRef_{nonce}"), Span::call_site())
}
/// Per-root erased re-entry **unparse** fn: `__reentry_unparse_<root>_<nonce>`.
pub(crate) fn reentry_unparse_name(root: &str, nonce: u64) -> Ident {
    Ident::new(&format!("__reentry_unparse_{root}_{nonce}"), Span::call_site())
}
/// Per-root erased re-entry **span** fn: `__reentry_span_<root>_<nonce>`.
pub(crate) fn reentry_span_name(root: &str, nonce: u64) -> Ident {
    Ident::new(&format!("__reentry_span_{root}_{nonce}"), Span::call_site())
}

/// The `Generics` of a (public) cycle item by name.
pub(crate) fn item_generics(items: &[Item], name: &str) -> Generics {
    items
        .iter()
        .find_map(|it| match it {
            Item::Enum(e) if e.ident == name => Some(e.generics.clone()),
            Item::Struct(s) if s.ident == name => Some(s.generics.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

/// A `Generics`' `where`-clause predicates as token lists (empty if none).
pub(crate) fn where_preds(generics: &Generics) -> Vec<TokenStream> {
    generics
        .where_clause
        .as_ref()
        .map(|w| w.predicates.iter().map(|p| quote!(#p)).collect())
        .unwrap_or_default()
}

use crate::util::{angle, gargs, gparams, to_snake};
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
pub(crate) struct SubastEntry {
    pub(crate) path: Path,
    pub(crate) alias: Option<Ident>,
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
    pub(crate) fn matchkey(&self) -> Ident {
        self.alias
            .clone()
            .unwrap_or_else(|| self.path.segments.last().unwrap().ident.clone())
    }
}

/// Render a `#[subast(..)]` allowlist as the `@subast { path as key, … }` token list carried in a
/// metadata macro: each entry's path is `$crate`-rooted (so it resolves downstream, exactly as
/// `derive_ast`) and paired with its matchkey. Shared by `#[derive(Ast)]` and `#[recurse]`'s
/// per-cycle-type metadata macros.
pub(crate) fn subast_tokens(entries: &[SubastEntry]) -> Vec<TokenStream> {
    entries
        .iter()
        .map(|e| {
            let path = crate_rooted_tokens(&e.path);
            let key = e.matchkey();
            quote!( #path as #key )
        })
        .collect()
}

/// Re-root a `crate::…` path at `$crate::…` for emission inside the `#[macro_export]` metadata
/// macro, so the path resolves to *this* (the defining) crate even when the metadata macro is
/// expanded in a downstream crate building a visitor that drills through these types. Non-`crate`
/// paths (`::abs`, `super`, `self`, bare) are emitted verbatim (only `crate`-rooted paths are
/// downstream-portable — the recommended canonical form).
pub(crate) fn crate_rooted_tokens(path: &Path) -> TokenStream {
    let rooted = path.leading_colon.is_none()
        && path
            .segments
            .first()
            .is_some_and(|s| s.ident == "crate" && matches!(s.arguments, PathArguments::None));
    if rooted {
        let rest: Vec<&PathSegment> = path.segments.iter().skip(1).collect();
        quote!( $crate #(:: #rest)* )
    } else {
        quote!( #path )
    }
}

/// Whether a `#[subast(..)]` path is **fully qualified (rooted)** — so it still resolves when this
/// type's metadata macro is expanded elsewhere (a sibling module, or a downstream crate building a
/// visitor that drills through it). Rooted forms: a leading `::` (absolute), `crate::…` (re-rooted to
/// `$crate::…` for downstream), or an external-crate path (`other_crate::…`) — i.e. a multi-segment path
/// whose first segment is not `self`/`super`. NOT rooted: a bare single-segment ident, or a
/// `self::`/`super::`-relative path — those resolve in the *consumer's* context, not the definition's.
fn subast_path_is_rooted(path: &Path) -> bool {
    if path.leading_colon.is_some() {
        return true;
    }
    match path.segments.first() {
        Some(seg) if seg.ident == "self" || seg.ident == "super" => false,
        _ => path.segments.len() >= 2,
    }
}

/// Collect the `#[subast(..)]` allowlist from the type's attributes. Each path must be fully qualified
/// (see `subast_path_is_rooted`). Two entries resolving to the same `matchkey` is an error (a bare field
/// head can't disambiguate them — alias one).
pub(crate) fn parse_subast(attrs: &[Attribute]) -> Vec<SubastEntry> {
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
    // Reject a non-fully-qualified path with a clear, actionable message — otherwise it surfaces far
    // away as a cryptic "cannot find macro/type" (or wrong-type) error when a visitor drills the entry.
    for e in &entries {
        if !subast_path_is_rooted(&e.path) {
            let p = &e.path;
            abort!(
                e.path,
                "`#[subast(..)]` path `{}` is not fully qualified. Use a `crate`-rooted path \
                 (`crate::path::to::{}`) — or an external-crate path (`other_crate::path::{}`) for a \
                 type from another crate. A bare ident or a `self::`/`super::`-relative path resolves \
                 in the consumer's scope (where the metadata macro is expanded), not here.",
                quote!(#p).to_string().replace(' ', ""),
                e.matchkey(),
                e.matchkey(),
            );
        }
        // Reject generic arguments on any segment: a subast entry names a type by *path only* — the path
        // is used verbatim as a metadata-macro fetch target (`path! { .. }`) and a drill scrutinee, where
        // `<..>` is illegal (otherwise a cryptic "expected `!` or `::`, found `<`" surfaces at the visitor).
        if e.path
            .segments
            .iter()
            .any(|s| !matches!(s.arguments, PathArguments::None))
        {
            let p = &e.path;
            let mut stripped = e.path.clone();
            for s in &mut stripped.segments {
                s.arguments = PathArguments::None;
            }
            abort!(
                e.path,
                "`#[subast(..)]` path `{}` carries generic arguments; a subast entry names a type by \
                 path only (it is used as a metadata-macro fetch target and a match scrutinee, where \
                 `<..>` is illegal). Drop the arguments — write `{}`.",
                quote!(#p).to_string().replace(' ', ""),
                quote!(#stripped).to_string().replace(' ', ""),
            );
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

/// Produce a cleaned copy of the input definition (attributes stripped, except the field-level
/// `#[seq]`/`#[opt]` view markers which are preserved so `__visitor_build` sees them) so it can be
/// embedded verbatim inside the metadata `macro_rules!` and re-parsed as a `syn::Item`.
pub(crate) fn cleaned_definition(input: &DeriveInput) -> DeriveInput {
    // Keep only the `#[seq]`/`#[opt]` field markers (the visitor reads them to dispatch a field through
    // its `SeqView`/`OptView` edit method); drop everything else.
    fn clean_field_attrs(f: &mut Field) {
        f.attrs.retain(|a| a.path().is_ident("seq") || a.path().is_ident("opt"));
        f.vis = Visibility::Inherited;
    }
    let mut di = input.clone();
    di.attrs.clear();
    di.vis = Visibility::Public(Default::default());
    match &mut di.data {
        Data::Enum(e) => {
            for v in &mut e.variants {
                v.attrs.clear();
                for f in &mut v.fields {
                    clean_field_attrs(f);
                }
            }
        }
        Data::Struct(s) => {
            for f in &mut s.fields {
                clean_field_attrs(f);
            }
        }
        Data::Union(u) => {
            for f in &mut u.fields.named {
                clean_field_attrs(f);
            }
        }
    }
    di
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
    for_each_field(&input.data, |ty| collect_type_idents(ty, &mut out));
    out
}

/// Peel a field type (containers + refs) to its innermost head ident — the same heads the visitor
/// follows. `None` for a non-path leaf or a tuple (a tuple contributes no single suspect head — the
/// "follows nothing" lint, this fn's only caller, conservatively ignores it).
fn peel_head(ty: &Type) -> Option<Ident> {
    match crate::util::peel(ty, &std::collections::HashSet::new())?.head {
        crate::util::Head::Path { head, .. } => Some(head),
        crate::util::Head::Tuple(_) => None,
    }
}

fn for_each_field(data: &Data, mut f: impl FnMut(&Type)) {
    match data {
        Data::Struct(s) => s.fields.iter().for_each(|fld| f(&fld.ty)),
        Data::Enum(e) => e
            .variants
            .iter()
            .for_each(|v| v.fields.iter().for_each(|fld| f(&fld.ty))),
        Data::Union(u) => u.fields.named.iter().for_each(|fld| f(&fld.ty)),
    }
}

/// `#[derive(Ast)]` expansion.
///
/// Emits:
/// * `impl Ast for T<..> {}` (the empty marker trait from `syan::visit`),
/// * one `impl Repeater<N> for T<..>` per context-dependent field type, so it can be named
///   portably as `<T<..> as Repeater<N>>::Type` (external-metadata fallback; the visitor/drill path
///   uses `#[subast]`-resolved paths),
/// * a `#[macro_export]` callback metadata `macro_rules!` carrying a cleaned copy of the definition
///   plus the `#[subast]` allowlist, and
/// * a macro-namespace re-export under the type's own name so a generated visitor can reach it as
///   `path::to::T! { .. }`.
pub fn derive_ast(input: &DeriveInput, nonce: u64, syan: &Path) -> TokenStream {
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // `#[subast(..)]` allowlist of this type's sub-AST children (+ their resolvable paths). Carried
    // verbatim in the metadata macro; the visitor matches field heads against it.
    let subast = parse_subast(&input.attrs);
    let has_subast_attr = input.attrs.iter().any(|a| a.path().is_ident("subast"));
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

    // "Follows nothing" lint: with no `#[subast]` at all, a field whose (peeled) head looks like an
    // AST node — UpperCamelCase, not this type itself (self-recursion is implicit), not a generic
    // param, not `PhantomData`/`String` — will be silently treated as a leaf by every visitor. Warn
    // so the omission is intentional; silence by adding `#[subast(..)]` (or `#[subast()]` to confirm
    // there are none). Heuristic — it can flag a genuine leaf node type (e.g. one with only a `Span`
    // field); `#[subast()]` documents that case.
    if !has_subast_attr {
        let self_name = ident.to_string();
        let param_names: std::collections::HashSet<String> = input
            .generics
            .params
            .iter()
            .map(|p| match p {
                GenericParam::Type(t) => t.ident.to_string(),
                GenericParam::Const(c) => c.ident.to_string(),
                GenericParam::Lifetime(l) => l.lifetime.ident.to_string(),
            })
            .collect();
        let mut suspects: Vec<String> = Vec::new();
        for_each_field(&input.data, |ty| {
            if let Some(head) = peel_head(ty) {
                let h = head.to_string();
                let looks_ast = h != self_name
                    && h != "PhantomData"
                    && h != "String"
                    && !param_names.contains(&h)
                    && h.chars().next().is_some_and(|c| c.is_uppercase());
                if looks_ast && !suspects.contains(&h) {
                    suspects.push(h);
                }
            }
        });
        if !suspects.is_empty() {
            emit_warning!(
                ident,
                "`{}` has field type(s) ({}) that look like AST children but no `#[subast]` is \
                 declared, so a visitor will not traverse them; add `#[subast(..)]` to follow them \
                 (or `#[subast()]` to confirm there are none)",
                ident,
                suspects.join(", ")
            );
        }
    }
    let subast_entry_tokens = subast_tokens(&subast);

    let cleaned = cleaned_definition(input);
    let macro_name = Ident::new(&format!("__{}_ast_{}", to_snake(ident), nonce), Span::call_site());

    // type-leak: one `Repeater<N>` impl **on the AST type itself** per context-dependent field type
    // (`<T as Repeater<N>>::Type` names that type portably from another crate). No separate leaker
    // marker — the type is its own host (same generics + where-clause).
    let referrer = build_referrer(input);
    let repeater_items: TokenStream = if let Some(referrer) = &referrer {
        let g_def = angle(&gparams(&input.generics));
        let g_use = angle(&gargs(&input.generics));
        let leak_tys: Vec<&Type> = referrer.iter().collect();
        quote! {
            #(for (n, ty) in leak_tys.iter().enumerate()) {
                #[automatically_derived]
                // This associated type mirrors a user field type verbatim, so a deliberate AST shape can
                // trip clippy's type-shape lints (`Box<Vec<_>>`, `Vec<Box<_>>`, deep nesting); silence them
                // on the generated mirror so they never surface in a consumer's lint output.
                #[allow(clippy::box_collection, clippy::vec_box, clippy::type_complexity)]
                impl #g_def #syan::visit::Repeater< #{Literal::usize_unsuffixed(n)} >
                    for #ident #g_use #where_clause
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

        #repeater_items

        // The cleaned definition / `#[subast]` paths embedded below may contain `crate::`-rooted paths
        // the user wrote; those resolve in *this* (defining) crate by design (downstream-portable paths
        // are `$crate`-rooted via `crate_rooted_tokens`). Suppress clippy's `crate_in_macro_def` for the
        // generated callback macro so it never surfaces in a consumer's lint output.
        #[allow(clippy::crate_in_macro_def)]
        #[macro_export]
        #[doc(hidden)]
        macro_rules! #macro_name {
            // Callback muncher: append this type's metadata, then re-invoke the continuation `$cb`.
            (@ast $cb:path { $($pre:tt)* }) => {
                $cb ! {
                    $($pre)*
                    @ast { #cleaned }
                    @subast { #(#subast_entry_tokens),* }
                }
            };
        }

        #[doc(hidden)]
        #{ &input.vis } use #macro_name as #ident;
    }
}

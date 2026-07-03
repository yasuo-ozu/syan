use super::*;

/// The deduped union of every target's generic params (first declaration wins), followed by the
/// base's params (for inheritance — the new trait must declare them to name `base::Visit<base params>`
/// as a supertrait, so the new union must ⊇ the base's). The caller normalizes order with
/// `sort_lifetimes_first`; the recurse path additionally filters this to the cycle roots' params.
pub(crate) fn param_union(targets: &[&DoneType], base_generics: &[GenericParam]) -> Vec<GenericParam> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for d in targets {
        for p in gparams(item_generics(&d.def).unwrap()) {
            if seen.insert(param_name(&p)) {
                out.push(p);
            }
        }
    }
    for bp in base_generics {
        if seen.insert(param_name(bp)) {
            out.push(bp.clone());
        }
    }
    out
}

/// Lifetimes must precede type/const params in every generated generic list, but a param union (and
/// inherited base params) can interleave them. Normalize lifetime-first — a *stable* partition, so
/// semantics are preserved and every `by_name`/`g_args`/`g_def`/`g_use` view shares this order.
pub(crate) fn sort_lifetimes_first(params: &mut [GenericParam]) {
    params.sort_by_key(|p| !matches!(p, GenericParam::Lifetime(_)));
}

/// The set of idents that count as user AST types when peeling a field of a type with the given
/// `self_ident` and `#[subast]` entries: the type's own ident plus every `#[subast]` matchkey.
pub(crate) fn self_and_subast_keys(self_ident: Option<&Ident>, subast: &[SubEntry]) -> HashSet<String> {
    let mut s: HashSet<String> = subast.iter().map(|e| e.key.to_string()).collect();
    if let Some(id) = self_ident {
        s.insert(id.to_string());
    }
    s
}

/// A visited type's `where`-clause predicates (e.g. `S: Bound`), or empty when it has none. These
/// must be repeated on every generated item that names the type so the type is well-formed there.
pub(crate) fn item_where_preds(item: &Item) -> Vec<WherePredicate> {
    item_generics(item)
        .and_then(|g| g.where_clause.as_ref())
        .map(|w| w.predicates.iter().cloned().collect())
        .unwrap_or_default()
}

/// The bare generic-param ident a `where`-predicate bounds (`S` in `S: Bound`), or `None` for a
/// predicate whose bounded type isn't a single bare param (`Vec<S>: Clone`, lifetime bounds, …).
pub(crate) fn where_pred_param(p: &WherePredicate) -> Option<&Ident> {
    if let WherePredicate::Type(pt) = p {
        if let Type::Path(tp) = &pt.bounded_ty {
            let seg = tp.path.segments.first()?;
            if tp.qself.is_none()
                && tp.path.segments.len() == 1
                && matches!(seg.arguments, PathArguments::None)
            {
                return Some(&seg.ident);
            }
        }
    }
    None
}

/// Render `where p0, p1, …` (or nothing when empty) for the given predicates.
pub(crate) fn where_clause(preds: &[WherePredicate]) -> TokenStream {
    if preds.is_empty() {
        quote!()
    } else {
        quote!( where #(#preds),* )
    }
}

/// One visited type's identifier (for method/struct names), the full path it is referenced by, its
/// own generic params (def-side) and use-side args, its `where`-clause predicates (repeated on the
/// inherent impl that names it), and its shared-ref and `&mut` bodies.
pub(crate) struct VType {
    pub(crate) ident: Ident,
    pub(crate) path: TokenStream,
    pub(crate) own_params: Vec<GenericParam>,
    pub(crate) own_use: TokenStream,
    pub(crate) own_where: Vec<WherePredicate>,
    /// In heterogeneous (method-generic) mode: this type's own params that are NOT trait-level (i.e.
    /// not shared by all visited types), declared as generics on its `visit_*` method + free fn. Empty
    /// in the common union mode (all params are trait-level).
    pub(crate) method_params: Vec<GenericParam>,
    /// Whether the visited type is crate-local (so an inherent `.visit()` impl is allowed; a foreign
    /// target would be E0116, so its inherent is skipped — call `Visit::visit_*` instead).
    pub(crate) local: bool,
    pub(crate) body: TokenStream,
    pub(crate) body_mut: TokenStream,
}

/// A transitive supertrait obligation (an ancestor visitor), resolved against the new union: the
/// ancestor's path, the union params it is parameterized by, and the matching use-side args.
pub(crate) struct Ancestor {
    pub(crate) path: TokenStream,
    pub(crate) g_params: Vec<GenericParam>,
    pub(crate) g_use: TokenStream,
}

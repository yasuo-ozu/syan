use super::*;

/// Reject two visited types whose paths end in the same identifier.
///
/// Every generated name (`visit_*`, `*Hook`, the inherent methods) derives from a visited type's
/// last segment, so such a pair would emit each item twice. Saying so here beats the downstream
/// cascade of duplicate-definition errors it would otherwise become.
pub(crate) fn check_last_segment_collisions(visited: &[Path]) {
    let mut seen: HashMap<String, String> = HashMap::new();
    for p in visited {
        let seg = last_ident(p).to_string();
        let np = norm_path(p);
        if let Some(prev) = seen.insert(seg.clone(), np.clone()) {
            if prev != np {
                abort!(
                    p,
                    "two visited types share the last segment `{}` (`{}` vs `{}`); their generated \
                     `visit_*`/`*Hook` names would collide — give them distinct final idents",
                    seg,
                    prev,
                    np
                );
            }
        }
    }
}

/// The entries of `m` named by `names`, in that order, skipping any the map does not hold — how an
/// ancestor's params and args are looked up out of the extending visitor's union.
fn pick<T: Clone>(names: &[Ident], m: &HashMap<String, T>) -> Vec<T> {
    names
        .iter()
        .filter_map(|n| m.get(&n.to_string()).cloned())
        .collect()
}

/// What the visitor *is*, derived once from the collected metadata: which types it names and how,
/// which of them get methods, the generic params its trait carries, and how it relates to a base.
///
/// Everything here is a question about the visited set as a whole, answered before any code is
/// emitted; the emitters below read it and never recompute it.
pub(crate) struct Model<'a> {
    /// Each visited type's last segment → the path `visitor!(..)` named it by. The generated module
    /// names the visited types by that path, so no import is needed for absolute ones.
    pub(crate) path_of: HashMap<String, &'a Path>,
    /// Last segments of the `visitor!(..)`-listed types.
    pub(crate) visited: HashSet<String>,
    /// Every head this visitor can name — its own targets plus what it inherits — with the path to
    /// name it by. This is what makes a field followed because its type is *visited*, rather than
    /// because the owning node repeated that fact in `#[subast]`.
    pub(crate) reachable: HashMap<String, Path>,
    pub(crate) reachable_keys: HashSet<String>,
    /// Heads that recurse through a `visit_*` method (visited here ∪ inherited). Every other
    /// followed head is an unlisted intermediate, drilled through inline.
    pub(crate) method_set: HashSet<String>,
    /// Fetched definitions keyed by `norm_path`, for resolving an intermediate when drilling.
    pub(crate) done_by_path: HashMap<String, &'a DoneType>,
    /// The types that get visitor methods; inherited and intermediate types do not.
    pub(crate) targets: Vec<&'a DoneType>,
    /// Params shared by every visited type (∪ the base's). The rest are per-method generics in
    /// [`method_mode`](Self::method_mode).
    pub(crate) shared_names: HashSet<String>,
    pub(crate) unshared_names: HashSet<String>,
    /// Heterogeneous mode: some non-shared param is concrete-filled across an edge or carries a
    /// `where`-bound, so it cannot be a trait param. The `visit_*` methods take it as a generic
    /// instead, which also rules out closures (a closure is not `for<T>` generic).
    pub(crate) method_mode: bool,
    /// The trait's generic params, and the four spellings of them every emitter needs.
    pub(crate) g_params: Vec<GenericParam>,
    pub(crate) g_args: Vec<TokenStream>,
    pub(crate) g_def: TokenStream,
    pub(crate) g_use: TokenStream,
    /// The visitor this one extends, if any — the supertrait of the generated `Visit`.
    pub(crate) base: &'a Option<Path>,
    /// The base's args named by the union's idents, for every `base::Visit<..>` reference.
    pub(crate) base_g_use: TokenStream,
    /// The transitive ancestor chain, direct base first — carried on so a further extender inherits
    /// resolvable ancestor paths too.
    pub(crate) chain: Vec<AncIn>,
    /// The same chain resolved against this visitor's params, for the supertrait impls.
    pub(crate) ancestors: Vec<Ancestor>,
}

impl<'a> Model<'a> {
    pub(crate) fn build(st: &'a BuildInput) -> Self {
        let path_of: HashMap<String, &Path> = st
            .visited
            .iter()
            .map(|p| (last_ident(p).to_string(), p))
            .collect();
        let visited: HashSet<String> = path_of.keys().cloned().collect();
        // Inherited types, rewritten where the base recorded a path that does not mean the same thing
        // here (see `needs_requalify`) — so a base and an extender at different nesting depths, or in
        // different crates, still name the same type.
        let inherited_paths = st.inherited.iter().map(|e| {
            let p = match &st.base {
                Some(b) if needs_requalify(&e.path, b) => requalify_ancestor(&e.path, b),
                _ => e.path.clone(),
            };
            (e.key.to_string(), p)
        });
        let reachable: HashMap<String, Path> = path_of
            .iter()
            .map(|(k, p)| (k.clone(), (*p).clone()))
            .chain(inherited_paths)
            .collect();
        let reachable_keys: HashSet<String> = reachable.keys().cloned().collect();
        let method_set = st.method_set();
        let done_by_path: HashMap<String, &DoneType> =
            st.done.iter().map(|d| (norm_path(&d.path), d)).collect();

        let targets: Vec<&DoneType> = st
            .done
            .iter()
            .filter(|d| item_ident(&d.def).is_some_and(|id| visited.contains(&id.to_string())))
            .collect();
        if targets.is_empty() {
            let at = st
                .visited
                .first()
                .map_or_else(Span::call_site, |p| last_ident(p).span());
            abort!(at, "no AST definitions resolved for the visitor");
        }

        // The visitor trait is parameterized by the *union* of every visited type's generic params (+
        // the base's, when inheriting), so one visitor can span e.g. `Expr<S, Tokens>` and `BinOp<S>`;
        // each type is referenced with its own subset, and `base_g_use` names the base's args by the
        // union's idents for every `base::Visit<..>` reference.
        let mut union_params = param_union(&targets, &st.base_generics);
        sort_lifetimes_first(&mut union_params);

        // Params shared by EVERY visited type (∪ the base's, which must stay trait-level to name
        // `base::Visit<base params>`). A non-shared param appears in only some types.
        let mut shared_names = targets
            .iter()
            .map(|d| -> HashSet<String> {
                gparams(item_generics(&d.def).unwrap())
                    .iter()
                    .map(param_name)
                    .collect()
            })
            .reduce(|acc, own| acc.intersection(&own).cloned().collect())
            .unwrap_or_default();
        shared_names.extend(st.base_generics.iter().map(param_name));

        // A union param that some visited type does NOT declare. Such a param can stay a trait param
        // (the union) only while it's *unbounded* — a type lacking it is then harmlessly quantified
        // over it. But a `where`-bounded one (`S: Bound`) can't: applied to items over the union, a
        // type lacking `S` carries an undischargeable `S: Bound`. So a bounded unshared param, like a
        // concrete-filled one, must become a per-method generic with the trait keyed on the shared
        // subset (method-mode, below).
        let unshared_names: HashSet<String> = union_params
            .iter()
            .map(param_name)
            .filter(|n| !shared_names.contains(n))
            .collect();
        let has_bounded_unshared = targets.iter().any(|d| {
            item_where_preds(&d.def).iter().any(|p| {
                where_pred_param(p).is_some_and(|id| unshared_names.contains(&id.to_string()))
            })
        });

        // Heterogeneous mode: a non-shared param is either *concrete-filled* in a cross-edge (e.g.
        // `Stmt<S, u8>`) — which the union-of-params trait can't express — or carries a `where`-bound
        // (above). Make non-shared params per-method generics and go struct-only (no closures — a
        // closure can't be `for<T>` generic). Gated to the no-inheritance case (a recurse/heterogeneous
        // base is out of scope) so the common union+closure path is untouched.
        let method_mode = st.base.is_none()
            && (has_concrete_fill(&targets, &shared_names) || has_bounded_unshared);

        // Trait params: the full union normally; only the shared subset in method-mode (non-shared
        // params become method generics instead).
        let mut g_params: Vec<GenericParam> = if method_mode {
            union_params
                .iter()
                .filter(|p| shared_names.contains(&param_name(p)))
                .cloned()
                .collect()
        } else {
            union_params.clone()
        };
        sort_lifetimes_first(&mut g_params);

        let by_name: HashMap<String, TokenStream> = union_params
            .iter()
            .map(|p| (param_name(p), param_use(p)))
            .collect();
        let by_name_param: HashMap<String, GenericParam> = g_params
            .iter()
            .map(|p| (param_name(p), p.clone()))
            .collect();
        let base_args: Vec<TokenStream> = st
            .base_generics
            .iter()
            .map(|bp| by_name[&param_name(bp)].clone())
            .collect();
        let base_g_use = angle(&base_args);

        // The full transitive ancestor chain (direct base first), so the new visitor's `Driver` can
        // satisfy *every* supertrait obligation — `mid::Visit: base::Visit` means a `mid => new`
        // visitor must impl both `mid::Visit` and `base::Visit` for its `Driver`. Each ancestor's
        // params are a subset of the union (the base's `@bg` transitively carries its own ancestors'
        // params), looked up by name; each impl is quantified over exactly those params (+ the hook)
        // to avoid E0207.
        let mut chain: Vec<AncIn> = Vec::new();
        if let Some(b) = &st.base {
            chain.push(AncIn {
                path: b.clone(),
                names: st
                    .base_generics
                    .iter()
                    .map(|p| Ident::new(&param_name(p), Span::call_site()))
                    .collect(),
            });
            // Requalify transitive ancestors that a `crate::`/`super::`/`self::`-relative *upstream*
            // intermediate recorded, resolving them against the direct base's full path (no-op for
            // same-crate / already-concrete chains). This also re-exports them concrete (the chain
            // feeds `anc_export`), so a further extender inherits resolvable ancestor paths too.
            let cross_crate = base_host_crate(b).is_some();
            chain.extend(st.base_ancestors.iter().map(|a| AncIn {
                path: if cross_crate {
                    requalify_ancestor(&a.path, b)
                } else {
                    a.path.clone()
                },
                names: a.names.clone(),
            }));
        }
        let ancestors: Vec<Ancestor> = chain
            .iter()
            .map(|a| {
                let path = &a.path;
                Ancestor {
                    path: quote!(#path),
                    g_params: pick(&a.names, &by_name_param),
                    g_use: angle(&pick(&a.names, &by_name)),
                }
            })
            .collect();

        let g_args: Vec<TokenStream> = g_params.iter().map(param_use).collect();
        let g_def = angle(&g_params);
        let g_use = angle(&g_args);

        Model {
            path_of,
            visited,
            reachable,
            reachable_keys,
            method_set,
            done_by_path,
            targets,
            shared_names,
            unshared_names,
            method_mode,
            g_params,
            g_args,
            g_def,
            g_use,
            base: &st.base,
            base_g_use,
            chain,
            ancestors,
        }
    }
}

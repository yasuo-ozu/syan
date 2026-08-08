use super::*;

/// Build the recurse machinery for ONE independent cycle (`scc`): pick its root, rename its types,
/// and produce (a) the `TransformCtx` that rewrites the cycle's items and (b) the *tail* tokens
/// appended to the module (terminators + their re-entry impls, the depth-default aliases, and the
/// engine→natural bridge + delegated `Parse`/`Unparse`/`Spanned` impls via `gen_natural_extras`). Each
/// cycle is handled independently, so a module may hold several (see `find_cycle_sccs`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_scc(
    scc: &HashSet<String>,
    items: &[Item],
    type_refs: &HashMap<String, HashSet<String>>,
    direct_type_refs: &HashMap<String, HashSet<String>>,
    recursion_depth: usize,
    mod_ident: &Ident,
    deleg: &DelegSets,
    nonce: u64,
) -> (TransformCtx, TokenStream) {
    // Root types: this cycle's types that directly reference themselves.
    let root_types: HashSet<String> = scc
        .iter()
        .filter(|name| {
            type_refs
                .get(*name)
                .is_some_and(|refs| refs.contains(*name))
        })
        .cloned()
        .collect();

    // Direct-reference counts within this cycle: how many of its types reference each as a bare field.
    let mut direct_ref_counts: HashMap<&str, usize> = HashMap::new();
    for (from, refs) in direct_type_refs {
        if !scc.contains(from) {
            continue;
        }
        for r in refs {
            if scc.contains(r) {
                *direct_ref_counts.entry(r.as_str()).or_insert(0) += 1;
            }
        }
    }

    // Root type: the one that controls recursion depth.
    // Priority: (1) self-referential types, (2) most directly referenced by other cycle types
    // (so that destructuring a non-root gives back root type directly), (3) most referenced
    // overall, (4) alphabetically smallest for determinism.
    let root_name: String = if !root_types.is_empty() {
        root_types.iter().min().cloned().unwrap()
    } else {
        let mut ref_counts: HashMap<&str, usize> = HashMap::new();
        for (from, refs) in type_refs {
            if !scc.contains(from) {
                continue;
            }
            for r in refs {
                if scc.contains(r) {
                    *ref_counts.entry(r.as_str()).or_insert(0) += 1;
                }
            }
        }
        let mut candidates: Vec<&String> = scc.iter().collect();
        candidates.sort_by(|a, b| {
            let da = direct_ref_counts.get(a.as_str()).copied().unwrap_or(0);
            let db = direct_ref_counts.get(b.as_str()).copied().unwrap_or(0);
            db.cmp(&da)
                .then_with(|| {
                    let ra = ref_counts.get(a.as_str()).copied().unwrap_or(0);
                    let rb = ref_counts.get(b.as_str()).copied().unwrap_or(0);
                    rb.cmp(&ra)
                })
                .then_with(|| a.as_str().cmp(b.as_str()))
        });
        candidates[0].clone()
    };

    let term_ident = term_name(&root_name, nonce);
    let default_alias = default_name(&root_name, nonce);

    // Internal (renamed) idents: "Expr" → "__ExprRec_<nonce>"
    let internal_names: HashMap<String, Ident> = scc
        .iter()
        .map(|n| (n.clone(), engine_name(n, nonce)))
        .collect();

    // The root's full generics drive the depth aliases. (The root is always one of the cycle types,
    // so the fallback is unreachable.)
    let root_generics: Generics = items
        .iter()
        .find_map(|item| match item {
            Item::Enum(e)
                if matches!(e.vis, Visibility::Public(_)) && e.ident == root_name =>
            {
                Some(e.generics.clone())
            }
            Item::Struct(s)
                if matches!(s.vis, Visibility::Public(_)) && s.ident == root_name =>
            {
                Some(s.generics.clone())
            }
            _ => None,
        })
        .unwrap_or_default();

    // Root generics as (declaration, use) token forms. The `__Rec` default `__RootDefault<root
    // params>` is referenced by every cycle type, so each must declare all of the root's params; a
    // cycle type may additionally carry its own extra params (threaded into its node type, and — for
    // the visitor — made generic on its `visit_*` method).
    let (gen_decl, gen_use) = generic_tokens(&root_generics);
    let gen_decl = &gen_decl;
    let gen_use = &gen_use;
    let root_keys: HashSet<String> = root_generics.params.iter().map(param_key).collect();

    // The terminator struct must be generic over the root's params when the cycle is generic, so the
    // depth-default alias `type __RootDefault<S> = …Term<S>` actually *uses* every param (otherwise an
    // unused-param E0091 fires on the user's own definition). When the cycle has no params the terminator
    // stays the byte-identical unit struct `pub struct RootTerm;`.
    let has_gen = !gen_decl.is_empty();
    // Self-type arguments for the terminator (`RootTerm<S, …>`), empty when non-generic.
    let term_args: TokenStream = if has_gen {
        quote!( < #(#gen_use),* > )
    } else {
        quote!()
    };
    for item in items {
        let (id, generics): (&Ident, &Generics) = match item {
            Item::Enum(e)
                if matches!(e.vis, Visibility::Public(_)) && scc.contains(&e.ident.to_string()) =>
            {
                (&e.ident, &e.generics)
            }
            Item::Struct(s)
                if matches!(s.vis, Visibility::Public(_)) && scc.contains(&s.ident.to_string()) =>
            {
                (&s.ident, &s.generics)
            }
            _ => continue,
        };
        // `param_key` encodes the *kind* alongside the name (`"type S"` / `"lifetime a"` /
        // `"const N: usize"`), so the kinds must match too: a root `type S` is not satisfied by a
        // child's `const S: usize` — they yield different keys and the root's key is reported missing.
        let have: HashSet<String> = generics.params.iter().map(param_key).collect();
        if let Some(missing) = root_keys.iter().find(|k| !have.contains(*k)) {
            abort!(
                id,
                "#[recurse]: cycle type `{}` is missing the recursion root `{}`'s generic parameter \
                 ({}); every cycle type must declare all of the root's parameters (it may add its \
                 own extras) so the depth machinery can thread them through",
                id,
                root_name,
                missing
            );
        }
    }

    // For root detection in the transformer: we treat the ROOT type as "replaced by __Rec"
    // and all other cycle types as "renamed + get __Rec appended".
    // The root type's direct references also become __Rec, so we add it to root_types set.
    let mut effective_roots = root_types.clone();
    effective_roots.insert(root_name.clone());
    // A visitor is only sound for a single effective root (else back-edges collapse ambiguously).
    let single_root = effective_roots.len() == 1;

    // Identity generic arguments per effective root, as use-form normalized token strings: a
    // back-edge to a root must repeat these verbatim (see `TransformCtx::root_ident_args`).
    let root_ident_args: HashMap<String, Vec<String>> = items
        .iter()
        .filter_map(|item| {
            let (id, generics): (&Ident, &Generics) = match item {
                Item::Enum(e)
                    if matches!(e.vis, Visibility::Public(_))
                        && effective_roots.contains(&e.ident.to_string()) =>
                {
                    (&e.ident, &e.generics)
                }
                Item::Struct(s)
                    if matches!(s.vis, Visibility::Public(_))
                        && effective_roots.contains(&s.ident.to_string()) =>
                {
                    (&s.ident, &s.generics)
                }
                _ => return None,
            };
            let (_, us) = generic_tokens(generics);
            Some((id.to_string(), us.iter().map(|t| t.to_string()).collect()))
        })
        .collect();

    // Depth parameters, one per root (canonical sorted order). A lone root keeps the legacy name
    // `__Rec`; several roots get one named `__Rec<Root>` each, so each self-edge stays an independent,
    // unambiguous depth dimension. Each defaults to that root's depth-chain alias `__<Root>Default`.
    let mut roots_sorted: Vec<String> = effective_roots.iter().cloned().collect();
    roots_sorted.sort();
    let rec_for_root: HashMap<String, Ident> = roots_sorted
        .iter()
        .map(|r| {
            let id = if single_root {
                Ident::new("__Rec", Span::call_site())
            } else {
                Ident::new(&format!("__Rec{r}"), Span::call_site())
            };
            (r.clone(), id)
        })
        .collect();
    let default_for_root: HashMap<String, Ident> = roots_sorted
        .iter()
        .map(|r| (r.clone(), default_name(r, nonce)))
        .collect();
    let rec_params: Vec<Ident> = roots_sorted.iter().map(|r| rec_for_root[r].clone()).collect();
    let rec_decls: Vec<TokenStream> = roots_sorted
        .iter()
        .map(|r| {
            let p = &rec_for_root[r];
            let d = &default_for_root[r];
            quote!(#p = #d<#(#gen_use),*>)
        })
        .collect();

    let ctx = TransformCtx {
        cycle_types: scc.clone(),
        root_types: effective_roots,
        internal_names: internal_names.clone(),
        rec_params,
        root_rec: rec_for_root.clone(),
        rec_decls,
        root_ident_args,
    };

    // Whether the cycle derives `Parse` (all-or-none across a cycle): controls whether the terminator
    // gets the runtime re-entry `Parse` machinery (vs. only the engine-derived `Unparse`/`Spanned` floor).
    let derives_parse = scc.iter().any(|n| deleg.parse.contains(n));

    let tail = if single_root {
        // Soundness: the depth decrements only at the root's back-edge, so a sub-cycle that never
        // touches the root would never terminate. The multi-root path checks this; do it here too
        // (it was previously skipped whenever ≤1 cycle type self-references, silently leaving such a
        // sub-cycle un-depth-limited).
        let root_set: HashSet<String> = ::std::iter::once(root_name.clone()).collect();
        if subgraph_is_cyclic(scc, &root_set, type_refs) {
            abort!(
                mod_ident,
                "#[recurse]: this cycle has a sub-cycle running entirely through non-root types, so \
                 the depth recursion (which only decrements at the root `{}`) would not terminate. \
                 Make a type on that sub-cycle directly self-referential, or split it into its own \
                 `#[derive(Ast)]` type.",
                root_name
            );
        }

        // Inner default: (recursion_depth - 1) levels of __ExprRec<P0, P1, …, depth_ty>. The public
        // Expr<…> alias adds one more layer so that matching Expr::Block { stmts } leaves
        // stmts: Vec<__StmtRec<…, __ExprDefault<…>>> which equals Vec<Stmt<…>>.
        let root_internal = &internal_names[&root_name];
        let mut depth_ty: TokenStream = quote!(#term_ident #term_args);
        for _ in 0..(recursion_depth - 1) {
            depth_ty = quote!(#root_internal<#(#gen_use,)* #depth_ty>);
        }

        // The public `pub type Expr = __ExprRec<…>` aliases are *not* emitted — the natural recursive
        // enums/structs own those names. Only the internal depth-chain alias `__ExprDefault` is kept
        // (the delegated `Parse` references `__ExprRec<…, __ExprDefault>`).
        // The inhabited terminator + erased re-entry give `Parse` unbounded depth (the terminator
        // re-enters the top-level parser at runtime instead of erroring at the depth floor).
        let terminator = emit_terminator_and_reentry(items, &root_name, nonce, derives_parse);

        quote! {
            #terminator
            type #default_alias<#(#gen_decl),*> = #depth_ty;
        }
    } else {
        build_multiroot_tail(
            scc,
            items,
            &root_types,
            &roots_sorted,
            &internal_names,
            &default_for_root,
            &root_generics,
            gen_decl,
            gen_use,
            &root_keys,
            &term_args,
            recursion_depth,
            type_refs,
            mod_ident,
            nonce,
            derives_parse,
        )
    };

    // Engine→natural bridge: `__ToNat_X` conversion traits/impls + terminator `__to_nat` +
    // delegated `impl Parse for X` (parse the depth-limited engine, then convert), plus the group-ful
    // `Unparse`/`Spanned` delegations.
    let extras = gen_natural_extras(
        scc,
        items,
        &RootData {
            internal_names: &internal_names,
            roots_sorted: &roots_sorted,
            rec_for_root: &rec_for_root,
            default_for_root: &default_for_root,
            root_generics: &root_generics,
        },
        deleg,
        nonce,
    );

    (ctx, quote!( #tail #extras ))
}

/// Emit the tail (terminators, depth-chain aliases, public aliases) for a cycle with **several
/// self-referential roots**. Each root keeps its own depth dimension: every cycle type carries one
/// depth param per root, a back-edge to root `X` is that root's param, and the per-root depth chains
/// are built mutually (level `k` of each root embeds level `k-1` of *all* roots). The depth only
/// decrements at a root back-edge, so every cycle in the SCC must pass through a root — checked via
/// `safegraph` (the SCC minus the roots must be acyclic).
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_multiroot_tail(
    scc: &HashSet<String>,
    items: &[Item],
    root_types: &HashSet<String>,
    roots_sorted: &[String],
    internal_names: &HashMap<String, Ident>,
    default_for_root: &HashMap<String, Ident>,
    _root_generics: &Generics,
    gen_decl: &[TokenStream],
    gen_use: &[TokenStream],
    root_keys: &HashSet<String>,
    term_args: &TokenStream,
    recursion_depth: usize,
    type_refs: &HashMap<String, HashSet<String>>,
    mod_ident: &Ident,
    nonce: u64,
    derives_parse: bool,
) -> TokenStream {
    // The depth decrements only at a root (self-referential) back-edge, so every cycle in the SCC
    // must pass through a root. If the SCC with all roots removed is still cyclic, the generated
    // types would not terminate — reject cleanly rather than emit an infinitely-recursive type.
    if subgraph_is_cyclic(scc, root_types, type_refs) {
        abort!(
            mod_ident,
            "#[recurse]: this multi-root cycle has a sub-cycle running entirely through \
             non-self-referential types, so the depth recursion (which only decrements at a \
             self-referential type) would not terminate. Make one type on that sub-cycle directly \
             self-referential, or split it into its own `#[derive(Ast)]` type."
        );
    }

    // Every root must declare *exactly* the canonical generic params (the depth chain instantiates
    // each root as `__XRec<gen_use, depth args…>`, spelled with the shared params). A root carrying
    // extra params can't be placed in the chain. (Non-root cycle types may still carry extras.)
    for r in roots_sorted {
        let g = items.iter().find_map(|it| match it {
            Item::Enum(e) if &e.ident.to_string() == r => Some(&e.generics),
            Item::Struct(s) if &s.ident.to_string() == r => Some(&s.generics),
            _ => None,
        });
        if let Some(g) = g {
            let keys: HashSet<String> = g.params.iter().map(param_key).collect();
            if keys != *root_keys {
                abort!(
                    g.params,
                    "#[recurse]: in a multi-root cycle every self-referential root must declare \
                     exactly the same generic parameters; `{}` differs. (Non-root cycle types may \
                     carry extra parameters, but roots may not.)",
                    r
                );
            }
        }
    }

    let term_for_root: HashMap<String, Ident> = roots_sorted
        .iter()
        .map(|r| (r.clone(), term_name(r, nonce)))
        .collect();

    // One terminator per root (inhabited newtype + erased re-entry `Parse` + group-ful `Unparse` floor),
    // giving each root's `Parse` unbounded depth via runtime re-entry.
    let term_items: Vec<TokenStream> = roots_sorted
        .iter()
        .map(|r| {
            emit_terminator_and_reentry(items, r, nonce, derives_parse)
        })
        .collect();

    // Mutual depth chain: level 0 is each root's terminator; level k of every root embeds level k-1
    // of *all* roots. After (recursion_depth - 1) steps, `level[r]` is `__<r>Default`'s body (the
    // public alias adds one final `__rRec` layer, mirroring the single-root case).
    let mut level: HashMap<String, TokenStream> = roots_sorted
        .iter()
        .map(|r| {
            let t = &term_for_root[r];
            (r.clone(), quote!(#t #term_args))
        })
        .collect();
    for _ in 0..(recursion_depth - 1) {
        let args: Vec<TokenStream> = roots_sorted.iter().map(|r| level[r].clone()).collect();
        level = roots_sorted
            .iter()
            .map(|r| {
                let internal = &internal_names[r];
                (r.clone(), quote!( #internal< #(#gen_use,)* #(#args,)* > ))
            })
            .collect();
    }
    let default_decls: Vec<TokenStream> = roots_sorted
        .iter()
        .map(|r| {
            let d = &default_for_root[r];
            let body = &level[r];
            quote!( type #d<#(#gen_decl),*> = #body; )
        })
        .collect();

    // NOTE (natural-type design): no public `pub type X = __XRec<…>` aliases — the natural recursive
    // types own those names. Only the internal terminators + depth-chain aliases are emitted.
    quote! {
        #(#term_items)*
        #(#default_decls)*
    }
}

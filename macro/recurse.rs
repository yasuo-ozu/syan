//! `#[recurse]` — mutually recursive AST types whose `Parse`/`Unparse`/`Spanned` obligations are
//! broken by the external [`decycle`](https://docs.rs/decycle) crate.
//!
//! The macro itself no longer contains a recursion algorithm, nor any reshaping of the obligations.
//! It is a **front end**: it states the module's type-reference graph, **expands syan's own
//! `#[derive(Parse/Unparse/Spanned)]` itself**, and hands both to
//! `decycle::ranked::process_module_with_graph`.
//!
//! **Nothing** is left here beyond routing: the stream reshape this module used to perform — wrapping
//! every recursive parse call's argument in `erase(…)` — is gone, because `Parse::parse_stream` now
//! takes `&mut S` and recursion reborrows, so `S` is a fixed point and no erasure is needed.
//! That one is irreducibly syan's — the growth it breaks is in the *stream type*
//! (`&mut &mut …`, one layer per descent level, because `Parse::parse` takes its stream by value), a
//! monomorphization cycle rather than a trait-obligation cycle, and the type it pins to
//! (`&mut dyn ParseStream`) is syan's own, not derivable from the trait being routed.
//!
//! Everything else moved to decycle, because in each case decycle either has the information or
//! caused the problem: deciding which types recurse (`analysis::cyclic_subgraph`), spelling the
//! impls so the engine reads back the cycle syan stated (`ranked::contract`), peeling a wrapped
//! cyclic bound to a rankable head (`ranked::peel`), sharing sibling premises across a cycle
//! (`ranked::sharing`), and aliasing names against decycle's own helper-module nesting
//! (`ranked::nesting`).
//!
//! The user's types stay exactly as written — genuine natural recursive enums/structs, one type at all
//! depths — so `#[derive(Ast)]`, `visitor!(…)`, `Debug`, user `impl`s and every other non-structural
//! derive are untouched, and a `visitor!` over a `#[recurse]` cycle is an ordinary acyclic visitor.

use proc_macro::TokenStream as TokenStream1;
use proc_macro2::{Ident, Span, TokenStream};
use proc_macro_error::{abort, set_dummy};
use std::collections::HashSet;
use syn::punctuated::Punctuated;
use syn::{
    DeriveInput, Field, Fields, Item, ItemMod, Path, Token,
};
use decycle::analysis::EdgeKind;
use decycle::safegraph::VecGraph;
use template_quote::quote;

/// How many ranks decycle unrolls before its floor. The rank ladder here only has to *discharge the
/// obligation* — the recursion itself always re-enters through the public, un-ranked delegating impl
/// (bodies call the qualified trait path), so the floor is not on the hot path and the depth is not a
/// ceiling. A small number keeps the generated code small.
const DECYCLE_RECURSE_LEVEL: usize = 2;

/// The three structural derives `#[recurse]` takes over from the ordinary derive machinery.
const STRUCTURAL_DERIVES: &[&str] = &["Parse", "Unparse", "Spanned"];

// ===========================================================================================
// The driver: one pass per phase, plus the preconditions checked before any of them run
// ===========================================================================================

pub fn recurse(attr: TokenStream1, input: TokenStream1, nonce: u64) -> TokenStream1 {
    let engine = match parse_engine(attr) {
        Ok(e) => e,
        Err(e) => return e.to_compile_error().into(),
    };

    let module: ItemMod = match syn::parse(input) {
        Ok(m) => m,
        Err(e) => return e.to_compile_error().into(),
    };
    let Some((_, items)) = module.content.clone() else {
        return quote!(#module).into();
    };

    // Error-recovery output. It must NOT carry the structural derives: `#[recurse]` expands those
    // itself, so leaving them on would follow every `abort!` below with the ordinary derive's own
    // diagnostics for the very shape just rejected.
    set_dummy(quote!(
        #(for attr in &module.attrs) { #attr }
        #{&module.vis} mod #{&module.ident} {
            #(for dummy in items.iter().map(make_natural_item)) { #dummy }
        }
    ));

    // No early-out for a cycle-free module: the pipeline already handles it. Nothing is selected
    // below, so `used_traits` stays empty and `emit` takes the verbatim path — the only difference is
    // that the structural derives are expanded here rather than left for the ordinary derive
    // machinery, which is what happens for every non-cycle type in a cycle-bearing module anyway.
    let syan = crate::attribute::FindAttribute::get_syan(&module.attrs[..]);
    let routed = routed_traits(&syan);
    // Expand every structural derive first — the pass is cycle-independent by construction — and read
    // the cycles out of the RESULT. The derive emits one where-bound per field, so its own bounds are
    // the obligation graph, which is the thing decycle actually breaks; syan does not need (and used
    // to duplicate) a separate walk over field types to guess at it.
    let mut expansion = expand_items(&items, &routed, &syan, nonce);
    let trait_idents: HashSet<Ident> = routed.iter().map(|(name, _)| bare_ident(name)).collect();
    let cycles = decycle::analysis::cyclic_subgraph(&decycle::analysis::analyze_items(
        &expansion.items,
        &trait_idents,
    ));
    expansion.select_cycle_members(&cycles);

    let prelude = build_prelude(&expansion, &routed, &syan);
    emit(module, prelude, expansion, engine, &syan)
}

/// Which decycle engine to route through.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Engine {
    /// The default: ranked twin traits and a rank ladder, with unbounded depth via decycle's
    /// re-entry registry.
    Ranked,
    /// `#[recurse(structural)]` — decycle's compile-time unroll. No runtime and no `type-leak`, but a
    /// narrower scope.
    Structural,
}

/// `#[recurse]` / `#[recurse(structural)]`.
fn parse_engine(attr: TokenStream1) -> syn::Result<Engine> {
    let tokens = TokenStream::from(attr);
    if tokens.is_empty() {
        return Ok(Engine::Ranked);
    }
    let span = tokens
        .clone()
        .into_iter()
        .next()
        .map(|t| t.span())
        .unwrap_or_else(Span::call_site);
    match syn::parse2::<Ident>(tokens) {
        Ok(id) if id == "structural" => Ok(Engine::Structural),
        _ => Err(syn::Error::new(
            span,
            "`#[recurse]` takes no arguments, or the single argument `structural` \
             (the `limit = N` argument was removed)",
        )),
    }
}

/// One structural derive, expanded. Purely a record of *what was generated* — whether any of it is
/// decycle's problem is decided separately, by [`Expansion::select_cycle_members`].
struct Derived {
    /// The type the derive was written on.
    owner: String,
    trait_name: &'static str,
    /// Indices into [`Expansion::items`] of the trait impls this derive produced.
    impls: Vec<usize>,
    /// The `#[group]` substructs it introduced.
    substructs: Vec<String>,
}

/// What the expansion pass produces, threaded through the reshaping passes that follow.
struct Expansion {
    /// The module's items: natural types, then the impls expanded from their structural derives.
    items: Vec<Item>,
    /// Every structural derive expanded above, in emission order.
    derived: Vec<Derived>,
    /// Routed traits a *cycle member* actually derives — decides the `#[decycle] use` prelude and
    /// whether decycle needs to run at all. Empty until [`Self::select_cycle_members`].
    used_traits: HashSet<&'static str>,
    /// `(trait name, index into `items`)` for every impl to be contracted. Ditto.
    contracted: Vec<(&'static str, usize)>,
    /// The cycle members, grown with the `#[group]` substructs of cycle members — and the single
    /// source of truth for "is this type in a cycle". Ditto. This is what `emit` hands to decycle,
    /// so nothing downstream can hold a view of the cycle that decycle does not share.
    cycle_graph: VecGraph<Ident, EdgeKind>,
}


/// Expand **every** structural derive in the module — cycle member or not.
///
/// Expanding all of them is what lets the module drop its `Parse`/`Unparse` *derive* imports
/// entirely, which it must: the derive and the alter macro decycle's ping-pong needs both live in the
/// macro namespace under the same name, and only one binding per name can exist there.
///
/// This pass is **cycle-independent by construction**: a derive expands to the same impl whether or
/// not its type recurses. Only what happens to the result afterwards differs, which is why the cycle
/// set is not a parameter here — see [`Expansion::select_cycle_members`].
fn expand_items(items: &[Item], routed: &[(&'static str, Path)], syan: &Path, nonce: u64) -> Expansion {
    let mut ex = Expansion {
        items: Vec::new(),
        derived: Vec::new(),
        used_traits: HashSet::new(),
        contracted: Vec::new(),
        cycle_graph: VecGraph::default(),
    };
    for item in items {
        let Some((ident, attrs)) = adt_parts(item).filter(|(_, a)| derives_any(a, STRUCTURAL_DERIVES))
        else {
            ex.items.push(rewrite_trait_imports(item.clone(), routed));
            continue;
        };
        let owner = ident.to_string();
        ex.items.push(make_natural_item(item));
        for tr in routed {
            let (name, _) = tr;
            if !derives_any(attrs, &[*name]) {
                continue;
            }
            let generated = expand_routed(item, tr, syan, nonce);
            let base = ex.items.len();
            ex.derived.push(Derived {
                owner: owner.clone(),
                trait_name: name,
                impls: generated
                    .iter()
                    .enumerate()
                    .filter(|(_, it)| matches!(it, Item::Impl(im) if im.trait_.is_some()))
                    .map(|(i, _)| base + i)
                    .collect(),
                substructs: substruct_idents(&generated),
            });
            ex.items.extend(generated);
        }
    }
    ex
}

impl Expansion {
    /// Decide which of the expanded impls decycle has to contract.
    ///
    /// A `#[group]` **substruct** of a cycle member joins the cycle set here: the member reaches its
    /// recursive children through it, so the member's `Substruct: Parse<Atom>` bound is a genuine
    /// cyclic bound with a bare, module-local head — exactly what decycle can rank. (The group wrapper
    /// bound `FieldTy: GroupShape<Atom>` mentions no content type and never names a routed trait, so
    /// decycle ignores it entirely.)
    fn select_cycle_members(&mut self, cycles: &VecGraph<Ident, EdgeKind>) {
        use decycle::safegraph::graph::Graph;

        let mut used_traits = HashSet::new();
        let mut contracted = Vec::new();
        let mut substructs: Vec<Ident> = Vec::new();
        for d in &self.derived {
            if !cycles.nodes().any(|n| n == &d.owner) {
                continue;
            }
            used_traits.insert(d.trait_name);
            substructs.extend(
                d.substructs
                    .iter()
                    .map(|n| Ident::new(n, Span::call_site())),
            );
            contracted.extend(d.impls.iter().map(|&pos| (d.trait_name, pos)));
        }
        self.used_traits = used_traits;
        self.contracted = contracted;
        // The substructs exist only in the derive's output, so they can only join the graph now.
        self.cycle_graph = decycle::analysis::with_nodes(cycles, substructs);
    }
}


/// The items that must precede the module's own: when decycle will run, the namespace-split trait
/// bindings the reshaped impls need. (The aliases that keep decycle's own helper-module nesting from
/// changing what a name means are decycle's to emit — see `decycle::ranked::nesting`.)
fn build_prelude(ex: &Expansion, routed: &[(&'static str, Path)], syan: &Path) -> Vec<Item> {
    let mut prelude = Vec::new();
    // Bind the two namespaces separately (see `syan::decycle_traits`): the trait itself comes from
    // the collision-free `decycle_traits` re-export, the alter macro decycle's ping-pong invokes
    // comes from its own `#[macro_export]`ed name. Binding both under the plain name `Parse` is what
    // lets the reshaped impls spell the trait as a single segment.
    for (name, _) in routed {
        if !ex.used_traits.contains(name) {
            continue;
        }
        let bare = bare_ident(name);
        let alter = Ident::new(&format!("__syan_decycle_{name}"), Span::call_site());
        prelude.push(syn::parse_quote! {
            #[allow(unused_imports)]
            use #syan::decycle_traits::#bare;
        });
        prelude.push(syn::parse_quote! {
            #[decycle]
            #[allow(unused_imports)]
            use #syan::#alter as #bare;
        });
    }
    prelude
}

/// Emit the module — through decycle when a cycle member derives a routed trait, verbatim otherwise
/// (an `Ast`-only cycle has no obligation to break).
fn emit(
    module: ItemMod,
    prelude: Vec<Item>,
    ex: Expansion,
    engine: Engine,
    syan: &Path,
) -> TokenStream1 {
    if ex.used_traits.is_empty() {
        let (attrs, vis, ident) = (&module.attrs, &module.vis, &module.ident);
        let items = &ex.items;
        return quote!( #(#attrs)* #vis mod #ident { #(#prelude)* #(#items)* } ).into();
    }
    let decycle_path: Path = syn::parse_quote!(#syan::__decycle);
    // Hand decycle the same graph everything else here keyed off. It would otherwise re-derive the
    // participant set from the trait spelling; instead it re-spells the impls FROM this graph
    // (`ranked::contract`). So the cycle is stated once, as one value, rather than encoded by syan
    // into every bound and decoded again by decycle — and syan's scoping rules (only `pub`, only
    // single-segment references) are visible to decycle instead of implied.
    let Expansion {
        items,
        cycle_graph,
        ..
    } = ex;
    let mut decycled = module;
    decycled.content = Some((Default::default(), prelude.into_iter().chain(items).collect()));
    if engine == Engine::Structural {
        // Structural adopts an impl by its trait path's LAST segment, so it reads the derive's
        // fully-qualified impls as-is — no re-spelling step, and hence no `emit_contracts`. The graph
        // can only narrow its participant set (its model is keyed on `(type, trait)` pairs), which is
        // exactly what is wanted: syan's cycle types and nothing else.
        return decycle::structural::process_module_with_graph(decycled, &cycle_graph, &decycle_path)
            .into();
    }
    decycle::ranked::process_module_with_graph(
        decycled,
        &cycle_graph,
        // Re-spell the impls from the graph. The derive writes every trait path fully qualified, which
        // decycle otherwise reads as "ordinary premise" — so without this it would adopt nothing.
        // Opting in is what lets syan state the cycle once instead of encoding it into the spelling of
        // every bound.
        /* emit_contracts */ true,
        &decycle_path,
        DECYCLE_RECURSE_LEVEL,
        /* support_infinite_cycle */ true,
    )
    .into()
}


/// The ident and attributes of an item `#[recurse]` can expand a derive for — an enum or a struct.
/// Every other item kind is passed through untouched, so `None` is the whole answer for it.
fn adt_parts(item: &Item) -> Option<(&Ident, &[syn::Attribute])> {
    match item {
        Item::Enum(e) => Some((&e.ident, &e.attrs)),
        Item::Struct(s) => Some((&s.ident, &s.attrs)),
        _ => None,
    }
}

/// Drop `Parse`/`Unparse` from the module's own `use` items.
///
/// Inside a `#[recurse]` module those names must bind, in the macro namespace, the **alter macro**
/// decycle's ping-pong invokes — and a name can hold only one macro-namespace binding, so a
/// user-written `use syan::parse::Parse;` (which brings in the *derive* under that name) would be a
/// hard `E0252`. It is also no longer needed: `#[recurse]` expands every structural derive in the
/// module itself, and re-supplies the trait from the collision-free `syan::decycle_traits` re-export.
/// A renamed import (`use …::Parse as P;`) binds a different name and is left alone.
fn rewrite_trait_imports(item: Item, routed: &[(&'static str, Path)]) -> Item {
    let Item::Use(mut u) = item else {
        return item;
    };
    let names: Vec<String> = routed.iter().map(|(name, _)| name.to_string()).collect();
    match prune_use_tree(u.tree.clone(), &names) {
        Some(tree) => {
            u.tree = tree;
            Item::Use(u)
        }
        // Every leaf was one of the routed traits — drop the whole `use`.
        None => Item::Verbatim(TokenStream::new()),
    }
}

/// `prune_use_tree(tree, names)` with every leaf whose **local** name is in `names` removed; `None`
/// when nothing is left.
fn prune_use_tree(tree: syn::UseTree, names: &[String]) -> Option<syn::UseTree> {
    use syn::UseTree;
    match tree {
        UseTree::Path(mut p) => {
            p.tree = Box::new(prune_use_tree(*p.tree, names)?);
            Some(UseTree::Path(p))
        }
        UseTree::Name(n) => (!names.contains(&n.ident.to_string())).then_some(UseTree::Name(n)),
        UseTree::Rename(r) => {
            (!names.contains(&r.rename.to_string())).then_some(UseTree::Rename(r))
        }
        UseTree::Glob(g) => Some(UseTree::Glob(g)),
        UseTree::Group(mut g) => {
            let kept: Punctuated<UseTree, Token![,]> = g
                .items
                .into_iter()
                .filter_map(|t| prune_use_tree(t, names))
                .collect();
            if kept.is_empty() {
                None
            } else {
                g.items = kept;
                Some(UseTree::Group(g))
            }
        }
    }
}





/// A `#[derive(..)]` or `#[macro_derive(..)]` attribute, as its own name plus the traits it lists.
///
/// A malformed list parses as **empty**, which is what both callers want: nothing is "derived" by it,
/// and `strip_structural_derives` drops the attribute rather than re-emitting something unparseable.
fn derive_list(attr: &syn::Attribute) -> Option<(&'static str, Vec<Path>)> {
    let name = ["derive", "macro_derive"]
        .into_iter()
        .find(|n| attr.path().is_ident(n))?;
    let syn::Meta::List(list) = &attr.meta else {
        return None;
    };
    let paths: Punctuated<Path, Token![,]> = list
        .parse_args_with(Punctuated::parse_terminated)
        .unwrap_or_default();
    Some((name, paths.into_iter().collect()))
}

/// Does this derive path name one of `names`? Keyed on the last segment, so `foo::Parse` counts.
fn names_one_of(path: &Path, names: &[&str]) -> bool {
    path.segments
        .last()
        .is_some_and(|s| names.iter().any(|n| s.ident == n))
}

fn derives_any(attrs: &[syn::Attribute], names: &[&str]) -> bool {
    attrs
        .iter()
        .filter_map(derive_list)
        .any(|(_, paths)| paths.iter().any(|p| names_one_of(p, names)))
}

/// Drop the structural derives (and the helper attributes they consume) from an item's attributes.
fn strip_structural_derives(attrs: &[syn::Attribute]) -> Vec<syn::Attribute> {
    attrs
        .iter()
        .filter_map(|attr| match derive_list(attr) {
            Some((name, paths)) => {
                let kept: Vec<Path> = paths
                    .into_iter()
                    .filter(|p| !names_one_of(p, STRUCTURAL_DERIVES))
                    .collect();
                let name = Ident::new(name, Span::call_site());
                (!kept.is_empty()).then(|| syn::parse_quote!( #[#name(#(#kept),*)] ))
            }
            // A helper attribute is consumed by the derive just removed; leaving it behind would be
            // "cannot find attribute".
            None => (!is_structural_helper(attr)).then(|| attr.clone()),
        })
        .collect()
}

/// The helper attributes the structural derives register. Once those derives are gone from the natural
/// type, an unconsumed helper is a hard "cannot find attribute" error — so they are stripped. The
/// visitor markers `#[subast]`/`#[seq]`/`#[opt]` are NOT in this list: `#[derive(Ast)]` stays on the
/// natural type and consumes them.
fn is_structural_helper(attr: &syn::Attribute) -> bool {
    ["group", "joint", "alone", "default"]
        .iter()
        .any(|n| attr.path().is_ident(n))
}

/// Strip the structural-derive field helper attributes from a field set.
fn strip_field_helper_attrs(fields: &mut Fields) {
    let go = |f: &mut Field| f.attrs.retain(|a| !is_structural_helper(a));
    match fields {
        Fields::Named(n) => n.named.iter_mut().for_each(go),
        Fields::Unnamed(u) => u.unnamed.iter_mut().for_each(go),
        Fields::Unit => {}
    }
}

/// The public **natural** form of a cycle type: the user's item verbatim, minus the structural derives
/// (which `#[recurse]` expands itself, and whose output decycle then contracts) and minus their now
/// unconsumed helper attributes. Everything else — `Ast`, `Debug`, `Default`, `#[subast]`, `#[seq]`,
/// docs, generics, `where`-clauses — is preserved exactly, which is what makes the public API a plain
/// recursive type.
fn make_natural_item(item: &Item) -> Item {
    let mut it = item.clone();
    match &mut it {
        Item::Enum(e) => {
            e.attrs = strip_structural_derives(&e.attrs);
            for v in &mut e.variants {
                strip_field_helper_attrs(&mut v.fields);
            }
        }
        Item::Struct(s) => {
            s.attrs = strip_structural_derives(&s.attrs);
            strip_field_helper_attrs(&mut s.fields);
        }
        _ => {}
    }
    it
}

// ===========================================================================================
// Running the structural derives and reshaping their output for decycle
// ===========================================================================================
// Reshaping is what makes the derive output acceptable to `decycle::process_module`.
//
// `#[recurse]` no longer generates a recursion engine. It **expands `#[derive(Parse)]` /
// `#[derive(Unparse)]` / `#[derive(Spanned)]` itself** (the same `attribute::{parse,unparse,spanned}`
// entry points the derives use), then hands the resulting `impl` blocks to `decycle`, which breaks the
// circular trait obligations they carry. Three reshapes are needed between those two steps — each one
// forced by a concrete rule of decycle's ranked engine:
//
// 1. **The impl's own trait path becomes the bare ident** (`Parse<A>`, not
//    `::syan::parse::parse::Parse<A>`). `process_module` only adopts an impl whose trait path is a
//    *single segment* naming a `#[decycle]`-listed trait; a qualified path is deliberately left as an
//    ordinary, non-contracted impl.
// 2. **A cyclic field bound is spelled bare** — `Vec<Box<Stmt<S>>>: <qualified>Parse<A>` becomes
//    `Vec<Box<Stmt<S>>>: Parse<A>`. The target is left wrapped: peeling it down to the rankable head
//    (`Stmt<S>: Parse<A>`) is decycle's job, in `ranked::peel`, since the ranked engine is what needs
//    a head with an impl in this module. All syan decides is *which* bounds are edges, by spelling
//    them bare. Every **non**-cyclic (leaf) bound is left exactly as the derive spelled it
//    — with the FULLY QUALIFIED trait path. That spelling is the signal: decycle treats a
//    crate-rooted or absolute reference as the caller's "this is an ordinary premise, not an edge"
//    opt-out and leaves it alone, while a bare single-segment reference is a cycle edge to contract.
//    So `Integer: ::syan::parse::parse::Parse<A>` and `__SyanMacro_Atom: ::syan::span::Spanned`
//    survive untouched, with no supertrait-alias laundering.
// (There used to be a third rewrite here: every field-parse call's stream argument was wrapped in
// `syan::parse::erase(…)`, because the growth was in the *stream type* — `&mut &mut …`, one layer per
// descent level — a monomorphization cycle rather than a trait cycle. `Parse`'s required method now
// takes `&mut S` and recursive calls reborrow, so the stream type no longer grows and the rewrite is
// gone.)
//
// Bodies are otherwise emitted **verbatim**, still calling the **fully-qualified** trait. That is NOT
// redundancy — the module does import the trait, so a bare `Parse` would resolve to the same item.
// The qualification is a *signal to decycle*: its `TraitReplacer` rewrites every single-segment trait
// path in an adopted impl — header, where-clause **and body call sites** — to the ranked twin. Spelling
// a body call bare therefore turns `<Integer as Parse<A>>::parse(..)` into
// `<Integer as ParseRanked<Rank, A>>::parse(..)`, and a leaf like `Integer`/`PhantomData<S>` has no rank
// ladder (only cycle members do), so it fails with `E0277: Integer: ParseRanked<..> is not satisfied`.
// decycle documents the convention in `process_module.rs`: a still-qualified trait path is deliberately
// left as an ordinary, non-decycled reference.
//
// Keeping bodies on the public trait is also what makes depth unbounded: a recursive call re-enters
// through the un-ranked *delegating* impl at full height every level, so the ranked ladder only
// discharges the *obligation* and its floor is never on the hot path (hence `recurse_level = 2`).
//
// `Spanned` goes through the same pipeline. Its one peculiarity is that every bound it generates
// carries the `Span = __Syan_Span` associated-type constraint. On a *cyclic* bound the constraint
// survives the peel verbatim (`Stmt<S>: Spanned<Span = __Syan_Span>`) — decycle admits an
// assoc-constrained cyclic bound whose target is a cycle self head, and its rank-lowering keeps the
// constraint. On a *leaf* bound the qualified spelling carries it through untouched, which is what
// keeps the derive's invented `__Syan_Span` parameter constrained (no E0207). Recursion depth is
// unbounded for the same reason as `Parse`/`Unparse`: `span()` re-enters through the delegating impl.

use crate::attribute;


/// An enum or struct item, as the `DeriveInput` a derive entry point takes.
///
/// A round-trip through tokens rather than a field-by-field rebuild: an `Item::Enum`/`Item::Struct`
/// prints exactly the grammar `DeriveInput` parses, so the parse is the conversion — and any other
/// item kind simply fails to parse, which is the `None` this returns anyway.
fn item_to_derive_input(item: &Item) -> Option<DeriveInput> {
    syn::parse2(quote!(#item)).ok()
}

/// The traits `#[recurse]` hands to decycle, in emission order: the **derive name** — which doubles as
/// the bare trait ident decycle re-spells to — paired with the **fully-qualified path** the derive is
/// run with, and which it writes into bodies and leaf bounds.
fn routed_traits(syan: &Path) -> Vec<(&'static str, Path)> {
    vec![
        ("Parse", syn::parse_quote!(#syan::parse::parse::Parse)),
        ("Unparse", syn::parse_quote!(#syan::parse::unparse::Unparse)),
        ("Spanned", syn::parse_quote!(#syan::span::Spanned)),
    ]
}

/// The bare trait ident, i.e. the derive name as an `Ident`.
fn bare_ident(name: &str) -> Ident {
    Ident::new(name, Span::call_site())
}





/// Run one structural derive over `item`, **unreshaped**.
///
/// The output is a `#[group]` substruct's definition (if any), that substruct's own impl, and the
/// type's impl. Reshaping is a separate pass ([`contract_generated`]) because a substruct is itself a
/// cycle participant — `Expr` reaches its recursive children *through* `Group<Substruct>` — so the set
/// of types a bound has to be peeled to is only known once every member has been expanded.
fn expand_routed(item: &Item, tr: &(&'static str, Path), syan: &Path, nonce: u64) -> Vec<Item> {
    let Some(input) = item_to_derive_input(item) else {
        return Vec::new();
    };
    let (name, qualified) = tr;
    let generated = match *name {
        "Parse" => attribute::parse(
            &input.ident,
            &input.generics,
            &input.data,
            nonce,
            syan,
            qualified,
        ),
        "Unparse" => attribute::unparse(
            &input.ident,
            &input.generics,
            &input.data,
            nonce,
            syan,
            qualified,
        ),
        // `Spanned` generates no substruct, so the salted nonce is unused; the entry point takes the
        // whole `DeriveInput` because it reads `#[syan]` off the item itself.
        "Spanned" => attribute::spanned(&input, qualified.clone()),
        other => abort!(&input.ident, "#[recurse]: unroutable trait `{}`", other),
    };
    let file: syn::File = match syn::parse2(generated.clone()) {
        Ok(f) => f,
        Err(e) => abort!(
            &input.ident,
            "#[recurse]: could not re-parse the generated `{}` impl: {}",
            name,
            e
        ),
    };
    file.items
}

/// The idents of the `#[group]` substruct definitions a derive emitted.
fn substruct_idents(items: &[Item]) -> Vec<String> {
    items
        .iter()
        .filter_map(|it| match it {
            Item::Struct(s) => Some(s.ident.to_string()),
            _ => None,
        })
        .collect()
}


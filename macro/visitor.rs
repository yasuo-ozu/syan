use crate::util::{
    angle, as_tuple, gargs, gparams, item_generics, item_ident, method_ident_m, mt, param_name,
    param_tokens, param_use, peel, recurse_lower_body, to_snake, Container,
};
use proc_macro2::{Span, TokenStream};
use proc_macro_error::abort;
use std::collections::{HashMap, HashSet};
use syn::parse::{Parse, ParseStream, Parser};
use syn::punctuated::Punctuated;
use syn::*;
use template_quote::quote;

// ---------------------------------------------------------------------------
// `#[visitor([base =>] T, U, ...)]` attribute: kicks off the metadata ping-pong.
// ---------------------------------------------------------------------------

struct VisitorArgs {
    base: Option<Path>,
    types: Vec<Path>,
}

impl Parse for VisitorArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        if input.is_empty() {
            return Ok(VisitorArgs {
                base: None,
                types: Vec::new(),
            });
        }
        let first: Path = input.parse()?;
        let base = if input.peek(Token![=>]) {
            input.parse::<Token![=>]>()?;
            Some(first.clone())
        } else {
            None
        };
        let mut types = Vec::new();
        if base.is_none() {
            types.push(first);
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }
        let rest: Punctuated<Path, Token![,]> = Punctuated::parse_terminated(input)?;
        types.extend(rest);
        Ok(VisitorArgs { base, types })
    }
}

fn last_ident(path: &Path) -> &Ident {
    &path.segments.last().unwrap().ident
}

/// Input to `__visitor_entry`: `@syan { <path> } [base =>] T, U, ...`.
struct EntryInput {
    syan: Path,
    args: VisitorArgs,
}

impl Parse for EntryInput {
    fn parse(input: ParseStream) -> Result<Self> {
        input.parse::<Token![@]>()?;
        let _kw: Ident = input.parse()?; // `syan`
        let content;
        braced!(content in input);
        let syan: Path = content.parse()?;
        let args: VisitorArgs = input.parse()?;
        Ok(EntryInput { syan, args })
    }
}

/// Kick off the metadata ping-pong from a `visitor!(...)` invocation (function-like, used inside the
/// visitor module). The syan path arrives via `$crate` captured by the `visitor!` macro_rules shim.
pub fn entry(input: TokenStream, nonce: u64) -> TokenStream {
    let EntryInput { syan, args } = match syn::parse2(input) {
        Ok(e) => e,
        Err(e) => return e.to_compile_error(),
    };
    if args.types.is_empty() {
        abort!(Span::call_site(), "visitor!(..) needs at least one AST type");
    }
    let build: Path = parse_quote!(#syan::_imp::syan_macro::__visitor_build);
    let base_tokens: TokenStream = match &args.base {
        Some(p) => quote!(#p),
        None => quote!(),
    };
    let nonce = nonce.to_string();
    let nonce: TokenStream = nonce.parse().unwrap();
    let all_types = &args.types;

    // `@visited` carries the *full paths* as written, so the generated items name the visited types
    // in the caller's path context. `@fetching` is the path of the type whose def trails the next
    // bounce (so the fetched def is recorded under it).
    let make_state = |fetching: TokenStream, rest: &[Path]| {
        quote! {
            @base { #base_tokens }
            @build { #build }
            @nonce { #nonce }
            @visited { #(#all_types),* }
            @inherited { }
            @fetching { #fetching }
            @done { }
            @rest { #(#rest),* }
        }
    };

    match &args.base {
        // With a base: first fetch the base module's visited-type list, then fetch all types. No type
        // is fetched yet, so `@fetching` is empty; the first `build` bounce pops `rest`.
        Some(base) => {
            let state = make_state(quote!(), all_types);
            quote! {
                #base::__syan_visited ! { @visited #build { #state } }
            }
        }
        // No base: pop the first type now (so `rest` carries the remainder), recording it under
        // `@fetching`.
        None => {
            let first = &args.types[0];
            let state = make_state(quote!(#first), &args.types[1..]);
            quote! {
                #first ! { @ast #build { #state } }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Subast records carried through the ping-pong.
// ---------------------------------------------------------------------------

/// One `<path> as <matchkey>` entry from a type's `#[subast]`, as carried in the metadata. `key` is
/// the ident a (container-peeled) field head is matched against; `path` is the resolvable path used
/// to fetch that sub-AST's metadata macro and as a drill match-scrutinee.
struct SubEntry {
    path: Path,
    key: Ident,
}

impl Parse for SubEntry {
    fn parse(input: ParseStream) -> Result<Self> {
        let path: Path = input.parse()?;
        input.parse::<Token![as]>()?;
        let key: Ident = input.parse()?;
        Ok(SubEntry { path, key })
    }
}

fn parse_subentries(ts: TokenStream) -> Result<Vec<SubEntry>> {
    Ok(Punctuated::<SubEntry, Token![,]>::parse_terminated
        .parse2(ts)?
        .into_iter()
        .collect())
}

/// Re-serialize subast entries as `<path> as <key>, ...` for the next ping-pong bounce.
fn subentries_tokens(entries: &[SubEntry]) -> TokenStream {
    let parts: Vec<TokenStream> = entries
        .iter()
        .map(|e| {
            let p = &e.path;
            let k = &e.key;
            quote!(#p as #k)
        })
        .collect();
    quote!( #(#parts),* )
}

/// Whitespace-insensitive string form of a path, for full-path fetch-dedup and drill lookup (so
/// `a::Cast` and `b::Cast` are distinct).
fn norm_path(p: &Path) -> String {
    quote!(#p).to_string().replace(' ', "")
}

/// The `@recurse { … }` metadata a `#[recurse]` cycle type carries (absent for acyclic types). It
/// drives depth-generic visitor generation: `node` is the `__XRec` depth-generic node type; `roots`
/// and `depth` are the parallel root idents and their depth params; `terms` the per-root terminators;
/// `cycle` every type ident in the SCC. (See the CLAUDE.md "Metadata contract" section.)
#[derive(Clone)]
struct RecurseMeta {
    node: Path,
    roots: Vec<Ident>,
    depth: Vec<Ident>,
    terms: Vec<Path>,
    cycle: Vec<Ident>,
}

/// Parse whitespace-separated paths (for `@terms`); each `Path` ends where the next segment lacks a
/// leading `::`, so `crate::a::ATerm crate::a::BTerm` parses as two paths.
fn parse_paths(ts: TokenStream) -> Result<Vec<Path>> {
    let parser = |input: ParseStream| {
        let mut out = Vec::new();
        while !input.is_empty() {
            out.push(input.parse::<Path>()?);
        }
        Ok(out)
    };
    parser.parse2(ts)
}

/// Parse a `@recurse` body: `@node {PATH} @roots {idents} @depth {idents} @terms {paths} @cycle {idents}`.
/// `@roots`/`@depth`/`@cycle` are whitespace-separated idents; `@terms` whitespace-separated paths
/// (all parallel to `@roots`). Matches what `#[recurse]` emits (see `macro/recurse.rs`).
fn parse_recurse(ts: TokenStream) -> Result<RecurseMeta> {
    let parser = |input: ParseStream| {
        let (mut node, mut roots, mut depth, mut terms, mut cycle) =
            (None, Vec::new(), Vec::new(), Vec::new(), Vec::new());
        while !input.is_empty() {
            let (name, content) = parse_section(input)?;
            match name.to_string().as_str() {
                "node" => node = Some(syn::parse2(content)?),
                "roots" => roots = parse_idents(content)?,
                "depth" => depth = parse_idents(content)?,
                "terms" => terms = parse_paths(content)?,
                "cycle" => cycle = parse_idents(content)?,
                other => {
                    return Err(Error::new(name.span(), format!("unknown @recurse section @{other}")))
                }
            }
        }
        let node = node.ok_or_else(|| Error::new(Span::call_site(), "missing @node in @recurse"))?;
        if roots.len() != depth.len() || roots.len() != terms.len() {
            return Err(Error::new(
                Span::call_site(),
                "@recurse: @roots, @depth, @terms must be parallel (same length)",
            ));
        }
        Ok(RecurseMeta { node, roots, depth, terms, cycle })
    };
    parser.parse2(ts)
}

/// Re-serialize a `RecurseMeta` for the next ping-pong bounce (mirrors `parse_recurse`).
fn emit_recurse(r: &RecurseMeta) -> TokenStream {
    let RecurseMeta { node, roots, depth, terms, cycle } = r;
    quote! {
        @node { #node }
        @roots { #(#roots)* }
        @depth { #(#depth)* }
        @terms { #(#terms)* }
        @cycle { #(#cycle)* }
    }
}

/// A fetched AST type: the path it was fetched by, its (cleaned) definition, its `#[subast]`, and —
/// for a `#[recurse]` cycle type — its `@recurse` coordinates (`None` for an acyclic type).
struct DoneType {
    path: Path,
    def: Item,
    subast: Vec<SubEntry>,
    recurse: Option<RecurseMeta>,
}

// ---------------------------------------------------------------------------
// `__visitor_build`: receives accumulated state + the just-resolved definition,
// fetches the next type or generates the module.
// ---------------------------------------------------------------------------

struct BuildInput {
    base: Option<Path>,
    build: Path,
    nonce: TokenStream,
    visited: Vec<Path>,
    inherited: Vec<Ident>,
    /// The base visitor's generic-param union (when inheriting), supplied by the base's
    /// `__syan_visited` macro, so the new trait can reference `base::Visit<..>` with the *base's*
    /// arity instead of the new union's.
    base_generics: Vec<GenericParam>,
    /// The direct base's own transitive ancestors (for multi-level `base => mid => new`), so the new
    /// visitor can emit the empty `Driver` impl for every transitive supertrait, not just the direct
    /// base.
    base_ancestors: Vec<AncIn>,
    /// Path of the type whose `@ast`/`@subast` trail in this bounce (so the fetched def is recorded
    /// under the path it was fetched by). Empty before any type is fetched.
    fetching: Option<Path>,
    done: Vec<DoneType>,
    rest: Vec<Path>,
    just_def: Option<Item>,
    just_subast: Vec<SubEntry>,
    /// The `@recurse` section of the type fetched this bounce (a `#[recurse]` cycle type), if any.
    just_recurse: Option<RecurseMeta>,
}

/// Parse one `@<name> { .. }` section, returning the name and the braced content as tokens.
fn parse_section(input: ParseStream) -> Result<(Ident, TokenStream)> {
    input.parse::<Token![@]>()?;
    let name: Ident = input.parse()?;
    let content;
    braced!(content in input);
    Ok((name, content.parse()?))
}

/// One transitive-base obligation for multi-level inheritance: an ancestor visitor's path and the
/// names of its generic params (re-mapped into the extending visitor's union when emitted).
#[derive(Clone)]
struct AncIn {
    path: Path,
    names: Vec<Ident>,
}

/// Parse `@anc { @a { @p {PATH} @n {name…} } … }`.
fn parse_ancestors(ts: TokenStream) -> Result<Vec<AncIn>> {
    let parser = |input: ParseStream| {
        let mut out = Vec::new();
        while !input.is_empty() {
            input.parse::<Token![@]>()?;
            let kw: Ident = input.parse()?;
            if kw != "a" {
                return Err(Error::new(kw.span(), "expected `@a` in @anc"));
            }
            let content;
            braced!(content in input);
            let mut path = None;
            let mut names = Vec::new();
            while !content.is_empty() {
                let (name, inner) = parse_section(&content)?;
                match name.to_string().as_str() {
                    "p" => path = Some(syn::parse2(inner)?),
                    "n" => names = parse_idents(inner)?,
                    other => {
                        return Err(Error::new(name.span(), format!("unknown @a section @{other}")))
                    }
                }
            }
            out.push(AncIn {
                path: path.ok_or_else(|| Error::new(Span::call_site(), "missing @p in @a"))?,
                names,
            });
        }
        Ok(out)
    };
    parser.parse2(ts)
}

fn emit_ancestors(anc: &[AncIn]) -> TokenStream {
    let blocks: Vec<TokenStream> = anc
        .iter()
        .map(|a| {
            let path = &a.path;
            let names = &a.names;
            quote! { @a { @p { #path } @n { #(#names)* } } }
        })
        .collect();
    quote!( #(#blocks)* )
}

/// The host crate of a direct-base path: `Some(ident)` when it is rooted at an *external* crate
/// (e.g. `syan_rust::inherit::mid`), `None` for same-crate roots (`crate`/`super`/`self`) or a
/// leading-`::` absolute path. Used to requalify a transitive ancestor that an *upstream*
/// intermediate recorded `crate::`-relative (its own crate) into a path the *downstream* extender
/// can resolve. (A `$crate` cannot do this: emitted by a proc-macro into a generated `macro_rules`
/// body it resolves only for fetch/macro-invocation paths, **not** for the trait path re-emitted
/// into the new `Driver`'s supertrait impl — so multi-level cross-crate `base => mid => new` with an
/// *upstream* `mid` needs this concrete requalification instead.)
/// Whether a path is rooted in the *current* crate (`crate::` / `self::` / `super::`). A foreign path
/// (an external crate name, or a leading `::`) is not — an inherent `impl` for such a type would be
/// E0116 (inherent impls must live in the type's defining crate), so the recurse path skips inherent
/// `.visit()`/`.visit_mut()` for them and the trait method (`Visit::visit_*`) is used instead.
fn path_is_crate_local(p: &Path) -> bool {
    if p.leading_colon.is_some() {
        return false;
    }
    matches!(
        p.segments.first().map(|s| s.ident.to_string()).as_deref(),
        Some("crate") | Some("self") | Some("super")
    )
}

fn base_host_crate(base: &Path) -> Option<Ident> {
    if base.leading_colon.is_some() {
        return None;
    }
    let first = base.segments.first()?;
    if !matches!(first.arguments, PathArguments::None) {
        return None;
    }
    let s = first.ident.to_string();
    if s == "crate" || s == "super" || s == "self" {
        None
    } else {
        Some(first.ident.clone())
    }
}

/// Replace a leading bare `crate` segment of an ancestor path with `host` (the direct base's crate),
/// so a `crate::…`-relative ancestor recorded by an upstream intermediate resolves downstream. Only a
/// leading bare `crate` is requalified: a path already concrete (rooted at a crate name / `::`) is
/// left alone (it points where it should), and a `super::`/`self::`-relative ancestor recorded by an
/// upstream intermediate is *also* left alone — and so remains unresolvable downstream (the same
/// residual hole as `ast.rs`'s `crate_rooted_tokens`; canonical `crate::`-rooted `visitor!` entry
/// paths, which the docs already recommend, avoid it).
fn requalify_ancestor(anc: &Path, host: &Ident) -> Path {
    if anc.leading_colon.is_none()
        && anc
            .segments
            .first()
            .is_some_and(|s| s.ident == "crate" && matches!(s.arguments, PathArguments::None))
    {
        let mut out = anc.clone();
        out.segments[0].ident = host.clone();
        out
    } else {
        anc.clone()
    }
}

/// Parse `@done { @t { @path {..} @def {..} @subast {..} } .. }`.
fn parse_done(ts: TokenStream) -> Result<Vec<DoneType>> {
    let parser = |input: ParseStream| {
        let mut out = Vec::new();
        while !input.is_empty() {
            input.parse::<Token![@]>()?;
            let kw: Ident = input.parse()?;
            if kw != "t" {
                return Err(Error::new(kw.span(), "expected `@t` in @done"));
            }
            let content;
            braced!(content in input);
            out.push(parse_done_type(&content)?);
        }
        Ok(out)
    };
    parser.parse2(ts)
}

fn parse_done_type(input: ParseStream) -> Result<DoneType> {
    let mut path = None;
    let mut def = None;
    let mut subast = Vec::new();
    let mut recurse = None;
    while !input.is_empty() {
        let (name, content) = parse_section(input)?;
        match name.to_string().as_str() {
            "path" => path = Some(syn::parse2(content)?),
            "def" => def = Some(syn::parse2(content)?),
            "subast" => subast = parse_subentries(content)?,
            "recurse" => recurse = Some(parse_recurse(content)?),
            other => {
                return Err(Error::new(name.span(), format!("unknown @t section @{other}")))
            }
        }
    }
    Ok(DoneType {
        path: path.ok_or_else(|| Error::new(Span::call_site(), "missing @path in @t"))?,
        def: def.ok_or_else(|| Error::new(Span::call_site(), "missing @def in @t"))?,
        subast,
        recurse,
    })
}

impl Parse for BuildInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut base = None;
        let mut build = None;
        let mut nonce = TokenStream::new();
        let mut visited = Vec::new();
        let mut inherited = Vec::new();
        let mut base_generics = Vec::new();
        let mut base_ancestors = Vec::new();
        let mut fetching = None;
        let mut done = Vec::new();
        let mut rest = Vec::new();
        let mut just_def = None;
        let mut just_subast = Vec::new();
        let mut just_recurse = None;

        while !input.is_empty() {
            let (name, content) = parse_section(input)?;
            match name.to_string().as_str() {
                "base" => {
                    if !content.is_empty() {
                        base = Some(syn::parse2(content)?);
                    }
                }
                "build" => build = Some(syn::parse2(content)?),
                "nonce" => nonce = content,
                "visited" => {
                    visited = Punctuated::<Path, Token![,]>::parse_terminated
                        .parse2(content)?
                        .into_iter()
                        .collect();
                }
                // `@inherited` is the carried set; `@inh` is appended by a base's visited-list macro.
                "inherited" | "inh" => inherited.extend(parse_idents(content)?),
                // `@baseg` is the carried base generics; `@bg` is appended by a base's macro.
                "baseg" | "bg" => {
                    if !content.is_empty() {
                        base_generics = Punctuated::<GenericParam, Token![,]>::parse_terminated
                            .parse2(content)?
                            .into_iter()
                            .collect();
                    }
                }
                // `@anc` is the carried ancestor chain; `@an` is appended by a base's macro.
                "anc" | "an" => base_ancestors = parse_ancestors(content)?,
                "fetching" => {
                    if !content.is_empty() {
                        fetching = Some(syn::parse2(content)?);
                    }
                }
                "done" => done = parse_done(content)?,
                "rest" => {
                    let paths =
                        Punctuated::<Path, Token![,]>::parse_terminated.parse2(content)?;
                    rest = paths.into_iter().collect();
                }
                "ast" => just_def = Some(syn::parse2(content)?),
                "subast" => just_subast = parse_subentries(content)?,
                "recurse" => just_recurse = Some(parse_recurse(content)?),
                other => {
                    return Err(Error::new(name.span(), format!("unknown section @{other}")))
                }
            }
        }

        Ok(BuildInput {
            base,
            build: build.ok_or_else(|| Error::new(Span::call_site(), "missing @build"))?,
            nonce,
            visited,
            inherited,
            base_generics,
            base_ancestors,
            fetching,
            done,
            rest,
            just_def,
            just_subast,
            just_recurse,
        })
    }
}

fn parse_idents(ts: TokenStream) -> Result<Vec<Ident>> {
    let parser = |input: ParseStream| {
        let mut out = Vec::new();
        while !input.is_empty() {
            out.push(input.parse::<Ident>()?);
        }
        Ok(out)
    };
    parser.parse2(ts)
}

pub fn build(input: TokenStream) -> TokenStream {
    let mut st: BuildInput = match syn::parse2(input) {
        Ok(s) => s,
        Err(e) => return e.to_compile_error(),
    };

    // Record the just-fetched type (def + subast) under the path it was fetched by, then discover
    // the followed-but-unvisited intermediates it references and enqueue them for drilling.
    if let Some(def) = st.just_def.take() {
        let path = match st.fetching.clone() {
            Some(p) => p,
            None => {
                return Error::new(Span::call_site(), "internal: @ast without @fetching")
                    .to_compile_error()
            }
        };
        let subast = std::mem::take(&mut st.just_subast);

        let method_set: HashSet<String> = st
            .visited
            .iter()
            .map(|p| last_ident(p).to_string())
            .chain(st.inherited.iter().map(|i| i.to_string()))
            .collect();
        let self_ident = item_ident(&def).map(|i| i.to_string());
        let mut seen: HashSet<String> = st
            .done
            .iter()
            .map(|d| norm_path(&d.path))
            .chain(st.rest.iter().map(norm_path))
            .collect();
        for entry_path in
            followed_intermediates(&def, &subast, &method_set, self_ident.as_deref())
        {
            if seen.insert(norm_path(&entry_path)) {
                st.rest.push(entry_path);
            }
        }
        let recurse = st.just_recurse.take();
        st.done.push(DoneType { path, def, subast, recurse });
    }
    st.fetching = None;

    if !st.rest.is_empty() {
        let next = st.rest.remove(0);
        let BuildInput {
            base,
            build,
            nonce,
            visited,
            inherited,
            base_generics,
            base_ancestors,
            done,
            rest,
            ..
        } = &st;
        let base_tokens: TokenStream = match base {
            Some(p) => quote!(#p),
            None => quote!(),
        };
        let done_tokens = emit_done(done);
        let anc_tokens = emit_ancestors(base_ancestors);
        return quote! {
            #next ! {
                @ast #build {
                    @base { #base_tokens }
                    @build { #build }
                    @nonce { #nonce }
                    @visited { #(#visited),* }
                    @inherited { #(#inherited)* }
                    @baseg { #(#base_generics),* }
                    @anc { #anc_tokens }
                    @fetching { #next }
                    @done { #done_tokens }
                    @rest { #(#rest),* }
                }
            }
        };
    }

    generate_module(&st)
}

/// Re-serialize `@done` (the fetched types) for the next ping-pong bounce.
fn emit_done(done: &[DoneType]) -> TokenStream {
    let blocks: Vec<TokenStream> = done
        .iter()
        .map(|d| {
            let path = &d.path;
            let def = &d.def;
            let subast = subentries_tokens(&d.subast);
            let recurse = match &d.recurse {
                Some(r) => {
                    let r = emit_recurse(r);
                    quote!( @recurse { #r } )
                }
                None => quote!(),
            };
            quote! { @t { @path { #path } @def { #def } @subast { #subast } #recurse } }
        })
        .collect();
    quote!( #(#blocks)* )
}

/// Resolvable paths of `def`'s field types that are *followed* (head in `subast`) but neither
/// visited/inherited (a method call) nor self (already in `done`) — i.e. unlisted intermediates to
/// fetch so they can be drilled through.
fn followed_intermediates(
    def: &Item,
    subast: &[SubEntry],
    method_set: &HashSet<String>,
    self_ident: Option<&str>,
) -> Vec<Path> {
    let mut user_types: HashSet<String> = subast.iter().map(|e| e.key.to_string()).collect();
    if let Some(s) = self_ident {
        user_types.insert(s.to_string());
    }
    let mut out = Vec::new();
    for_each_field_type(def, &mut |ty| {
        discover_followed(ty, subast, method_set, self_ident, &user_types, &mut out)
    });
    out
}

/// Recurse a field type (descending into tuple elements) collecting followed-but-unlisted
/// intermediate paths to fetch for inline drilling.
fn discover_followed(
    ty: &Type,
    subast: &[SubEntry],
    method_set: &HashSet<String>,
    self_ident: Option<&str>,
    user_types: &HashSet<String>,
    out: &mut Vec<Path>,
) {
    // Tuple element types must be inspected too (a followed type may be nested in a tuple field).
    if let Some(elems) = as_tuple(ty) {
        for elem in elems {
            discover_followed(elem, subast, method_set, self_ident, user_types, out);
        }
        return;
    }
    if let Some(p) = peel(ty, user_types) {
        let hs = p.head.to_string();
        if Some(hs.as_str()) == self_ident {
            return; // self -> already in `done`
        }
        if let Some(e) = subast.iter().find(|e| e.key == p.head) {
            // Fetch only when the entry's *real* type isn't visited/inherited (else a method,
            // already fetched under its `visitor!(..)` path — even when the head is aliased).
            if !method_set.contains(&last_ident(&e.path).to_string()) {
                out.push(e.path.clone());
            }
        }
    }
}

fn for_each_field_type(def: &Item, f: &mut dyn FnMut(&Type)) {
    match def {
        Item::Enum(e) => {
            for v in &e.variants {
                for field in &v.fields {
                    f(&field.ty);
                }
            }
        }
        Item::Struct(s) => {
            for field in &s.fields {
                f(&field.ty);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Module generation
// ---------------------------------------------------------------------------

/// Mint a generated helper param ident whose name avoids every name in `reserved` (the visited
/// types' generic params), appending `_` until free. Rust rejects two generic params with the same
/// name string in one item regardless of hygiene, so this — not just `mixed_site` — is what lets a
/// visited type declare a param literally named `__V`/etc. The `mixed_site` span is kept for extra
/// isolation from other call-site idents.
fn fresh_ident(base: &str, reserved: &HashSet<String>) -> Ident {
    let mut name = base.to_string();
    while reserved.contains(&name) {
        name.push('_');
    }
    Ident::new(&name, Span::mixed_site())
}

/// Like [`fresh_ident`] but for an indexed family `<prefix>0..<prefix>{max}` (tuple closure params):
/// returns a prefix such that no `<prefix>{i}` collides with `reserved`.
fn fresh_prefix(base: &str, reserved: &HashSet<String>, max: usize) -> String {
    let mut prefix = base.to_string();
    while (0..max).any(|i| reserved.contains(&format!("{prefix}{i}"))) {
        prefix.push('_');
    }
    prefix
}

/// Right-nested `Chain(self.0.into_hook(), Chain(self.1.into_hook(), ...))` over tuple members.
fn build_chain(members: &[Index], chain: &Ident, into_hook: &Ident) -> TokenStream {
    let m = &members[0];
    if members.len() == 1 {
        quote!(self.#m.#into_hook())
    } else {
        let rest = build_chain(&members[1..], chain, into_hook);
        quote!(#chain(self.#m.#into_hook(), #rest))
    }
}

/// `IntoVisitor[Mut]` impls for tuples of closures, arity 2..=`max_arity`. `union_where` are the
/// visited types' `where`-predicates, appended to each impl (it names the visited types via the
/// `IntoHook<.., T>` bounds, so they must stay well-formed).
fn tuple_impls(
    max_arity: usize,
    g_params: &[GenericParam],
    g_args: &[TokenStream],
    g_use: &TokenStream,
    mutable: bool,
    union_where: &[WherePredicate],
) -> Vec<TokenStream> {
    let suffix = if mutable { "Mut" } else { "" };
    let into_vis_tr = Ident::new(&format!("IntoVisitor{suffix}"), Span::call_site());
    let into_hook_tr = Ident::new(&format!("IntoHook{suffix}"), Span::call_site());
    let into_vis_fn = Ident::new(&format!("into_visitor{}", mt(mutable)), Span::call_site());
    let into_hook_fn = Ident::new(&format!("into_hook{}", mt(mutable)), Span::call_site());
    let visit_tr = Ident::new(&format!("Visit{suffix}"), Span::call_site());
    let driver = Ident::new(&format!("Driver{suffix}"), Span::call_site());
    let chain_id = Ident::new(&format!("Chain{suffix}"), Span::call_site());
    // Helper param prefixes that avoid the visited types' own generic param names (so a visited type
    // may declare a param literally named `__F0`/`__T0`/…).
    let reserved: HashSet<String> = g_params.iter().map(param_name).collect();
    let pf = fresh_prefix("__F", &reserved, max_arity);
    let pt = fresh_prefix("__T", &reserved, max_arity);
    (2..=max_arity)
        .map(|n| {
            let fs: Vec<Ident> = (0..n)
                .map(|i| Ident::new(&format!("{pf}{i}"), Span::mixed_site()))
                .collect();
            let ts: Vec<Ident> = (0..n)
                .map(|i| Ident::new(&format!("{pt}{i}"), Span::mixed_site()))
                .collect();
            let members: Vec<Index> = (0..n).map(Index::from).collect();
            let wheres: Vec<TokenStream> = fs
                .iter()
                .zip(&ts)
                .map(|(f, t)| quote!(#f: #into_hook_tr< #(#g_args,)* #t >))
                .collect();
            let chain = build_chain(&members, &chain_id, &into_hook_fn);
            quote! {
                impl< #(#g_params,)* #(#fs,)* #(#ts,)* >
                    #into_vis_tr< #(#g_args,)* ( #(#ts,)* ) > for ( #(#fs,)* )
                where #(#wheres,)* #(#union_where,)*
                {
                    fn #into_vis_fn(self) -> impl #visit_tr #g_use {
                        #driver( #chain )
                    }
                }
            }
        })
        .collect()
}

/// Lowers a visited type's `visit_*` body: a field followed via a *visited/inherited* head becomes a
/// `this.visit_<head>(..)` method call; a field followed via an *unlisted intermediate* is drilled
/// through inline (its def destructured, recursing into its `#[subast]` fields); any other field is
/// a leaf.
struct Lower<'a> {
    /// Heads that get a method call (the `visitor!(..)` set ∪ inherited).
    method_set: &'a HashSet<String>,
    /// Fetched types keyed by `norm_path`, for resolving an intermediate's def when drilling.
    done_by_path: &'a HashMap<String, &'a DoneType>,
    mutable: bool,
}

impl<'a> Lower<'a> {
    fn iter_fn(&self) -> Ident {
        Ident::new(if self.mutable { "iter_mut" } else { "iter" }, Span::call_site())
    }

    fn amp(&self) -> TokenStream {
        if self.mutable {
            quote!(&mut)
        } else {
            quote!(&)
        }
    }

    /// Visit a value `access` (an expression of reference type `&Box^head_box<head>` / `&mut ...`),
    /// where `head` is the *effective* (real) head type. A method head emits a call (deref-coercion
    /// handles any `Box`); an intermediate is drilled inline (deref `head_box+1` times to a `&head`
    /// scrutinee, then destructure). May be empty (a finite drill that reaches no visited type).
    fn visit_value(
        &self,
        access: &TokenStream,
        head: &Ident,
        drill_path: &Path,
        head_box: usize,
        depth: usize,
        stack: &mut Vec<String>,
    ) -> TokenStream {
        if self.method_set.contains(&head.to_string()) {
            let m = method_ident_m(head, self.mutable);
            return quote!( this.#m(#access); );
        }
        // Unlisted intermediate -> inline drill.
        let key = norm_path(drill_path);
        if stack.iter().any(|s| s == &key) {
            abort!(
                Span::call_site(),
                "`#[subast]` cycle through unlisted intermediate `{}`: it cannot be drilled inline. \
                 List one of the cycle's types in `visitor!(..)` so a method call breaks the recursion",
                head
            );
        }
        let dt = match self.done_by_path.get(&key) {
            Some(dt) => *dt,
            None => abort!(
                Span::call_site(),
                "internal: no metadata fetched for drilled type `{}` ({})",
                head,
                key
            ),
        };
        stack.push(key);
        let stars: TokenStream = (0..=head_box).map(|_| quote!(*)).collect();
        let amp = self.amp();
        let scrut = quote!( #amp #stars #access );
        let block = self.destructure(&dt.def, &dt.subast, &dt.path, &scrut, depth + 1, stack);
        stack.pop();
        block
    }

    /// Destructure `scrutinee` (a `&T`/`&mut T` expr) per `def`/`subast` and visit followed fields.
    /// Empty when no followed field anywhere reaches a visited type.
    fn destructure(
        &self,
        def: &Item,
        subast: &[SubEntry],
        path: &Path,
        scrutinee: &TokenStream,
        depth: usize,
        stack: &mut Vec<String>,
    ) -> TokenStream {
        let self_ident = item_ident(def);
        match def {
            Item::Enum(e) => {
                let mut arms = Vec::new();
                let mut any = false;
                for v in &e.variants {
                    let (pat, stmts, has) =
                        self.fields(&v.fields, subast, self_ident, path, depth, stack);
                    any |= has;
                    let vident = &v.ident;
                    arms.push(quote!( #path::#vident #pat => { #stmts } ));
                }
                if !any {
                    return quote!();
                }
                quote!( match #scrutinee { #(#arms)* } )
            }
            Item::Struct(s) => {
                let (pat, stmts, has) =
                    self.fields(&s.fields, subast, self_ident, path, depth, stack);
                if !has {
                    return quote!();
                }
                match &s.fields {
                    Fields::Unit => quote!(),
                    _ => quote!( { let #path #pat = #scrutinee; #stmts } ),
                }
            }
            _ => quote!(),
        }
    }

    /// Build `(pattern, statements, has_any_visit)` for a field set. `self_ident` is the type being
    /// destructured (a field whose head is it is followed by implicit self-recursion).
    fn fields(
        &self,
        fields: &Fields,
        subast: &[SubEntry],
        self_ident: Option<&Ident>,
        path: &Path,
        depth: usize,
        stack: &mut Vec<String>,
    ) -> (TokenStream, TokenStream, bool) {
        match fields {
            Fields::Named(named) => {
                let mut binds = Vec::new();
                let mut stmts = Vec::new();
                for (idx, f) in named.named.iter().enumerate() {
                    let name = f.ident.clone().unwrap();
                    let bind = quote!(#name);
                    if let Some(stmt) =
                        self.lower_field(&f.ty, &bind, idx, subast, self_ident, path, depth, stack)
                    {
                        binds.push(quote!(#name));
                        stmts.push(stmt);
                    }
                }
                let has = !stmts.is_empty();
                (quote!( { #(#binds,)* .. } ), quote!( #(#stmts)* ), has)
            }
            Fields::Unnamed(unnamed) => {
                let mut pats = Vec::new();
                let mut stmts = Vec::new();
                for (idx, f) in unnamed.unnamed.iter().enumerate() {
                    let bind_id = Ident::new(&format!("__f{depth}_{idx}"), Span::call_site());
                    let bind = quote!(#bind_id);
                    if let Some(stmt) =
                        self.lower_field(&f.ty, &bind, idx, subast, self_ident, path, depth, stack)
                    {
                        pats.push(quote!(#bind_id));
                        stmts.push(stmt);
                    } else {
                        pats.push(quote!(_));
                    }
                }
                let has = !stmts.is_empty();
                (quote!( ( #(#pats),* ) ), quote!( #(#stmts)* ), has)
            }
            Fields::Unit => (quote!(), quote!(), false),
        }
    }

    /// Lower one field. `binding` is the destructured field (a `&Field`/`&mut Field`). Returns the
    /// visit statement(s), or `None` for a leaf / finite dead-end (the caller binds `_`).
    #[allow(clippy::too_many_arguments)]
    fn lower_field(
        &self,
        ty: &Type,
        binding: &TokenStream,
        idx: usize,
        subast: &[SubEntry],
        self_ident: Option<&Ident>,
        path: &Path,
        depth: usize,
        stack: &mut Vec<String>,
    ) -> Option<TokenStream> {
        // A tuple field: destructure it and lower each element (an element may itself be a followed
        // type, a container of one, or a nested tuple). Leaf elements bind `_`; if no element is
        // followed the whole tuple is a leaf (`None`, caller binds `_`). Mirrors the `#[recurse]`
        // path's tuple handling.
        if let Some(elems) = as_tuple(ty) {
            let mut pats = Vec::new();
            let mut stmts = Vec::new();
            for (i, elem) in elems.iter().enumerate() {
                let ebind = Ident::new(&format!("__t{depth}_{idx}_{i}"), Span::call_site());
                if let Some(stmt) = self.lower_field(
                    elem,
                    &quote!(#ebind),
                    idx,
                    subast,
                    self_ident,
                    path,
                    depth + 1,
                    stack,
                ) {
                    pats.push(quote!(#ebind));
                    stmts.push(stmt);
                } else {
                    pats.push(quote!(_));
                }
            }
            if stmts.is_empty() {
                return None; // tuple of only leaves -> leaf
            }
            return Some(quote!( { let ( #(#pats,)* ) = #binding; #(#stmts)* } ));
        }

        let user_types = self_and_subast_keys(self_ident, subast);
        let p = peel(ty, &user_types)?;
        if p.nested {
            abort!(
                Span::call_site(),
                "field type `{}` uses nested containers (e.g. `Vec<Option<_>>`), which the visitor \
                 does not support; flatten it or wrap the inner part in its own `#[derive(Ast)]` type",
                quote!(#ty)
            );
        }
        // A field behind a shared reference (`&T`/`&[T]`) is visitable on the shared side but a leaf
        // for `visit_mut` — there is no `&mut head` reachable through a `&`.
        if self.mutable && p.shared_ref {
            return None;
        }
        // Followed iff the head is self (implicit) or listed in this type's `#[subast]`. The
        // *effective* head — the real type name + path — comes from the matched entry (so an
        // aliased entry `Real as Aliased` dispatches to `visit_real`, not `visit_aliased`).
        let (head, drill_path): (Ident, Path) = if Some(&p.head) == self_ident {
            (p.head.clone(), path.clone())
        } else if let Some(e) = subast.iter().find(|e| e.key == p.head) {
            (last_ident(&e.path).clone(), e.path.clone())
        } else {
            return None; // leaf
        };
        match p.container {
            Container::Direct => {
                let s = self.visit_value(binding, &head, &drill_path, p.head_box, depth, stack);
                (!s.is_empty()).then_some(s)
            }
            Container::Seq => {
                let elem = Ident::new(&format!("__e{depth}_{idx}"), Span::call_site());
                let inner =
                    self.visit_value(&quote!(#elem), &head, &drill_path, p.head_box, depth, stack);
                (!inner.is_empty()).then(|| {
                    let iter = self.iter_fn();
                    quote!( for #elem in #binding.#iter() { #inner } )
                })
            }
            Container::Opt => {
                let elem = Ident::new(&format!("__e{depth}_{idx}"), Span::call_site());
                let inner =
                    self.visit_value(&quote!(#elem), &head, &drill_path, p.head_box, depth, stack);
                (!inner.is_empty()).then(|| {
                    // Deref through any `Box` around the Option, then match-ergonomics binds `&elem`.
                    let amp = self.amp();
                    let stars: TokenStream = (0..=p.cont_box).map(|_| quote!(*)).collect();
                    quote!( if let Some(#elem) = #amp #stars #binding { #inner } )
                })
            }
        }
    }
}

/// The set of idents that count as user AST types when peeling a field of a type with the given
/// `self_ident` and `#[subast]` entries: the type's own ident plus every `#[subast]` matchkey.
fn self_and_subast_keys(self_ident: Option<&Ident>, subast: &[SubEntry]) -> HashSet<String> {
    let mut s: HashSet<String> = subast.iter().map(|e| e.key.to_string()).collect();
    if let Some(id) = self_ident {
        s.insert(id.to_string());
    }
    s
}

/// A visited type's `where`-clause predicates (e.g. `S: Bound`), or empty when it has none. These
/// must be repeated on every generated item that names the type so the type is well-formed there.
fn item_where_preds(item: &Item) -> Vec<WherePredicate> {
    item_generics(item)
        .and_then(|g| g.where_clause.as_ref())
        .map(|w| w.predicates.iter().cloned().collect())
        .unwrap_or_default()
}

/// Render `where p0, p1, …` (or nothing when empty) for the given predicates.
fn where_clause(preds: &[WherePredicate]) -> TokenStream {
    if preds.is_empty() {
        quote!()
    } else {
        quote!( where #(#preds),* )
    }
}

/// One visited type's identifier (for method/struct names), the full path it is referenced by, its
/// own generic params (def-side) and use-side args, its `where`-clause predicates (repeated on the
/// inherent impl that names it), and its shared-ref and `&mut` bodies.
struct VType {
    ident: Ident,
    path: TokenStream,
    own_params: Vec<GenericParam>,
    own_use: TokenStream,
    own_where: Vec<WherePredicate>,
    body: TokenStream,
    body_mut: TokenStream,
}

/// A transitive supertrait obligation (an ancestor visitor), resolved against the new union: the
/// ancestor's path, the union params it is parameterized by, and the matching use-side args.
struct Ancestor {
    path: TokenStream,
    g_params: Vec<GenericParam>,
    g_use: TokenStream,
}

/// Generate every item for one mutability "side" (`Visit`/`VisitMut`, etc.).
#[allow(clippy::too_many_arguments)]
fn gen_side(
    mutable: bool,
    vtypes: &[VType],
    g_params: &[GenericParam],
    g_args: &[TokenStream],
    g_def: &TokenStream,
    g_use: &TokenStream,
    base_g_use: &TokenStream,
    ancestors: &[Ancestor],
    base: &Option<Path>,
    union_where: &[WherePredicate],
) -> TokenStream {
    let suffix = if mutable { "Mut" } else { "" };
    let id = |s: &str| Ident::new(s, Span::call_site());
    let visit_tr = id(&format!("Visit{suffix}"));
    let into_vis_tr = id(&format!("IntoVisitor{suffix}"));
    let into_hook_tr = id(&format!("IntoHook{suffix}"));
    let hook_tr = id(&format!("Hook{suffix}"));
    let driver = id(&format!("Driver{suffix}"));
    let chain = id(&format!("Chain{suffix}"));
    let into_vis_fn = id(&format!("into_visitor{}", mt(mutable)));
    let into_hook_fn = id(&format!("into_hook{}", mt(mutable)));
    let visit_method = id(&format!("visit{}", mt(mutable)));
    let amp = if mutable { quote!(&mut) } else { quote!(&) };
    let recv = if mutable { quote!(&mut self) } else { quote!(&self) };
    let self_ret = if mutable { quote!(&mut Self) } else { quote!(&Self) };

    // Generated helper type params, named to avoid collision with the visited types' own generic
    // params (which arrive verbatim via `#g_params`). A visited type may thus declare a param
    // literally named `__V`/`__T`/`__H`/`__F`/`__A`/`__B` (or `__F0`…).
    let reserved: HashSet<String> = g_params.iter().map(param_name).collect();
    let p_v = fresh_ident("__V", &reserved);
    let p_t = fresh_ident("__T", &reserved);
    let p_h = fresh_ident("__H", &reserved);
    let p_f = fresh_ident("__F", &reserved);
    let p_a = fresh_ident("__A", &reserved);
    let p_b = fresh_ident("__B", &reserved);

    struct S {
        ty: TokenStream,
        method: Ident,
        hook: Ident,
        hook_struct: Ident,
        body: TokenStream,
    }
    let sides: Vec<S> = vtypes
        .iter()
        .map(|t| {
            let ident = t.ident.clone();
            let own = &t.own_use;
            let path = &t.path;
            let ty = quote!( #path #own );
            S {
                ty,
                method: method_ident_m(&ident, mutable),
                hook: Ident::new(
                    &format!("hook_{}{}", to_snake(&ident), mt(mutable)),
                    Span::call_site(),
                ),
                hook_struct: Ident::new(&format!("{ident}Hook{suffix}"), Span::call_site()),
                body: if mutable { t.body_mut.clone() } else { t.body.clone() },
            }
        })
        .collect();

    let tup = tuple_impls(8, g_params, g_args, g_use, mutable, union_where);
    // The union of every visited type's `where`-predicates, repeated on each generated item that is
    // quantified over the full param union (the trait, free fns, the `&mut V` / Driver / closure /
    // Chain impls) so a visited type like `enum Expr<S> where S: Bound { .. }` stays well-formed.
    let uw = where_clause(union_where);

    // Inherent `visit` / `visit_mut` per type (replaces the Visitable trait). Each type's own
    // params go on the impl; any extra union params go on the method (so a type that doesn't use
    // every union param doesn't leave the impl param unconstrained). The type's own `where`-clause
    // (referencing only its own params) goes on the impl so naming `Expr<S>` stays well-formed.
    let inherent: Vec<TokenStream> = vtypes
        .iter()
        .map(|vt| {
            let own_names: HashSet<String> = vt.own_params.iter().map(param_name).collect();
            let extra: Vec<&GenericParam> = g_params
                .iter()
                .filter(|p| !own_names.contains(&param_name(p)))
                .collect();
            let own_def = angle(&vt.own_params);
            let own_w = where_clause(&vt.own_where);
            let path = &vt.path;
            let own_use = &vt.own_use;
            let method = method_ident_m(&vt.ident, mutable);
            quote! {
                impl #own_def #path #own_use #own_w {
                    pub fn #visit_method< #(#extra,)* #p_t >(
                        #recv,
                        visitor: impl #into_vis_tr< #(#g_args,)* #p_t >,
                    ) -> #self_ret {
                        let mut visitor = visitor.#into_vis_fn();
                        visitor.#method(self);
                        self
                    }
                }
            }
        })
        .collect();

    quote! {
        pub trait #visit_tr #g_def #(if let Some(b) = base) { : #b::#visit_tr #base_g_use } #uw {
            #(for s in &sides) {
                fn #{&s.method}(&mut self, i: #amp #{&s.ty}) {
                    #{&s.method}(self, i)
                }
            }
        }

        impl< #(#g_params,)* #p_v: #visit_tr #g_use > #visit_tr #g_use for &mut #p_v #uw {
            #(for s in &sides) {
                fn #{&s.method}(&mut self, i: #amp #{&s.ty}) {
                    <#p_v as #visit_tr #g_use>::#{&s.method}(self, i)
                }
            }
        }

        #(for s in &sides) {
            pub fn #{&s.method}< #(#g_params,)* #p_v: #visit_tr #g_use + ?Sized >(
                this: &mut #p_v,
                i: #amp #{&s.ty},
            ) #uw {
                #{&s.body}
            }
        }

        pub trait #into_vis_tr< #(#g_params,)* #p_t > #uw {
            fn #into_vis_fn(self) -> impl #visit_tr #g_use;
        }
        impl< #(#g_params,)* #p_v: #visit_tr #g_use > #into_vis_tr< #(#g_args,)* () > for #p_v #uw {
            fn #into_vis_fn(self) -> impl #visit_tr #g_use { self }
        }

        // --- closures: shallow Hook + single-pass Driver ---------------------------------
        pub trait #hook_tr #g_def #uw {
            #(for s in &sides) {
                fn #{&s.hook}(&mut self, i: #amp #{&s.ty}) { let _ = i; }
            }
        }
        pub trait #into_hook_tr< #(#g_params,)* #p_t > #uw {
            fn #into_hook_fn(self) -> impl #hook_tr #g_use;
        }

        pub struct #driver<#p_h>(pub #p_h);
        impl< #(#g_params,)* #p_h: #hook_tr #g_use > #visit_tr #g_use for #driver<#p_h> #uw {
            #(for s in &sides) {
                fn #{&s.method}(&mut self, i: #amp #{&s.ty}) {
                    self.0.#{&s.hook}(i);
                    #{&s.method}(self, i);
                }
            }
        }
        // The new trait extends the base (transitively), so Driver must satisfy *every* ancestor
        // supertrait (via their defaults). Each empty impl is quantified over only that ancestor's
        // params (+ the wrapped hook) so a wider new-union param is not an unconstrained impl param.
        #(for a in ancestors) {
            impl< #(for p in &a.g_params) { #p, } #p_h >
                #{&a.path}::#visit_tr #{&a.g_use} for #driver<#p_h> {}
        }

        #(for s in &sides) {
            pub struct #{&s.hook_struct}<#p_f>(pub #p_f);
            impl< #(#g_params,)* #p_f: ::core::ops::FnMut( #amp #{&s.ty} ) >
                #hook_tr #g_use for #{&s.hook_struct}<#p_f> #uw
            {
                fn #{&s.hook}(&mut self, i: #amp #{&s.ty}) { (self.0)(i); }
            }
            impl< #(#g_params,)* #p_f: ::core::ops::FnMut( #amp #{&s.ty} ) >
                #into_hook_tr< #(#g_args,)* #{&s.ty} > for #p_f #uw
            {
                fn #into_hook_fn(self) -> impl #hook_tr #g_use { #{&s.hook_struct}(self) }
            }
            impl< #(#g_params,)* #p_f: ::core::ops::FnMut( #amp #{&s.ty} ) >
                #into_vis_tr< #(#g_args,)* #{&s.ty} > for #p_f #uw
            {
                fn #into_vis_fn(self) -> impl #visit_tr #g_use { #driver(#{&s.hook_struct}(self)) }
            }
        }

        // --- multiple closures: Chain combinator + tuple impls ---------------------------
        pub struct #chain<#p_a, #p_b>(pub #p_a, pub #p_b);
        impl< #(#g_params,)* #p_a: #hook_tr #g_use, #p_b: #hook_tr #g_use >
            #hook_tr #g_use for #chain<#p_a, #p_b> #uw
        {
            #(for s in &sides) {
                fn #{&s.hook}(&mut self, i: #amp #{&s.ty}) {
                    self.0.#{&s.hook}(i);
                    self.1.#{&s.hook}(i);
                }
            }
        }
        #(for imp in &tup) { #imp }

        // Inherent entry points (no trait import needed at the call site).
        #(for imp in &inherent) { #imp }
    }
}

/// A `visitor!()` listing one or more `#[recurse]` cycle types (one cycle), optionally **mixed** with
/// acyclic types. Emits, for **both** sides (`Visit`/`visit_*` and `VisitMut`/`visit_*_mut`): a
/// `VisitRec`/`VisitRecMut` dispatch trait, the unified `Visit`/`VisitMut` trait (fixed `visit_X(&X)`
/// for acyclic targets + depth-generic `visit_Y<R…>(&YNode<…>)` for recurse targets), free fns,
/// `VisitRec`/`VisitRecMut` impls (each root's node → its `visit_*`, each terminator → no-op), `XNode`
/// aliases, and inherent `.visit()`/`.visit_mut()`. An acyclic body that follows a recurse field lowers
/// (via `Lower`) to `this.visit_Y(field)`, which type-checks because the public alias
/// `Y<…> = __YRec<…, default>` infers `R = default` — so one impl + one `.visit()` crosses the boundary
/// automatically. Struct/`&mut`-visitors only (depth-generic methods can't back a closure `Driver`).
fn generate_module_mixed(
    targets: &[&DoneType],
    method_set: &HashSet<String>,
    done_by_path: &HashMap<String, &DoneType>,
    path_of: &HashMap<String, &Path>,
) -> TokenStream {
    let rec: Vec<&DoneType> = targets.iter().copied().filter(|d| d.recurse.is_some()).collect();
    let metas: Vec<&RecurseMeta> = rec.iter().map(|d| d.recurse.as_ref().unwrap()).collect();
    // Each recurse target carries ITS cycle's roots/depth/terminators (a `visitor!()` may span several
    // independent cycles); per-target depth info is computed in the `Rec` build below. Here we only
    // need the union of every cycle's terminators (deduped by path) for the no-op dispatch impls.
    let mut seen_t = HashSet::new();
    let all_terms: Vec<Path> = metas
        .iter()
        .flat_map(|m| m.terms.iter().cloned())
        .filter(|p| seen_t.insert(quote!(#p).to_string()))
        .collect();

    // Trait keying = the recurse cycles' ROOTS' params. Roots share params; a non-root cycle type's
    // params *beyond* the roots' become `visit_*` method generics (`extra_decl` below) rather than
    // trait params — only root nodes get `VisitRec` impls, so keeping non-root extras off the trait
    // avoids unconstrained impl params (E0207) and lets a heterogeneous cycle (`Expr<S>` + `Stmt<S, T>`)
    // be expressed. Built as the union of every target's params (deduped, `targets` order) filtered to
    // those a root carries — so for a homogeneous cycle (every cycle type shares the roots' params,
    // acyclic params ⊆ roots') this equals the full union and the emitted code is unchanged.
    let mut seen = HashSet::new();
    let mut union_params: Vec<GenericParam> = Vec::new();
    for d in targets {
        for p in gparams(item_generics(&d.def).unwrap()) {
            if seen.insert(param_name(&p)) {
                union_params.push(p);
            }
        }
    }
    let root_key_names: HashSet<String> = rec
        .iter()
        .filter(|d| {
            let id = item_ident(&d.def).unwrap();
            d.recurse.as_ref().unwrap().roots.iter().any(|rr| rr == id)
        })
        .flat_map(|d| gparams(item_generics(&d.def).unwrap()))
        .map(|p| param_name(&p))
        .collect();
    let mut g_params: Vec<GenericParam> = union_params
        .iter()
        .filter(|p| root_key_names.contains(&param_name(p)))
        .cloned()
        .collect();
    // Lifetimes must precede type/const params in every generated generic list, but the union above
    // can interleave them (one cycle's lifetime after another's type param). Normalize lifetime-first
    // — a stable partition; reordering generic params is semantics-preserving.
    g_params.sort_by_key(|p| !matches!(p, GenericParam::Lifetime(_)));
    let g_args: Vec<TokenStream> = g_params.iter().map(param_use).collect();
    let g_use_angle: TokenStream = if g_args.is_empty() {
        quote!()
    } else {
        quote!( < #(#g_args),* > )
    };

    // Independent cycles spanned by one `visitor!()` must share identical ROOT params: the depth
    // dispatch trait is keyed on the union of all roots' params and each cycle's terminator implements
    // it, so a root (hence its terminator) lacking a param another cycle contributes would be
    // unconstrained (the E0107/E0277 cascade). Reject cleanly. A single cycle (incl. heterogeneous
    // non-root extras) and same-param multi-cycle are unaffected.
    for d in &rec {
        let id = item_ident(&d.def).unwrap();
        if d.recurse.as_ref().unwrap().roots.iter().any(|rr| rr == id) {
            let names: HashSet<String> =
                gparams(item_generics(&d.def).unwrap()).iter().map(param_name).collect();
            if names != root_key_names {
                abort!(
                    Span::call_site(),
                    "visitor!() over `#[recurse]` cycles: independent cycles in one `visitor!()` must \
                     share identical root generic params, but root `{}` declares a different set. \
                     Visit the cycles from separate `visitor!()` invocations.",
                    id
                );
            }
        }
    }

    // Mixed-visitor guard: an acyclic target's params must be ⊆ the roots' params. The depth-generic
    // `VisitRec` impls are keyed on the roots' params only, so an acyclic-only param would be
    // unconstrained there (E0207). Pure-recurse heterogeneity is supported; only an acyclic type that
    // introduces an extra param is walled (split it out of this visitor, or share the roots' params).
    for d in targets.iter().filter(|d| d.recurse.is_none()) {
        for p in gparams(item_generics(&d.def).unwrap()) {
            if !root_key_names.contains(&param_name(&p)) {
                let id = item_ident(&d.def).unwrap();
                abort!(
                    Span::call_site(),
                    "visitor!() over a `#[recurse]` cycle: acyclic type `{}` has generic param `{}` \
                     not carried by any cycle root; the depth-generic `VisitRec` impls are keyed on \
                     the roots' params, so this param would be unconstrained. Give `{}` only params \
                     the roots also have, or visit it from a separate `visitor!()`.",
                    id,
                    param_name(&p),
                    id
                );
            }
        }
    }
    let mut seen_pred = HashSet::new();
    let union_where: Vec<WherePredicate> = targets
        .iter()
        .flat_map(|d| item_where_preds(&d.def))
        .filter(|p| seen_pred.insert(quote!(#p).to_string()))
        .collect();
    let uw = where_clause(&union_where);

    // Fresh-name the generated helper idents — the visitor type param (`__V`/`__W`) and the per-root
    // depth params (`__R{i}`) — against every target's param names, so a cycle/target type that
    // declares a param literally named `__V` / `__R0` / `__W` doesn't collide (as the acyclic path
    // already does via `fresh_ident`).
    let reserved: HashSet<String> = targets
        .iter()
        .flat_map(|d| gparams(item_generics(&d.def).unwrap()))
        .map(|p| param_name(&p))
        .collect();
    let p_v = fresh_ident("__V", &reserved);
    let p_w = {
        let mut r = reserved.clone();
        r.insert(p_v.to_string());
        fresh_ident("__W", &r)
    };
    let max_roots = rec
        .iter()
        .map(|d| d.recurse.as_ref().unwrap().roots.len())
        .max()
        .unwrap_or(1)
        .max(1);
    let rpfx = {
        let mut r = reserved.clone();
        r.insert(p_v.to_string());
        r.insert(p_w.to_string());
        fresh_prefix("__R", &r, max_roots)
    };

    // Per recurse target: its own cycle's depth params `dps` (`__R0…`, one per root of THAT cycle),
    // both sides' method names + bodies; `is_root` selects the `VisitRec` impls.
    struct Rec {
        node: Path,
        own_args: Vec<TokenStream>,
        /// This cycle type's params beyond the roots' (decl form) — lowered to `visit_*` method
        /// generics, since the trait is keyed on the roots' params only. Empty for a root.
        /// `extra_lts` / `extra_rest` are the same params split lifetime / non-lifetime, so the free
        /// fn can emit all lifetimes before the root's type params (lifetimes-first rule).
        extra_decl: Vec<TokenStream>,
        extra_lts: Vec<TokenStream>,
        extra_rest: Vec<TokenStream>,
        dps: Vec<Ident>,
        is_root: bool,
        vm: Ident,
        vm_mut: Ident,
        body: TokenStream,
        body_mut: TokenStream,
    }
    let recs: Vec<Rec> = rec
        .iter()
        .map(|d| {
            let id = item_ident(&d.def).unwrap();
            let r = d.recurse.as_ref().unwrap();
            let dps: Vec<Ident> = (0..r.roots.len())
                .map(|i| Ident::new(&format!("{rpfx}{i}"), Span::call_site()))
                .collect();
            let root_dp: HashMap<String, Ident> = r
                .roots
                .iter()
                .map(|x| x.to_string())
                .zip(dps.iter().cloned())
                .collect();
            let cycle_set: HashSet<String> = r.cycle.iter().map(|i| i.to_string()).collect();
            let extra: Vec<GenericParam> = gparams(item_generics(&d.def).unwrap())
                .into_iter()
                .filter(|p| !root_key_names.contains(&param_name(p)))
                .collect();
            let is_lt = |p: &&GenericParam| matches!(p, GenericParam::Lifetime(_));
            Rec {
                node: r.node.clone(),
                own_args: gargs(item_generics(&d.def).unwrap()),
                extra_decl: extra.iter().map(|p| param_tokens(p).0).collect(),
                extra_lts: extra.iter().filter(is_lt).map(|p| param_tokens(p).0).collect(),
                extra_rest: extra.iter().filter(|p| !is_lt(p)).map(|p| param_tokens(p).0).collect(),
                is_root: r.roots.iter().any(|rr| rr == id),
                vm: method_ident_m(id, false),
                vm_mut: method_ident_m(id, true),
                body: recurse_lower_body(&d.def, &r.node, method_set, &root_dp, &cycle_set, false),
                body_mut: recurse_lower_body(&d.def, &r.node, method_set, &root_dp, &cycle_set, true),
                dps,
            }
        })
        .collect();

    let lower = Lower { method_set, done_by_path, mutable: false };
    let lower_mut = Lower { method_set, done_by_path, mutable: true };
    struct Acy {
        path: TokenStream,
        own_use: TokenStream,
        vm: Ident,
        vm_mut: Ident,
        body: TokenStream,
        body_mut: TokenStream,
    }
    let acys: Vec<Acy> = targets
        .iter()
        .filter(|d| d.recurse.is_none())
        .map(|d| {
            let id = item_ident(&d.def).unwrap();
            let scrut: &Path = path_of.get(&id.to_string()).copied().unwrap_or(&d.path);
            let mut s0 = Vec::new();
            let mut s1 = Vec::new();
            Acy {
                path: quote!(#scrut),
                own_use: angle(&gargs(item_generics(&d.def).unwrap())),
                vm: method_ident_m(id, false),
                vm_mut: method_ident_m(id, true),
                body: lower.destructure(&d.def, &d.subast, scrut, &quote!(i), 0, &mut s0),
                body_mut: lower_mut.destructure(&d.def, &d.subast, scrut, &quote!(i), 0, &mut s1),
            }
        })
        .collect();

    // Inherent `.visit()`/`.visit_mut()` per type (acyclic + recurse alike): a recurse alias `Y<…>`
    // infers the depth default, so `v.visit_Y(self)` works the same as for an acyclic type. Skipped for
    // a **foreign** target (defined in another crate) — an inherent impl there is E0116, so a
    // cross-crate `visitor!(upstream::Expr, …)` uses the `Visit::visit_*` trait method instead.
    struct Inh {
        scrut: TokenStream,
        own_def: TokenStream,
        own_use: TokenStream,
        own_w: TokenStream,
        extra: Vec<GenericParam>,
        vm: Ident,
        vm_mut: Ident,
    }
    let inhs: Vec<Inh> = targets
        .iter()
        .filter_map(|d| {
            let id = item_ident(&d.def).unwrap();
            let scrut: &Path = path_of.get(&id.to_string()).copied().unwrap_or(&d.path);
            if !path_is_crate_local(scrut) {
                return None;
            }
            let own_params = gparams(item_generics(&d.def).unwrap());
            let own_names: HashSet<String> = own_params.iter().map(param_name).collect();
            let extra: Vec<GenericParam> = g_params
                .iter()
                .filter(|p| !own_names.contains(&param_name(p)))
                .cloned()
                .collect();
            Some(Inh {
                scrut: quote!(#scrut),
                own_def: angle(&own_params),
                own_use: angle(&gargs(item_generics(&d.def).unwrap())),
                own_w: where_clause(&item_where_preds(&d.def)),
                extra,
                vm: method_ident_m(id, false),
                vm_mut: method_ident_m(id, true),
            })
        })
        .collect();

    let node_aliases: Vec<TokenStream> = rec
        .iter()
        .map(|d| {
            let id = item_ident(&d.def).unwrap();
            let node = &d.recurse.as_ref().unwrap().node;
            let alias = Ident::new(&format!("{id}Node"), Span::call_site());
            quote!( #[doc = "Depth-generic node type for the visitor."] pub use #node as #alias; )
        })
        .collect();

    // Emit one mutability side (shared `false` / mutable `true`): the dispatch trait, the unified
    // visitor trait, free fns, the dispatch impls (root nodes + terminators), and inherent entries.
    let emit_side = |mutable: bool| -> TokenStream {
        let visit_tr = Ident::new(if mutable { "VisitMut" } else { "Visit" }, Span::call_site());
        let rec_tr = Ident::new(
            if mutable { "VisitRecMut" } else { "VisitRec" },
            Span::call_site(),
        );
        let rec_fn = Ident::new(
            if mutable { "visit_rec_mut" } else { "visit_rec" },
            Span::call_site(),
        );
        let amp = if mutable { quote!(&mut) } else { quote!(&) };
        let inh_fn = Ident::new(if mutable { "visit_mut" } else { "visit" }, Span::call_site());
        let inh_recv = if mutable { quote!(&mut self) } else { quote!(&self) };
        let inh_ret = if mutable { quote!(&mut Self) } else { quote!(&Self) };
        quote! {
            /// Dispatch trait turning the cycle's depth recursion into trait calls.
            pub trait #rec_tr < #(#g_params,)* #p_v > {
                fn #rec_fn(#amp self, v: &mut #p_v);
            }

            /// Unified depth-aware visitor: fixed methods for acyclic types, depth-generic for
            /// `#[recurse]` cycle types. One impl walks both; a field crossing into the cycle is
            /// dispatched automatically.
            pub trait #visit_tr < #(#g_params),* > #uw {
                #(for a in &acys) {
                    fn #{ if mutable { &a.vm_mut } else { &a.vm } }(&mut self, i: #amp #{&a.path} #{&a.own_use})
                    where Self: ::core::marker::Sized {
                        #{ if mutable { &a.vm_mut } else { &a.vm } }(self, i)
                    }
                }
                #(for c in &recs) {
                    fn #{ if mutable { &c.vm_mut } else { &c.vm } }< #(for e in &c.extra_decl) { #e, } #(for d in &c.dps) { #d: #rec_tr < #(#g_args,)* Self >, } >(
                        &mut self,
                        i: #amp #{&c.node} < #(for x in &c.own_args) { #x, } #(for d in &c.dps) { #d, } >,
                    ) where Self: ::core::marker::Sized {
                        #{ if mutable { &c.vm_mut } else { &c.vm } }(self, i)
                    }
                }
            }

            #(for a in &acys) {
                // No `?Sized`: an acyclic body may cross into the cycle via `this.visit_<rec>(…)`,
                // whose method requires `Self: Sized`.
                pub fn #{ if mutable { &a.vm_mut } else { &a.vm } }< #(#g_params,)* #p_v: #visit_tr #g_use_angle >(
                    this: &mut #p_v,
                    i: #amp #{&a.path} #{&a.own_use},
                ) #uw {
                    #{ if mutable { &a.body_mut } else { &a.body } }
                }
            }
            #(for c in &recs) {
                pub fn #{ if mutable { &c.vm_mut } else { &c.vm } }<
                    // lifetimes first (extra lifetimes, then the root's via `#g_params`), then types/consts
                    #(for l in &c.extra_lts) { #l, }
                    #(#g_params,)*
                    #(for r in &c.extra_rest) { #r, }
                    #p_v: #visit_tr #g_use_angle,
                    #(for d in &c.dps) { #d: #rec_tr < #(#g_args,)* #p_v >, }
                >(
                    v: &mut #p_v,
                    i: #amp #{&c.node} < #(for x in &c.own_args) { #x, } #(for d in &c.dps) { #d, } >,
                ) {
                    #{ if mutable { &c.body_mut } else { &c.body } }
                }
            }

            // One dispatch impl per ROOT node (drives its visit) + one per terminator (no-op).
            #(for c in &recs) {
                #(if c.is_root) {
                    impl< #(#g_params,)* #(for d in &c.dps) { #d: #rec_tr < #(#g_args,)* #p_v >, } #p_v: #visit_tr #g_use_angle >
                        #rec_tr < #(#g_args,)* #p_v > for #{&c.node} < #(for x in &c.own_args) { #x, } #(for d in &c.dps) { #d, } >
                    {
                        fn #rec_fn(#amp self, v: &mut #p_v) {
                            <#p_v as #visit_tr #g_use_angle>::#{ if mutable { &c.vm_mut } else { &c.vm } }(v, self);
                        }
                    }
                }
            }
            #(for t in &all_terms) {
                impl< #(#g_params,)* #p_v: #visit_tr #g_use_angle > #rec_tr < #(#g_args,)* #p_v >
                    for #t #g_use_angle
                {
                    fn #rec_fn(#amp self, _v: &mut #p_v) {}
                }
            }

            #(for h in &inhs) {
                impl #{&h.own_def} #{&h.scrut} #{&h.own_use} #{&h.own_w} {
                    pub fn #inh_fn < #(for e in &h.extra) { #e, } #p_w: #visit_tr #g_use_angle >(
                        #inh_recv,
                        visitor: &mut #p_w,
                    ) -> #inh_ret {
                        visitor.#{ if mutable { &h.vm_mut } else { &h.vm } }(self);
                        self
                    }
                }
            }
        }
    };

    let shared = emit_side(false);
    let mutable = emit_side(true);
    quote! {
        #(#node_aliases)*
        #shared
        #mutable
    }
}

fn generate_module(st: &BuildInput) -> TokenStream {
    // Every generated name (`visit_*`, `*Hook`, inherent methods) derives from a visited type's
    // last-segment ident, so two visited types sharing a last segment would collide. Catch it here
    // with a clear message instead of a downstream cascade of duplicate-definition errors.
    let mut seg_seen: HashMap<String, String> = HashMap::new();
    for p in &st.visited {
        let seg = last_ident(p).to_string();
        let np = norm_path(p);
        if let Some(prev) = seg_seen.insert(seg.clone(), np.clone()) {
            if prev != np {
                abort!(
                    Span::call_site(),
                    "two visited types share the last segment `{}` (`{}` vs `{}`); their generated \
                     `visit_*`/`*Hook` names would collide — give them distinct final idents",
                    seg,
                    prev,
                    np
                );
            }
        }
    }

    // Map each visited type's last-segment ident -> the full path the user wrote, so the generated
    // module names the visited types by that path (portable: no import needed for absolute paths).
    let path_of: HashMap<String, &Path> = st
        .visited
        .iter()
        .map(|p| (last_ident(p).to_string(), p))
        .collect();
    let visited: HashSet<String> = path_of.keys().cloned().collect();
    // Heads that recurse via a `visit_*` method (visited here + inherited from a base); every other
    // followed head is an unlisted intermediate that gets drilled through inline.
    let method_set: HashSet<String> = visited
        .iter()
        .cloned()
        .chain(st.inherited.iter().map(|i| i.to_string()))
        .collect();
    // Fetched types keyed by full path, for resolving an intermediate's def while drilling.
    let done_by_path: HashMap<String, &DoneType> =
        st.done.iter().map(|d| (norm_path(&d.path), d)).collect();

    // Types that get visitor methods (named in `visitor!(..)`); inherited/intermediate types don't.
    let targets: Vec<&DoneType> = st
        .done
        .iter()
        .filter(|d| item_ident(&d.def).is_some_and(|id| visited.contains(&id.to_string())))
        .collect();
    if targets.is_empty() {
        abort!(Span::call_site(), "no AST definitions resolved for the visitor");
    }

    // `#[recurse]` cycle types carry `@recurse`; they need a depth-generic visitor (Phase 1a). For
    // now a single visitor!() is either all-recurse (one cycle) or all-acyclic; mixing is a later
    // phase. An all-recurse visitor goes through `generate_recurse_module`; the acyclic path below is
    // unchanged.
    let recurse_targets = targets.iter().filter(|d| d.recurse.is_some()).count();
    if recurse_targets > 0 {
        if st.base.is_some() {
            abort!(
                Span::call_site(),
                "visitor!(base => …) inheritance over `#[recurse]` cycle types is not yet supported"
            );
        }
        // Recurse cycle types present (all-recurse or mixed with acyclic types): one unified
        // depth-aware `Visit` trait, with any outer→inner boundary auto-crossed. (`generate_module_mixed`
        // handles zero acyclic targets fine, so the all-recurse case routes here too.)
        return generate_module_mixed(&targets, &method_set, &done_by_path, &path_of);
    }

    // The visitor trait is parameterized by the *union* of every visited type's generic params
    // (by name, first declaration wins); each type is then referenced with its own subset. This
    // lets one visitor span e.g. `Expr<S, Tokens>` and `BinOp<S>`.
    let mut seen = HashSet::new();
    let mut g_params: Vec<GenericParam> = Vec::new();
    for d in &targets {
        for p in gparams(item_generics(&d.def).unwrap()) {
            if seen.insert(param_name(&p)) {
                g_params.push(p);
            }
        }
    }
    // When inheriting, the union must also contain the base's generic params (so the new trait can
    // declare them and reference `base::Visit<base params>`). `base_g_use` is the base's args named
    // by the union's idents — used for every `base::Visit<..>` reference (its own arity, which may
    // differ from the new union's).
    for bp in &st.base_generics {
        if seen.insert(param_name(bp)) {
            g_params.push(bp.clone());
        }
    }
    let by_name: HashMap<String, TokenStream> =
        g_params.iter().map(|p| (param_name(p), param_use(p))).collect();
    let by_name_param: HashMap<String, GenericParam> =
        g_params.iter().map(|p| (param_name(p), p.clone())).collect();
    let base_args: Vec<TokenStream> = st
        .base_generics
        .iter()
        .map(|bp| by_name[&param_name(bp)].clone())
        .collect();
    let base_g_use = angle(&base_args);

    // The full transitive ancestor chain (direct base first), so the new visitor's `Driver` can
    // satisfy *every* supertrait obligation — `mid::Visit: base::Visit` means a `mid => new` visitor
    // must impl both `mid::Visit` and `base::Visit` for its `Driver`. Each ancestor's params are a
    // subset of the union (the base's `@bg` transitively carries its own ancestors' params), looked
    // up by name; each impl is quantified over exactly those params (+ the hook) to avoid E0207.
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
        // Requalify transitive ancestors a `crate::`-relative *upstream* intermediate recorded
        // against the direct base's host crate (no-op for same-crate / already-concrete chains).
        // This also re-exports them concrete (the chain feeds `anc_export`), so a further extender
        // inherits resolvable ancestor paths too.
        let host = base_host_crate(b);
        for a in &st.base_ancestors {
            let path = match &host {
                Some(h) => requalify_ancestor(&a.path, h),
                None => a.path.clone(),
            };
            chain.push(AncIn {
                path,
                names: a.names.clone(),
            });
        }
    }
    let ancestors: Vec<Ancestor> = chain
        .iter()
        .map(|a| {
            let g_params: Vec<GenericParam> = a
                .names
                .iter()
                .filter_map(|n| by_name_param.get(&n.to_string()).cloned())
                .collect();
            let args: Vec<TokenStream> = a
                .names
                .iter()
                .filter_map(|n| by_name.get(&n.to_string()).cloned())
                .collect();
            let g_use = angle(&args);
            let path = &a.path;
            Ancestor {
                path: quote!(#path),
                g_params,
                g_use,
            }
        })
        .collect();

    let g_args: Vec<TokenStream> = g_params.iter().map(param_use).collect();
    let g_def = angle(&g_params);
    let g_use = angle(&g_args);

    let lower = Lower {
        method_set: &method_set,
        done_by_path: &done_by_path,
        mutable: false,
    };
    let lower_mut = Lower {
        method_set: &method_set,
        done_by_path: &done_by_path,
        mutable: true,
    };

    let vtypes: Vec<VType> = targets
        .iter()
        .map(|d| {
            let def = &d.def;
            let ident = item_ident(def).unwrap().clone();
            let own_params = gparams(item_generics(def).unwrap());
            let own_use = angle(&gargs(item_generics(def).unwrap()));
            let own_where = item_where_preds(def);
            // The path the visited type is named by (its `visitor!(..)` path), also the scrutinee
            // path for its own body; falls back to the fetched path if somehow unmapped.
            let scrut_path: &Path = path_of.get(&ident.to_string()).copied().unwrap_or(&d.path);
            let path_tokens = quote!(#scrut_path);
            let mut stack = Vec::new();
            let body = lower.destructure(def, &d.subast, scrut_path, &quote!(i), 0, &mut stack);
            let mut stack = Vec::new();
            let body_mut =
                lower_mut.destructure(def, &d.subast, scrut_path, &quote!(i), 0, &mut stack);
            VType {
                ident,
                path: path_tokens,
                own_params,
                own_use,
                own_where,
                body,
                body_mut,
            }
        })
        .collect();

    // The union of every visited type's `where`-predicates (deduped by rendered text — identical
    // predicates from two types are harmless but noisy), repeated on each generated item quantified
    // over the full param union so a `enum Expr<S> where S: Bound { .. }` stays well-formed there.
    let mut seen_pred: HashSet<String> = HashSet::new();
    let union_where: Vec<WherePredicate> = vtypes
        .iter()
        .flat_map(|vt| vt.own_where.iter().cloned())
        .filter(|p| seen_pred.insert(quote!(#p).to_string()))
        .collect();

    let shared = gen_side(
        false, &vtypes, &g_params, &g_args, &g_def, &g_use, &base_g_use, &ancestors, &st.base,
        &union_where,
    );
    let mutable = gen_side(
        true, &vtypes, &g_params, &g_args, &g_def, &g_use, &base_g_use, &ancestors, &st.base,
        &union_where,
    );

    // Every visitor module exports its full visited-type set (idents), its generic-param union
    // (`@bg`), and its full ancestor chain (`@an`) so another visitor can inherit it (transitively).
    let all_visible: Vec<Ident> = st
        .visited
        .iter()
        .map(|p| last_ident(p).clone())
        .chain(st.inherited.iter().cloned())
        .collect();
    let anc_export = emit_ancestors(&chain);
    let vmacro = Ident::new(&format!("__syan_visited_{}", st.nonce), Span::call_site());

    // Items are emitted directly into the enclosing module (where `visitor!(...)` was invoked).
    quote! {
        #[macro_export]
        #[doc(hidden)]
        macro_rules! #vmacro {
            (@visited $cb:path { $($pre:tt)* }) => {
                $cb ! {
                    $($pre)* @inh { #(#all_visible)* } @bg { #(#g_params),* } @an { #anc_export }
                }
            };
        }
        #[doc(hidden)]
        pub use #vmacro as __syan_visited;

        // Bring every ancestor's traits in scope so the generated `Driver` impls / method calls
        // resolve (transitive supertraits included).
        #(for a in &ancestors) {
            #[allow(unused_imports)]
            use #{&a.path}::{Visit as _, VisitMut as _};
        }

        #shared
        #mutable
    }
}


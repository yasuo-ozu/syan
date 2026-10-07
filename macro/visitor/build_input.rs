use super::*;

/// A fetched AST type: the path it was fetched by, its (cleaned) definition, and its `#[subast]`.
pub(crate) struct DoneType {
    pub(crate) path: Path,
    pub(crate) def: Item,
    pub(crate) subast: Vec<SubEntry>,
}

// `__visitor_build`: receives accumulated state + the just-resolved definition, fetches the next type
// or generates the module.

/// One visitor being extended: its path and the generic params its trait carries. Kept per base
/// rather than merged, because `base::Visit<..>` has to be spelled with *that* base's arity.
pub(crate) struct BaseIn {
    pub(crate) path: Path,
    pub(crate) generics: Vec<GenericParam>,
}

pub(crate) struct BuildInput {
    /// Every visitor this one extends, in the order written. Each becomes a supertrait.
    pub(crate) bases: Vec<BaseIn>,
    pub(crate) build: Path,
    pub(crate) nonce: TokenStream,
    pub(crate) visited: Vec<Path>,
    /// Types inherited from the base, as `path as matchkey` rather than bare idents, so a field
    /// holding an inherited type is followed without a `#[subast]` entry repeating what the base
    /// already knows. Requalified against the base path on arrival, so the paths stay resolvable
    /// through any number of generations.
    pub(crate) inherited: Vec<SubEntry>,
    /// Bases whose metadata has not been asked for yet. The one being asked *now* travels as
    /// `@bnow` and is consumed while parsing — a reply carries no name of its own, so it has to be
    /// told which base it answers for — and so is not kept here.
    pub(crate) base_fetch: Vec<Path>,
    /// The direct base's own transitive ancestors (for multi-level `base => mid => new`), so the new
    /// visitor can emit the empty `Driver` impl for every transitive supertrait, not just the direct
    /// base.
    pub(crate) base_ancestors: Vec<AncIn>,
    /// Path of the type whose `@ast`/`@subast` trail in this bounce (so the fetched def is recorded
    /// under the path it was fetched by). Empty before any type is fetched.
    pub(crate) fetching: Option<Path>,
    pub(crate) done: Vec<DoneType>,
    pub(crate) rest: Vec<Path>,
    pub(crate) just_def: Option<Item>,
    pub(crate) just_subast: Vec<SubEntry>,
}

impl BuildInput {
    /// Visited types' last-idents ∪ inherited idents — the set of heads that dispatch via a
    /// `visit_*` method call rather than being drilled/leaf.
    pub(crate) fn method_set(&self) -> HashSet<String> {
        self.visited
            .iter()
            .map(|p| last_ident(p).to_string())
            .chain(self.inherited.iter().map(|e| e.key.to_string()))
            .collect()
    }
}

/// Parse one `@<name> { .. }` section, returning the name and the braced content as tokens.
pub(crate) fn parse_section(input: ParseStream) -> Result<(Ident, TokenStream)> {
    input.parse::<Token![@]>()?;
    let name: Ident = input.parse()?;
    let content;
    braced!(content in input);
    Ok((name, content.parse()?))
}

/// One transitive-base obligation for multi-level inheritance: an ancestor visitor's path and the
/// names of its generic params (re-mapped into the extending visitor's union when emitted).
pub(crate) struct AncIn {
    pub(crate) path: Path,
    pub(crate) names: Vec<Ident>,
}

/// `@bases { <path> { <params> } … }` — the bases whose metadata has already arrived.
pub(crate) fn emit_bases(bases: &[BaseIn]) -> TokenStream {
    let parts: Vec<TokenStream> = bases
        .iter()
        .map(|b| {
            let (p, g) = (&b.path, &b.generics);
            quote!( #p { #(#g),* } )
        })
        .collect();
    quote!( #(#parts)* )
}

fn parse_bases(ts: TokenStream) -> Result<Vec<BaseIn>> {
    let parser = |input: ParseStream| {
        let mut out = Vec::new();
        while !input.is_empty() {
            let path: Path = input.parse()?;
            let inner;
            braced!(inner in input);
            let generics = Punctuated::<GenericParam, Token![,]>::parse_terminated
                .parse2(inner.parse()?)?
                .into_iter()
                .collect();
            out.push(BaseIn { path, generics });
        }
        Ok(out)
    };
    parser.parse2(ts)
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
                        return Err(Error::new(
                            name.span(),
                            format!("unknown @a section @{other}"),
                        ))
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

pub(crate) fn emit_ancestors(anc: &[AncIn]) -> TokenStream {
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

/// The `__syan_visited` export — a `#[macro_export]` muncher that, when a downstream `visitor!(self =>
/// New)` invokes it, appends everything this visitor can reach as `path as matchkey` (`@inh`), its
/// param union (`@bg`), and its ancestor chain (`@an`).
///
/// Paths, not bare idents: the extender needs to *name* an inherited type to emit its `Walk` impl and
/// to follow a field holding it, and it has no other way to learn where that type lives. The same
/// channel already carries the ancestor chain, and for the same reason.
pub(crate) fn emit_visited_macro(
    st: &BuildInput,
    g_params: &[GenericParam],
    anc_export: TokenStream,
) -> TokenStream {
    let all_visible: Vec<TokenStream> = st
        .visited
        .iter()
        .map(|p| {
            let k = last_ident(p);
            quote!( #p as #k )
        })
        .chain(st.inherited.iter().map(|e| {
            let (p, k) = (&e.path, &e.key);
            quote!( #p as #k )
        }))
        .collect();
    let vmacro = Ident::new(&format!("__syan_visited_{}", st.nonce), Span::call_site());
    quote! {
        // The embedded visited-type / ancestor paths may be `crate::`-rooted by design (they resolve in
        // the base's defining crate); suppress clippy's `crate_in_macro_def` for the generated macro.
        #[allow(clippy::crate_in_macro_def)]
        #[macro_export]
        #[doc(hidden)]
        macro_rules! #vmacro {
            (@visited $cb:path { $($pre:tt)* }) => {
                $cb ! {
                    $($pre)* @inh { #(#all_visible),* } @bg { #(#g_params),* } @an { #anc_export }
                }
            };
        }
        #[doc(hidden)]
        pub use #vmacro as __syan_visited;
    }
}

/// Whether a path is rooted in the *current* crate (`crate::` / `self::` / `super::`). A foreign path
/// (an external crate name, or a leading `::`) is not — an inherent `impl` for such a type would be
/// E0116 (inherent impls must live in the type's defining crate), so the visitor skips the inherent
/// `.visit()`/`.visit_mut()` for a foreign target and the trait method (`Visit::visit_*`) is used.
pub(crate) fn path_is_crate_local(p: &Path) -> bool {
    if p.leading_colon.is_some() {
        return false;
    }
    matches!(
        p.segments.first().map(|s| s.ident.to_string()).as_deref(),
        Some("crate") | Some("self") | Some("super")
    )
}

/// The host crate of a direct-base path: `Some(ident)` when it is rooted at an *external* crate —
/// `syan_rust::inherit::mid`, or `::syan_rust::inherit::mid`, which names the same crate and is
/// equally external — and `None` for a same-crate root (`crate`/`super`/`self`).
/// Gates the ancestor requalification (`requalify_ancestor`): a transitive
/// ancestor an *upstream* intermediate recorded relative to its own crate must be rewritten into a
/// path the *downstream* extender can resolve. (A `$crate` cannot do this: emitted by a proc-macro
/// into a generated `macro_rules` body it resolves only for fetch/macro-invocation paths, **not** for
/// the trait path re-emitted into the new `Driver`'s supertrait impl — so a cross-crate `base => mid
/// => new` with an *upstream* `mid` needs this concrete requalification instead.)
pub(crate) fn base_host_crate(base: &Path) -> Option<Ident> {
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

/// Resolve a transitive ancestor path that an *upstream* intermediate recorded **relative to its own
/// module** into one the *downstream* extender can resolve, using the direct `base` path. Downstream,
/// `base` is the path the extender named the intermediate by (e.g. `syan_rust::inherit::mid_ss`), and
/// the intermediate's `visitor!()` was invoked *inside that module* — so the ancestor's leading
/// relative segment resolves against `base`:
///   - `crate::REST` → `<host>::REST`   (host = `base`'s first segment, the upstream crate)
///   - `super::REST` → pop one trailing segment of `base` per leading `super`, then append `REST`
///   - `self::REST`  → `base::REST`
///
/// A path already concrete (external-crate-rooted or leading `::`) — or a bare ident — is left alone.
/// Only called for a cross-crate base (`base_host_crate(base).is_some()`); same-crate chains, which
/// resolve in place, keep their recorded paths. This is why a `super`/`self`-relative `visitor!` entry
/// path (not just the canonical `crate::`-rooted one) now works cross-crate.
pub(crate) fn requalify_ancestor(anc: &Path, base: &Path) -> Path {
    if anc.leading_colon.is_some() {
        return anc.clone();
    }
    let Some(first) = anc.segments.first() else {
        return anc.clone();
    };
    if !matches!(first.arguments, PathArguments::None) {
        return anc.clone();
    }
    // `base`'s segments ARE the intermediate's module path (the `visitor!()` ran inside it).
    let base_mod: Vec<PathSegment> = base.segments.iter().cloned().collect();
    let join = |prefix: &[PathSegment], tail: &[PathSegment]| -> Path {
        let mut segments = Punctuated::new();
        for s in prefix.iter().chain(tail.iter()) {
            segments.push(s.clone());
        }
        Path {
            leading_colon: None,
            segments,
        }
    };
    match first.ident.to_string().as_str() {
        "crate" => {
            let mut out = anc.clone();
            out.segments[0].ident = base_mod[0].ident.clone();
            out
        }
        "self" => {
            let tail: Vec<PathSegment> = anc.segments.iter().skip(1).cloned().collect();
            join(&base_mod, &tail)
        }
        "super" => {
            let supers = anc
                .segments
                .iter()
                .take_while(|s| s.ident == "super" && matches!(s.arguments, PathArguments::None))
                .count();
            let keep = base_mod.len().saturating_sub(supers);
            let tail: Vec<PathSegment> = anc.segments.iter().skip(supers).cloned().collect();
            join(&base_mod[..keep], &tail)
        }
        _ => anc.clone(),
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
    while !input.is_empty() {
        let (name, content) = parse_section(input)?;
        match name.to_string().as_str() {
            "path" => path = Some(syn::parse2(content)?),
            "def" => def = Some(syn::parse2(content)?),
            "subast" => subast = parse_subentries(content)?,
            other => {
                return Err(Error::new(
                    name.span(),
                    format!("unknown @t section @{other}"),
                ))
            }
        }
    }
    Ok(DoneType {
        path: path.ok_or_else(|| Error::new(Span::call_site(), "missing @path in @t"))?,
        def: def.ok_or_else(|| Error::new(Span::call_site(), "missing @def in @t"))?,
        subast,
    })
}

impl Parse for BuildInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut fresh_inh: Vec<SubEntry> = Vec::new();
        let mut fresh_bg: Vec<GenericParam> = Vec::new();
        let mut fresh_an: Vec<AncIn> = Vec::new();
        let mut base_fetch: Vec<Path> = Vec::new();
        let mut base_now: Option<Path> = None;
        let mut bases: Vec<BaseIn> = Vec::new();
        let mut build = None;
        let mut nonce = TokenStream::new();
        let mut visited = Vec::new();
        let mut inherited = Vec::new();
        let mut base_ancestors = Vec::new();
        let mut fetching = None;
        let mut done = Vec::new();
        let mut rest = Vec::new();
        let mut just_def = None;
        let mut just_subast = Vec::new();

        while !input.is_empty() {
            let (name, content) = parse_section(input)?;
            match name.to_string().as_str() {
                "bfetch" => {
                    base_fetch = Punctuated::<Path, Token![,]>::parse_terminated
                        .parse2(content)?
                        .into_iter()
                        .collect();
                }
                "bnow" => {
                    base_now = Punctuated::<Path, Token![,]>::parse_terminated
                        .parse2(content)?
                        .into_iter()
                        .next();
                }
                // Each base's reply, attributed to `@bnow` and requalified on arrival so nothing
                // downstream has to remember which base an entry came from.
                "bases" => bases = parse_bases(content)?,
                "build" => build = Some(syn::parse2(content)?),
                "nonce" => nonce = content,
                "visited" => {
                    visited = Punctuated::<Path, Token![,]>::parse_terminated
                        .parse2(content)?
                        .into_iter()
                        .collect();
                }
                // `@inherited`/`@anc` are the carried, already-requalified state; `@inh`/`@bg`/`@an`
                // are what the base just asked answers with, and are folded in after the loop once
                // `@bnow` says which base they belong to.
                "inherited" => inherited.extend(parse_subentries(content)?),
                "inh" => fresh_inh = parse_subentries(content)?,
                "bg" => {
                    if !content.is_empty() {
                        fresh_bg = Punctuated::<GenericParam, Token![,]>::parse_terminated
                            .parse2(content)?
                            .into_iter()
                            .collect();
                    }
                }
                "anc" => base_ancestors = parse_ancestors(content)?,
                "an" => fresh_an = parse_ancestors(content)?,
                "fetching" => {
                    if !content.is_empty() {
                        fetching = Some(syn::parse2(content)?);
                    }
                }
                "done" => done = parse_done(content)?,
                "rest" => {
                    let paths = Punctuated::<Path, Token![,]>::parse_terminated.parse2(content)?;
                    rest = paths.into_iter().collect();
                }
                "ast" => just_def = Some(syn::parse2(content)?),
                "subast" => just_subast = parse_subentries(content)?,
                other => return Err(Error::new(name.span(), format!("unknown section @{other}"))),
            }
        }

        // Resolve the reply against the base that gave it, so the merged lists are absolute and
        // nothing downstream has to remember which base an entry came from. A path that two bases
        // both reach (a shared ancestor) is kept once.
        if let Some(bn) = &base_now {
            let fix = |p: &Path| {
                if needs_requalify(p, bn) {
                    requalify_ancestor(p, bn)
                } else {
                    p.clone()
                }
            };
            let seen: HashSet<String> = inherited.iter().map(|e| norm_path(&e.path)).collect();
            for e in fresh_inh {
                let path = fix(&e.path);
                if !seen.contains(&norm_path(&path)) {
                    inherited.push(SubEntry { path, key: e.key });
                }
            }
            let seen: HashSet<String> = base_ancestors.iter().map(|a| norm_path(&a.path)).collect();
            for a in fresh_an {
                let path = fix(&a.path);
                if !seen.contains(&norm_path(&path)) {
                    base_ancestors.push(AncIn {
                        path,
                        names: a.names,
                    });
                }
            }
            if !bases.iter().any(|b| norm_path(&b.path) == norm_path(bn)) {
                bases.push(BaseIn {
                    path: bn.clone(),
                    generics: fresh_bg,
                });
            }
        }

        Ok(BuildInput {
            bases,
            base_fetch,
            build: build.ok_or_else(|| Error::new(Span::call_site(), "missing @build"))?,
            nonce,
            visited,
            inherited,
            base_ancestors,
            fetching,
            done,
            rest,
            just_def,
            just_subast,
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

/// The accumulated state, ready for the next bounce: `fetching` names the type whose definition
/// will trail it, `bnow` the base whose reply will.
fn carry(st: &BuildInput, fetching: &TokenStream, bnow: &TokenStream) -> TokenStream {
    state_tokens(
        &emit_bases(&st.bases),
        &{
            let bf = &st.base_fetch;
            quote!( #(#bf),* )
        },
        bnow,
        &st.build,
        &st.nonce,
        &st.visited,
        &st.inherited,
        &emit_ancestors(&st.base_ancestors),
        fetching,
        &emit_done(&st.done),
        &st.rest,
    )
}

/// Serialize one `__visitor_build` ping-pong bounce's full state payload. Shared by `entry` (the
/// first bounce — `inherited`/`base_generics`/`anc`/`done` are always empty, nothing fetched yet)
/// and `build` (every later bounce, carrying the accumulated state). Content pieces that need
/// their own rendering (`@base`, `@anc`, `@done`) are passed pre-rendered so this fn stays a pure
/// section-list assembler.
#[allow(clippy::too_many_arguments)]
pub(crate) fn state_tokens(
    bases: &TokenStream, // emit_bases(&bases)
    bfetch: &TokenStream,
    bnow: &TokenStream,
    build: &Path,
    nonce: &TokenStream,
    visited: &[Path],
    inherited: &[SubEntry],
    anc: &TokenStream, // emit_ancestors(&base_ancestors) or quote!()
    fetching: &TokenStream,
    done: &TokenStream, // emit_done(&done) or quote!()
    rest: &[Path],
) -> TokenStream {
    quote! {
        @bases { #bases }
        @bfetch { #bfetch }
        @bnow { #bnow }
        @build { #build }
        @nonce { #nonce }
        @visited { #(#visited),* }
        @inherited { #(for e in inherited), { #{&e.path} as #{&e.key} } }
        @anc { #anc }
        @fetching { #fetching }
        @done { #done }
        @rest { #(#rest),* }
    }
}

pub fn build(input: TokenStream) -> TokenStream {
    let mut st: BuildInput = match syn::parse2(input) {
        Ok(s) => s,
        Err(e) => return e.to_compile_error(),
    };

    if let Some(def) = st.just_def.take() {
        let path = match st.fetching.clone() {
            Some(p) => p,
            None => {
                return Error::new(Span::call_site(), "internal: @ast without @fetching")
                    .to_compile_error()
            }
        };
        let subast = std::mem::take(&mut st.just_subast);

        let method_set = st.method_set();
        let self_ident = item_ident(&def);
        let mut seen: HashSet<String> = st
            .done
            .iter()
            .map(|d| norm_path(&d.path))
            .chain(st.rest.iter().map(norm_path))
            .collect();
        for entry_path in
            followed_intermediates(&def, &subast, &method_set, self_ident, &method_set)
        {
            if seen.insert(norm_path(&entry_path)) {
                st.rest.push(entry_path);
            }
        }
        st.done.push(DoneType { path, def, subast });
    }
    st.fetching = None;

    // Ask the next base for its metadata before any type is fetched, so the inherited set is
    // complete by the time a field is lowered against it.
    if !st.base_fetch.is_empty() {
        let next = st.base_fetch.remove(0);
        let state = carry(&st, &quote!(), &quote!(#next));
        return quote! { #next::__syan_visited ! { @visited #{&st.build} { #state } } };
    }

    if !st.rest.is_empty() {
        let next = st.rest.remove(0);
        let state = carry(&st, &quote!(#next), &quote!());
        let build = &st.build;
        return quote! { #next ! { @ast #build { #state } } };
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
            quote! { @t { @path { #path } @def { #def } @subast { #subast } } }
        })
        .collect();
    quote!( #(#blocks)* )
}

/// Whether a path recorded by `base`'s visitor has to be rewritten before this module can use it.
///
/// `self::`/`super::` are relative to the *base's* module, which is never this one — always rewrite,
/// or a base and an extender at different nesting depths silently name different types. `crate::`
/// already names the right crate when the base lives in this one, and must be re-rooted only when it
/// does not. An absolute or external-crate path needs nothing.
pub(crate) fn needs_requalify(p: &Path, base: &Path) -> bool {
    if p.leading_colon.is_some() {
        return false;
    }
    match p.segments.first().map(|s| s.ident.to_string()).as_deref() {
        Some("self") | Some("super") => true,
        Some("crate") => base_host_crate(base).is_some(),
        _ => false,
    }
}

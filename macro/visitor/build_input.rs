use super::*;

/// A fetched AST type: the path it was fetched by, its (cleaned) definition, and its `#[subast]`.
pub(crate) struct DoneType {
    pub(crate) path: Path,
    pub(crate) def: Item,
    pub(crate) subast: Vec<SubEntry>,
}

// `__visitor_build`: receives accumulated state + the just-resolved definition, fetches the next type
// or generates the module.

pub(crate) struct BuildInput {
    pub(crate) base: Option<Path>,
    pub(crate) build: Path,
    pub(crate) nonce: TokenStream,
    pub(crate) visited: Vec<Path>,
    pub(crate) inherited: Vec<Ident>,
    /// The base visitor's generic-param union (when inheriting), supplied by the base's
    /// `__syan_visited` macro, so the new trait can reference `base::Visit<..>` with the *base's*
    /// arity instead of the new union's.
    pub(crate) base_generics: Vec<GenericParam>,
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
            .chain(self.inherited.iter().map(|i| i.to_string()))
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
/// New)` invokes it, appends this visitor's visited+inherited idents (`@inh`), param union (`@bg`), and
/// ancestor chain (`@an`).
pub(crate) fn emit_visited_macro(
    st: &BuildInput,
    g_params: &[GenericParam],
    anc_export: TokenStream,
) -> TokenStream {
    let all_visible: Vec<Ident> = st
        .visited
        .iter()
        .map(|p| last_ident(p).clone())
        .chain(st.inherited.iter().cloned())
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
                    $($pre)* @inh { #(#all_visible)* } @bg { #(#g_params),* } @an { #anc_export }
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

/// The host crate of a direct-base path: `Some(ident)` when it is rooted at an *external* crate
/// (e.g. `syan_rust::inherit::mid`), `None` for same-crate roots (`crate`/`super`/`self`) or a
/// leading-`::` absolute path. Gates the ancestor requalification (`requalify_ancestor`): a transitive
/// ancestor an *upstream* intermediate recorded relative to its own crate must be rewritten into a
/// path the *downstream* extender can resolve. (A `$crate` cannot do this: emitted by a proc-macro
/// into a generated `macro_rules` body it resolves only for fetch/macro-invocation paths, **not** for
/// the trait path re-emitted into the new `Driver`'s supertrait impl — so a cross-crate `base => mid
/// => new` with an *upstream* `mid` needs this concrete requalification instead.)
pub(crate) fn base_host_crate(base: &Path) -> Option<Ident> {
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
                    let paths = Punctuated::<Path, Token![,]>::parse_terminated.parse2(content)?;
                    rest = paths.into_iter().collect();
                }
                "ast" => just_def = Some(syn::parse2(content)?),
                "subast" => just_subast = parse_subentries(content)?,
                other => return Err(Error::new(name.span(), format!("unknown section @{other}"))),
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

/// Serialize one `__visitor_build` ping-pong bounce's full state payload. Shared by `entry` (the
/// first bounce — `inherited`/`base_generics`/`anc`/`done` are always empty, nothing fetched yet)
/// and `build` (every later bounce, carrying the accumulated state). Content pieces that need
/// their own rendering (`@base`, `@anc`, `@done`) are passed pre-rendered so this fn stays a pure
/// section-list assembler.
#[allow(clippy::too_many_arguments)]
pub(crate) fn state_tokens(
    base: &TokenStream, // base_tokens(&base_path) or quote!()
    build: &Path,
    nonce: &TokenStream,
    visited: &[Path],
    inherited: &[Ident],
    base_generics: &[GenericParam],
    anc: &TokenStream, // emit_ancestors(&base_ancestors) or quote!()
    fetching: &TokenStream,
    done: &TokenStream, // emit_done(&done) or quote!()
    rest: &[Path],
) -> TokenStream {
    quote! {
        @base { #base }
        @build { #build }
        @nonce { #nonce }
        @visited { #(#visited),* }
        @inherited { #(#inherited)* }
        @baseg { #(#base_generics),* }
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
        for entry_path in followed_intermediates(&def, &subast, &method_set, self_ident) {
            if seen.insert(norm_path(&entry_path)) {
                st.rest.push(entry_path);
            }
        }
        st.done.push(DoneType { path, def, subast });
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
        let base_ts = base_tokens(base);
        let done_ts = emit_done(done);
        let anc_ts = emit_ancestors(base_ancestors);
        let state = state_tokens(
            &base_ts,
            build,
            nonce,
            visited,
            inherited,
            base_generics,
            &anc_ts,
            &quote!(#next),
            &done_ts,
            rest,
        );
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

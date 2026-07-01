use crate::util::{
    angle, fold_containers, gargs, gparams, innermost_acc, item_generics, item_ident,
    method_ident_m, mt, param_name, param_use, peel, to_snake, Container, ContLayer, Head,
};
use proc_macro2::{Span, TokenStream};
use proc_macro_error::abort;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use syn::parse::{Parse, ParseStream, Parser};
use syn::punctuated::Punctuated;
use syn::*;
use template_quote::quote;

// `#[visitor([base =>] T, U, ...)]` attribute: kicks off the metadata ping-pong.

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

// Subast records carried through the ping-pong.

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

/// A fetched AST type: the path it was fetched by, its (cleaned) definition, and its `#[subast]`.
struct DoneType {
    path: Path,
    def: Item,
    subast: Vec<SubEntry>,
}

// `__visitor_build`: receives accumulated state + the just-resolved definition, fetches the next type
// or generates the module.

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

/// The `__syan_visited` export — a `#[macro_export]` muncher that, when a downstream `visitor!(self =>
/// New)` invokes it, appends this visitor's visited+inherited idents (`@inh`), param union (`@bg`), and
/// ancestor chain (`@an`).
fn emit_visited_macro(
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
fn path_is_crate_local(p: &Path) -> bool {
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
fn requalify_ancestor(anc: &Path, base: &Path) -> Path {
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
        Path { leading_colon: None, segments }
    };
    match first.ident.to_string().as_str() {
        "crate" => {
            // crate::REST -> <host>::REST (replace only the leading segment)
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
                return Err(Error::new(name.span(), format!("unknown @t section @{other}")))
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
                    let paths =
                        Punctuated::<Path, Token![,]>::parse_terminated.parse2(content)?;
                    rest = paths.into_iter().collect();
                }
                "ast" => just_def = Some(syn::parse2(content)?),
                "subast" => just_subast = parse_subentries(content)?,
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
            quote! { @t { @path { #path } @def { #def } @subast { #subast } } }
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
    if let Some(p) = peel(ty, user_types) {
        match &p.head {
            // Tuple element types must be inspected too (a followed type may be nested in a tuple, or
            // a tuple nested behind containers — `Vec<(Cast, Type)>`).
            Head::Tuple(elems) => {
                for elem in elems {
                    discover_followed(elem, subast, method_set, self_ident, user_types, out);
                }
            }
            Head::Path { head, .. } => {
                let hs = head.to_string();
                if Some(hs.as_str()) == self_ident {
                    return; // self -> already in `done`
                }
                if let Some(e) = subast.iter().find(|e| &e.key == head) {
                    // Fetch only when the entry's *real* type isn't visited/inherited (else a method,
                    // already fetched under its `visitor!(..)` path — even when the head is aliased).
                    if !method_set.contains(&last_ident(&e.path).to_string()) {
                        out.push(e.path.clone());
                    }
                }
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

// Module generation.

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

/// Right-nested `(self.0.into_hook(), (self.1.into_hook(), ...))` over tuple members — a tuple of hooks
/// is a hook (see `gen_side`), so this composes them with no combinator type.
fn build_chain(members: &[Index], into_hook: &Ident) -> TokenStream {
    let m = &members[0];
    if members.len() == 1 {
        quote!(self.#m.#into_hook())
    } else {
        let rest = build_chain(&members[1..], into_hook);
        quote!( (self.#m.#into_hook(), #rest) )
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
            let chain = build_chain(&members, &into_hook_fn);
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

/// The `#[seq]`/`#[opt]` marker on a field (`None` if unmarked), preserved into the `#[derive(Ast)]`
/// metadata so the visitor can dispatch the field through its `SeqView`/`OptView` view.
fn field_view(attrs: &[Attribute]) -> Option<Container> {
    let seq = attrs.iter().any(|a| a.path().is_ident("seq"));
    let opt = attrs.iter().any(|a| a.path().is_ident("opt"));
    match (seq, opt) {
        (true, true) => {
            let bad = attrs.iter().find(|a| a.path().is_ident("opt")).unwrap();
            abort!(bad, "a field cannot be both `#[seq]` and `#[opt]`");
        }
        (true, false) => Some(Container::Seq),
        (false, true) => Some(Container::Opt),
        (false, false) => None,
    }
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
    /// (mut walk) heads reached in a `#[seq]`/`#[opt]` field — drive which `visit_<t>_seq`/`_opt`
    /// methods `gen_side` emits.
    seq_used: &'a RefCell<HashSet<String>>,
    opt_used: &'a RefCell<HashSet<String>>,
}

impl<'a> Lower<'a> {
    fn amp(&self) -> TokenStream {
        if self.mutable {
            quote!(&mut)
        } else {
            quote!(&)
        }
    }

    /// Emit `this.visit_<head>_seq(binding)` / `_opt(binding)` for a `#[seq]`/`#[opt]` field, recording
    /// the `(head, kind)` usage so `gen_side` emits the method. `binding` is the `&mut <field>`, whose
    /// type `impl`s `SeqView<head>`/`OptView<head>` (box-transparently), so it is passed as-is.
    fn view_dispatch(&self, head: &Ident, binding: &TokenStream, kind: &Container) -> TokenStream {
        let snake = to_snake(head);
        match kind {
            Container::Seq => {
                self.seq_used.borrow_mut().insert(head.to_string());
                let m = Ident::new(&format!("visit_{snake}_seq"), Span::call_site());
                quote!( this.#m(#binding); )
            }
            Container::Opt => {
                self.opt_used.borrow_mut().insert(head.to_string());
                let m = Ident::new(&format!("visit_{snake}_opt"), Span::call_site());
                quote!( this.#m(#binding); )
            }
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
                    let view = field_view(&f.attrs);
                    if let Some(stmt) = self
                        .lower_field(&f.ty, &bind, view, idx, subast, self_ident, path, depth, stack)
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
                    let view = field_view(&f.attrs);
                    if let Some(stmt) = self
                        .lower_field(&f.ty, &bind, view, idx, subast, self_ident, path, depth, stack)
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

    /// Lower one field. `binding` is the destructured field (a `&Field`/`&mut Field`). `view` is the
    /// field's `#[seq]`/`#[opt]` marker (the visitor dispatches such a field through its container-edit
    /// view). Returns the visit statement(s), or `None` for a leaf / finite dead-end (binds `_`).
    #[allow(clippy::too_many_arguments)]
    fn lower_field(
        &self,
        ty: &Type,
        binding: &TokenStream,
        view: Option<Container>,
        idx: usize,
        subast: &[SubEntry],
        self_ident: Option<&Ident>,
        path: &Path,
        depth: usize,
        stack: &mut Vec<String>,
    ) -> Option<TokenStream> {
        let user_types = self_and_subast_keys(self_ident, subast);
        let p = peel(ty, &user_types)?;
        // A field behind a shared reference (`&T`/`&[T]`) is visitable on the shared side but a leaf
        // for `visit_mut` — there is no `&mut head` reachable through a `&`.
        if self.mutable && p.shared_ref {
            return None;
        }
        // Dispatch at the innermost (container-peeled) accessor, then wrap the container chain
        // (handles nested containers like `Vec<Option<T>>`, and now `Vec<(A, B)>`). An empty body
        // (a leaf head, or a finite drill reaching nothing) ⇒ the whole field is a leaf.
        let acc = innermost_acc(&p.conts, binding);
        // The effective head type (real ident + path) when this is a followed `Head::Path`: self
        // (implicit) or a `#[subast]` entry (an aliased `Real as Aliased` dispatches to `visit_real`).
        let resolved: Option<(Ident, Path)> = match &p.head {
            Head::Path { head: phead, .. } if Some(phead) == self_ident => {
                Some((phead.clone(), path.clone()))
            }
            Head::Path { head: phead, .. } => subast
                .iter()
                .find(|e| &e.key == phead)
                .map(|e| (last_ident(&e.path).clone(), e.path.clone())),
            Head::Tuple(_) => None,
        };

        // A `#[seq]`/`#[opt]`-marked field whose peeled head is a visited type dispatches to its
        // container-edit view (`visit_mut` only); an unmarked field falls through to the ordinary descent.
        // The marker names the field's INNERMOST container; any outer layers (`Vec<Option<T>>`,
        // `Option<Vec<T>>`, …) are traversed normally and the innermost element gets the view.
        if self.mutable {
            if let Some(kind) = view {
                if let Some((head, _)) = &resolved {
                    if self.method_set.contains(&head.to_string()) {
                        let outer: &[ContLayer] = match p.conts.split_last() {
                            Some((inner, outer)) => {
                                if inner.kind != kind {
                                    let (marked, found) = match kind {
                                        Container::Seq => ("seq", "an `Option`"),
                                        Container::Opt => ("opt", "a sequence"),
                                    };
                                    abort!(
                                        Span::call_site(),
                                        "`#[{}]` field's innermost container is {} — the marker must \
                                         name the innermost container",
                                        marked,
                                        found
                                    );
                                }
                                outer
                            }
                            None => &[],
                        };
                        let inner_acc = innermost_acc(outer, binding);
                        let dispatch = self.view_dispatch(head, &inner_acc, &kind);
                        return Some(fold_containers(outer, binding, dispatch, self.mutable));
                    }
                }
            }
        }

        let body = match &p.head {
            // A tuple at the innermost position: destructure and lower each element (an element may
            // itself be a followed type, a container of one, or a nested tuple).
            Head::Tuple(elems) => {
                self.lower_tuple(elems, &acc, p.head_box, idx, subast, self_ident, path, depth, stack)
            }
            Head::Path { .. } => match &resolved {
                Some((head, drill_path)) => {
                    self.visit_value(&acc, head, drill_path, p.head_box, depth, stack)
                }
                None => quote!(),
            },
        };
        (!body.is_empty()).then(|| fold_containers(&p.conts, binding, body, self.mutable))
    }

    /// Lower a tuple at the (container-peeled, box-dereffed) accessor `acc`: destructure it and lower
    /// each element. Leaf elements bind `_`; an empty result (no followed element) makes the tuple a
    /// leaf. Mirrors the `#[recurse]` path's `recurse_lower_tuple`.
    #[allow(clippy::too_many_arguments)]
    fn lower_tuple(
        &self,
        elems: &[Type],
        acc: &TokenStream,
        head_box: usize,
        idx: usize,
        subast: &[SubEntry],
        self_ident: Option<&Ident>,
        path: &Path,
        depth: usize,
        stack: &mut Vec<String>,
    ) -> TokenStream {
        let mut pats = Vec::new();
        let mut stmts = Vec::new();
        for (i, elem) in elems.iter().enumerate() {
            let ebind = Ident::new(&format!("__t{depth}_{idx}_{i}"), Span::call_site());
            // A tuple element is a bare type with no field attrs, so it can carry no `#[seq]`/`#[opt]`.
            if let Some(stmt) = self.lower_field(
                elem,
                &quote!(#ebind),
                None,
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
            return quote!(); // tuple of only leaves -> leaf (empty body)
        }
        let amp = self.amp();
        let stars: TokenStream = (0..=head_box).map(|_| quote!(*)).collect();
        quote!( { let ( #(#pats,)* ) = #amp #stars #acc; #(#stmts)* } )
    }
}


/// The deduped union of every target's generic params (first declaration wins), followed by the
/// base's params (for inheritance — the new trait must declare them to name `base::Visit<base params>`
/// as a supertrait, so the new union must ⊇ the base's). The caller normalizes order with
/// `sort_lifetimes_first`; the recurse path additionally filters this to the cycle roots' params.
fn param_union(targets: &[&DoneType], base_generics: &[GenericParam]) -> Vec<GenericParam> {
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
fn sort_lifetimes_first(params: &mut [GenericParam]) {
    params.sort_by_key(|p| !matches!(p, GenericParam::Lifetime(_)));
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

/// The bare generic-param ident a `where`-predicate bounds (`S` in `S: Bound`), or `None` for a
/// predicate whose bounded type isn't a single bare param (`Vec<S>: Clone`, lifetime bounds, …).
fn where_pred_param(p: &WherePredicate) -> Option<&Ident> {
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
    /// In heterogeneous (method-generic) mode: this type's own params that are NOT trait-level (i.e.
    /// not shared by all visited types), declared as generics on its `visit_*` method + free fn. Empty
    /// in the common union mode (all params are trait-level).
    method_params: Vec<GenericParam>,
    /// Whether the visited type is crate-local (so an inherent `.visit()` impl is allowed; a foreign
    /// target would be E0116, so its inherent is skipped — call `Visit::visit_*` instead).
    local: bool,
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
    // Heterogeneous (method-generic) mode: a non-shared param is concrete-filled in a cross-edge, so
    // each `visit_*` carries its type's non-shared params as method generics. A closure can't be
    // `for<T>` generic, so the closure machinery (`&mut V` blanket / `Driver`/`Hook`/`Chain`/
    // `IntoVisitor`) is omitted and the inherent `.visit()` takes `&mut impl Visit` directly.
    struct_only: bool,
    // (mut side) which visited types are held Vec-like / Option-like by some AST → get a
    // `visit_<t>_seq` / `visit_<t>_opt` container-edit method. Ignored on the immutable side.
    seq_used: &HashSet<String>,
    opt_used: &HashSet<String>,
) -> TokenStream {
    let suffix = if mutable { "Mut" } else { "" };
    let id = |s: &str| Ident::new(s, Span::call_site());
    let visit_tr = id(&format!("Visit{suffix}"));
    let into_vis_tr = id(&format!("IntoVisitor{suffix}"));
    let into_hook_tr = id(&format!("IntoHook{suffix}"));
    let hook_tr = id(&format!("Hook{suffix}"));
    let driver = id(&format!("Driver{suffix}"));
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
    // Per-method generics for the container-edit views (`visit_*_seq`/`_opt` take `&mut impl SeqView<T>`
    // / `OptView<T>` — the field type itself, no wrapper).
    let p_vw = fresh_ident("__VW", &reserved);
    let p_ow = fresh_ident("__OW", &reserved);

    struct S {
        ty: TokenStream,
        /// The visited type's bare name (e.g. `Expr`), for generated doc comments.
        name: String,
        /// Generated doc for the trait method (`fn visit_<name>`) and the free fn (`visit_<name>`).
        tdoc: String,
        fdoc: String,
        /// Docs for the `visit_mut`-side container-edit methods (`visit_<name>_seq`/`_opt`).
        seq_doc: String,
        opt_doc: String,
        method: Ident,
        /// Container-edit methods (`visit_<name>_seq`/`_opt`); emitted only when `has_seq`/`has_opt`.
        seq_method: Ident,
        opt_method: Ident,
        /// Whether some AST holds this type Vec-like / Option-like (drives emission of `seq`/`opt`).
        has_seq: bool,
        has_opt: bool,
        /// This type's non-shared params (heterogeneous mode), as the trait method's generics —
        /// lifetimes-first; empty in union mode.
        method_params: Vec<GenericParam>,
        /// The free fn's full generic list (trait params ∪ this type's non-shared params), normalized
        /// lifetimes-first so a non-shared lifetime never lands after a type/const param.
        free_params: Vec<GenericParam>,
        /// `where`-clause for the trait method (`where Self: Sized` in struct-only mode, plus any bound
        /// on this type's method-generic params, e.g. `S: Bound`); empty in the common union mode.
        trait_where: TokenStream,
        /// `where`-clause for the free fn — the union predicates (trait-level) plus this type's
        /// method-generic-param bounds; covers naming `Bounded<S>` where `S` is a method generic.
        free_where: TokenStream,
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
            let mut method_params = t.method_params.clone();
            sort_lifetimes_first(&mut method_params);
            let mut free_params: Vec<GenericParam> =
                g_params.iter().cloned().chain(t.method_params.iter().cloned()).collect();
            sort_lifetimes_first(&mut free_params);
            // This type's `where`-bounds on its method-generic params (e.g. `S: Bound` when `S` is
            // non-shared). They can't live on the trait (it's keyed on the shared params), so they ride
            // the per-type `visit_*` method + free fn that declares the param.
            let mp_names: HashSet<String> = t.method_params.iter().map(param_name).collect();
            let method_where: Vec<WherePredicate> = t
                .own_where
                .iter()
                .filter(|p| where_pred_param(p).is_some_and(|id| mp_names.contains(&id.to_string())))
                .cloned()
                .collect();
            // Trait method: `where Self: Sized` (struct-only) + the method-param bounds.
            let trait_where = if struct_only {
                let mut preds: Vec<WherePredicate> = vec![parse_quote!(Self: ::core::marker::Sized)];
                preds.extend(method_where.iter().cloned());
                where_clause(&preds)
            } else {
                quote!()
            };
            // Free fn: the trait-level union predicates + this type's method-param bounds.
            let free_where = {
                let mut preds = union_where.to_vec();
                preds.extend(method_where.iter().cloned());
                where_clause(&preds)
            };
            let name = ident.to_string();
            let mname = method_ident_m(&ident, mutable).to_string();
            let tdoc = format!(
                "Visit an `{name}` node. The default recurses into its children (via the free \
                 [`{mname}`]); override to act on the node, calling `{mname}(self, i)` to continue \
                 the descent."
            );
            let fdoc = format!(
                "Recurse into the children of an `{name}` node, dispatching each child to the \
                 visitor's `visit_*{mut_sfx}` methods. [`{visit_tr}::{mname}`]'s default delegates \
                 here; call it from an override to keep descending.",
                mut_sfx = mt(mutable),
            );
            let seq_doc = format!(
                "Edit the `{name}` nodes held in a `Vec`-like slot of their parent — given a \
                 [`SeqView`](::syan::visit::SeqView) over the collection (edit in place via `get_mut`, \
                 or `push`/`insert`/`remove`/`retain_mut`/`edit_each`). The default visits each element \
                 in place via `{mname}`; override to restructure the collection.",
            );
            let opt_doc = format!(
                "Edit the `{name}` node held in an `Option`-like slot of their parent — given an \
                 [`OptView`](::syan::visit::OptView) (`get_mut`/`set`/`take`). The default visits the \
                 present node via `{mname}`.",
            );
            let has_seq = mutable && seq_used.contains(&name);
            let has_opt = mutable && opt_used.contains(&name);
            S {
                ty,
                name,
                tdoc,
                fdoc,
                seq_doc,
                opt_doc,
                method: method_ident_m(&ident, mutable),
                seq_method: Ident::new(&format!("visit_{}_seq", to_snake(&ident)), Span::call_site()),
                opt_method: Ident::new(&format!("visit_{}_opt", to_snake(&ident)), Span::call_site()),
                has_seq,
                has_opt,
                method_params,
                free_params,
                trait_where,
                free_where,
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

    // ── generated API docs ────────────────────────────────────────────────────────────────────────
    let visited_list = sides.iter().map(|s| format!("`{}`", s.name)).collect::<Vec<_>>().join(", ");
    let entry = visit_method.to_string();
    let trait_doc = format!(
        "Visitor over the AST node type(s) {visited_list} — generated by `visitor!`.\n\n\
         Implement this trait on your visitor type and override the `visit_*{mut_sfx}` methods for the \
         nodes you want to handle; each method's default recurses into that node's children. Start a \
         traversal by calling `node.{entry}(&mut visitor)` on a root node.{base_note}",
        mut_sfx = mt(mutable),
        base_note = if mutable { " The by-`&mut`, in-place variant of `Visit`." } else { "" },
    );
    let inherent_doc = format!(
        "Visit `self` with `visitor` (any `{visit_tr}` impl) and return `self` so calls can chain. \
         Entry point for {a} traversal.",
        a = if mutable { "an in-place" } else { "a" },
    );

    // Inherent `visit` / `visit_mut` per type (replaces the Visitable trait). Each type's own
    // params go on the impl; any extra union params go on the method (so a type that doesn't use
    // every union param doesn't leave the impl param unconstrained). The type's own `where`-clause
    // (referencing only its own params) goes on the impl so naming `Expr<S>` stays well-formed.
    let inherent: Vec<TokenStream> = vtypes
        .iter()
        .map(|vt| {
            // A foreign target can't carry an inherent impl (E0116); callers use `Visit::visit_*`.
            if !vt.local {
                return quote!();
            }
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
            if struct_only {
                // Direct `&mut impl Visit` (the closure/`IntoVisitor` machinery is off in method-mode).
                quote! {
                    impl #own_def #path #own_use #own_w {
                        #[doc = #inherent_doc]
                        pub fn #visit_method< #(#extra,)* #p_v: #visit_tr #g_use >(
                            #recv,
                            visitor: &mut #p_v,
                        ) -> #self_ret {
                            visitor.#method(self);
                            self
                        }
                    }
                }
            } else {
                quote! {
                    impl #own_def #path #own_use #own_w {
                        #[doc = #inherent_doc]
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
            }
        })
        .collect();

    quote! {
        #[doc = #trait_doc]
        pub trait #visit_tr #g_def #(if let Some(b) = base) { : #b::#visit_tr #base_g_use } #uw {
            #(for s in &sides) {
                // In heterogeneous (struct-only) mode the method carries this type's non-shared params
                // as generics, and `where Self: Sized` (the method-generic dispatch needs a sized Self).
                #[doc = #{&s.tdoc}]
                fn #{&s.method}< #(for mp in &s.method_params) { #mp, } >(&mut self, i: #amp #{&s.ty})
                    #{&s.trait_where}
                {
                    #{&s.method}(self, i)
                }
                // Opt-in container-edit hooks (mut side; emitted only where the type is held that way).
                // Default: descend each held node via `visit_*_mut`. Override to restructure the parent.
                #(if s.has_seq) {
                    #[doc = #{&s.seq_doc}]
                    fn #{&s.seq_method}< #(for mp in &s.method_params) { #mp, } #p_vw: ::syan::visit::SeqView< #{&s.ty} > >(
                        &mut self,
                        v: &mut #p_vw,
                    ) #{&s.trait_where} {
                        #{&s.seq_method}(self, v)
                    }
                }
                #(if s.has_opt) {
                    #[doc = #{&s.opt_doc}]
                    fn #{&s.opt_method}< #(for mp in &s.method_params) { #mp, } #p_ow: ::syan::visit::OptView< #{&s.ty} > >(
                        &mut self,
                        v: &mut #p_ow,
                    ) #{&s.trait_where} {
                        #{&s.opt_method}(self, v)
                    }
                }
            }
        }

        #(if !struct_only) {
            impl< #(#g_params,)* #p_v: #visit_tr #g_use > #visit_tr #g_use for &mut #p_v #uw {
                #(for s in &sides) {
                    fn #{&s.method}(&mut self, i: #amp #{&s.ty}) {
                        <#p_v as #visit_tr #g_use>::#{&s.method}(self, i)
                    }
                    #(if s.has_seq) {
                        fn #{&s.seq_method}< #p_vw: ::syan::visit::SeqView< #{&s.ty} > >(&mut self, v: &mut #p_vw) {
                            <#p_v as #visit_tr #g_use>::#{&s.seq_method}(self, v)
                        }
                    }
                    #(if s.has_opt) {
                        fn #{&s.opt_method}< #p_ow: ::syan::visit::OptView< #{&s.ty} > >(&mut self, v: &mut #p_ow) {
                            <#p_v as #visit_tr #g_use>::#{&s.opt_method}(self, v)
                        }
                    }
                }
            }
        }

        #(for s in &sides) {
            // No `?Sized` under struct-only: the body may dispatch through `Self`'s method-generic
            // `visit_*` (which requires `Self: Sized`). `free_params` = trait params ∪ this type's
            // non-shared params, lifetimes-first.
            #[doc = #{&s.fdoc}]
            pub fn #{&s.method}< #(for gp in &s.free_params) { #gp, } #p_v: #visit_tr #g_use #(if !struct_only) { + ?Sized } >(
                this: &mut #p_v,
                i: #amp #{&s.ty},
            ) #{&s.free_where} {
                #{&s.body}
            }
            // Default container-edit descent: visit each held node in place via the per-node `visit_*_mut`
            // (so a `visit_*_mut` override / closure hook still runs for every element).
            #(if s.has_seq) {
                #[doc = #{&s.seq_doc}]
                pub fn #{&s.seq_method}< #(for gp in &s.free_params) { #gp, } #p_vw: ::syan::visit::SeqView< #{&s.ty} >, #p_v: #visit_tr #g_use #(if !struct_only) { + ?Sized } >(
                    this: &mut #p_v,
                    v: &mut #p_vw,
                ) #{&s.free_where} {
                    ::syan::visit::SeqView::for_each_mut(v, |__syan_e| this.#{&s.method}(__syan_e));
                }
            }
            #(if s.has_opt) {
                #[doc = #{&s.opt_doc}]
                pub fn #{&s.opt_method}< #(for gp in &s.free_params) { #gp, } #p_ow: ::syan::visit::OptView< #{&s.ty} >, #p_v: #visit_tr #g_use #(if !struct_only) { + ?Sized } >(
                    this: &mut #p_v,
                    v: &mut #p_ow,
                ) #{&s.free_where} {
                    if let ::core::option::Option::Some(__syan_e) = ::syan::visit::OptView::get_mut(v) {
                        this.#{&s.method}(__syan_e);
                    }
                }
            }
        }

        #(if !struct_only) {
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

        // --- multiple closures: a 2-tuple of hooks is itself a hook (calls both), so it is the
        // tuple-of-closures combinator directly — no `Chain` newtype. `build_chain` nests them right.
        impl< #(#g_params,)* #p_a: #hook_tr #g_use, #p_b: #hook_tr #g_use >
            #hook_tr #g_use for ( #p_a, #p_b ) #uw
        {
            #(for s in &sides) {
                fn #{&s.hook}(&mut self, i: #amp #{&s.ty}) {
                    self.0.#{&s.hook}(i);
                    self.1.#{&s.hook}(i);
                }
            }
        }
        #(for imp in &tup) { #imp }
        } // end #(if !struct_only) — closure/Driver machinery off for a recurse base

        // Inherent entry points (no trait import needed at the call site).
        #(for imp in &inherent) { #imp }
    }
}


/// Does any visited type's field reference another visited type while filling a **non-shared** generic
/// param position with something other than that param verbatim (e.g. `Box<Stmt<S, u8>>` where `Stmt`'s
/// `T` is non-shared)? Such a *concrete fill* can't be expressed with the union-of-params trait model
/// (the trait would fix `T`, but the cross-edge needs a specific `T`), so the visitor must instead make
/// the non-shared params **per-method generics** (`visit_stmt<T>`). Closures can't be `for<T>` generic,
/// so that mode is struct-only. The common case (no concrete fill) keeps the union model + closures.
fn has_concrete_fill(targets: &[&DoneType], shared: &HashSet<String>) -> bool {
    // Each visited type's own params, in declaration order (lifetimes precede types/consts, matching how
    // generic *arguments* must be ordered — so args zip directly onto params).
    let params_of: HashMap<String, Vec<GenericParam>> = targets
        .iter()
        .filter_map(|d| {
            let id = item_ident(&d.def)?;
            Some((id.to_string(), gparams(item_generics(&d.def)?)))
        })
        .collect();

    fn ty_fills(
        ty: &Type,
        params_of: &HashMap<String, Vec<GenericParam>>,
        shared: &HashSet<String>,
    ) -> bool {
        match ty {
            Type::Path(tp) => {
                for seg in &tp.path.segments {
                    if let PathArguments::AngleBracketed(ab) = &seg.arguments {
                        // Zip the actual args onto the referenced type's declared params (same order).
                        // Only a non-shared TYPE or CONST param filled with a non-identity arg forces
                        // method-generic mode — a lifetime fill (`Stmt<'static, S>`) is fine in the
                        // union model via subtyping, so it does NOT trigger it.
                        if let Some(decl) = params_of.get(&seg.ident.to_string()) {
                            for (param, arg) in decl.iter().zip(ab.args.iter()) {
                                match (param, arg) {
                                    (GenericParam::Type(tp_), GenericArgument::Type(at)) => {
                                        let pname = tp_.ident.to_string();
                                        let bare = matches!(at, Type::Path(p)
                                            if p.qself.is_none() && p.path.is_ident(&pname));
                                        if !shared.contains(&pname) && !bare {
                                            return true;
                                        }
                                    }
                                    (GenericParam::Const(cp), GenericArgument::Const(ce)) => {
                                        let pname = cp.ident.to_string();
                                        let bare = matches!(ce, syn::Expr::Path(p)
                                            if p.qself.is_none() && p.path.is_ident(&pname));
                                        if !shared.contains(&pname) && !bare {
                                            return true;
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        // Recurse into every type argument (nested containers / cross-edges).
                        for arg in &ab.args {
                            if let GenericArgument::Type(at) = arg {
                                if ty_fills(at, params_of, shared) {
                                    return true;
                                }
                            }
                        }
                    }
                }
                false
            }
            Type::Reference(r) => ty_fills(&r.elem, params_of, shared),
            Type::Slice(s) => ty_fills(&s.elem, params_of, shared),
            Type::Array(a) => ty_fills(&a.elem, params_of, shared),
            Type::Paren(p) => ty_fills(&p.elem, params_of, shared),
            Type::Group(g) => ty_fills(&g.elem, params_of, shared),
            Type::Tuple(t) => t.elems.iter().any(|e| ty_fills(e, params_of, shared)),
            _ => false,
        }
    }

    let fields_of = |def: &Item| -> Vec<Type> {
        let mut out = Vec::new();
        match def {
            Item::Enum(e) => {
                for v in &e.variants {
                    out.extend(v.fields.iter().map(|f| f.ty.clone()));
                }
            }
            Item::Struct(s) => out.extend(s.fields.iter().map(|f| f.ty.clone())),
            _ => {}
        }
        out
    };
    targets
        .iter()
        .any(|d| fields_of(&d.def).iter().any(|t| ty_fills(t, &params_of, shared)))
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

    // The visitor trait is parameterized by the *union* of every visited type's generic params (+ the
    // base's, when inheriting), so one visitor can span e.g. `Expr<S, Tokens>` and `BinOp<S>`; each
    // type is referenced with its own subset, and `base_g_use` (below) names the base's args by the
    // union's idents for every `base::Visit<..>` reference.
    let mut union_params = param_union(&targets, &st.base_generics);
    sort_lifetimes_first(&mut union_params);

    // Params shared by EVERY visited type (∪ the base's, which must stay trait-level to name
    // `base::Visit<base params>`). A non-shared param appears in only some types.
    let mut shared_names: Option<HashSet<String>> = None;
    for d in &targets {
        let own: HashSet<String> = gparams(item_generics(&d.def).unwrap())
            .iter()
            .map(param_name)
            .collect();
        shared_names = Some(match shared_names {
            None => own,
            Some(acc) => acc.intersection(&own).cloned().collect(),
        });
    }
    let mut shared_names = shared_names.unwrap_or_default();
    for bp in &st.base_generics {
        shared_names.insert(param_name(bp));
    }

    // A union param that some visited type does NOT declare. Such a param can stay a trait param (the
    // union) only while it's *unbounded* — a type lacking it is then harmlessly quantified over it. But a
    // `where`-bounded one (`S: Bound`) can't: applied to items over the union, a type lacking `S` carries
    // an undischargeable `S: Bound`. So a bounded unshared param, like a concrete-filled one, must become
    // a per-method generic with the trait keyed on the shared subset (method-mode, below).
    let unshared_names: HashSet<String> =
        union_params.iter().map(param_name).filter(|n| !shared_names.contains(n)).collect();
    let has_bounded_unshared = targets.iter().any(|d| {
        item_where_preds(&d.def)
            .iter()
            .any(|p| where_pred_param(p).is_some_and(|id| unshared_names.contains(&id.to_string())))
    });

    // Heterogeneous mode: a non-shared param is either *concrete-filled* in a cross-edge (e.g.
    // `Stmt<S, u8>`) — which the union-of-params trait can't express — or carries a `where`-bound (above).
    // Make non-shared params per-method generics and go struct-only (no closures — a closure can't be
    // `for<T>` generic). Gated to the no-inheritance case (a recurse/heterogeneous base is out of scope)
    // so the common union+closure path is untouched.
    let method_mode =
        st.base.is_none() && (has_concrete_fill(&targets, &shared_names) || has_bounded_unshared);

    // Trait params: the full union normally; only the shared subset in method-mode (non-shared params
    // become method generics instead).
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
    let struct_only = method_mode;
    let by_name: HashMap<String, TokenStream> =
        union_params.iter().map(|p| (param_name(p), param_use(p))).collect();
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
        // Requalify transitive ancestors that a `crate::`/`super::`/`self::`-relative *upstream*
        // intermediate recorded, resolving them against the direct base's full path (no-op for
        // same-crate / already-concrete chains). This also re-exports them concrete (the chain feeds
        // `anc_export`), so a further extender inherits resolvable ancestor paths too.
        let cross_crate = base_host_crate(b).is_some();
        for a in &st.base_ancestors {
            let path = if cross_crate {
                requalify_ancestor(&a.path, b)
            } else {
                a.path.clone()
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

    // Container-edit usage, populated by the mut walk below; consumed by `gen_side(true, ..)` to decide
    // which `visit_<t>_seq` / `visit_<t>_opt` to emit. Shared by both `Lower`s (only the mut one records).
    let seq_used: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
    let opt_used: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
    let lower = Lower {
        method_set: &method_set,
        done_by_path: &done_by_path,
        mutable: false,
        seq_used: &seq_used,
        opt_used: &opt_used,
    };
    let lower_mut = Lower {
        method_set: &method_set,
        done_by_path: &done_by_path,
        mutable: true,
        seq_used: &seq_used,
        opt_used: &opt_used,
    };

    let vtypes: Vec<VType> = targets
        .iter()
        .map(|d| {
            let def = &d.def;
            let ident = item_ident(def).unwrap().clone();
            let own_params = gparams(item_generics(def).unwrap());
            let own_use = angle(&gargs(item_generics(def).unwrap()));
            let own_where = item_where_preds(def);
            // In method-mode, this type's non-shared params become method generics; in union mode the
            // trait already carries every param, so none.
            let method_params: Vec<GenericParam> = if method_mode {
                own_params
                    .iter()
                    .filter(|p| !shared_names.contains(&param_name(p)))
                    .cloned()
                    .collect()
            } else {
                Vec::new()
            };
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
                method_params,
                local: path_is_crate_local(scrut_path),
                body,
                body_mut,
            }
        })
        .collect();

    // The union of every visited type's `where`-predicates (deduped by rendered text — identical
    // predicates from two types are harmless but noisy), applied (as `uw`) to each generated item
    // quantified over the param union so a `enum Expr<S> where S: Bound { .. }` stays well-formed there.
    // In method-mode the *non-shared* params are method generics, not trait params, so a bound on one
    // would reference an undeclared param at the trait level — drop it here; `gen_side` re-attaches it
    // to the per-type `visit_*` method + free fn that actually carries the param.
    let mut seen_pred: HashSet<String> = HashSet::new();
    let union_where: Vec<WherePredicate> = vtypes
        .iter()
        .flat_map(|vt| vt.own_where.iter().cloned())
        .filter(|p| {
            !method_mode
                || where_pred_param(p).is_none_or(|id| !unshared_names.contains(&id.to_string()))
        })
        .filter(|p| seen_pred.insert(quote!(#p).to_string()))
        .collect();

    // The mut walk has finished recording container-edit usage; take the populated sets.
    let seq_used = seq_used.into_inner();
    let opt_used = opt_used.into_inner();

    let shared = gen_side(
        false, &vtypes, &g_params, &g_args, &g_def, &g_use, &base_g_use, &ancestors, &st.base,
        &union_where, struct_only, &seq_used, &opt_used,
    );
    let mutable = gen_side(
        true, &vtypes, &g_params, &g_args, &g_def, &g_use, &base_g_use, &ancestors, &st.base,
        &union_where, struct_only, &seq_used, &opt_used,
    );

    // Every visitor module exports its full visited-type set (idents), its generic-param union
    // (`@bg`), and its full ancestor chain (`@an`) so another visitor can inherit it (transitively).
    let anc_export = emit_ancestors(&chain);
    let visited_macro = emit_visited_macro(st, &g_params, anc_export);

    // Items are emitted directly into the enclosing module (where `visitor!(...)` was invoked).
    quote! {
        #visited_macro

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


use crate::ast::to_snake;
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

/// A fetched AST type: the path it was fetched by, its (cleaned) definition, and its `#[subast]`.
struct DoneType {
    path: Path,
    def: Item,
    subast: Vec<SubEntry>,
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
            done,
            rest,
            ..
        } = &st;
        let base_tokens: TokenStream = match base {
            Some(p) => quote!(#p),
            None => quote!(),
        };
        let done_tokens = emit_done(done);
        return quote! {
            #next ! {
                @ast #build {
                    @base { #base_tokens }
                    @build { #build }
                    @nonce { #nonce }
                    @visited { #(#visited),* }
                    @inherited { #(#inherited)* }
                    @baseg { #(#base_generics),* }
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
        if let Some(p) = peel(ty, &user_types) {
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
    });
    out
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

/// How a field type wraps its (visitable) head: a single value, a sequence (`Vec`/`VecDeque`/slice/
/// array/`Punctuated`), or an `Option`. `Box` is transparent (tracked as box-depth).
#[derive(Clone, Copy, PartialEq)]
enum Container {
    Direct,
    Seq,
    Opt,
}

/// The result of peeling a field type to its visitable head.
struct Peeled {
    container: Container,
    head: Ident,
    /// `Box` layers between the container (or the top, for `Direct`) and the head; a drill derefs
    /// through these (`&**…`) to reach a `&head` scrutinee.
    head_box: usize,
    /// `Box` layers around the container itself; the `Opt` `if let` must deref through these (the
    /// `Seq` `.iter()`/`.iter_mut()` already auto-derefs them).
    cont_box: usize,
    /// A second container layer was found nested inside the first (e.g. `Vec<Option<T>>`); such a
    /// field is unsupported and the caller turns this into a clear error.
    nested: bool,
}

fn first_ty_arg(seg: &PathSegment) -> Option<&Type> {
    if let PathArguments::AngleBracketed(ab) = &seg.arguments {
        for arg in &ab.args {
            if let GenericArgument::Type(t) = arg {
                return Some(t);
            }
        }
    }
    None
}

/// Wrap a peeled element in an outer container, flagging nesting if the element already had one.
fn container_of(c: Container, inner: Peeled) -> Peeled {
    Peeled {
        container: c,
        head: inner.head,
        head_box: inner.head_box,
        cont_box: 0,
        nested: inner.nested || inner.container != Container::Direct,
    }
}

fn direct(head: Ident) -> Peeled {
    Peeled {
        container: Container::Direct,
        head,
        head_box: 0,
        cont_box: 0,
        nested: false,
    }
}

/// Peel a field type to its visitable head. A path head listed in `user_types` (this type's
/// `#[subast]` matchkeys plus its own ident) is always a `Direct` head, so a user AST type named
/// like a container keyword (`Option`, `Vec`, …) wins over the built-in container handling. `None`
/// for a non-path leaf. The caller decides whether `head` is actually followed.
fn peel(ty: &Type, user_types: &HashSet<String>) -> Option<Peeled> {
    match ty {
        Type::Reference(r) => peel(&r.elem, user_types),
        Type::Group(g) => peel(&g.elem, user_types),
        Type::Paren(p) => peel(&p.elem, user_types),
        Type::Slice(s) => peel(&s.elem, user_types).map(|inner| container_of(Container::Seq, inner)),
        Type::Array(a) => peel(&a.elem, user_types).map(|inner| container_of(Container::Seq, inner)),
        Type::Path(tp) => {
            let seg = tp.path.segments.last()?;
            let name = seg.ident.to_string();
            // A user AST type wins over a same-named container keyword.
            if user_types.contains(&name) {
                return Some(direct(seg.ident.clone()));
            }
            match name.as_str() {
                "Box" => {
                    let inner = peel(first_ty_arg(seg)?, user_types)?;
                    Some(match inner.container {
                        // Box directly around the head: deepen so a drill derefs through it.
                        Container::Direct => Peeled {
                            head_box: inner.head_box + 1,
                            ..inner
                        },
                        // Box around a container: the Opt `if let` derefs through it (Seq auto-derefs).
                        _ => Peeled {
                            cont_box: inner.cont_box + 1,
                            ..inner
                        },
                    })
                }
                "Vec" | "VecDeque" | "Punctuated" => {
                    Some(container_of(Container::Seq, peel(first_ty_arg(seg)?, user_types)?))
                }
                "Option" => Some(container_of(Container::Opt, peel(first_ty_arg(seg)?, user_types)?)),
                _ => Some(direct(seg.ident.clone())),
            }
        }
        _ => None,
    }
}

/// `_mut` suffix for the mutable traversal variant.
fn mt(mutable: bool) -> &'static str {
    if mutable {
        "_mut"
    } else {
        ""
    }
}

fn method_ident_m(head: &Ident, mutable: bool) -> Ident {
    Ident::new(
        &format!("visit_{}{}", to_snake(head), mt(mutable)),
        Span::call_site(),
    )
}

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

/// `IntoVisitor[Mut]` impls for tuples of closures, arity 2..=`max_arity`.
fn tuple_impls(
    max_arity: usize,
    g_params: &[GenericParam],
    g_args: &[TokenStream],
    g_use: &TokenStream,
    mutable: bool,
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
                where #(#wheres,)*
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

fn item_ident(item: &Item) -> Option<&Ident> {
    match item {
        Item::Enum(e) => Some(&e.ident),
        Item::Struct(s) => Some(&s.ident),
        _ => None,
    }
}

fn item_generics(item: &Item) -> Option<&Generics> {
    match item {
        Item::Enum(e) => Some(&e.generics),
        Item::Struct(s) => Some(&s.generics),
        _ => None,
    }
}

/// Name of a generic param (for deduping the union across visited types).
fn param_name(p: &GenericParam) -> String {
    match p {
        GenericParam::Type(t) => t.ident.to_string(),
        GenericParam::Const(c) => c.ident.to_string(),
        GenericParam::Lifetime(l) => l.lifetime.ident.to_string(),
    }
}

/// Use-side token for one generic param (ident / lifetime).
fn param_use(p: &GenericParam) -> TokenStream {
    match p {
        GenericParam::Lifetime(l) => {
            let lt = &l.lifetime;
            quote!(#lt)
        }
        GenericParam::Type(t) => {
            let i = &t.ident;
            quote!(#i)
        }
        GenericParam::Const(c) => {
            let i = &c.ident;
            quote!(#i)
        }
    }
}

/// Generic params with defaults stripped (for `impl<...>` / `trait<...>` headers).
fn gparams(g: &Generics) -> Vec<GenericParam> {
    g.params
        .iter()
        .cloned()
        .map(|mut p| {
            match &mut p {
                GenericParam::Type(t) => {
                    t.eq_token = None;
                    t.default = None;
                }
                GenericParam::Const(c) => {
                    c.eq_token = None;
                    c.default = None;
                }
                _ => {}
            }
            p
        })
        .collect()
}

/// Use-side generic arguments (idents / lifetimes).
fn gargs(g: &Generics) -> Vec<TokenStream> {
    g.params
        .iter()
        .map(|p| match p {
            GenericParam::Lifetime(l) => {
                let lt = &l.lifetime;
                quote!(#lt)
            }
            GenericParam::Type(t) => {
                let i = &t.ident;
                quote!(#i)
            }
            GenericParam::Const(c) => {
                let i = &c.ident;
                quote!(#i)
            }
        })
        .collect()
}

/// One visited type's identifier (for method/struct names), the full path it is referenced by, its
/// own generic params (def-side) and use-side args, and its shared-ref and `&mut` bodies.
struct VType {
    ident: Ident,
    path: TokenStream,
    own_params: Vec<GenericParam>,
    own_use: TokenStream,
    body: TokenStream,
    body_mut: TokenStream,
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
    base_g_params: &[GenericParam],
    base: &Option<Path>,
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

    let tup = tuple_impls(8, g_params, g_args, g_use, mutable);

    // Inherent `visit` / `visit_mut` per type (replaces the Visitable trait). Each type's own
    // params go on the impl; any extra union params go on the method (so a type that doesn't use
    // every union param doesn't leave the impl param unconstrained).
    let inherent: Vec<TokenStream> = vtypes
        .iter()
        .map(|vt| {
            let own_names: HashSet<String> = vt.own_params.iter().map(param_name).collect();
            let extra: Vec<&GenericParam> = g_params
                .iter()
                .filter(|p| !own_names.contains(&param_name(p)))
                .collect();
            let own_params = &vt.own_params;
            let own_def = if own_params.is_empty() {
                quote!()
            } else {
                quote!( < #(#own_params),* > )
            };
            let path = &vt.path;
            let own_use = &vt.own_use;
            let method = method_ident_m(&vt.ident, mutable);
            quote! {
                impl #own_def #path #own_use {
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
        pub trait #visit_tr #g_def #(if let Some(b) = base) { : #b::#visit_tr #base_g_use } {
            #(for s in &sides) {
                fn #{&s.method}(&mut self, i: #amp #{&s.ty}) {
                    #{&s.method}(self, i)
                }
            }
        }

        impl< #(#g_params,)* #p_v: #visit_tr #g_use > #visit_tr #g_use for &mut #p_v {
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
            ) {
                #{&s.body}
            }
        }

        pub trait #into_vis_tr< #(#g_params,)* #p_t > {
            fn #into_vis_fn(self) -> impl #visit_tr #g_use;
        }
        impl< #(#g_params,)* #p_v: #visit_tr #g_use > #into_vis_tr< #(#g_args,)* () > for #p_v {
            fn #into_vis_fn(self) -> impl #visit_tr #g_use { self }
        }

        // --- closures: shallow Hook + single-pass Driver ---------------------------------
        pub trait #hook_tr #g_def {
            #(for s in &sides) {
                fn #{&s.hook}(&mut self, i: #amp #{&s.ty}) { let _ = i; }
            }
        }
        pub trait #into_hook_tr< #(#g_params,)* #p_t > {
            fn #into_hook_fn(self) -> impl #hook_tr #g_use;
        }

        pub struct #driver<#p_h>(pub #p_h);
        impl< #(#g_params,)* #p_h: #hook_tr #g_use > #visit_tr #g_use for #driver<#p_h> {
            #(for s in &sides) {
                fn #{&s.method}(&mut self, i: #amp #{&s.ty}) {
                    self.0.#{&s.hook}(i);
                    #{&s.method}(self, i);
                }
            }
        }
        // The new trait extends the base, so Driver must satisfy the base too (via base defaults).
        // Quantified over only the base's params (+ the wrapped hook) so a wider new-union param
        // does not become an unconstrained impl param.
        #(if let Some(b) = base) {
            impl< #(#base_g_params,)* #p_h > #b::#visit_tr #base_g_use for #driver<#p_h> {}
        }

        #(for s in &sides) {
            pub struct #{&s.hook_struct}<#p_f>(pub #p_f);
            impl< #(#g_params,)* #p_f: ::core::ops::FnMut( #amp #{&s.ty} ) >
                #hook_tr #g_use for #{&s.hook_struct}<#p_f>
            {
                fn #{&s.hook}(&mut self, i: #amp #{&s.ty}) { (self.0)(i); }
            }
            impl< #(#g_params,)* #p_f: ::core::ops::FnMut( #amp #{&s.ty} ) >
                #into_hook_tr< #(#g_args,)* #{&s.ty} > for #p_f
            {
                fn #into_hook_fn(self) -> impl #hook_tr #g_use { #{&s.hook_struct}(self) }
            }
            impl< #(#g_params,)* #p_f: ::core::ops::FnMut( #amp #{&s.ty} ) >
                #into_vis_tr< #(#g_args,)* #{&s.ty} > for #p_f
            {
                fn #into_vis_fn(self) -> impl #visit_tr #g_use { #driver(#{&s.hook_struct}(self)) }
            }
        }

        // --- multiple closures: Chain combinator + tuple impls ---------------------------
        pub struct #chain<#p_a, #p_b>(pub #p_a, pub #p_b);
        impl< #(#g_params,)* #p_a: #hook_tr #g_use, #p_b: #hook_tr #g_use >
            #hook_tr #g_use for #chain<#p_a, #p_b>
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
        .filter(|d| item_ident(&d.def).map_or(false, |id| visited.contains(&id.to_string())))
        .collect();
    if targets.is_empty() {
        abort!(Span::call_site(), "no AST definitions resolved for the visitor");
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
    let base_args: Vec<TokenStream> = st
        .base_generics
        .iter()
        .map(|bp| by_name[&param_name(bp)].clone())
        .collect();
    let base_g_use = if base_args.is_empty() {
        quote!()
    } else {
        quote!( < #(#base_args),* > )
    };
    // The base's own params (a subset of the union) — used to quantify the empty `base::Visit` impl
    // for `Driver` over exactly the base's params, so a wider new-union param stays out of it (it
    // would otherwise be an unconstrained impl param: E0207).
    let base_names: HashSet<String> = st.base_generics.iter().map(param_name).collect();
    let base_g_params: Vec<GenericParam> = g_params
        .iter()
        .filter(|p| base_names.contains(&param_name(p)))
        .cloned()
        .collect();

    let g_args: Vec<TokenStream> = g_params.iter().map(param_use).collect();
    let has_g = !g_params.is_empty();
    let g_def = if has_g {
        quote!( < #(#g_params),* > )
    } else {
        quote!()
    };
    let g_use = if has_g {
        quote!( < #(#g_args),* > )
    } else {
        quote!()
    };

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
            let own = gargs(item_generics(def).unwrap());
            let own_use = if own.is_empty() {
                quote!()
            } else {
                quote!( < #(#own),* > )
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
                body,
                body_mut,
            }
        })
        .collect();

    let shared = gen_side(
        false, &vtypes, &g_params, &g_args, &g_def, &g_use, &base_g_use, &base_g_params, &st.base,
    );
    let mutable = gen_side(
        true, &vtypes, &g_params, &g_args, &g_def, &g_use, &base_g_use, &base_g_params, &st.base,
    );

    let base = &st.base;

    // Every visitor module exports its full visited-type set (idents) so another visitor can
    // inherit it; inherited types are reached only by method, so idents suffice.
    let all_visible: Vec<Ident> = st
        .visited
        .iter()
        .map(|p| last_ident(p).clone())
        .chain(st.inherited.iter().cloned())
        .collect();
    let vmacro = Ident::new(&format!("__syan_visited_{}", st.nonce), Span::call_site());

    // Items are emitted directly into the enclosing module (where `visitor!(...)` was invoked).
    quote! {
        #[macro_export]
        #[doc(hidden)]
        macro_rules! #vmacro {
            (@visited $cb:path { $($pre:tt)* }) => {
                $cb ! { $($pre)* @inh { #(#all_visible)* } @bg { #(#g_params),* } }
            };
        }
        #[doc(hidden)]
        pub use #vmacro as __syan_visited;

        #(if let Some(b) = base) {
            #[allow(unused_imports)]
            use #b::{Visit as _, VisitMut as _};
        }

        #shared
        #mutable
    }
}

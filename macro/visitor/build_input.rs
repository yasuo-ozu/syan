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
    /// Types inherited from the base, as `path as matchkey` rather than bare idents, so a field
    /// holding an inherited type is followed without a `#[subast]` entry repeating what the base
    /// already knows. Requalified against the base path on arrival, so the paths stay resolvable
    /// through any number of generations.
    pub(crate) inherited: Vec<SubEntry>,
    /// The base visitor's generic-param union (when inheriting), supplied by the base's
    /// `__syan_visited` macro, so the new trait can reference `base::Visit<..>` with the *base's*
    /// arity instead of the new union's.
    pub(crate) base_generics: Vec<GenericParam>,
    /// The direct base's own transitive ancestors (for multi-level `base => mid => new`), so the new
    /// visitor can emit the empty `Driver` impl for every transitive supertrait, not just the direct
    /// base.
    pub(crate) base_ancestors: Vec<AncIn>,
    /// The direct base's *own* targets, as opposed to what it inherited in turn — the `declares` set
    /// for the first link of the chain, which `@anc` only carries for the links above it.
    pub(crate) base_own: Vec<Decl>,
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
/// One type a visitor declares a `visit_*` method for, with the arguments that method's parameter
/// is written with (`Expr` + `<S>`). The arguments are carried rather than recomputed because an
/// extender knows an ancestor's parameters as a whole but not how each of its types divides them
/// up, and an override's signature has to match the trait's exactly.
#[derive(Clone)]
pub(crate) struct Decl {
    pub(crate) ident: Ident,
    pub(crate) args: TokenStream,
}

impl Parse for Decl {
    fn parse(input: ParseStream) -> Result<Self> {
        let ident: Ident = input.parse()?;
        let args = if input.peek(Token![<]) {
            let a: AngleBracketedGenericArguments = input.parse()?;
            quote!(#a)
        } else {
            TokenStream::new()
        };
        Ok(Decl { ident, args })
    }
}

fn parse_decls(ts: TokenStream) -> Result<Vec<Decl>> {
    let parser = |input: ParseStream| {
        let mut out = Vec::new();
        while !input.is_empty() {
            out.push(input.parse::<Decl>()?);
        }
        Ok(out)
    };
    parser.parse2(ts)
}

pub(crate) fn emit_decls(decls: &[Decl]) -> TokenStream {
    let parts: Vec<TokenStream> = decls
        .iter()
        .map(|d| {
            let (i, a) = (&d.ident, &d.args);
            quote!(#i #a)
        })
        .collect();
    quote!( #(#parts)* )
}

pub(crate) struct AncIn {
    pub(crate) path: Path,
    pub(crate) names: Vec<Ident>,
    /// The types whose `visit_*` methods this ancestor *declares* — its own `visitor!` targets, not
    /// what it in turn inherited. An override for one of them belongs in the impl of this
    /// ancestor's trait and nowhere else, so the closure driver needs to know where each inherited
    /// type's method lives.
    pub(crate) declares: Vec<Decl>,
}

/// Parse `@anc { @a { @p {PATH} @n {name…} @d {decl…} } … }`.
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
            let mut declares = Vec::new();
            while !content.is_empty() {
                let (name, inner) = parse_section(&content)?;
                match name.to_string().as_str() {
                    "p" => path = Some(syn::parse2(inner)?),
                    "n" => names = parse_idents(inner)?,
                    "d" => declares = parse_decls(inner)?,
                    other => {
                        return Err(Error::new(
                            name.span(),
                            format!("unknown @a section @{other}"),
                        ))
                    }
                }
            }
            let path: Path =
                path.ok_or_else(|| Error::new(Span::call_site(), "missing @p in @a"))?;
            out.push(AncIn {
                path,
                names,
                declares,
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
            let (path, names) = (&a.path, &a.names);
            let declares = emit_decls(&a.declares);
            quote! { @a { @p { #path } @n { #(#names)* } @d { #declares } } }
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
    own: &[Decl],
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
    let own = emit_decls(own);
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
                    @own { #own }
                }
            };
        }
        #[doc(hidden)]
        pub use #vmacro as __syan_visited;
    }
}

/// `impl_chain!`, the hidden helper that writes the ancestor impls a visitor would otherwise have
/// to spell out by hand.
///
/// The generated `Visit` has every ancestor's `Visit` as a supertrait, so a visitor type must
/// implement all of them even when it only overrides methods of this visitor's own types. Each of
/// those impls is usually empty — every method has a walking default — but Rust has no way to ask
/// for them, and their generic arity differs per ancestor. This writes them:
///
/// ```ignore
/// top::impl_chain!(top; MyPass);
/// ```
///
/// To act on an inherited node, hand the method to the macro rather than writing an impl of your
/// own, which would be a second impl of the same trait (E0119). The argument's type may be left
/// off — the macro knows it, and that is the one thing a caller would otherwise have to look up:
///
/// ```ignore
/// top::impl_chain! { top; MyPass;
///     fn visit_type(&mut self, i) { self.types += 1; base::visit_type(self, i); }
/// }
/// ```
///
/// Spelling the type out works too, for a signature you want to read at a glance.
///
/// `fn <name>;` with no body hands that ancestor back: nothing is generated for it, and you write
/// its impls yourself. That is the way out when a body wants ordinary tooling — rustfmt does not
/// reach inside a macro call:
///
/// ```ignore
/// top::impl_chain! { top; MyPass; fn visit_type; }
/// impl<S> base::Visit<S> for MyPass { fn visit_type(&mut self, i: &Type<S>) { .. } }
/// impl<S> base::VisitMut<S> for MyPass {}
/// ```
///
/// Everything is keyed by **method name**, which is unique across a chain — two ancestor *modules*
/// may share a last segment, so naming the ancestor would not be. One muncher per ancestor keeps
/// the methods that are its own and drops the rest, accumulating the two sides and the hand-back
/// markers separately; a last one rejects a name no ancestor declares, which would otherwise be
/// dropped in silence.
///
/// The module path is repeated as an argument because the body cannot name the ancestors any other
/// way: `$crate` here is *`syan`*, not the expanding crate — `visitor!` reaches the proc macro
/// through a `macro_rules` wrapper in `syan::visit`, and `$crate` resolves against that definition.
/// Taking the path instead makes the expansion position-independent: every ancestor is reached as
/// `<module>::__syan_base…`, the relay chain, which needs no crate name and no re-rooting.
///
/// Emitted even with no ancestors (where it expands to nothing) so that code generating a visitor
/// can call it unconditionally.
pub(crate) fn emit_impl_chain(nonce: &TokenStream, ancestors: &[Ancestor]) -> TokenStream {
    let name = Ident::new(&format!("__syan_impl_chain_{nonce}"), Span::call_site());
    let check = Ident::new(&format!("__syan_anc_{nonce}_check"), Span::call_site());
    let relay = Ident::new(BASE_RELAY, Span::call_site());
    let per: Vec<Ident> = (0..ancestors.len())
        .map(|i| Ident::new(&format!("__syan_anc_{nonce}_{i}"), Span::call_site()))
        .collect();
    // Re-exported beside `impl_chain`, under names without the nonce, so the body can reach them as
    // `<module>::__syan_anc_0!`. A bare name would do for a caller in this crate — a `#[macro_export]`
    // macro is in its own crate's textual scope — but not for one in another crate, and `$crate` here
    // means `syan`. The module path the caller passes is the only spelling that works everywhere.
    let per_pub: Vec<Ident> = (0..ancestors.len())
        .map(|i| Ident::new(&format!("__syan_anc_{i}"), Span::call_site()))
        .collect();
    let check_pub = Ident::new("__syan_anc_check", Span::call_site());
    // Per declared type: the two method names and the alias the argument's type is named by. The
    // alias, not the type's own path, because the body is expanded in whatever crate invokes
    // `impl_chain!` and a `crate::`-rooted path would resolve there.
    let decl_parts = |a: &Ancestor, hops: &[&Ident]| -> Vec<(Ident, Ident, TokenStream)> {
        a.declares
            .iter()
            .map(|d| {
                let alias = Ident::new(&format!("__syan_ty_{}", d.ident), Span::call_site());
                let args = &d.args;
                (
                    Side::SHARED.method(&d.ident),
                    Side::MUT.method(&d.ident),
                    quote!( $($vm)::+ #(:: #hops)* :: #alias #args ),
                )
            })
            .collect()
    };
    let munchers: Vec<TokenStream> = ancestors
        .iter()
        .zip(&per)
        .enumerate()
        .map(|(i, (a, m))| {
            let (g_params, g_use) = (&a.g_params, &a.g_use);
            let me = &per_pub[i];
            let hops = vec![&relay; i + 1];
            let parts = decl_parts(a, &hops);
            quote! {
                #[macro_export]
                #[doc(hidden)]
                macro_rules! #m {
                    #(for (sh, mt, ty) in &parts) {
                        // Argument type left off: supply it.
                        ($($vm:ident)::+; $ty:ty; [$($k:tt)*] [$($km:tt)*] [$($s:tt)*]
                         $(#[$at:meta])* fn #sh (&mut $slf:ident, $i:ident) $b:block
                         $($rest:tt)*) => {
                            $($vm)::+ ::#me!($($vm)::+; $ty;
                                [$($k)* $(#[$at])* fn #sh (&mut $slf, $i: & #ty) $b]
                                [$($km)*] [$($s)*] $($rest)*);
                        };
                        ($($vm:ident)::+; $ty:ty; [$($k:tt)*] [$($km:tt)*] [$($s:tt)*]
                         $(#[$at:meta])* fn #mt (&mut $slf:ident, $i:ident) $b:block
                         $($rest:tt)*) => {
                            $($vm)::+ ::#me!($($vm)::+; $ty; [$($k)*]
                                [$($km)* $(#[$at])* fn #mt (&mut $slf, $i: &mut #ty) $b]
                                [$($s)*] $($rest)*);
                        };
                        // Written out in full.
                        ($($vm:ident)::+; $ty:ty; [$($k:tt)*] [$($km:tt)*] [$($s:tt)*]
                         $(#[$at:meta])* fn #sh ($($sig:tt)*) $(-> $rt:ty)? $b:block $($rest:tt)*) => {
                            $($vm)::+ ::#me!($($vm)::+; $ty;
                                [$($k)* $(#[$at])* fn #sh ($($sig)*) $(-> $rt)? $b]
                                [$($km)*] [$($s)*] $($rest)*);
                        };
                        ($($vm:ident)::+; $ty:ty; [$($k:tt)*] [$($km:tt)*] [$($s:tt)*]
                         $(#[$at:meta])* fn #mt ($($sig:tt)*) $(-> $rt:ty)? $b:block $($rest:tt)*) => {
                            $($vm)::+ ::#me!($($vm)::+; $ty; [$($k)*]
                                [$($km)* $(#[$at])* fn #mt ($($sig)*) $(-> $rt)? $b]
                                [$($s)*] $($rest)*);
                        };
                        // Handed back: this ancestor is the caller's to write.
                        ($($vm:ident)::+; $ty:ty; [$($k:tt)*] [$($km:tt)*] [$($s:tt)*]
                         fn #sh ; $($rest:tt)*) => {
                            $($vm)::+ ::#me!($($vm)::+; $ty; [$($k)*] [$($km)*] [$($s)* mine]
                                $($rest)*);
                        };
                        ($($vm:ident)::+; $ty:ty; [$($k:tt)*] [$($km:tt)*] [$($s:tt)*]
                         fn #mt ; $($rest:tt)*) => {
                            $($vm)::+ ::#me!($($vm)::+; $ty; [$($k)*] [$($km)*] [$($s)* mine]
                                $($rest)*);
                        };
                    }
                    // Someone else's.
                    ($($vm:ident)::+; $ty:ty; [$($k:tt)*] [$($km:tt)*] [$($s:tt)*]
                     $(#[$at:meta])* fn $other:ident ($($sig:tt)*) $(-> $rt:ty)? $b:block
                     $($rest:tt)*) => {
                        $($vm)::+ ::#me!($($vm)::+; $ty; [$($k)*] [$($km)*] [$($s)*] $($rest)*);
                    };
                    ($($vm:ident)::+; $ty:ty; [$($k:tt)*] [$($km:tt)*] [$($s:tt)*]
                     fn $other:ident ; $($rest:tt)*) => {
                        $($vm)::+ ::#me!($($vm)::+; $ty; [$($k)*] [$($km)*] [$($s)*] $($rest)*);
                    };
                    ($($vm:ident)::+; $ty:ty; [$($k:tt)*] [$($km:tt)*] []) => {
                        impl< #(#g_params,)* > $($vm)::+ #(:: #hops)* ::Visit #g_use for $ty {
                            $($k)*
                        }
                        impl< #(#g_params,)* > $($vm)::+ #(:: #hops)* ::VisitMut #g_use for $ty {
                            $($km)*
                        }
                    };
                    ($($vm:ident)::+; $ty:ty; [$($k:tt)*] [$($km:tt)*] [$($s:tt)+]) => {};
                }
            }
        })
        .collect();
    let all: Vec<Ident> = ancestors
        .iter()
        .flat_map(|a| a.declares.iter())
        .flat_map(|d| Side::both().map(|side| side.method(&d.ident)))
        .collect();
    quote! {
        #(#munchers)*

        // A method no ancestor declares would be dropped by every muncher and quietly do nothing.
        #[macro_export]
        #[doc(hidden)]
        macro_rules! #check {
            #(
                ($($vm:ident)::+; $(#[$at:meta])* fn #all ($($sig:tt)*) $(-> $rt:ty)? $b:block
                 $($rest:tt)*) => {
                    $($vm)::+ ::#check_pub!($($vm)::+; $($rest)*);
                };
                ($($vm:ident)::+; fn #all ; $($rest:tt)*) => {
                    $($vm)::+ ::#check_pub!($($vm)::+; $($rest)*);
                };
            )*
            ($($vm:ident)::+; $(#[$at:meta])* fn $other:ident $($rest:tt)*) => {
                ::core::compile_error!(::core::concat!(
                    "`", ::core::stringify!($other),
                    "` is not a method of any visitor this one extends; a method of its own trait                      belongs in your `impl Visit`, not in `impl_chain!`"
                ));
            };
            ($($vm:ident)::+;) => {};
        }

        #[macro_export]
        #[doc(hidden)]
        macro_rules! #name {
            ($($vm:ident)::+; $ty:ty) => { #name!($($vm)::+; $ty;); };
            ($($vm:ident)::+; $ty:ty; $($body:tt)*) => {
                $($vm)::+ ::#check_pub!($($vm)::+; $($body)*);
                #( $($vm)::+ ::#per_pub!($($vm)::+; $ty; [] [] [] $($body)*); )*
            };
        }
        #(
            #[doc(hidden)]
            pub use #per as #per_pub;
        )*
        #[doc(hidden)]
        pub use #check as #check_pub;
        #[doc(hidden)]
        pub use #name as impl_chain;
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

/// The name each visitor module re-exports its direct base under, so an ancestor is reachable
/// through the chain rather than by its own path. A base module that is private, or that a
/// downstream crate cannot otherwise name, is still reachable as `<base>::__syan_base`.
pub(crate) const BASE_RELAY: &str = "__syan_base";

/// The path to the ancestor `depth` links above `base`, walked through the `__syan_base` re-exports:
/// `depth` 0 is `base` itself, 1 is its base, and so on.
///
/// This is what lets an ancestor chain cross a crate boundary without any path arithmetic: every
/// link is named relative to the one below it, so no segment of it has to be nameable from the
/// extending crate. (Contrast `requalify_ancestor`, which rewrites a recorded path and therefore
/// needs every module on it to be public.)
pub(crate) fn base_relay(base: &Path, depth: usize) -> Path {
    let mut p = base.clone();
    for _ in 0..depth {
        p.segments.push(PathSegment {
            ident: Ident::new(BASE_RELAY, Span::call_site()),
            arguments: PathArguments::None,
        });
    }
    p
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
        let mut base_own = Vec::new();
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
                "inherited" | "inh" => inherited.extend(parse_subentries(content)?),
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
                "own" => base_own = parse_decls(content)?,
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
            base_own,
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
    inherited: &[SubEntry],
    base_generics: &[GenericParam],
    anc: &TokenStream, // emit_ancestors(&base_ancestors) or quote!()
    own: &TokenStream,
    fetching: &TokenStream,
    done: &TokenStream, // emit_done(&done) or quote!()
    rest: &[Path],
) -> TokenStream {
    quote! {
        @base { #base }
        @build { #build }
        @nonce { #nonce }
        @visited { #(#visited),* }
        @inherited { #(for e in inherited), { #{&e.path} as #{&e.key} } }
        @baseg { #(#base_generics),* }
        @anc { #anc }
        @own { #own }
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
            base_own,
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
            &emit_decls(base_own),
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

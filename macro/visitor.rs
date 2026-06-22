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
    // in the caller's path context.
    let make_state = |rest: &[Path]| {
        quote! {
            @base { #base_tokens }
            @build { #build }
            @nonce { #nonce }
            @visited { #(#all_types),* }
            @inherited { }
            @done { }
            @rest { #(#rest),* }
        }
    };

    match &args.base {
        // With a base: first fetch the base module's visited-type list, then fetch all types.
        Some(base) => {
            let state = make_state(all_types);
            quote! {
                #base::__syan_visited ! { @visited #build { #state } }
            }
        }
        // No base: pop the first type now (so `rest` carries the remainder).
        None => {
            let first = &args.types[0];
            let state = make_state(&args.types[1..]);
            quote! {
                #first ! { @ast #build { #state } }
            }
        }
    }
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
    done: Vec<Item>,
    rest: Vec<Path>,
    just: Option<Item>,
}

/// Parse one `@<name> { .. }` section, returning the name and the braced content as tokens.
fn parse_section(input: ParseStream) -> Result<(Ident, TokenStream)> {
    input.parse::<Token![@]>()?;
    let name: Ident = input.parse()?;
    let content;
    braced!(content in input);
    Ok((name, content.parse()?))
}

impl Parse for BuildInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut base = None;
        let mut build = None;
        let mut nonce = TokenStream::new();
        let mut visited = Vec::new();
        let mut inherited = Vec::new();
        let mut done = Vec::new();
        let mut rest = Vec::new();
        let mut just = None;

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
                "done" => done = parse_items(content)?,
                "rest" => {
                    let paths =
                        Punctuated::<Path, Token![,]>::parse_terminated.parse2(content)?;
                    rest = paths.into_iter().collect();
                }
                "ast" => just = Some(syn::parse2(content)?),
                // Carried in the metadata ping-pong; consumed by drilling in a later stage.
                "subast" | "fetching" | "subdone" => {}
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
            done,
            rest,
            just,
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

fn parse_items(ts: TokenStream) -> Result<Vec<Item>> {
    let parser = |input: ParseStream| {
        let mut out = Vec::new();
        while !input.is_empty() {
            out.push(input.parse::<Item>()?);
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
    if let Some(item) = st.just.take() {
        st.done.push(item);
    }

    if !st.rest.is_empty() {
        let next = st.rest.remove(0);
        let BuildInput {
            base,
            build,
            nonce,
            visited,
            inherited,
            done,
            rest,
            ..
        } = &st;
        let base_tokens: TokenStream = match base {
            Some(p) => quote!(#p),
            None => quote!(),
        };
        return quote! {
            #next ! {
                @ast #build {
                    @base { #base_tokens }
                    @build { #build }
                    @nonce { #nonce }
                    @visited { #(#visited),* }
                    @inherited { #(#inherited)* }
                    @done { #(#done)* }
                    @rest { #(#rest),* }
                }
            }
        };
    }

    generate_module(&st)
}

// ---------------------------------------------------------------------------
// Module generation
// ---------------------------------------------------------------------------

fn strip_ref(ty: &Type) -> &Type {
    match ty {
        Type::Reference(r) => strip_ref(&r.elem),
        other => other,
    }
}

fn type_head(ty: &Type) -> Option<&PathSegment> {
    match strip_ref(ty) {
        Type::Path(TypePath { path, .. }) => path.segments.last(),
        _ => None,
    }
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

/// Head identifier of a field type, peeling `Box` (so `Box<Stmt<S>>` -> `Stmt`). `None` => not a
/// path type. The caller treats a head that isn't a visited type as a leaf.
fn classify(ty: &Type) -> Option<Ident> {
    let seg = type_head(ty)?;
    if seg.ident == "Box" {
        return classify(first_ty_arg(seg)?);
    }
    Some(seg.ident.clone())
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
    (2..=max_arity)
        .map(|n| {
            let fs: Vec<Ident> = (0..n)
                .map(|i| Ident::new(&format!("__F{i}"), Span::call_site()))
                .collect();
            let ts: Vec<Ident> = (0..n)
                .map(|i| Ident::new(&format!("__T{i}"), Span::call_site()))
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

fn emit_visit(head: &Ident, binding: TokenStream, mutable: bool) -> TokenStream {
    let m = method_ident_m(head, mutable);
    quote!( this.#m(#binding); )
}

/// Build a pattern + visit statements for a set of fields.
fn build_fields(
    fields: &Fields,
    visited: &HashSet<String>,
    mutable: bool,
) -> (TokenStream, TokenStream) {
    let mut stmts = Vec::new();
    match fields {
        Fields::Named(named) => {
            let mut binds = Vec::new();
            for f in &named.named {
                let name = f.ident.clone().unwrap();
                if let Some(head) = classify(&f.ty) {
                    if visited.contains(&head.to_string()) {
                        binds.push(quote!(#name));
                        stmts.push(emit_visit(&head, quote!(#name), mutable));
                    }
                }
            }
            (quote!( { #(#binds,)* .. } ), quote!( #(#stmts)* ))
        }
        Fields::Unnamed(unnamed) => {
            let mut pats = Vec::new();
            for (idx, f) in unnamed.unnamed.iter().enumerate() {
                let visit = classify(&f.ty).filter(|h| visited.contains(&h.to_string()));
                if let Some(head) = visit {
                    let b = Ident::new(&format!("__f{idx}"), Span::call_site());
                    pats.push(quote!(#b));
                    stmts.push(emit_visit(&head, quote!(#b), mutable));
                } else {
                    pats.push(quote!(_));
                }
            }
            (quote!( ( #(#pats),* ) ), quote!( #(#stmts)* ))
        }
        Fields::Unit => (quote!(), quote!()),
    }
}

/// Body of the free `visit_*` traversal function for one item. `ty_path` is the full path the type
/// is referenced by (so match scrutinees are portable across crates).
fn build_body(
    item: &Item,
    visited: &HashSet<String>,
    mutable: bool,
    ty_path: &TokenStream,
) -> TokenStream {
    match item {
        Item::Enum(e) => {
            let arms = e.variants.iter().map(|v| {
                let vident = &v.ident;
                let (pat, stmts) = build_fields(&v.fields, visited, mutable);
                quote!( #ty_path::#vident #pat => { #stmts } )
            });
            quote!( match i { #(#arms)* } )
        }
        Item::Struct(s) => {
            let (pat, stmts) = build_fields(&s.fields, visited, mutable);
            match &s.fields {
                Fields::Unit => quote!(),
                _ => quote!( let #ty_path #pat = i; #stmts ),
            }
        }
        _ => quote!(),
    }
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
fn gen_side(
    mutable: bool,
    vtypes: &[VType],
    g_params: &[GenericParam],
    g_args: &[TokenStream],
    g_def: &TokenStream,
    g_use: &TokenStream,
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
                    pub fn #visit_method< #(#extra,)* __T >(
                        #recv,
                        visitor: impl #into_vis_tr< #(#g_args,)* __T >,
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
        pub trait #visit_tr #g_def #(if let Some(b) = base) { : #b::#visit_tr #g_use } {
            #(for s in &sides) {
                fn #{&s.method}(&mut self, i: #amp #{&s.ty}) {
                    #{&s.method}(self, i)
                }
            }
        }

        impl< #(#g_params,)* __V: #visit_tr #g_use > #visit_tr #g_use for &mut __V {
            #(for s in &sides) {
                fn #{&s.method}(&mut self, i: #amp #{&s.ty}) {
                    <__V as #visit_tr #g_use>::#{&s.method}(self, i)
                }
            }
        }

        #(for s in &sides) {
            pub fn #{&s.method}< #(#g_params,)* __V: #visit_tr #g_use + ?Sized >(
                this: &mut __V,
                i: #amp #{&s.ty},
            ) {
                #{&s.body}
            }
        }

        pub trait #into_vis_tr< #(#g_params,)* __T > {
            fn #into_vis_fn(self) -> impl #visit_tr #g_use;
        }
        impl< #(#g_params,)* __V: #visit_tr #g_use > #into_vis_tr< #(#g_args,)* () > for __V {
            fn #into_vis_fn(self) -> impl #visit_tr #g_use { self }
        }

        // --- closures: shallow Hook + single-pass Driver ---------------------------------
        pub trait #hook_tr #g_def {
            #(for s in &sides) {
                fn #{&s.hook}(&mut self, i: #amp #{&s.ty}) { let _ = i; }
            }
        }
        pub trait #into_hook_tr< #(#g_params,)* __T > {
            fn #into_hook_fn(self) -> impl #hook_tr #g_use;
        }

        pub struct #driver<__H>(pub __H);
        impl< #(#g_params,)* __H: #hook_tr #g_use > #visit_tr #g_use for #driver<__H> {
            #(for s in &sides) {
                fn #{&s.method}(&mut self, i: #amp #{&s.ty}) {
                    self.0.#{&s.hook}(i);
                    #{&s.method}(self, i);
                }
            }
        }
        // The new trait extends the base, so Driver must satisfy the base too (via base defaults).
        #(if let Some(b) = base) {
            impl< #(#g_params,)* __H: #hook_tr #g_use > #b::#visit_tr #g_use for #driver<__H> {}
        }

        #(for s in &sides) {
            pub struct #{&s.hook_struct}<__F>(pub __F);
            impl< #(#g_params,)* __F: ::core::ops::FnMut( #amp #{&s.ty} ) >
                #hook_tr #g_use for #{&s.hook_struct}<__F>
            {
                fn #{&s.hook}(&mut self, i: #amp #{&s.ty}) { (self.0)(i); }
            }
            impl< #(#g_params,)* __F: ::core::ops::FnMut( #amp #{&s.ty} ) >
                #into_hook_tr< #(#g_args,)* #{&s.ty} > for __F
            {
                fn #into_hook_fn(self) -> impl #hook_tr #g_use { #{&s.hook_struct}(self) }
            }
            impl< #(#g_params,)* __F: ::core::ops::FnMut( #amp #{&s.ty} ) >
                #into_vis_tr< #(#g_args,)* #{&s.ty} > for __F
            {
                fn #into_vis_fn(self) -> impl #visit_tr #g_use { #driver(#{&s.hook_struct}(self)) }
            }
        }

        // --- multiple closures: Chain combinator + tuple impls ---------------------------
        pub struct #chain<__A, __B>(pub __A, pub __B);
        impl< #(#g_params,)* __A: #hook_tr #g_use, __B: #hook_tr #g_use >
            #hook_tr #g_use for #chain<__A, __B>
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
    // Map each visited type's last-segment ident -> the full path the user wrote, so the generated
    // module names the visited types by that path (portable: no import needed for absolute paths).
    let path_of: HashMap<String, &Path> = st
        .visited
        .iter()
        .map(|p| (last_ident(p).to_string(), p))
        .collect();
    let visited: HashSet<String> = path_of.keys().cloned().collect();
    // Fields whose head is any of these recurse via a `visit_*` method (new ones generated here,
    // inherited ones provided by the base trait).
    let visitable: HashSet<String> = visited
        .iter()
        .cloned()
        .chain(st.inherited.iter().map(|i| i.to_string()))
        .collect();

    // Items that get visitor methods (named in #[visitor(..)]); inherited types are not regenerated.
    let targets: Vec<&Item> = st
        .done
        .iter()
        .filter(|it| item_ident(it).map_or(false, |id| visited.contains(&id.to_string())))
        .collect();
    if targets.is_empty() {
        abort!(Span::call_site(), "no AST definitions resolved for the visitor");
    }

    // The visitor trait is parameterized by the *union* of every visited type's generic params
    // (by name, first declaration wins); each type is then referenced with its own subset. This
    // lets one visitor span e.g. `Expr<S, Tokens>` and `BinOp<S>`.
    let mut seen = HashSet::new();
    let mut g_params: Vec<GenericParam> = Vec::new();
    for it in &targets {
        for p in gparams(item_generics(it).unwrap()) {
            if seen.insert(param_name(&p)) {
                g_params.push(p);
            }
        }
    }
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

    let vtypes: Vec<VType> = targets
        .iter()
        .map(|it| {
            let ident = item_ident(it).unwrap().clone();
            let own_params = gparams(item_generics(it).unwrap());
            let own = gargs(item_generics(it).unwrap());
            let own_use = if own.is_empty() {
                quote!()
            } else {
                quote!( < #(#own),* > )
            };
            // Full path for type references; falls back to the bare ident if unmapped.
            let path = path_of
                .get(&ident.to_string())
                .map(|p| quote!(#p))
                .unwrap_or_else(|| quote!(#ident));
            let body = build_body(it, &visitable, false, &path);
            let body_mut = build_body(it, &visitable, true, &path);
            VType {
                ident,
                path,
                own_params,
                own_use,
                body,
                body_mut,
            }
        })
        .collect();

    let shared = gen_side(false, &vtypes, &g_params, &g_args, &g_def, &g_use, &st.base);
    let mutable = gen_side(true, &vtypes, &g_params, &g_args, &g_def, &g_use, &st.base);

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
                $cb ! { $($pre)* @inh { #(#all_visible)* } }
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

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

pub fn visitor(attr: TokenStream, item: TokenStream, nonce: u64) -> TokenStream {
    let args: VisitorArgs = match syn::parse2(attr) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error(),
    };
    let module: ItemMod = match syn::parse2(item) {
        Ok(m) => m,
        Err(e) => return e.to_compile_error(),
    };
    if args.types.is_empty() {
        abort!(module.ident, "#[visitor(..)] needs at least one AST type");
    }

    // syan path: honor #[syan(path)] on the module, else `::syan`.
    let syan: Path = {
        use crate::attribute::FindAttribute;
        module.attrs.get_syan()
    };
    let build: Path = parse_quote!(#syan::_imp::syan_macro::__visitor_build);

    let vis = &module.vis;
    let ident = &module.ident;
    let base_tokens: TokenStream = match &args.base {
        Some(p) => quote!(#p),
        None => quote!(),
    };
    let nonce = nonce.to_string();
    let nonce: TokenStream = nonce.parse().unwrap();
    let all_types = &args.types;

    // `@visited` carries the *full paths* as written, so the generated module can name the visited
    // types portably (no import needed when an absolute/crate path is given).
    let make_state = |rest: &[Path]| {
        quote! {
            @vis { #vis }
            @ident { #ident }
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
    vis: Visibility,
    ident: Ident,
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
        let mut vis = Visibility::Inherited;
        let mut ident = None;
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
                "vis" => vis = syn::parse2(content)?,
                "ident" => ident = Some(syn::parse2(content)?),
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
                other => {
                    return Err(Error::new(name.span(), format!("unknown section @{other}")))
                }
            }
        }

        Ok(BuildInput {
            vis,
            ident: ident.ok_or_else(|| Error::new(Span::call_site(), "missing @ident"))?,
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
            vis,
            ident,
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
                    @vis { #vis }
                    @ident { #ident }
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

#[derive(Clone, Copy)]
enum Cont {
    Direct,
    Vec,
    Option,
}

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

/// Identifier at the head of a type, peeling `Box` (so `Box<Stmt<S>>` -> `Stmt`).
fn unbox_head(ty: &Type) -> Option<Ident> {
    let seg = type_head(ty)?;
    if seg.ident == "Box" {
        return unbox_head(first_ty_arg(seg)?);
    }
    Some(seg.ident.clone())
}

fn inner_is_boxed(ty: &Type) -> bool {
    type_head(ty).map_or(false, |s| s.ident == "Box")
}

/// Classify a field type into `(container, inner-AST-head, route_to_method)`. `None` => leaf.
///
/// `route_to_method` is true only for plain `Vec<X>` / `Option<X>` with a non-boxed inner, which
/// can be passed to the `visit_*_seq(&[X])` / `visit_*_opt(&Option<X>)` hooks directly. Boxed
/// inners and `VecDeque` fall back to an inline loop (deref-coercion handles the element type).
fn classify(ty: &Type) -> Option<(Cont, Ident, bool)> {
    let seg = type_head(ty)?;
    match seg.ident.to_string().as_str() {
        "Box" => classify(first_ty_arg(seg)?),
        "Vec" => {
            let arg = first_ty_arg(seg)?;
            Some((Cont::Vec, unbox_head(arg)?, !inner_is_boxed(arg)))
        }
        "VecDeque" => {
            let arg = first_ty_arg(seg)?;
            Some((Cont::Vec, unbox_head(arg)?, false))
        }
        "Option" => {
            let arg = first_ty_arg(seg)?;
            Some((Cont::Option, unbox_head(arg)?, !inner_is_boxed(arg)))
        }
        _ => Some((Cont::Direct, seg.ident.clone(), false)),
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

fn seq_ident_m(head: &Ident, mutable: bool) -> Ident {
    Ident::new(
        &format!("visit_{}_seq{}", to_snake(head), mt(mutable)),
        Span::call_site(),
    )
}

fn opt_ident_m(head: &Ident, mutable: bool) -> Ident {
    Ident::new(
        &format!("visit_{}_opt{}", to_snake(head), mt(mutable)),
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

fn emit_visit(
    cont: Cont,
    head: &Ident,
    method_ok: bool,
    binding: TokenStream,
    mutable: bool,
) -> TokenStream {
    let m = method_ident_m(head, mutable);
    match cont {
        Cont::Direct => quote!( this.#m(#binding); ),
        Cont::Vec if method_ok => {
            let seq = seq_ident_m(head, mutable);
            quote!( this.#seq(#binding); )
        }
        Cont::Vec => quote!( for __x in #binding { this.#m(__x); } ),
        Cont::Option if method_ok => {
            let opt = opt_ident_m(head, mutable);
            quote!( this.#opt(#binding); )
        }
        Cont::Option => {
            quote!( if let ::core::option::Option::Some(__x) = #binding { this.#m(__x); } )
        }
    }
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
                if let Some((cont, head, ok)) = classify(&f.ty) {
                    if visited.contains(&head.to_string()) {
                        binds.push(quote!(#name));
                        stmts.push(emit_visit(cont, &head, ok, quote!(#name), mutable));
                    }
                }
            }
            (quote!( { #(#binds,)* .. } ), quote!( #(#stmts)* ))
        }
        Fields::Unnamed(unnamed) => {
            let mut pats = Vec::new();
            for (idx, f) in unnamed.unnamed.iter().enumerate() {
                let visit = classify(&f.ty).filter(|(_, h, _)| visited.contains(&h.to_string()));
                if let Some((cont, head, ok)) = visit {
                    let b = Ident::new(&format!("__f{idx}"), Span::call_site());
                    pats.push(quote!(#b));
                    stmts.push(emit_visit(cont, &head, ok, quote!(#b), mutable));
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
/// own use-side generics (e.g. `<S>` or `<S, Tokens>`), and its shared-ref and `&mut` bodies.
struct VType {
    ident: Ident,
    path: TokenStream,
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
    let visitable_tr = id(&format!("Visitable{suffix}"));
    let into_vis_fn = id(&format!("into_visitor{}", mt(mutable)));
    let into_hook_fn = id(&format!("into_hook{}", mt(mutable)));
    let visit_method = id(&format!("visit{}", mt(mutable)));
    let amp = if mutable { quote!(&mut) } else { quote!(&) };
    let recv = if mutable { quote!(&mut self) } else { quote!(&self) };
    let self_ret = if mutable { quote!(&mut Self) } else { quote!(&Self) };
    let seq_iter = if mutable {
        quote!(seq.iter_mut())
    } else {
        quote!(seq)
    };

    struct S {
        ty: TokenStream,
        method: Ident,
        seq: Ident,
        opt: Ident,
        hook: Ident,
        hook_struct: Ident,
        seq_ty: TokenStream,
        opt_ty: TokenStream,
        body: TokenStream,
    }
    let sides: Vec<S> = vtypes
        .iter()
        .map(|t| {
            let ident = t.ident.clone();
            let own = &t.own_use;
            let path = &t.path;
            let ty = quote!( #path #own );
            let seq_ty = if mutable {
                quote!( &mut Vec< #ty > )
            } else {
                quote!( &[ #ty ] )
            };
            let opt_ty = if mutable {
                quote!( &mut ::core::option::Option< #ty > )
            } else {
                quote!( &::core::option::Option< #ty > )
            };
            S {
                ty,
                method: method_ident_m(&ident, mutable),
                seq: seq_ident_m(&ident, mutable),
                opt: opt_ident_m(&ident, mutable),
                hook: Ident::new(
                    &format!("hook_{}{}", to_snake(&ident), mt(mutable)),
                    Span::call_site(),
                ),
                hook_struct: Ident::new(&format!("{ident}Hook{suffix}"), Span::call_site()),
                seq_ty,
                opt_ty,
                body: if mutable { t.body_mut.clone() } else { t.body.clone() },
            }
        })
        .collect();

    let tup = tuple_impls(8, g_params, g_args, g_use, mutable);

    quote! {
        pub trait #visit_tr #g_def #(if let Some(b) = base) { : #b::#visit_tr #g_use } {
            #(for s in &sides) {
                fn #{&s.method}(&mut self, i: #amp #{&s.ty}) {
                    #{&s.method}(self, i)
                }
                fn #{&s.seq}(&mut self, seq: #{&s.seq_ty}) {
                    for __x in #seq_iter { self.#{&s.method}(__x); }
                }
                fn #{&s.opt}(&mut self, opt: #{&s.opt_ty}) {
                    if let ::core::option::Option::Some(__x) = opt { self.#{&s.method}(__x); }
                }
            }
        }

        impl< #(#g_params,)* __V: #visit_tr #g_use > #visit_tr #g_use for &mut __V {
            #(for s in &sides) {
                fn #{&s.method}(&mut self, i: #amp #{&s.ty}) {
                    <__V as #visit_tr #g_use>::#{&s.method}(self, i)
                }
                fn #{&s.seq}(&mut self, seq: #{&s.seq_ty}) {
                    <__V as #visit_tr #g_use>::#{&s.seq}(self, seq)
                }
                fn #{&s.opt}(&mut self, opt: #{&s.opt_ty}) {
                    <__V as #visit_tr #g_use>::#{&s.opt}(self, opt)
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

        pub trait #visitable_tr #g_def {
            fn #visit_method<__T>(#recv, visitor: impl #into_vis_tr< #(#g_args,)* __T >) -> #self_ret;
        }
        #(for s in &sides) {
            impl #g_def #visitable_tr #g_use for #{&s.ty} {
                fn #visit_method<__T>(#recv, visitor: impl #into_vis_tr< #(#g_args,)* __T >) -> #self_ret {
                    let mut visitor = visitor.#into_vis_fn();
                    visitor.#{&s.method}(self);
                    self
                }
            }
        }
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
        abort!(st.ident, "no AST definitions resolved for the visitor");
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
                own_use,
                body,
                body_mut,
            }
        })
        .collect();

    let shared = gen_side(false, &vtypes, &g_params, &g_args, &g_def, &g_use, &st.base);
    let mutable = gen_side(true, &vtypes, &g_params, &g_args, &g_def, &g_use, &st.base);

    let vis = &st.vis;
    let mod_ident = &st.ident;
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

    quote! {
        #[macro_export]
        #[doc(hidden)]
        macro_rules! #vmacro {
            (@visited $cb:path { $($pre:tt)* }) => {
                $cb ! { $($pre)* @inh { #(#all_visible)* } }
            };
        }

        #[allow(non_snake_case, unused_variables, unused_mut, dead_code, clippy::all)]
        #vis mod #mod_ident {
            use super::*;
            #(if let Some(b) = base) {
                #[allow(unused_imports)]
                use #b::{Visit as _, VisitMut as _};
            }

            #[doc(hidden)]
            pub use #vmacro as __syan_visited;

            #shared
            #mutable
        }
    }
}

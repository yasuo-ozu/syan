use crate::ast::to_snake;
use proc_macro2::{Span, TokenStream};
use proc_macro_error::abort;
use std::collections::HashSet;
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

pub fn visitor(attr: TokenStream, item: TokenStream) -> TokenStream {
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
    let visited_idents: Vec<&Ident> = args.types.iter().map(last_ident).collect();
    let first = &args.types[0];
    let rest = &args.types[1..];

    quote! {
        #first ! {
            @ast #build {
                @vis { #vis }
                @ident { #ident }
                @base { #base_tokens }
                @build { #build }
                @visited { #(#visited_idents)* }
                @done { }
                @rest { #(#rest),* }
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
    visited: Vec<Ident>,
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
        let mut visited = Vec::new();
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
                "visited" => visited = parse_idents(content)?,
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
            visited,
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
            visited,
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
                    @visited { #(#visited)* }
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

fn method_ident(head: &Ident) -> Ident {
    Ident::new(&format!("visit_{}", to_snake(head)), Span::call_site())
}

fn seq_ident(head: &Ident) -> Ident {
    Ident::new(&format!("visit_{}_seq", to_snake(head)), Span::call_site())
}

fn opt_ident(head: &Ident) -> Ident {
    Ident::new(&format!("visit_{}_opt", to_snake(head)), Span::call_site())
}

/// Right-nested `Chain(self.0.into_hook(), Chain(self.1.into_hook(), ...))` over tuple members.
fn build_chain(members: &[Index]) -> TokenStream {
    let m = &members[0];
    if members.len() == 1 {
        quote!(self.#m.into_hook())
    } else {
        let rest = build_chain(&members[1..]);
        quote!(Chain(self.#m.into_hook(), #rest))
    }
}

/// `IntoVisitor` impls for tuples of closures, arity 2..=`max_arity`.
fn tuple_impls(
    max_arity: usize,
    g_params: &[GenericParam],
    g_args: &[TokenStream],
    g_use: &TokenStream,
) -> Vec<TokenStream> {
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
                .map(|(f, t)| quote!(#f: IntoHook< #(#g_args,)* #t >))
                .collect();
            let chain = build_chain(&members);
            quote! {
                impl< #(#g_params,)* #(#fs,)* #(#ts,)* >
                    IntoVisitor< #(#g_args,)* ( #(#ts,)* ) > for ( #(#fs,)* )
                where #(#wheres,)*
                {
                    fn into_visitor(self) -> impl Visit #g_use {
                        Driver( #chain )
                    }
                }
            }
        })
        .collect()
}

fn emit_visit(cont: Cont, head: &Ident, method_ok: bool, binding: TokenStream) -> TokenStream {
    let m = method_ident(head);
    match cont {
        Cont::Direct => quote!( this.#m(#binding); ),
        Cont::Vec if method_ok => {
            let seq = seq_ident(head);
            quote!( this.#seq(#binding); )
        }
        Cont::Vec => quote!( for __x in #binding { this.#m(__x); } ),
        Cont::Option if method_ok => {
            let opt = opt_ident(head);
            quote!( this.#opt(#binding); )
        }
        Cont::Option => {
            quote!( if let ::core::option::Option::Some(__x) = #binding { this.#m(__x); } )
        }
    }
}

/// Build a pattern + visit statements for a set of fields.
fn build_fields(fields: &Fields, visited: &HashSet<String>) -> (TokenStream, TokenStream) {
    let mut stmts = Vec::new();
    match fields {
        Fields::Named(named) => {
            let mut binds = Vec::new();
            for f in &named.named {
                let name = f.ident.clone().unwrap();
                if let Some((cont, head, ok)) = classify(&f.ty) {
                    if visited.contains(&head.to_string()) {
                        binds.push(quote!(#name));
                        stmts.push(emit_visit(cont, &head, ok, quote!(#name)));
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
                    stmts.push(emit_visit(cont, &head, ok, quote!(#b)));
                } else {
                    pats.push(quote!(_));
                }
            }
            (quote!( ( #(#pats),* ) ), quote!( #(#stmts)* ))
        }
        Fields::Unit => (quote!(), quote!()),
    }
}

/// Body of the free `visit_*` traversal function for one item.
fn build_body(item: &Item, visited: &HashSet<String>) -> TokenStream {
    match item {
        Item::Enum(e) => {
            let ident = &e.ident;
            let arms = e.variants.iter().map(|v| {
                let vident = &v.ident;
                let (pat, stmts) = build_fields(&v.fields, visited);
                quote!( #ident::#vident #pat => { #stmts } )
            });
            quote!( match i { #(#arms)* } )
        }
        Item::Struct(s) => {
            let ident = &s.ident;
            let (pat, stmts) = build_fields(&s.fields, visited);
            match &s.fields {
                Fields::Unit => quote!(),
                _ => quote!( let #ident #pat = i; #stmts ),
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

fn generate_module(st: &BuildInput) -> TokenStream {
    let visited: HashSet<String> = st.visited.iter().map(|i| i.to_string()).collect();

    // Items that get visitor methods (named in #[visitor(..)]).
    let targets: Vec<&Item> = st
        .done
        .iter()
        .filter(|it| item_ident(it).map_or(false, |id| visited.contains(&id.to_string())))
        .collect();
    if targets.is_empty() {
        abort!(st.ident, "no AST definitions resolved for the visitor");
    }

    // Shared generics taken from the first target; require all to match by ident.
    let base_generics = item_generics(targets[0]).unwrap().clone();
    let base_idents: Vec<String> = gargs(&base_generics).iter().map(|t| t.to_string()).collect();
    for t in &targets[1..] {
        let g = item_generics(t).unwrap();
        let idents: Vec<String> = gargs(g).iter().map(|t| t.to_string()).collect();
        if idents != base_idents {
            abort!(
                item_ident(t).unwrap(),
                "all visited types must share identical generic parameters (`{}` vs `{}`)",
                idents.join(", "),
                base_idents.join(", ")
            );
        }
    }
    let g_params = gparams(&base_generics);
    let g_args = gargs(&base_generics);
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

    // Per-target precomputed pieces.
    struct VType {
        ident: Ident,
        method: Ident,
        seq: Ident,
        opt: Ident,
        hook: Ident,
        hook_struct: Ident,
        body: TokenStream,
    }
    let vtypes: Vec<VType> = targets
        .iter()
        .map(|it| {
            let ident = item_ident(it).unwrap().clone();
            VType {
                method: method_ident(&ident),
                seq: seq_ident(&ident),
                opt: opt_ident(&ident),
                hook: Ident::new(&format!("hook_{}", to_snake(&ident)), Span::call_site()),
                hook_struct: Ident::new(&format!("{ident}Hook"), Span::call_site()),
                body: build_body(it, &visited),
                ident,
            }
        })
        .collect();

    let tuple_impls = tuple_impls(8, &g_params, &g_args, &g_use);

    let vis = &st.vis;
    let mod_ident = &st.ident;
    let base = &st.base;

    quote! {
        #[allow(non_snake_case, unused_variables, unused_mut, dead_code, clippy::all)]
        #vis mod #mod_ident {
            use super::*;

            pub trait Visit #g_def #(if let Some(b) = base) { : #b::Visit #g_use } {
                #(for t in &vtypes) {
                    fn #{&t.method}(&mut self, i: & #{&t.ident} #g_use) {
                        #{&t.method}(self, i)
                    }
                    fn #{&t.seq}(&mut self, seq: &[ #{&t.ident} #g_use ]) {
                        for __x in seq { self.#{&t.method}(__x); }
                    }
                    fn #{&t.opt}(&mut self, opt: &::core::option::Option< #{&t.ident} #g_use >) {
                        if let ::core::option::Option::Some(__x) = opt { self.#{&t.method}(__x); }
                    }
                }
            }

            impl< #(#g_params,)* __V: Visit #g_use > Visit #g_use for &mut __V {
                #(for t in &vtypes) {
                    fn #{&t.method}(&mut self, i: & #{&t.ident} #g_use) {
                        <__V as Visit #g_use>::#{&t.method}(self, i)
                    }
                    fn #{&t.seq}(&mut self, seq: &[ #{&t.ident} #g_use ]) {
                        <__V as Visit #g_use>::#{&t.seq}(self, seq)
                    }
                    fn #{&t.opt}(&mut self, opt: &::core::option::Option< #{&t.ident} #g_use >) {
                        <__V as Visit #g_use>::#{&t.opt}(self, opt)
                    }
                }
            }

            #(for t in &vtypes) {
                pub fn #{&t.method}< #(#g_params,)* __V: Visit #g_use + ?Sized >(
                    this: &mut __V,
                    i: & #{&t.ident} #g_use,
                ) {
                    #{&t.body}
                }
            }

            pub trait IntoVisitor< #(#g_params,)* __T > {
                fn into_visitor(self) -> impl Visit #g_use;
            }

            impl< #(#g_params,)* __V: Visit #g_use > IntoVisitor< #(#g_args,)* () > for __V {
                fn into_visitor(self) -> impl Visit #g_use {
                    self
                }
            }

            // --- closures: shallow Hook + single-pass Driver ---------------------------------

            pub trait Hook #g_def {
                #(for t in &vtypes) {
                    fn #{&t.hook}(&mut self, i: & #{&t.ident} #g_use) { let _ = i; }
                }
            }

            pub trait IntoHook< #(#g_params,)* __T > {
                fn into_hook(self) -> impl Hook #g_use;
            }

            pub struct Driver<__H>(pub __H);

            impl< #(#g_params,)* __H: Hook #g_use > Visit #g_use for Driver<__H> {
                #(for t in &vtypes) {
                    fn #{&t.method}(&mut self, i: & #{&t.ident} #g_use) {
                        self.0.#{&t.hook}(i);
                        #{&t.method}(self, i);
                    }
                }
            }

            #(for t in &vtypes) {
                pub struct #{&t.hook_struct}<__F>(pub __F);

                impl< #(#g_params,)* __F: ::core::ops::FnMut(& #{&t.ident} #g_use) >
                    Hook #g_use for #{&t.hook_struct}<__F>
                {
                    fn #{&t.hook}(&mut self, i: & #{&t.ident} #g_use) {
                        (self.0)(i);
                    }
                }

                impl< #(#g_params,)* __F: ::core::ops::FnMut(& #{&t.ident} #g_use) >
                    IntoHook< #(#g_args,)* #{&t.ident} #g_use > for __F
                {
                    fn into_hook(self) -> impl Hook #g_use {
                        #{&t.hook_struct}(self)
                    }
                }

                impl< #(#g_params,)* __F: ::core::ops::FnMut(& #{&t.ident} #g_use) >
                    IntoVisitor< #(#g_args,)* #{&t.ident} #g_use > for __F
                {
                    fn into_visitor(self) -> impl Visit #g_use {
                        Driver(#{&t.hook_struct}(self))
                    }
                }
            }

            // --- multiple closures: Chain combinator + tuple IntoVisitor impls --------------

            pub struct Chain<__A, __B>(pub __A, pub __B);

            impl< #(#g_params,)* __A: Hook #g_use, __B: Hook #g_use > Hook #g_use for Chain<__A, __B> {
                #(for t in &vtypes) {
                    fn #{&t.hook}(&mut self, i: & #{&t.ident} #g_use) {
                        self.0.#{&t.hook}(i);
                        self.1.#{&t.hook}(i);
                    }
                }
            }

            #(for imp in &tuple_impls) { #imp }

            pub trait Visitable #g_def {
                fn visit<__T>(&self, visitor: impl IntoVisitor< #(#g_args,)* __T >) -> &Self;
            }

            #(for t in &vtypes) {
                impl #g_def Visitable #g_use for #{&t.ident} #g_use {
                    fn visit<__T>(&self, visitor: impl IntoVisitor< #(#g_args,)* __T >) -> &Self {
                        let mut visitor = visitor.into_visitor();
                        visitor.#{&t.method}(self);
                        self
                    }
                }
            }
        }
    }
}

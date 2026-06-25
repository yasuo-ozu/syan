//! Helpers shared across the macro crate (`ast`, `visitor`, `recurse`): identifier casing, generic
//! param handling, and field-type "peeling" (container + box unwrapping to a visitable head).

use proc_macro2::{Ident, Span, TokenStream};
use proc_macro_error::abort;
use std::collections::{HashMap, HashSet};
use syn::*;
use template_quote::quote;

/// Convert a CamelCase / PascalCase identifier to snake_case (for `visit_<head>` / hidden names).
pub(crate) fn to_snake(ident: &Ident) -> String {
    let s = ident.to_string();
    let mut out = String::with_capacity(s.len() + 4);
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() {
            if i != 0 {
                out.push('_');
            }
            out.extend(ch.to_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// Name of a generic param (for deduping / reserving names).
pub(crate) fn param_name(p: &GenericParam) -> String {
    match p {
        GenericParam::Type(t) => t.ident.to_string(),
        GenericParam::Const(c) => c.ident.to_string(),
        GenericParam::Lifetime(l) => l.lifetime.ident.to_string(),
    }
}

/// Use-side token for one generic param (ident / lifetime).
pub(crate) fn param_use(p: &GenericParam) -> TokenStream {
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

/// One generic param's `(declaration, use)` token forms. They coincide for lifetimes (`'a`) and type
/// params (`T`) but differ for const params (`const N: usize` vs `N`). The declaration form is bare
/// (no bounds/defaults), so it suits a method generic too.
pub(crate) fn param_tokens(p: &GenericParam) -> (TokenStream, TokenStream) {
    match p {
        GenericParam::Lifetime(lt) => {
            let l = &lt.lifetime;
            (quote!(#l), quote!(#l))
        }
        GenericParam::Type(t) => {
            let i = &t.ident;
            (quote!(#i), quote!(#i))
        }
        GenericParam::Const(c) => {
            let (i, ty) = (&c.ident, &c.ty);
            (quote!(const #i: #ty), quote!(#i))
        }
    }
}

/// Generic params with defaults stripped (for `impl<...>` / `trait<...>` / `struct<...>` headers).
pub(crate) fn gparams(g: &Generics) -> Vec<GenericParam> {
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
pub(crate) fn gargs(g: &Generics) -> Vec<TokenStream> {
    g.params.iter().map(param_use).collect()
}

/// Wrap items in an angle-bracket clause `< a, b, c >`, or nothing when empty — for the optional
/// generic clauses that pepper the generators.
pub(crate) fn angle<T: quote::ToTokens>(items: &[T]) -> TokenStream {
    if items.is_empty() {
        quote!()
    } else {
        quote!( < #(#items),* > )
    }
}

/// First type argument of a path segment's `<...>` (e.g. the `T` of `Vec<T>`).
pub(crate) fn first_ty_arg(seg: &PathSegment) -> Option<&Type> {
    if let PathArguments::AngleBracketed(ab) = &seg.arguments {
        ab.args.iter().find_map(|a| match a {
            GenericArgument::Type(t) => Some(t),
            _ => None,
        })
    } else {
        None
    }
}

/// The element list of a tuple type, seeing through transparent `Group`/`Paren` wrappers; `None` if
/// `ty` is not a tuple. (A visitor dispatches each followed element of a tuple field.)
pub(crate) fn as_tuple(ty: &Type) -> Option<&punctuated::Punctuated<Type, Token![,]>> {
    match ty {
        Type::Tuple(t) => Some(&t.elems),
        Type::Group(g) => as_tuple(&g.elem),
        Type::Paren(p) => as_tuple(&p.elem),
        _ => None,
    }
}

/// The identifier of an enum/struct item (`None` for anything else).
pub(crate) fn item_ident(item: &Item) -> Option<&Ident> {
    match item {
        Item::Enum(e) => Some(&e.ident),
        Item::Struct(s) => Some(&s.ident),
        _ => None,
    }
}

/// The generics of an enum/struct item (`None` for anything else).
pub(crate) fn item_generics(item: &Item) -> Option<&Generics> {
    match item {
        Item::Enum(e) => Some(&e.generics),
        Item::Struct(s) => Some(&s.generics),
        _ => None,
    }
}

/// How a field type wraps its (visitable) head: a single value, a sequence (`Vec`/`VecDeque`/slice/
/// array/`Punctuated`), or an `Option`. `Box` is transparent (tracked as box-depth).
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Container {
    Direct,
    Seq,
    Opt,
}

/// The result of peeling a field type to its visitable head.
pub(crate) struct Peeled {
    pub container: Container,
    pub head: Ident,
    /// The FIRST path segment ident of the innermost peeled path (for a single-segment path
    /// `head_lead == head`). A same-module cycle reference is always a bare single-segment ident, so
    /// a caller deciding cycle membership (e.g. `recurse`) keys on this to reject a foreign
    /// multi-segment path whose last segment merely happens to equal a cycle type name.
    pub head_lead: Ident,
    /// `Box` layers between the container (or the top, for `Direct`) and the head; a drill derefs
    /// through these (`&**…`) to reach a `&head` scrutinee.
    pub head_box: usize,
    /// `Box` layers around the container itself; the `Opt` `if let` must deref through these (the
    /// `Seq` `.iter()`/`.iter_mut()` already auto-derefs them).
    pub cont_box: usize,
    /// A second container layer was found nested inside the first (e.g. `Vec<Option<T>>`); such a
    /// field is unsupported and the caller turns this into a clear error.
    pub nested: bool,
    /// The head sits behind a shared reference (`&T`, `&[T]`, …) — peeled transparently. Such a field
    /// can be visited on the shared side but NOT on the `&mut` side (no `&mut head` through a `&`), so
    /// the mut side treats it as a leaf. (A `&mut T` field is owned-enough to mutate, so it is *not*
    /// flagged.)
    pub shared_ref: bool,
}

/// Wrap a peeled element in an outer container, flagging nesting if the element already had one.
fn container_of(c: Container, inner: Peeled) -> Peeled {
    Peeled {
        container: c,
        head: inner.head,
        head_lead: inner.head_lead,
        head_box: inner.head_box,
        cont_box: 0,
        nested: inner.nested || inner.container != Container::Direct,
        shared_ref: inner.shared_ref,
    }
}

fn direct(head: Ident, head_lead: Ident) -> Peeled {
    Peeled {
        container: Container::Direct,
        head,
        head_lead,
        head_box: 0,
        cont_box: 0,
        nested: false,
        shared_ref: false,
    }
}

/// Peel a field type to its visitable head. A path head listed in `user_types` (e.g. a type's
/// `#[subast]` matchkeys plus its own ident) is always a `Direct` head, so a user AST type named
/// like a container keyword (`Option`, `Vec`, …) wins over the built-in container handling. `None`
/// for a non-path leaf. The caller decides whether `head` is actually followed.
pub(crate) fn peel(ty: &Type, user_types: &HashSet<String>) -> Option<Peeled> {
    match ty {
        // A shared `&` makes the head unmutable-through; flag it (the mut side will treat it as a
        // leaf). `&mut` is not flagged — it can be reborrowed mutably.
        Type::Reference(r) => peel(&r.elem, user_types).map(|mut inner| {
            inner.shared_ref |= r.mutability.is_none();
            inner
        }),
        Type::Group(g) => peel(&g.elem, user_types),
        Type::Paren(p) => peel(&p.elem, user_types),
        Type::Slice(s) => peel(&s.elem, user_types).map(|inner| container_of(Container::Seq, inner)),
        Type::Array(a) => peel(&a.elem, user_types).map(|inner| container_of(Container::Seq, inner)),
        Type::Path(tp) => {
            let seg = tp.path.segments.last()?;
            let lead = tp.path.segments.first()?.ident.clone();
            let name = seg.ident.to_string();
            // A user AST type wins over a same-named container keyword.
            if user_types.contains(&name) {
                return Some(direct(seg.ident.clone(), lead));
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
                _ => Some(direct(seg.ident.clone(), lead)),
            }
        }
        _ => None,
    }
}

/// `"_mut"` for the mutable visitor side, `""` for the shared side — the `visit_*` / `visit_*_mut`
/// method-name suffix.
pub(crate) fn mt(mutable: bool) -> &'static str {
    if mutable {
        "_mut"
    } else {
        ""
    }
}

/// The `visit_<snake(head)>` / `visit_<snake(head)>_mut` method ident for a visited head.
pub(crate) fn method_ident_m(head: &Ident, mutable: bool) -> Ident {
    Ident::new(
        &format!("visit_{}{}", to_snake(head), mt(mutable)),
        Span::call_site(),
    )
}

// ---------------------------------------------------------------------------
// Recurse cycle-body lowering for `visitor!()` over a `#[recurse]` cycle (visitor.rs's
// `generate_module_mixed`). Classifies a cycle type's fields: a back-edge to a root drives via that
// root's depth param (`root_dp[head]::visit_rec{,_mut}`); a cross-edge to a *listed* type
// (`method_set`) calls `v.visit_<head>{,_mut}`; an unlisted cycle type aborts (no inline drill yet);
// anything else is a leaf. `mutable` selects the `&`/`&mut`, `.iter()`/`.iter_mut()`, and method/
// `visit_rec` suffix.
// ---------------------------------------------------------------------------

/// Lower one field (see the module-comment above). `binding` is the destructured field (`&Field` /
/// `&mut Field`); `None` is a leaf (caller binds `_`).
pub(crate) fn recurse_lower_field(
    ty: &Type,
    binding: &TokenStream,
    method_set: &HashSet<String>,
    root_dp: &HashMap<String, Ident>,
    cycle: &HashSet<String>,
    mutable: bool,
) -> Option<TokenStream> {
    if let Some(elems) = as_tuple(ty) {
        let mut pats = Vec::new();
        let mut stmts = Vec::new();
        for (i, elem) in elems.iter().enumerate() {
            let bi = Ident::new(&format!("__t{i}"), Span::call_site());
            if let Some(s) = recurse_lower_field(elem, &quote!(#bi), method_set, root_dp, cycle, mutable)
            {
                pats.push(quote!(#bi));
                stmts.push(s);
            } else {
                pats.push(quote!(_));
            }
        }
        if stmts.is_empty() {
            return None;
        }
        return Some(quote!( { let ( #(#pats,)* ) = #binding; #(#stmts)* } ));
    }
    let p = peel(ty, &HashSet::new())?;
    let hs = p.head_lead.to_string();
    let dp = root_dp.get(&hs);
    let listed = method_set.contains(&hs);
    if dp.is_none() && !listed {
        if cycle.contains(&hs) {
            abort!(
                ty,
                "visitor!() over `#[recurse]`: cross-edge to cycle type `{}` is not listed in \
                 visitor!(...); list it (inline drilling of unlisted recurse types is not yet supported)",
                hs
            );
        }
        return None; // leaf
    }
    if p.nested {
        abort!(
            ty,
            "`#[recurse]` visitor cannot traverse a nested container (e.g. `Vec<Option<_>>`); wrap the \
             inner part in its own `#[derive(Ast)]` type"
        );
    }
    let stars: TokenStream = (0..=p.head_box).map(|_| quote!(*)).collect();
    let amp = if mutable { quote!(&mut) } else { quote!(&) };
    let visit_rec_fn = if mutable {
        quote!(visit_rec_mut)
    } else {
        quote!(visit_rec)
    };
    let one = |acc: &TokenStream| -> TokenStream {
        match dp {
            Some(d) => quote!( #d::#visit_rec_fn(#amp #stars #acc, v); ),
            None => {
                let m = method_ident_m(&p.head_lead, mutable);
                quote!( v.#m(#amp #stars #acc); )
            }
        }
    };
    Some(match p.container {
        Container::Direct => one(binding),
        Container::Seq => {
            let iter = if mutable { quote!(iter_mut) } else { quote!(iter) };
            let inner = one(&quote!(__x));
            quote!( for __x in #binding.#iter() { #inner } )
        }
        Container::Opt => {
            let cont_stars: TokenStream = (0..=p.cont_box).map(|_| quote!(*)).collect();
            let inner = one(&quote!(__x));
            quote!( if let ::core::option::Option::Some(__x) = #amp #cont_stars #binding { #inner } )
        }
    })
}

/// `(pattern, statements)` for a recurse cycle type's fields.
fn recurse_lower_fields(
    fields: &Fields,
    method_set: &HashSet<String>,
    root_dp: &HashMap<String, Ident>,
    cycle: &HashSet<String>,
    mutable: bool,
) -> (TokenStream, TokenStream) {
    match fields {
        Fields::Named(named) => {
            let mut binds = Vec::new();
            let mut stmts = Vec::new();
            for f in &named.named {
                let name = f.ident.clone().unwrap();
                if let Some(s) =
                    recurse_lower_field(&f.ty, &quote!(#name), method_set, root_dp, cycle, mutable)
                {
                    binds.push(quote!(#name));
                    stmts.push(s);
                }
            }
            (quote!( { #(#binds,)* .. } ), quote!( #(#stmts)* ))
        }
        Fields::Unnamed(unnamed) => {
            let mut pats = Vec::new();
            let mut stmts = Vec::new();
            for (i, f) in unnamed.unnamed.iter().enumerate() {
                let b = Ident::new(&format!("__f{i}"), Span::call_site());
                if let Some(s) =
                    recurse_lower_field(&f.ty, &quote!(#b), method_set, root_dp, cycle, mutable)
                {
                    pats.push(quote!(#b));
                    stmts.push(s);
                } else {
                    pats.push(quote!(_));
                }
            }
            (quote!( ( #(#pats),* ) ), quote!( #(#stmts)* ))
        }
        Fields::Unit => (quote!(), quote!()),
    }
}

/// Body of a recurse cycle type's `visit_*` / `visit_*_mut` drive fn: destructure `i` (a
/// `&__XRec<…>` / `&mut __XRec<…>`, matched via the `node` path/ident) and dispatch followed fields.
pub(crate) fn recurse_lower_body(
    def: &Item,
    node: &impl quote::ToTokens,
    method_set: &HashSet<String>,
    root_dp: &HashMap<String, Ident>,
    cycle: &HashSet<String>,
    mutable: bool,
) -> TokenStream {
    match def {
        Item::Enum(e) => {
            let arms = e.variants.iter().map(|v| {
                let (pat, stmts) = recurse_lower_fields(&v.fields, method_set, root_dp, cycle, mutable);
                let vid = &v.ident;
                quote!( #node::#vid #pat => { #stmts } )
            });
            quote!( match i { #(#arms)* } )
        }
        Item::Struct(s) => {
            let (pat, stmts) = recurse_lower_fields(&s.fields, method_set, root_dp, cycle, mutable);
            match &s.fields {
                Fields::Unit => quote!(),
                _ => quote!( let #node #pat = i; #stmts ),
            }
        }
        _ => quote!(),
    }
}

//! Helpers shared across the macro crate (`ast`, `visitor`, `recurse`): identifier casing, generic
//! param handling, and field-type "peeling" (container + box unwrapping to a visitable head).

use proc_macro2::{Ident, Span, TokenStream};
use std::collections::HashSet;
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
    let decl = match p {
        GenericParam::Lifetime(lt) => {
            let l = &lt.lifetime;
            quote!(#l)
        }
        GenericParam::Type(t) => {
            let i = &t.ident;
            quote!(#i)
        }
        GenericParam::Const(c) => {
            let (i, ty) = (&c.ident, &c.ty);
            quote!(const #i: #ty)
        }
    };
    (decl, param_use(p))
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

/// A `#[seq]` / `#[opt]` field marker: the owning collection is edited through a `SeqView` (`Seq`) or an
/// `OptView` (`Opt`). Used only for the edit-view path; ordinary descent does not distinguish the two.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Container {
    Seq,
    Opt,
}

/// How one wrapper level of a field descends. `View`: a `SeqView`/`OptView` container
/// (`Vec`/`Option`/`Box`/`Punctuated`/user wrapper) — descended by the `view_iter[_mut]` method, resolved
/// to the right view by the compiler (**no container type name is matched**). `Raw`: a fixed-size array or
/// slice — descended by the slice `iter[_mut]` (arrays/slices have no `SeqView` impl).
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum LayerKind {
    View,
    Raw,
}

/// What sits at the innermost peeled position: a path head (a visited type) or a tuple (destructured, each
/// element lowered recursively).
pub(crate) enum Head {
    Path { head: Ident },
    Tuple(Vec<Type>),
}

/// The result of peeling a field type to its visitable head. `peel` returns `Some` only when a followed
/// head is reachable; a type with no followed head is a leaf (`None`).
pub(crate) struct Peeled {
    /// Wrapper levels between the field and the head, OUTER→INNER (empty ⇒ the field *is* the head).
    pub conts: Vec<LayerKind>,
    pub head: Head,
    /// The head sits behind a shared reference (`&T`) — visitable on the shared side but a leaf on the
    /// `&mut` side (no `&mut head` through a `&`). (`&mut T` is not flagged.)
    pub shared_ref: bool,
}

fn prepend(kind: LayerKind, mut inner: Peeled) -> Peeled {
    inner.conts.insert(0, kind);
    inner
}

/// Peel a field type to its head + the wrapper levels around it, **without matching any container type
/// name**. A path whose last segment is in `user_types` (a type's `#[subast]` matchkeys + its own ident)
/// is the head; any *other* path is a `View` wrapper level iff a head is reachable through its first type
/// argument — so `Vec`/`Option`/`Box`/`Punctuated` and user wrappers are handled uniformly, while
/// `Vec<String>` (no head below) is a leaf. Arrays/slices are `Raw` levels; a tuple with a followed
/// element is a `Head::Tuple`. `None` ⇒ no followed head (a leaf).
pub(crate) fn peel(ty: &Type, user_types: &HashSet<String>) -> Option<Peeled> {
    match ty {
        // A shared `&` makes the head unmutable-through; flag it (the mut side treats it as a leaf).
        // `&mut` is not flagged — it can be reborrowed mutably.
        Type::Reference(r) => peel(&r.elem, user_types).map(|mut inner| {
            inner.shared_ref |= r.mutability.is_none();
            inner
        }),
        Type::Group(g) => peel(&g.elem, user_types),
        Type::Paren(p) => peel(&p.elem, user_types),
        Type::Slice(s) => peel(&s.elem, user_types).map(|inner| prepend(LayerKind::Raw, inner)),
        Type::Array(a) => peel(&a.elem, user_types).map(|inner| prepend(LayerKind::Raw, inner)),
        Type::Path(tp) => {
            let seg = tp.path.segments.last()?;
            if user_types.contains(&seg.ident.to_string()) {
                return Some(Peeled {
                    conts: Vec::new(),
                    head: Head::Path { head: seg.ident.clone() },
                    shared_ref: false,
                });
            }
            peel(first_ty_arg(seg)?, user_types).map(|inner| prepend(LayerKind::View, inner))
        }
        // A tuple is a head iff some element is followed; each element is lowered by the caller.
        Type::Tuple(t) if t.elems.iter().any(|e| peel(e, user_types).is_some()) => Some(Peeled {
            conts: Vec::new(),
            head: Head::Tuple(t.elems.iter().cloned().collect()),
            shared_ref: false,
        }),
        _ => None,
    }
}

/// The accessor for the head after peeling all `conts`: the field `binding` for a direct head, else the
/// innermost loop var that `fold_containers` introduces.
pub(crate) fn innermost_acc(conts: &[LayerKind], binding: &TokenStream) -> TokenStream {
    if conts.is_empty() {
        binding.clone()
    } else {
        let e = Ident::new(&format!("__nc{}", conts.len()), Span::call_site());
        quote!(#e)
    }
}

/// Wrap an already-lowered `body` (dispatching at `innermost_acc(conts, binding)`) in the wrapper levels
/// `conts` (outer→inner): a `View` level is a `for` over `view_iter[_mut]()` (resolved to `SeqView`/
/// `OptView` by the compiler), a `Raw` level a `for` over the slice `iter[_mut]()`. Level `i` binds
/// `__nc{i+1}`, iterating `__nc{i}` (or `binding` at `i == 0`) — so nested wrappers nest the loops.
pub(crate) fn fold_containers(
    conts: &[LayerKind],
    binding: &TokenStream,
    mut body: TokenStream,
    mutable: bool,
) -> TokenStream {
    for (i, layer) in conts.iter().enumerate().rev() {
        let bind = if i == 0 {
            binding.clone()
        } else {
            let e = Ident::new(&format!("__nc{i}"), Span::call_site());
            quote!(#e)
        };
        let elem = Ident::new(&format!("__nc{}", i + 1), Span::call_site());
        let iter = match (*layer, mutable) {
            (LayerKind::View, true) => quote!(view_iter_mut),
            (LayerKind::View, false) => quote!(view_iter),
            (LayerKind::Raw, true) => quote!(iter_mut),
            (LayerKind::Raw, false) => quote!(iter),
        };
        body = quote!( for #elem in #bind.#iter() { #body } );
    }
    body
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


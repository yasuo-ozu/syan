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

/// A container layer wrapping the head: a sequence (`Vec`/`VecDeque`/slice/array/`Punctuated`) or an
/// `Option`. `Box` is transparent (tracked as box-depth).
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Container {
    Seq,
    Opt,
}

/// One container layer + the `Box` layers wrapping THIS container (the `Opt` `if let` derefs through
/// them; a `Seq`'s `.iter()`/`.iter_mut()` auto-derefs, so it ignores them).
#[derive(Clone, Copy)]
pub(crate) struct ContLayer {
    pub kind: Container,
    pub boxes: usize,
    /// Whether this layer has a `SeqView`/`OptView` impl (Vec/VecDeque/Punctuated/Option — yes; a
    /// fixed-size array or a slice maps to `Seq` for traversal but is NOT structurally editable). Drives
    /// the `#[seq]`/`#[opt]` marker validation, which otherwise couldn't tell them apart.
    pub viewable: bool,
}

/// What sits at the innermost peeled position (after the container/box layers): the common case is a
/// path head, but a tuple can appear there too — directly (`(A, B)`) or nested behind containers/boxes
/// (`Vec<(A, B)>`, `Box<(A, B)>`). A tuple has no single head ident; its elements are lowered
/// recursively (each may itself be a followed type, a container of one, or a nested tuple).
pub(crate) enum Head {
    Path { head: Ident },
    Tuple(Vec<Type>),
}

/// The result of peeling a field type to its visitable head.
pub(crate) struct Peeled {
    /// Container layers wrapping the head, OUTER→INNER. Empty ⇒ a direct (single-value) head; a chain
    /// of length > 1 is a nested container (`Vec<Option<T>>` ⇒ `[Seq, Opt]`).
    pub conts: Vec<ContLayer>,
    pub head: Head,
    /// `Box` layers directly around the head/tuple; a dispatch derefs through these (`&**…`).
    pub head_box: usize,
    /// The head sits behind a shared reference (`&T`, `&[T]`, …) — peeled transparently. Such a field
    /// can be visited on the shared side but NOT on the `&mut` side (no `&mut head` through a `&`), so
    /// the mut side treats it as a leaf. (A `&mut T` field is owned-enough to mutate, so it is *not*
    /// flagged.)
    pub shared_ref: bool,
}

/// Wrap a peeled head in an outer container layer (prepended — `conts` is outer→inner). `viewable` marks
/// whether the container has a `SeqView`/`OptView` impl (false for arrays/slices).
fn container_of(c: Container, viewable: bool, mut inner: Peeled) -> Peeled {
    inner.conts.insert(0, ContLayer { kind: c, boxes: 0, viewable });
    inner
}

fn direct(head: Ident) -> Peeled {
    Peeled {
        conts: Vec::new(),
        head: Head::Path { head },
        head_box: 0,
        shared_ref: false,
    }
}

/// Peel a field type to its visitable head + its container chain. A path head listed in `user_types`
/// (e.g. a type's `#[subast]` matchkeys plus its own ident) is always a direct head, so a user AST
/// type named like a container keyword (`Option`, `Vec`, …) wins over the built-in container handling.
/// `None` for a non-path leaf. The caller decides whether `head` is actually followed.
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
        // Slice/array traverse as a `Seq` but have no `SeqView` impl (not structurally editable).
        Type::Slice(s) => {
            peel(&s.elem, user_types).map(|inner| container_of(Container::Seq, false, inner))
        }
        Type::Array(a) => {
            peel(&a.elem, user_types).map(|inner| container_of(Container::Seq, false, inner))
        }
        Type::Path(tp) => {
            let seg = tp.path.segments.last()?;
            let name = seg.ident.to_string();
            // A user AST type wins over a same-named container keyword.
            if user_types.contains(&name) {
                return Some(direct(seg.ident.clone()));
            }
            match name.as_str() {
                // `Box` and `Attempt` are both transparent single-`Deref` wrappers — peel through them
                // (`*x` yields the inner value), counting one deref layer like a box.
                "Box" | "Attempt" => {
                    let mut inner = peel(first_ty_arg(seg)?, user_types)?;
                    match inner.conts.first_mut() {
                        // Wrapper around the outermost container: that layer derefs through it.
                        Some(layer) => layer.boxes += 1,
                        // Wrapper directly around the head.
                        None => inner.head_box += 1,
                    }
                    Some(inner)
                }
                "Vec" | "VecDeque" | "Punctuated" => {
                    Some(container_of(Container::Seq, true, peel(first_ty_arg(seg)?, user_types)?))
                }
                "Option" => {
                    Some(container_of(Container::Opt, true, peel(first_ty_arg(seg)?, user_types)?))
                }
                _ => Some(direct(seg.ident.clone())),
            }
        }
        // A tuple at the innermost peeled position (`(A, B)`, or `Vec<(A, B)>` / `Box<(A, B)>` after
        // its container/box layers are peeled): each element is lowered recursively by the caller.
        Type::Tuple(t) => Some(Peeled {
            conts: Vec::new(),
            head: Head::Tuple(t.elems.iter().cloned().collect()),
            head_box: 0,
            shared_ref: false,
        }),
        _ => None,
    }
}

/// The accessor for the head after peeling all `conts`: the field `binding` itself for a direct head,
/// else the innermost loop / `if let` var that `fold_containers` introduces.
pub(crate) fn innermost_acc(conts: &[ContLayer], binding: &TokenStream) -> TokenStream {
    if conts.is_empty() {
        binding.clone()
    } else {
        let e = Ident::new(&format!("__nc{}", conts.len()), Span::call_site());
        quote!(#e)
    }
}

/// Wrap an already-lowered `body` (which dispatches at `innermost_acc(conts, binding)`) in the
/// container layers `conts` (outer→inner): `Seq` ⇒ a `for` over `.iter()`/`.iter_mut()`, `Opt` ⇒ an
/// `if let Some(..)` (dereffing the layer's `Box`es). Layer `i` (outer→inner) binds `__nc{i+1}`,
/// iterating `__nc{i}` (or `binding` at `i == 0`) — so nested containers nest the loops/ifs.
pub(crate) fn fold_containers(
    conts: &[ContLayer],
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
        body = match layer.kind {
            Container::Seq => {
                let iter = if mutable { quote!(iter_mut) } else { quote!(iter) };
                quote!( for #elem in #bind.#iter() { #body } )
            }
            Container::Opt => {
                let amp = if mutable { quote!(&mut) } else { quote!(&) };
                let stars: TokenStream = (0..=layer.boxes).map(|_| quote!(*)).collect();
                quote!( if let ::core::option::Option::Some(#elem) = #amp #stars #bind { #body } )
            }
        };
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


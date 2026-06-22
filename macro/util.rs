//! Helpers shared across the macro crate (`ast`, `visitor`, `recurse`): identifier casing, generic
//! param handling, and field-type "peeling" (container + box unwrapping to a visitable head).

use proc_macro2::{Ident, TokenStream};
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
    /// `Box` layers between the container (or the top, for `Direct`) and the head; a drill derefs
    /// through these (`&**…`) to reach a `&head` scrutinee.
    pub head_box: usize,
    /// `Box` layers around the container itself; the `Opt` `if let` must deref through these (the
    /// `Seq` `.iter()`/`.iter_mut()` already auto-derefs them).
    pub cont_box: usize,
    /// A second container layer was found nested inside the first (e.g. `Vec<Option<T>>`); such a
    /// field is unsupported and the caller turns this into a clear error.
    pub nested: bool,
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

/// Peel a field type to its visitable head. A path head listed in `user_types` (e.g. a type's
/// `#[subast]` matchkeys plus its own ident) is always a `Direct` head, so a user AST type named
/// like a container keyword (`Option`, `Vec`, …) wins over the built-in container handling. `None`
/// for a non-path leaf. The caller decides whether `head` is actually followed.
pub(crate) fn peel(ty: &Type, user_types: &HashSet<String>) -> Option<Peeled> {
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

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
}

/// The result of peeling a field type to its visitable head.
pub(crate) struct Peeled {
    /// Container layers wrapping the head, OUTER→INNER. Empty ⇒ a direct (single-value) head; a chain
    /// of length > 1 is a nested container (`Vec<Option<T>>` ⇒ `[Seq, Opt]`).
    pub conts: Vec<ContLayer>,
    pub head: Ident,
    /// The FIRST path segment ident of the innermost peeled path (for a single-segment path
    /// `head_lead == head`). A same-module cycle reference is always a bare single-segment ident, so
    /// a caller deciding cycle membership (e.g. `recurse`) keys on this to reject a foreign
    /// multi-segment path whose last segment merely happens to equal a cycle type name.
    pub head_lead: Ident,
    /// `Box` layers directly around the head; a dispatch derefs through these (`&**…`) to reach `&head`.
    pub head_box: usize,
    /// The head sits behind a shared reference (`&T`, `&[T]`, …) — peeled transparently. Such a field
    /// can be visited on the shared side but NOT on the `&mut` side (no `&mut head` through a `&`), so
    /// the mut side treats it as a leaf. (A `&mut T` field is owned-enough to mutate, so it is *not*
    /// flagged.)
    pub shared_ref: bool,
}

/// Wrap a peeled head in an outer container layer (prepended — `conts` is outer→inner).
fn container_of(c: Container, mut inner: Peeled) -> Peeled {
    inner.conts.insert(0, ContLayer { kind: c, boxes: 0 });
    inner
}

fn direct(head: Ident, head_lead: Ident) -> Peeled {
    Peeled { conts: Vec::new(), head, head_lead, head_box: 0, shared_ref: false }
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
                    let mut inner = peel(first_ty_arg(seg)?, user_types)?;
                    match inner.conts.first_mut() {
                        // Box around the outermost container: that layer derefs through it.
                        Some(layer) => layer.boxes += 1,
                        // Box directly around the head.
                        None => inner.head_box += 1,
                    }
                    Some(inner)
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

// ---------------------------------------------------------------------------
// Recurse cycle-body lowering for `visitor!()` over a `#[recurse]` cycle (visitor.rs's
// `generate_module_mixed`). Classifies a cycle type's fields: a back-edge to a root drives via that
// root's depth param (`root_dp[head]::visit_rec{,_mut}`); a cross-edge to a *listed* type
// (`method_set`) calls `v.visit_<head>{,_mut}`; an unlisted cycle type is drilled inline (its node is
// destructured and its fields recursed, back-edges still via the depth params — with a cycle guard
// against a loop of unlisted intermediates); anything else is a leaf. `mutable` selects the `&`/`&mut`,
// `.iter()`/`.iter_mut()`, and method/`visit_rec` suffix.
// ---------------------------------------------------------------------------

/// Immutable context for recurse cycle-body lowering, threaded through the (now recursive, via
/// inline drilling) `recurse_lower_*` helpers.
pub(crate) struct RecLower<'a> {
    /// Cycle types that get a `visit_*` method (this visitor's listed + inherited recurse types).
    pub method_set: &'a HashSet<String>,
    /// Root type name → its depth param (`__R{i}`); a back-edge to a root dispatches via it.
    pub root_dp: &'a HashMap<String, Ident>,
    /// All idents of this cycle (the `@recurse` `@cycle` set).
    pub cycle: &'a HashSet<String>,
    /// Every fetched cycle type (listed or not): ident → (its def, its `__*Rec` node path). Used to
    /// inline-drill an *unlisted* cross-edge cycle type (it gets no `visit_*`, like the acyclic drill).
    pub cycle_defs: &'a HashMap<String, (Item, Path)>,
    pub mutable: bool,
}

/// Lower one field (see the module-comment above). `binding` is the destructured field (`&Field` /
/// `&mut Field`); `None` is a leaf (caller binds `_`). `stack` carries the unlisted cycle heads
/// currently being inline-drilled, to reject a *cycle* of unlisted intermediates.
pub(crate) fn recurse_lower_field(
    ctx: &RecLower,
    stack: &mut Vec<String>,
    ty: &Type,
    binding: &TokenStream,
) -> Option<TokenStream> {
    if let Some(elems) = as_tuple(ty) {
        let mut pats = Vec::new();
        let mut stmts = Vec::new();
        for (i, elem) in elems.iter().enumerate() {
            let bi = Ident::new(&format!("__t{i}"), Span::call_site());
            if let Some(s) = recurse_lower_field(ctx, stack, elem, &quote!(#bi)) {
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
    let dp = ctx.root_dp.get(&hs);
    let listed = ctx.method_set.contains(&hs);
    if dp.is_none() && !listed && !ctx.cycle.contains(&hs) {
        return None; // leaf (not a followed cycle type)
    }
    let stars: TokenStream = (0..=p.head_box).map(|_| quote!(*)).collect();
    let amp = if ctx.mutable { quote!(&mut) } else { quote!(&) };
    let visit_rec_fn = if ctx.mutable {
        quote!(visit_rec_mut)
    } else {
        quote!(visit_rec)
    };
    // The action on one (container-peeled, box-dereffed) element accessor: a back-edge dispatches
    // through the depth param, a listed head calls its `visit_*`, and an *unlisted* cycle head is
    // drilled inline (destructure its node + recurse into its fields, back-edges still via the depth
    // params) — the recurse analogue of the acyclic drill-in.
    let one = |acc: &TokenStream, stack: &mut Vec<String>| -> TokenStream {
        if let Some(d) = dp {
            quote!( #d::#visit_rec_fn(#amp #stars #acc, v); )
        } else if listed {
            let m = method_ident_m(&p.head_lead, ctx.mutable);
            quote!( v.#m(#amp #stars #acc); )
        } else {
            let Some((y_def, y_node)) = ctx.cycle_defs.get(&hs) else {
                abort!(
                    ty,
                    "visitor!() over `#[recurse]`: cannot drill the unlisted cross-edge cycle type \
                     `{}` (its definition was not fetched — list it in `#[subast(...)]`)",
                    hs
                );
            };
            if stack.contains(&hs) {
                abort!(
                    ty,
                    "visitor!() over `#[recurse]`: a cycle of *unlisted* intermediates (through `{}`) \
                     can't be drilled inline; list one of them in `visitor!(...)` to break it",
                    hs
                );
            }
            stack.push(hs.clone());
            let body = recurse_lower_scrut(ctx, stack, y_def, y_node, &quote!(#amp #stars #acc));
            stack.pop();
            body
        }
    };
    let acc = innermost_acc(&p.conts, binding);
    let body = one(&acc, stack);
    Some(fold_containers(&p.conts, binding, body, ctx.mutable))
}

/// `(pattern, statements)` for a recurse cycle type's fields.
fn recurse_lower_fields(
    ctx: &RecLower,
    stack: &mut Vec<String>,
    fields: &Fields,
) -> (TokenStream, TokenStream) {
    match fields {
        Fields::Named(named) => {
            let mut binds = Vec::new();
            let mut stmts = Vec::new();
            for f in &named.named {
                let name = f.ident.clone().unwrap();
                if let Some(s) = recurse_lower_field(ctx, stack, &f.ty, &quote!(#name)) {
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
                if let Some(s) = recurse_lower_field(ctx, stack, &f.ty, &quote!(#b)) {
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
    ctx: &RecLower,
    def: &Item,
    node: &impl quote::ToTokens,
) -> TokenStream {
    recurse_lower_scrut(ctx, &mut Vec::new(), def, node, &quote!(i))
}

/// Destructure `scrut` (a `&Node` / `&mut Node`, matched via `node`) and dispatch its fields. Shared
/// by the top-level drive fn (`scrut = i`) and inline drilling of an unlisted cross-edge (`scrut` is
/// the dereffed element accessor).
fn recurse_lower_scrut(
    ctx: &RecLower,
    stack: &mut Vec<String>,
    def: &Item,
    node: &impl quote::ToTokens,
    scrut: &TokenStream,
) -> TokenStream {
    match def {
        Item::Enum(e) => {
            let mut arms = Vec::new();
            for v in &e.variants {
                let (pat, stmts) = recurse_lower_fields(ctx, stack, &v.fields);
                let vid = &v.ident;
                arms.push(quote!( #node::#vid #pat => { #stmts } ));
            }
            quote!( match #scrut { #(#arms)* } )
        }
        Item::Struct(s) => {
            let (pat, stmts) = recurse_lower_fields(ctx, stack, &s.fields);
            match &s.fields {
                Fields::Unit => quote!(),
                _ => quote!( let #node #pat = #scrut; #stmts ),
            }
        }
        _ => quote!(),
    }
}

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

/// The type arguments of a path segment's `<...>`, in order (the `T` of `Vec<T>`; the `K`, `V` of
/// `HashMap<K, V>`).
pub(crate) fn ty_args(seg: &PathSegment) -> impl Iterator<Item = &Type> {
    let args = match &seg.arguments {
        PathArguments::AngleBracketed(ab) => Some(ab.args.iter()),
        _ => None,
    };
    args.into_iter().flatten().filter_map(|a| match a {
        GenericArgument::Type(t) => Some(t),
        _ => None,
    })
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

/// Every field of an enum/struct item, in declaration order (each variant's fields in turn, for an
/// enum); empty for any other item kind.
pub(crate) fn item_field_iter(def: &Item) -> Box<dyn Iterator<Item = &Field> + '_> {
    match def {
        Item::Enum(e) => Box::new(e.variants.iter().flat_map(|v| v.fields.iter())),
        Item::Struct(s) => Box::new(s.fields.iter()),
        _ => Box::new(std::iter::empty()),
    }
}

/// Every field of a derive input's body, as [`item_field_iter`].
pub(crate) fn data_field_iter(data: &Data) -> Box<dyn Iterator<Item = &Field> + '_> {
    match data {
        Data::Enum(e) => Box::new(e.variants.iter().flat_map(|v| v.fields.iter())),
        Data::Struct(s) => Box::new(s.fields.iter()),
        Data::Union(u) => Box::new(u.fields.named.iter()),
    }
}

/// Call `f` for every field type of an enum/struct item.
pub(crate) fn for_each_field_type(def: &Item, f: &mut dyn FnMut(&Type)) {
    item_field_iter(def).for_each(|field| f(&field.ty));
}

/// Call `f` for every `Type::Path` reachable inside `ty`, outermost first: the type itself, then the
/// arguments of each of its segments, descending through references, slices, arrays, parens, groups
/// and tuples. So `Vec<Box<Stmt<S>>>` yields `Vec<..>`, `Box<..>`, `Stmt<S>` and `S`.
///
/// This is the "everything mentioned" traversal, unlike [`peel`], which stops at the first followed
/// head. Callers that need to know what a field *names* — rather than what a walk descends into —
/// use this.
pub(crate) fn for_each_type_path(ty: &Type, f: &mut impl FnMut(&TypePath)) {
    match ty {
        Type::Path(tp) => {
            f(tp);
            for seg in &tp.path.segments {
                if let PathArguments::AngleBracketed(ab) = &seg.arguments {
                    for arg in &ab.args {
                        if let GenericArgument::Type(t) = arg {
                            for_each_type_path(t, f);
                        }
                    }
                }
            }
        }
        Type::Reference(r) => for_each_type_path(&r.elem, f),
        Type::Slice(s) => for_each_type_path(&s.elem, f),
        Type::Array(a) => for_each_type_path(&a.elem, f),
        Type::Paren(p) => for_each_type_path(&p.elem, f),
        Type::Group(g) => for_each_type_path(&g.elem, f),
        Type::Tuple(t) => t.elems.iter().for_each(|e| for_each_type_path(e, f)),
        _ => {}
    }
}

/// A `#[seq]` / `#[opt]` field marker: the owning collection is edited through a `SeqView` (`Seq`) or an
/// `OptView` (`Opt`). Used only for the edit-view path; ordinary descent does not distinguish the two.
#[derive(Clone, Copy)]
pub(crate) enum Container {
    Seq,
    Opt,
}

impl Container {
    /// The bare marker word, as the user writes it: `#[seq]` / `#[opt]`.
    pub(crate) fn word(self) -> &'static str {
        match self {
            Container::Seq => "seq",
            Container::Opt => "opt",
        }
    }

    /// The view trait a field of this shape is dispatched through.
    pub(crate) fn view_trait(self) -> TokenStream {
        match self {
            Container::Seq => quote!(::syan::visit::SeqView),
            Container::Opt => quote!(::syan::visit::OptView),
        }
    }
}

/// How one wrapper level of a field descends. Both become a `Thru<_>` in the field's
/// [`indicator`](indicator), which `syan::visit`'s `Walk` impls peel one level at a time — the
/// distinction survives only because `peel` records it. `View`: a container (`Vec`/`Option`/`Box`/
/// `Punctuated`/`HashMap`/user wrapper). `Raw`: a fixed-size array or slice. **No container type name
/// is ever matched.**
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum LayerKind {
    View,
    Raw,
}

/// What sits at the innermost peeled position: a path head (a visited type) or a tuple (destructured, each
/// element lowered recursively).
pub(crate) enum Head {
    Path {
        head: Ident,
        /// The path exactly as the field wrote it, so a caller can tell `super::other::Stmt` from the
        /// visited `crate::ast::Stmt` — `peel` matches last segments, which alone cannot.
        wrote: Path,
        /// The head segment's generic arguments as written in the field (`<S>` in `Expr<S>`), so a
        /// caller can rebuild the head type against its `#[subast]` path.
        args: syn::PathArguments,
    },
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
/// is the head; any *other* path is a `View` wrapper level iff a head is reachable through one of its type
/// arguments (the first such, scanned in order — so a map's VALUE slot is found once its KEY turns out to
/// hold no head) — so `Vec`/`Option`/`Box`/`Punctuated`/`HashMap` and user wrappers are handled uniformly,
/// while `Vec<String>` (no head below) is a leaf. Arrays/slices are `Raw` levels; a tuple with a followed
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
                    head: Head::Path {
                        head: seg.ident.clone(),
                        args: seg.arguments.clone(),
                        wrote: tp.path.clone(),
                    },
                    shared_ref: false,
                });
            }
            ty_args(seg)
                .find_map(|a| peel(a, user_types))
                .map(|inner| prepend(LayerKind::View, inner))
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

/// The [`Walk`] indicator for `ty`: which parts of it a descent should enter. `None` when nothing in
/// it is followed — the caller then emits `Skip`, or treats the whole field as a leaf.
///
/// Mirrors `peel` exactly: one `Thru<_>` per wrapper level, `Here` at a followed head, and a tuple of
/// the elements' own indicators at a tuple. Because the indicator carries the shape, the container
/// impls in `syan::visit` stay generic in what they hold — no leaf type is ever named.
pub(crate) fn indicator(ty: &Type, user_types: &HashSet<String>) -> Option<TokenStream> {
    // `peel` treats a reference as transparent and records no level for it, but `syan::visit` gives
    // `&T` a `Thru` impl like any other wrapper — so account for it here.
    match ty {
        Type::Reference(r) => {
            return indicator(&r.elem, user_types)
                .map(|h| quote!(::syan::visit::indicator::Thru<#h>))
        }
        Type::Paren(p) => return indicator(&p.elem, user_types),
        Type::Group(g) => return indicator(&g.elem, user_types),
        _ => {}
    }
    let p = peel(ty, user_types)?;
    let inner = match &p.head {
        Head::Path { .. } => quote!(::syan::visit::indicator::Here),
        Head::Tuple(elems) => {
            let parts: Vec<TokenStream> = elems
                .iter()
                .map(|e| {
                    indicator(e, user_types)
                        .unwrap_or_else(|| quote!(::syan::visit::indicator::Skip))
                })
                .collect();
            quote!( ( #(#parts,)* ) )
        }
    };
    Some(
        p.conts
            .iter()
            .rev()
            .fold(inner, |acc, _| quote!(::syan::visit::indicator::Thru<#acc>)),
    )
}

/// Which of the two visitors an item belongs to: the shared one (`Visit`, `&T`) or the `&mut` one
/// (`VisitMut`, `&mut T`). Every generated item exists on both sides in the same shape, differing
/// only in the names and the borrow — so a generator writes the shape once and takes each piece
/// from here, rather than carrying a `mutable: bool` and re-deriving them at each use.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Side {
    pub(crate) mutable: bool,
}

impl Side {
    pub(crate) const SHARED: Side = Side { mutable: false };
    pub(crate) const MUT: Side = Side { mutable: true };

    /// Both sides, shared first — the order every generator emits them in.
    pub(crate) fn both() -> [Side; 2] {
        [Side::SHARED, Side::MUT]
    }

    /// Index into a two-element array laid out as [`Side::both`].
    pub(crate) fn index(self) -> usize {
        self.mutable as usize
    }

    /// Type-name suffix: `Visit` / `VisitMut`, `Hook` / `HookMut`.
    pub(crate) fn type_suffix(self) -> &'static str {
        if self.mutable {
            "Mut"
        } else {
            ""
        }
    }

    /// Method-name suffix: `visit_expr` / `visit_expr_mut`.
    pub(crate) fn fn_suffix(self) -> &'static str {
        if self.mutable {
            "_mut"
        } else {
            ""
        }
    }

    /// How this side borrows a visited node: `&` / `&mut`.
    pub(crate) fn amp(self) -> TokenStream {
        if self.mutable {
            quote!(&mut)
        } else {
            quote!(&)
        }
    }

    /// The receiver of a method that walks `self`: `&self` / `&mut self`.
    pub(crate) fn recv(self) -> TokenStream {
        if self.mutable {
            quote!(&mut self)
        } else {
            quote!(&self)
        }
    }

    /// A generated type name on this side: `name` + [`type_suffix`](Self::type_suffix).
    pub(crate) fn ty(self, name: &str) -> Ident {
        Ident::new(&format!("{name}{}", self.type_suffix()), Span::call_site())
    }

    /// A generated fn name on this side: `name` + [`fn_suffix`](Self::fn_suffix).
    pub(crate) fn func(self, name: &str) -> Ident {
        Ident::new(&format!("{name}{}", self.fn_suffix()), Span::call_site())
    }

    /// `syan::visit::Walk` / `WalkMut` — the descent trait.
    pub(crate) fn walk_trait(self) -> Ident {
        self.ty("Walk")
    }

    /// `Walk::walk` / `WalkMut::walk_mut`.
    pub(crate) fn walk_fn(self) -> Ident {
        self.func("walk")
    }

    /// The generated `Visit` / `VisitMut` trait.
    pub(crate) fn visit_trait(self) -> Ident {
        self.ty("Visit")
    }

    /// The `visit_<snake(head)>` / `visit_<snake(head)>_mut` method ident for a visited head.
    pub(crate) fn method(self, head: &Ident) -> Ident {
        self.func(&format!("visit_{}", to_snake(head)))
    }
}

/// Whether the path a field *wrote* can denote the type at `visited` — i.e. it is a bare ident, or
/// its segments are a suffix of the visited path's.
///
/// `peel` recognises a head by last segment alone, which is enough while `#[subast]` supplies the
/// authoritative path. Resolving a head against the `visitor!(..)` set instead has no such anchor, so
/// `super::other::Stmt` would be walked as `crate::ast::Stmt` — a real bug syan already has a
/// regression test for. Declining here leaves such a field a leaf, which is what it was.
///
/// Conservative in one direction only: a path that names the visited type by a route this cannot see
/// through (`super::ast::Stmt` from a sibling module) is declined too, and needs a `#[subast]` entry
/// as it does today.
pub(crate) fn path_may_denote(wrote: &Path, visited: &Path, owner: &Path) -> bool {
    let w: Vec<String> = wrote.segments.iter().map(|s| s.ident.to_string()).collect();
    let v: Vec<String> = visited
        .segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect();
    // A bare ident is the ordinary way to write the visited type, and a `use` could point it
    // anywhere, so accept it. When it turns out to name something else, the generated `Walk` call
    // fails at the field with "cannot be walked" — a type the visitor does not know has no impl.
    if w.len() <= 1 {
        return true;
    }
    // Otherwise resolve the spelling against the owning type's module and compare outright. This is
    // what separates `super::ast::Node` (the visited type by a relative route — walk it) from
    // `super::other::Stmt` (a different type whose last segment collides — leave it a leaf).
    if let Some(abs) = absolutize(wrote, owner) {
        let a: Vec<String> = abs.segments.iter().map(|s| s.ident.to_string()).collect();
        return a == v;
    }
    // Unresolvable here (external crate, leading `::`): fall back to comparing spellings.
    v.len() >= w.len() && v[v.len() - w.len()..] == w[..]
}

/// Resolve a field's written path against the module of the type that owns the field, so a relative
/// spelling can be compared with an absolute one. `owner` is the owning type's own path, whose module
/// is everything but its last segment.
///
/// `None` when the path cannot be resolved here — a leading `::`, or a root that is an external crate
/// — in which case the caller falls back to comparing spellings.
pub(crate) fn absolutize(wrote: &Path, owner: &Path) -> Option<Path> {
    if wrote.leading_colon.is_some() {
        return None;
    }
    let mut module: Vec<Ident> = owner.segments.iter().map(|s| s.ident.clone()).collect();
    module.pop()?; // drop the type itself, leaving its module
    let segs: Vec<Ident> = wrote.segments.iter().map(|s| s.ident.clone()).collect();
    let first = segs.first()?;
    let out: Vec<Ident> = if first == "crate" {
        segs
    } else if first == "self" {
        module
            .into_iter()
            .chain(segs[1..].iter().cloned())
            .collect()
    } else if first == "super" {
        let ups = segs.iter().take_while(|s| *s == "super").count();
        if ups > module.len() {
            return None;
        }
        module.truncate(module.len() - ups);
        module
            .into_iter()
            .chain(segs[ups..].iter().cloned())
            .collect()
    } else {
        return None; // an external-crate root — nothing here can resolve it
    };
    let mut segments = syn::punctuated::Punctuated::new();
    for id in out {
        segments.push(PathSegment {
            ident: id,
            arguments: PathArguments::None,
        });
    }
    Some(Path {
        leading_colon: None,
        segments,
    })
}

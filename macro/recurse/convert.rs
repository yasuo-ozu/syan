use super::*;

// The public cycle types are emitted as *natural* recursive types; `Parse` is delegated to the
// depth-limited engine (`__XRec`) and the parsed engine value is converted back to the natural type by
// a generated `__ToNat_X` trait. See `docs/recurse-natural-types-plan.md` §4. A field is a *recursive
// child* iff its container-peeled head ∈ the SCC (`child_heads`); the conversion descends children via
// `.__to_nat()` (resolved by receiver type) and moves leaves as-is.

/// Direction of a natural↔engine field/variant conversion. The tree-walk (`conv_expr`/`conv_body`) is
/// identical either way; only the recursive-child call, the by-value-vs-by-reference container access,
/// and the leaf handling differ — captured here so one pair of functions serves both bridges.
#[derive(Clone, Copy)]
pub(crate) enum ConvDir {
    /// engine → natural, **by value**: a recursive child is `val.__to_nat()`; containers move
    /// (`into_iter`/`*box`/`Option::map`); a leaf is used unchanged. Backs `__ToNat` (delegated `Parse`).
    ToNat,
    /// natural → engine, **by reference**: a recursive child is `__FromNat_<head>::__from_nat(val)`;
    /// containers borrow (`iter`/`&**box`/`Option::as_ref().map`); a leaf is `Clone`d. Backs `__FromNat`
    /// (delegated `Unparse`/`Spanned`). Carries the `nonce` (the `__FromNat_<head>` trait name needs it).
    FromNat { nonce: u64 },
}

impl ConvDir {
    /// The recursive-child conversion of `val` whose peeled head ident is `head`.
    fn child_call(self, head: &str, val: &TokenStream) -> TokenStream {
        match self {
            ConvDir::ToNat => quote!( #val.__to_nat() ),
            ConvDir::FromNat { nonce } => {
                let tn = from_nat_name(head, nonce);
                quote!( #tn::__from_nat(#val) )
            }
        }
    }
    fn box_elem(self, val: &TokenStream) -> TokenStream {
        match self {
            ConvDir::ToNat => quote!( (*#val) ),
            ConvDir::FromNat { .. } => quote!( (&**#val) ),
        }
    }
    fn map_seq(self, val: &TokenStream, body: &TokenStream) -> TokenStream {
        match self {
            ConvDir::ToNat => quote!( #val.into_iter().map(|__e| #body).collect() ),
            ConvDir::FromNat { .. } => quote!( #val.iter().map(|__e| #body).collect() ),
        }
    }
    fn map_opt(self, val: &TokenStream, body: &TokenStream) -> TokenStream {
        match self {
            ConvDir::ToNat => quote!( #val.map(|__e| #body) ),
            ConvDir::FromNat { .. } => quote!( #val.as_ref().map(|__e| #body) ),
        }
    }
    fn leaf(self, b: &TokenStream) -> TokenStream {
        match self {
            ConvDir::ToNat => quote!( #b ),
            ConvDir::FromNat { .. } => quote!( ::core::clone::Clone::clone(#b) ),
        }
    }
}

/// Build the conversion *expression* for one field value `val` of (original) type `ty` in direction
/// `dir`: `None` for a leaf (the caller carries it with `dir.leaf`), else the converted value.
/// `child_heads` is the set of SCC type names; a peeled head in it is a recursive child. Containers
/// (`Box`/`Vec`/`VecDeque`/`Punctuated`/`Option`) and tuples are lowered recursively; anything else is a
/// leaf. Leaf-ness does not depend on `dir`.
pub(crate) fn conv_expr(ty: &Type, val: TokenStream, child_heads: &HashSet<String>, dir: ConvDir) -> Option<TokenStream> {
    match ty {
        Type::Path(TypePath { qself: None, path }) => {
            let seg = path.segments.last()?;
            let name = seg.ident.to_string();
            // A recursive-child reference is always a same-module *bare* ident (`Stmt`, `Stmt<S>`); a
            // foreign multi-segment path (`other::Stmt`) whose last segment merely collides with a cycle
            // name is a leaf. (Mirrors `transform_type`/`collect_refs` keying on the first segment.)
            if path.segments.len() == 1 && child_heads.contains(&name) {
                return Some(dir.child_call(&name, &val));
            }
            match name.as_str() {
                "Box" => conv_expr(first_ty_arg(seg)?, dir.box_elem(&val), child_heads, dir)
                    .map(|c| quote!( ::std::boxed::Box::new(#c) )),
                "Vec" | "VecDeque" | "Punctuated" => conv_expr(first_ty_arg(seg)?, quote!(__e), child_heads, dir)
                    .map(|c| dir.map_seq(&val, &c)),
                "Option" => conv_expr(first_ty_arg(seg)?, quote!(__e), child_heads, dir)
                    .map(|c| dir.map_opt(&val, &c)),
                _ => None,
            }
        }
        Type::Tuple(t) => {
            let binds: Vec<Ident> = (0..t.elems.len())
                .map(|i| Ident::new(&format!("__t{i}"), Span::call_site()))
                .collect();
            let mut any = false;
            let convs: Vec<TokenStream> = t
                .elems
                .iter()
                .zip(&binds)
                .map(|(e, b)| match conv_expr(e, quote!(#b), child_heads, dir) {
                    Some(c) => {
                        any = true;
                        c
                    }
                    None => dir.leaf(&quote!(#b)),
                })
                .collect();
            any.then(|| quote!( { let (#(#binds,)*) = #val; (#(#convs,)*) } ))
        }
        _ => None,
    }
}

/// The conversion *body* (a `match`/`let` expression) that builds a `tgt_id` value from a `src_id` value
/// bound to `scrutinee`, in direction `dir`. Engine and natural share variant/field names. Field-level
/// conversion is `conv_expr`; a leaf field is carried by `dir.leaf`. For `ToNat`: `src=engine`,
/// `tgt=natural`, `scrutinee=self`. For `FromNat`: `src=natural`, `tgt=engine`, `scrutinee=__nat` (a
/// `&Natural`, so its bindings are references the leaf `Clone`s).
pub(crate) fn conv_body(
    item: &Item,
    src_id: &Ident,
    tgt_id: &Ident,
    scrutinee: TokenStream,
    child_heads: &HashSet<String>,
    dir: ConvDir,
) -> TokenStream {
    let arm_fields = |fields: &Fields| -> (TokenStream, TokenStream) {
        match fields {
            Fields::Named(FieldsNamed { named, .. }) => {
                let names: Vec<&Ident> = named.iter().map(|f| f.ident.as_ref().unwrap()).collect();
                let vals: Vec<TokenStream> = named
                    .iter()
                    .map(|f| {
                        let n = f.ident.as_ref().unwrap();
                        let v = conv_expr(&f.ty, quote!(#n), child_heads, dir)
                            .unwrap_or_else(|| dir.leaf(&quote!(#n)));
                        quote!( #n: #v )
                    })
                    .collect();
                (quote!( { #(#names),* } ), quote!( { #(#vals),* } ))
            }
            Fields::Unnamed(FieldsUnnamed { unnamed, .. }) => {
                let binds: Vec<Ident> = (0..unnamed.len())
                    .map(|i| Ident::new(&format!("__f{i}"), Span::call_site()))
                    .collect();
                let vals: Vec<TokenStream> = unnamed
                    .iter()
                    .zip(&binds)
                    .map(|(f, b)| {
                        conv_expr(&f.ty, quote!(#b), child_heads, dir)
                            .unwrap_or_else(|| dir.leaf(&quote!(#b)))
                    })
                    .collect();
                (quote!( ( #(#binds),* ) ), quote!( ( #(#vals),* ) ))
            }
            Fields::Unit => (quote!(), quote!()),
        }
    };
    match item {
        Item::Enum(e) => {
            let arms: Vec<TokenStream> = e
                .variants
                .iter()
                .map(|v| {
                    let vn = &v.ident;
                    let (pat, ctor) = arm_fields(&v.fields);
                    quote!( #src_id::#vn #pat => #tgt_id::#vn #ctor, )
                })
                .collect();
            quote!( match #scrutinee { #(#arms)* } )
        }
        Item::Struct(s) => {
            let (pat, ctor) = arm_fields(&s.fields);
            match &s.fields {
                Fields::Unit => quote!( #tgt_id ),
                _ => quote!( { let #src_id #pat = #scrutinee; #tgt_id #ctor } ),
            }
        }
        _ => quote!(),
    }
}

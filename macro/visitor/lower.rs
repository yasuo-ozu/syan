use super::*;

/// Right-nested `(self.0.into_hook(), (self.1.into_hook(), ...))` over tuple members — a tuple of hooks
/// is a hook (see `gen_side`), so this composes them with no combinator type.
pub(crate) fn build_chain(members: &[Index], into_hook: &Ident) -> TokenStream {
    let m = &members[0];
    if members.len() == 1 {
        quote!(self.#m.#into_hook())
    } else {
        let rest = build_chain(&members[1..], into_hook);
        quote!( (self.#m.#into_hook(), #rest) )
    }
}

/// `IntoVisitor[Mut]` impls for tuples of closures, arity 2..=`max_arity`. `union_where` are the
/// visited types' `where`-predicates, appended to each impl (it names the visited types via the
/// `IntoHook<.., T>` bounds, so they must stay well-formed).
pub(crate) fn tuple_impls(
    max_arity: usize,
    g_params: &[GenericParam],
    g_args: &[TokenStream],
    g_use: &TokenStream,
    mutable: bool,
    union_where: &[WherePredicate],
) -> Vec<TokenStream> {
    let suffix = if mutable { "Mut" } else { "" };
    let into_vis_tr = Ident::new(&format!("IntoVisitor{suffix}"), Span::call_site());
    let into_hook_tr = Ident::new(&format!("IntoHook{suffix}"), Span::call_site());
    let into_vis_fn = Ident::new(&format!("into_visitor{}", mt(mutable)), Span::call_site());
    let into_hook_fn = Ident::new(&format!("into_hook{}", mt(mutable)), Span::call_site());
    let visit_tr = Ident::new(&format!("Visit{suffix}"), Span::call_site());
    let driver = Ident::new(&format!("Driver{suffix}"), Span::call_site());
    // Helper param prefixes that avoid the visited types' own generic param names (so a visited type
    // may declare a param literally named `__F0`/`__T0`/…).
    let reserved: HashSet<String> = g_params.iter().map(param_name).collect();
    let pf = fresh_prefix("__F", &reserved, max_arity);
    let pt = fresh_prefix("__T", &reserved, max_arity);
    (2..=max_arity)
        .map(|n| {
            let fs: Vec<Ident> = (0..n)
                .map(|i| Ident::new(&format!("{pf}{i}"), Span::mixed_site()))
                .collect();
            let ts: Vec<Ident> = (0..n)
                .map(|i| Ident::new(&format!("{pt}{i}"), Span::mixed_site()))
                .collect();
            let members: Vec<Index> = (0..n).map(Index::from).collect();
            let wheres: Vec<TokenStream> = fs
                .iter()
                .zip(&ts)
                .map(|(f, t)| quote!(#f: #into_hook_tr< #(#g_args,)* #t >))
                .collect();
            let chain = build_chain(&members, &into_hook_fn);
            quote! {
                impl< #(#g_params,)* #(#fs,)* #(#ts,)* >
                    #into_vis_tr< #(#g_args,)* ( #(#ts,)* ) > for ( #(#fs,)* )
                where #(#wheres,)* #(#union_where,)*
                {
                    fn #into_vis_fn(self) -> impl #visit_tr #g_use {
                        #driver( #chain )
                    }
                }
            }
        })
        .collect()
}

/// The `#[seq]`/`#[opt]` marker on a field (`None` if unmarked), preserved into the `#[derive(Ast)]`
/// metadata so the visitor can dispatch the field through its `SeqView`/`OptView` view.
fn field_view(attrs: &[Attribute]) -> Option<Container> {
    let seq = attrs.iter().any(|a| a.path().is_ident("seq"));
    let opt = attrs.iter().any(|a| a.path().is_ident("opt"));
    match (seq, opt) {
        (true, true) => {
            let bad = attrs.iter().find(|a| a.path().is_ident("opt")).unwrap();
            abort!(bad, "a field cannot be both `#[seq]` and `#[opt]`");
        }
        (true, false) => Some(Container::Seq),
        (false, true) => Some(Container::Opt),
        (false, false) => None,
    }
}

/// The bare marker word (`"seq"`/`"opt"`) for a view kind, for diagnostics.
fn marker_word(kind: &Container) -> &'static str {
    match kind {
        Container::Seq => "seq",
        Container::Opt => "opt",
    }
}

/// Lowers a visited type's `visit_*` body: a field followed via a *visited/inherited* head becomes a
/// `this.visit_<head>(..)` method call; a field followed via an *unlisted intermediate* is drilled
/// through inline (its def destructured, recursing into its `#[subast]` fields); any other field is
/// a leaf.
pub(crate) struct Lower<'a> {
    /// Heads that get a method call (the `visitor!(..)` set ∪ inherited).
    pub(crate) method_set: &'a HashSet<String>,
    /// Fetched types keyed by `norm_path`, for resolving an intermediate's def when drilling.
    pub(crate) done_by_path: &'a HashMap<String, &'a DoneType>,
    pub(crate) mutable: bool,
    /// (mut walk) heads reached in a `#[seq]`/`#[opt]` field — drive which `visit_<t>_seq`/`_opt`
    /// methods `gen_side` emits.
    pub(crate) seq_used: &'a RefCell<HashSet<String>>,
    pub(crate) opt_used: &'a RefCell<HashSet<String>>,
}

impl<'a> Lower<'a> {
    fn amp(&self) -> TokenStream {
        if self.mutable {
            quote!(&mut)
        } else {
            quote!(&)
        }
    }

    /// Emit `this.visit_<head>_seq(binding)` / `_opt(binding)` for a `#[seq]`/`#[opt]` field, recording
    /// the `(head, kind)` usage so `gen_side` emits the method. `binding` is the `&mut <field>`, whose
    /// type `impl`s `SeqView<head>`/`OptView<head>` (box-transparently), so it is passed as-is.
    fn view_dispatch(&self, head: &Ident, binding: &TokenStream, kind: &Container) -> TokenStream {
        let (used, suffix) = match kind {
            Container::Seq => (self.seq_used, "seq"),
            Container::Opt => (self.opt_used, "opt"),
        };
        used.borrow_mut().insert(head.to_string());
        let m = Ident::new(&format!("visit_{}_{suffix}", to_snake(head)), Span::call_site());
        quote!( this.#m(#binding); )
    }

    /// Visit a value `access` (a `&head` / `&mut head` expression — the head is reached directly, any
    /// `Box`/`Attempt` wrappers having been descended as `View` levels). A method head emits a call; an
    /// unlisted intermediate is drilled inline (reborrow to a `&head` scrutinee, then destructure). May
    /// be empty (a finite drill that reaches no visited type).
    fn visit_value(
        &self,
        access: &TokenStream,
        head: &Ident,
        drill_path: &Path,
        depth: usize,
        stack: &mut Vec<String>,
    ) -> TokenStream {
        if self.method_set.contains(&head.to_string()) {
            let m = method_ident_m(head, self.mutable);
            return quote!( this.#m(#access); );
        }
        // Unlisted intermediate -> inline drill.
        let key = norm_path(drill_path);
        if stack.iter().any(|s| s == &key) {
            abort!(
                head,
                "`#[subast]` cycle through unlisted intermediate `{}`: it cannot be drilled inline. \
                 List one of the cycle's types in `visitor!(..)` so a method call breaks the recursion",
                head
            );
        }
        let dt = match self.done_by_path.get(&key) {
            Some(dt) => *dt,
            None => abort!(
                head,
                "internal: no metadata fetched for drilled type `{}` ({})",
                head,
                key
            ),
        };
        stack.push(key);
        let amp = self.amp();
        let scrut = quote!( #amp * #access );
        let block = self.destructure(&dt.def, &dt.subast, &dt.path, &scrut, depth + 1, stack);
        stack.pop();
        block
    }

    /// Destructure `scrutinee` (a `&T`/`&mut T` expr) per `def`/`subast` and visit followed fields.
    /// Empty when no followed field anywhere reaches a visited type.
    pub(crate) fn destructure(
        &self,
        def: &Item,
        subast: &[SubEntry],
        path: &Path,
        scrutinee: &TokenStream,
        depth: usize,
        stack: &mut Vec<String>,
    ) -> TokenStream {
        let self_ident = item_ident(def);
        match def {
            Item::Enum(e) => {
                let mut arms = Vec::new();
                let mut any = false;
                for v in &e.variants {
                    let (pat, stmts, has) =
                        self.fields(&v.fields, subast, self_ident, path, depth, stack);
                    any |= has;
                    let vident = &v.ident;
                    arms.push(quote!( #path::#vident #pat => { #stmts } ));
                }
                if !any {
                    return quote!();
                }
                quote!( match #scrutinee { #(#arms)* } )
            }
            Item::Struct(s) => {
                let (pat, stmts, has) =
                    self.fields(&s.fields, subast, self_ident, path, depth, stack);
                if !has {
                    return quote!();
                }
                match &s.fields {
                    Fields::Unit => quote!(),
                    _ => quote!( { let #path #pat = #scrutinee; #stmts } ),
                }
            }
            _ => quote!(),
        }
    }

    /// Build `(pattern, statements, has_any_visit)` for a field set. `self_ident` is the type being
    /// destructured (a field whose head is it is followed by implicit self-recursion).
    fn fields(
        &self,
        fields: &Fields,
        subast: &[SubEntry],
        self_ident: Option<&Ident>,
        path: &Path,
        depth: usize,
        stack: &mut Vec<String>,
    ) -> (TokenStream, TokenStream, bool) {
        match fields {
            Fields::Named(named) => {
                let mut binds = Vec::new();
                let mut stmts = Vec::new();
                for (idx, f) in named.named.iter().enumerate() {
                    let name = f.ident.clone().unwrap();
                    let bind = quote!(#name);
                    let view = field_view(&f.attrs);
                    if let Some(stmt) = self
                        .lower_field(&f.ty, &bind, view, idx, subast, self_ident, path, depth, stack)
                    {
                        binds.push(quote!(#name));
                        stmts.push(stmt);
                    }
                }
                let has = !stmts.is_empty();
                (quote!( { #(#binds,)* .. } ), quote!( #(#stmts)* ), has)
            }
            Fields::Unnamed(unnamed) => {
                let mut pats = Vec::new();
                let mut stmts = Vec::new();
                for (idx, f) in unnamed.unnamed.iter().enumerate() {
                    let bind_id = Ident::new(&format!("__f{depth}_{idx}"), Span::call_site());
                    let bind = quote!(#bind_id);
                    let view = field_view(&f.attrs);
                    if let Some(stmt) = self
                        .lower_field(&f.ty, &bind, view, idx, subast, self_ident, path, depth, stack)
                    {
                        pats.push(quote!(#bind_id));
                        stmts.push(stmt);
                    } else {
                        pats.push(quote!(_));
                    }
                }
                let has = !stmts.is_empty();
                (quote!( ( #(#pats),* ) ), quote!( #(#stmts)* ), has)
            }
            Fields::Unit => (quote!(), quote!(), false),
        }
    }

    /// Lower one field. `binding` is the destructured field (a `&Field`/`&mut Field`). `view` is the
    /// field's `#[seq]`/`#[opt]` marker (the visitor dispatches such a field through its container-edit
    /// view). Returns the visit statement(s), or `None` for a leaf / finite dead-end (binds `_`).
    #[allow(clippy::too_many_arguments)]
    fn lower_field(
        &self,
        ty: &Type,
        binding: &TokenStream,
        view: Option<Container>,
        idx: usize,
        subast: &[SubEntry],
        self_ident: Option<&Ident>,
        path: &Path,
        depth: usize,
        stack: &mut Vec<String>,
    ) -> Option<TokenStream> {
        let user_types = self_and_subast_keys(self_ident, subast);
        let p = match peel(ty, &user_types) {
            Some(p) => p,
            // No followed head reachable ⇒ the field is a leaf. A `#[seq]`/`#[opt]` marker on such a
            // field would route nowhere and silently no-op — abort so the mistake is caught here.
            None => {
                if let Some(kind) = view {
                    let marker = marker_word(&kind);
                    abort!(
                        ty,
                        "a `#[{}]` field's element type is not a visited type — mark only a field whose \
                         element is a container of a type listed in `visitor!(..)` (or reached via \
                         `#[subast]`)",
                        marker
                    );
                }
                return None;
            }
        };
        // A field behind a shared reference (`&T`/`&[T]`) is visitable on the shared side but a leaf
        // for `visit_mut` — there is no `&mut head` reachable through a `&`.
        if self.mutable && p.shared_ref {
            return None;
        }
        // Dispatch at the innermost (container-peeled) accessor, then wrap the container chain
        // (handles nested containers like `Vec<Option<T>>`, and now `Vec<(A, B)>`). An empty body
        // (a leaf head, or a finite drill reaching nothing) ⇒ the whole field is a leaf.
        let acc = innermost_acc(&p.conts, binding);
        // The effective head type (real ident + path) when this is a followed `Head::Path`: self
        // (implicit) or a `#[subast]` entry (an aliased `Real as Aliased` dispatches to `visit_real`).
        let resolved: Option<(Ident, Path)> = match &p.head {
            Head::Path { head: phead, .. } if Some(phead) == self_ident => {
                Some((phead.clone(), path.clone()))
            }
            Head::Path { head: phead, .. } => subast
                .iter()
                .find(|e| &e.key == phead)
                .map(|e| (last_ident(&e.path).clone(), e.path.clone())),
            Head::Tuple(_) => None,
        };

        // A `#[seq]`/`#[opt]`-marked field is edited in place through the whole field's `SeqView`/`OptView`
        // (`visit_mut` only). The field must be a **single** container of the visited head
        // (`Vec<Head>`/`Option<Head>`/…): the marker picks the view method, and the `SeqView<Head>` /
        // `OptView<Head>` bound on the generated method self-validates Seq-vs-Opt (a `#[seq]` on an
        // `Option` fails the bound). No container name is matched.
        if self.mutable {
            if let Some(kind) = view {
                let marker = marker_word(&kind);
                let head = match &resolved {
                    Some((h, _)) if self.method_set.contains(&h.to_string()) => h,
                    _ => abort!(
                        ty,
                        "a `#[{}]` field's element type is not a visited type — mark only a field whose \
                         element is a single container of a type listed in `visitor!(..)` (or reached via \
                         `#[subast]`)",
                        marker
                    ),
                };
                match p.conts.as_slice() {
                    [LayerKind::View] => {}
                    [] => abort!(
                        ty,
                        "a `#[{}]` marker needs a container field (e.g. `Vec<{}>` / `Option<{}>`); this is a \
                         direct/single-value field",
                        marker,
                        head,
                        head
                    ),
                    [_] => abort!(
                        ty,
                        "a `#[{}]` field's container is an array/slice — not structurally editable; use \
                         `Vec`/`VecDeque`/`Punctuated`/`Option`",
                        marker
                    ),
                    _ => abort!(
                        ty,
                        "a `#[{}]` field must be a *single* container of the element (e.g. `Vec<{}>`); a \
                         nested or wrapped container (`Vec<Box<{}>>`, `Box<Vec<{}>>`, …) can't be edited in \
                         place — use a bare `Vec<{}>` / `Option<{}>`",
                        marker,
                        head,
                        head,
                        head,
                        head,
                        head
                    ),
                }
                let dispatch = self.view_dispatch(head, binding, &kind);
                return Some(dispatch);
            }
        }

        let body = match &p.head {
            // A tuple at the innermost position: destructure and lower each element (an element may
            // itself be a followed type, a container of one, or a nested tuple).
            Head::Tuple(elems) => {
                self.lower_tuple(elems, &acc, idx, subast, self_ident, path, depth, stack)
            }
            Head::Path { .. } => match &resolved {
                Some((head, drill_path)) => self.visit_value(&acc, head, drill_path, depth, stack),
                None => quote!(),
            },
        };
        (!body.is_empty()).then(|| fold_containers(&p.conts, binding, body, self.mutable))
    }

    /// Lower a tuple at the (container-peeled, box-dereffed) accessor `acc`: destructure it and lower
    /// each element. Leaf elements bind `_`; an empty result (no followed element) makes the tuple a
    /// leaf. Mirrors the `#[recurse]` path's `recurse_lower_tuple`.
    #[allow(clippy::too_many_arguments)]
    fn lower_tuple(
        &self,
        elems: &[Type],
        acc: &TokenStream,
        idx: usize,
        subast: &[SubEntry],
        self_ident: Option<&Ident>,
        path: &Path,
        depth: usize,
        stack: &mut Vec<String>,
    ) -> TokenStream {
        let mut pats = Vec::new();
        let mut stmts = Vec::new();
        for (i, elem) in elems.iter().enumerate() {
            let ebind = Ident::new(&format!("__t{depth}_{idx}_{i}"), Span::call_site());
            // A tuple element is a bare type with no field attrs, so it can carry no `#[seq]`/`#[opt]`.
            if let Some(stmt) = self.lower_field(
                elem,
                &quote!(#ebind),
                None,
                idx,
                subast,
                self_ident,
                path,
                depth + 1,
                stack,
            ) {
                pats.push(quote!(#ebind));
                stmts.push(stmt);
            } else {
                pats.push(quote!(_));
            }
        }
        if stmts.is_empty() {
            return quote!(); // tuple of only leaves -> leaf (empty body)
        }
        let amp = self.amp();
        quote!( { let ( #(#pats,)* ) = #amp * #acc; #(#stmts)* } )
    }
}

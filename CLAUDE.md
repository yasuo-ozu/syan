# Visitor system — current state

Generate `syn`-style visitors from AST type definitions.

Code: `core/src/visit.rs` (`Ast`, `Repeater`, `SeqView`/`OptView`), `macro/ast.rs` (`#[derive(Ast)]`),
`macro/visitor.rs` + `macro/visitor/{entry,build_input,discover,lower,params,side}.rs` (`visitor!`
shim → `__visitor_entry` → `__visitor_build`; the `Lower` walk in `lower.rs`, `gen_side` in `side.rs`),
peel/fold helpers in `macro/util.rs`. (`macro/recurse.rs` and `macro/attribute.rs` are likewise split
into `macro/recurse/{graph,transform,convert,names,emit,build,items}.rs` and
`macro/attribute/{find,substruct,adt}.rs`; every source file is <1000 lines.) Tests:
`core/tests/visitor_*.rs`, `core/tests/ast_derive.rs`; cross-crate:
`rust/tests/cross_crate.rs` with the AST sample in `rust/src/lib.rs`.

## Shipped & tested

- **`#[derive(Ast)]`**: empty `Ast` impl + a `#[macro_export]` metadata macro carrying a cleaned def
  (re-parsed downstream as a `syn::Item`), re-exported under the type's own name (`path::to::T!{..}`;
  type/macro namespaces coexist). Plus one `Repeater<N>` impl **on the type itself** per context-
  dependent field type. `crate::`-rooted `#[subast]` paths are emitted `$crate`-rooted for downstream.
- **`visitor!([base =>] T, …)`**: invoked **inside** an (empty) `mod`; a `macro_rules!` shim captures
  `$crate`. A metadata ping-pong (`__visitor_build`) emits, per visited type, a `Visit`/`VisitMut`
  trait method, a free `visit_*`/`visit_*_mut` fn, and **inherent** `visit`/`visit_mut`. Type args are
  paths resolvable from inside the visitor module (`super::Expr`, `crate::ast::Expr`).
- **`#[subast(<path> [as Alias])]` + drill-in**: a type-level allowlist of a type's Ast children
  (carried as `@subast{path as key}`); a field is *followed* iff its peeled head is a matchkey or the
  type's own ident, else a leaf. `visit_*` is generated **only** for `visitor!(…)`-listed types; a
  followed *unlisted* intermediate is **drilled inline** (no `visit_*`); a cycle of unlisted
  intermediates errors, a finite dead-end is a no-op. Diagnostics: same-last-segment collision (error),
  `unused entry` warning, "follows nothing" lint (nightly). Tests: `visitor_drill*.rs` (+ "Drill-in"
  section below).
- **Containers — zero hardcoded type names**: `peel` (`macro/util.rs`) walks the field's `syn::Type`
  to the `#[subast]`-matching (or self-ident) **head**; every path wrapper around it is a
  `LayerKind::View` layer descended by a `view_iter[_mut]()` **method call** — **method resolution**
  picks the `SeqView`/`OptView` impl (`Vec`/`VecDeque`/`Punctuated` are `SeqView<T>`; `Option` is
  `OptView<T>`; `Box`/`Attempt` are single-slot `OptView<T>`; a **user wrapper joins by implementing a
  view** — no annotation, no name list). Arrays/slices are `LayerKind::Raw` layers (slice
  `iter[_mut]()`; syntax shapes, not names). Descent **recurses per layer**, so **nested** containers
  (`Vec<Option<T>>`), boxed elements (`Vec<Box<T>>` = a `SeqView` layer then an `OptView` layer), and a
  **tuple** at the peeled position — top-level *or* behind containers/boxes (`Vec<(A,B)>`, `Box<(A,B)>`)
  — all descend for free (`Head::{Path,Tuple}`; a tuple is destructured, each element lowered). Tests:
  `visitor_containers.rs` (nested / container-of-tuple / tuple field), `visitor_recurse_shapes.rs`
  (the same shapes through a `#[recurse]` cycle), `seqview_iter.rs` (the view iterators).
- **Inputs**: struct visitors (`&mut`), single closures, and **tuples of closures** (2..=8) in **one**
  pass (`Hook` + `Driver`; a tuple of hooks is itself a `Hook`, so it is the chaining combinator — no
  newtype).
- **`visit_mut`**: full in-place mirror. Reduce/append by overriding the *parent*'s `visit_*_mut`
  (it owns the `&mut Vec`/`&mut Option`) — `visitor_edit.rs::views::parent_override_still_works`.
- **Structural edits (container views — `visit_*_seq` / `visit_*_opt`)**: a field **explicitly marked
  `#[seq]` or `#[opt]`** (a `#[derive(Ast)]` helper attr, preserved into the metadata by
  `cleaned_definition`) makes its head visited type get a **new, additive** mut-trait method that
  receives a *view of the owning collection/Option* and edits it **in place, no clone** —
  `visit_<t>_seq(&mut self, &mut impl SeqView<T>)` / `visit_<t>_opt(&mut self, &mut impl OptView<T>)`.
  There is **no auto-detection from the container type**: an *unmarked* `Vec`/`Option` field is an
  ordinary (non-structural) descent and yields *no* view method (`ui/visitor_edit_unmarked_no_view.rs` —
  overriding it is "not a member of trait"). `SeqView<T>`/`OptView<T>` are **public traits in
  `core::visit`, bare-element** (the element *is* the viewed node): `impl<T> SeqView<T> for
  Vec<T>`/`VecDeque<T>`/`Punctuated<T, P: Default>`, `impl<T> OptView<T> for Option<T>`, and
  `Box<T>`/`Attempt<T>` as **single-slot `OptView<T>`** (`is_some`=true, `get`/`get_mut` via deref,
  `set` replaces, `take` unreachable — a single wrapper is descent-only). There are **no wrapped-element
  impls and no forwarders** (the old `Vec<Box<T>>: SeqView<T>` element forms, the
  `Box<inner-view>` forwarder, and the `Wrap` trait are all gone — wrapped shapes descend by per-layer
  recursion instead). A marked field must therefore be a **bare single container of the head**:
  `#[seq] Vec<Head>` / `#[opt] Option<Head>` (also `#[opt] Box<Head>` — a valid always-full slot view;
  `set`/`get_mut` work, `take`/`clear` would panic). A **wrapped or nested** marked container
  (`#[seq] Box<Vec<T>>`, `#[seq] Vec<Box<T>>`, `Vec<Option<T>>`) is a clean macro `abort!` at the field
  type (`ui/visitor_edit_marker_boxed.rs`); a **kind mismatch** on
  a single container (`#[seq]` on an `Option`/`Box`) fails the generated method's `SeqView<Head>` bound
  (E0277 — the marker + bound self-validate); array/non-container/unvisited-element markers are macro
  aborts (`ui/visitor_edit_marker_{array,noncontainer,unvisited}.rs`). Such fields still *descend* —
  they just aren't edit targets. The descent passes the field `&mut` directly (no wrapper).
  `SeqView`: `len`/`get`/`get_mut`/`insert`/`remove` core +
  `push`/`retain_mut`/`view_iter`/`view_iter_mut` (`SeqIter`/`SeqIterMut` yield `&T`/`&mut T` by index for
  in-place edits — structural changes go through `push`/`insert`/`remove`/`retain_mut`); `OptView`:
  `is_some`/`get`/`get_mut`/`set`/`take` + `view_iter`/`view_iter_mut` (0-or-1 via `get().into_iter()`).
  The iterators are `view_iter`/`view_iter_mut` (not `iter`/`iter_mut`) precisely so they never shadow the
  `Deref`-reached slice `iter`/`iter_mut` when `SeqView` is imported — `vec.iter()` stays the std method.
  (`push`/`retain_mut` are inherent on `Vec` and win regardless; `get`/`get_mut` are still on `Vec`
  directly, so those two shadow the slice's — use UFCS if you need the slice ones.) **`visit_*_mut` interface unchanged**
  (`visit_<t>_mut(&mut self, &mut T) -> ()`); the seq/opt trait-method **defaults inline the descent**
  (iterate the view calling `visit_<t>_mut` — there are no free `visit_*_seq`/`_opt` fns), so a
  `visit_*_mut`-only visitor (and closures via `Driver`) are unaffected. A marked
  field anywhere (listed *or* drilled) drives emission — usage collected by the mut `Lower` walk into
  `seq_used`/`opt_used` via `field_view`/`view_dispatch` (`macro/visitor/lower.rs`); emission in
  `gen_side` (`macro/visitor/side.rs`). Closures/`Hook` are
  non-editing (deferred). Tests: `visitor_edit.rs` (`#[seq]`/`#[opt]` on Vec+Option with parent-override
  style still working, unmarked fixed-slot in-place + plain-`visit_*_mut` regression,
  marked Vec+Option views with `push`/`set`/`take`, a self-recursive `#[seq] Vec<Expr>` inside a
  `#[recurse]` cycle (the `Vec` gives the indirection), a **multi-type cycle** cross-edge
  `#[seq] Vec<Stmt>`, a self-recursive `Option<Box<_>>` slot descended per level (not an edit target),
  `#[opt] Box<Leaf>` as a single-slot view, and a marked Vec/Option
  inside a **drilled** unlisted intermediate); `visitor_edit_containers.rs` (`#[seq]` `VecDeque` +
  `Punctuated` views, an **unmarked** Vec/Option still traversed by a closure, and `boxed_container` —
  bare `Punctuated`/`Option` edit views next to box-wrapped shapes that only descend);
  `visitor_edit_group.rs` (`#[seq]`/`#[opt]` on `#[group]`-carrying types, edit + `Unparse` round-trip,
  incl. inside a `#[recurse]` cycle);
  `rust/tests/cross_crate_edit.rs` (downstream `VisitMut` edits an upstream marked `Vec`/`Option` via core
  `SeqView`/`OptView`); `ui/visitor_edit_unmarked_no_view.rs`, `ui/visitor_edit_marker_*.rs`,
  `ui/visitor_edit_seq_inherited.rs`.
- **Inheritance `visitor!(base => New)`**: base exports a `__syan_visited` macro (visited idents +
  param union `@bg` + ancestor chain `@an`); New extends it via supertrait at the base's own arity.
  Wider new union and multi-level `base => mid => New` chains work — `visitor_inherit.rs` (mods
  `basic`/`arity`/`multilevel`).
- **Generics**: the trait is keyed on the **union** of visited types' params; each type uses its subset
  (`visitor_core.rs::generics`). Generated helper params avoid visited types' param names
  (`visitor_core.rs::hygiene`). Caveat: `.visit()` on a root that omits a union param may need a turbofish. A
  **`where`-bounded param not shared by all visited types** can't be a union trait param (a type lacking
  it would carry an undischargeable bound), so it becomes a **per-method generic** (`visit_bounded<S:
  Bound>`) with the trait keyed on the shared subset — going **struct-only** (a closure can't be
  `for<S>`), same machinery as the heterogeneous concrete-fill case (`method_mode`,
  `visitor_core.rs::union_where_unshared`). An *unbounded* unshared param instead stays in the union +
  closure path.
- **Cross-crate**: visited types named by full path → no downstream import (`cross_crate.rs`);
  downstream drill through upstream types via `$crate`-rooted `#[subast]` (`cross_crate_drill.rs`).
  Cross-crate **inheritance** is keyed on the base **path** (supertrait, inherited `base::visit_*`, the
  `pub use`'d `__syan_visited`); multi-level incl. an *upstream* intermediate works because
  `__visitor_build` **requalifies** a relative ancestor against the direct base's full path —
  `crate::` → the host crate, `super::`/`self::` → resolved against the base module (the consumer is
  given the intermediate's path, e.g. `super::base` off `syan_rust::inherit::mid_ss` ⇒
  `syan_rust::inherit::base`), via `base_host_crate`/`requalify_ancestor`. Tests:
  `cross_crate_inherit{,_multilevel,_4level,_downstream_mid}.rs`, `cross_crate_super_self.rs`.
- **`#[recurse]`** (type transformer; **takes no arguments** — the former `limit = N` was removed, the
  engine depth is the fixed internal `DEFAULT_RECURSION_DEPTH` in `macro/recurse.rs`): turns a module of
  mutually-recursive AST types into **natural recursive public types** + an internal fixed-depth
  **engine** that backs `Parse` (and group-ful `Unparse`/`Spanned`). Per SCC it emits: (1) the user's
  cycle types as **genuine natural recursive enums/structs** — the public API `Expr<S>` (one type at all
  depths), carrying `#[derive(Ast)]` + `Debug`/`Default`/… (`make_natural_item`). **`Parse`** is always
  re-supplied by **delegation** through the engine. **`Unparse`/`Spanned`** split by group-ness: a
  **group-free** cycle derives them **directly on the natural type** — `#[ignore_bounds]` on
  recursive-child fields drops the per-field `field_ty: Trait` where-bound (no E0275 where-cycle) and an
  injected item-level `#[predicate_unparse/spanned(<cycle leaf-field-type UNION>)]` re-adds exactly the
  leaf bounds a member's body needs to unparse/span its siblings (the *union* across all cycle members,
  so e.g. a `Stmt` lacking an `Integer` leaf still gets `Integer: Unparse` so it can build an `Expr`
  sibling) — making them **unbounded** (any depth); a **group-ful** cycle delegates them through a DEPTH-1
  **borrow** engine whose terminator re-enters the top-level impl at runtime (also **unbounded** — see
  below), because the self-recursive `#[group]` field's `for<'a> Fill<Substruct>: Unparse` HRTB forms a
  trait-solver cycle `#[ignore_bounds]` can't break (the engine's distinct finite types do); (2) a
  `pub(crate)` fixed-depth engine `__XxxRec<…, __Rec = __XxxDefault<…>>` family + **inhabited**
  terminators — `__XxxTerm` (newtype `Box<Root<…>>`, owned, for `Parse`) and, for group-ful U/S, a
  **borrow** terminator `__XxxTermRef<'a>(&'a Root<…>)` — + `__XxxDefault` depth chains (all
  **nonce-stamped**, so a user item named e.g. `ExprTerm` never collides), deriving the engine-routed
  traits, emitted **only when needed** (`scc_needs_engine` = derives `Parse`, or is group-ful and derives
  `Unparse`/`Spanned`; a cycle deriving none of those, e.g. Ast-only, gets no engine) (`make_engine_item`,
  `emit_terminator_and_reentry`, `emit_borrow_terminator_and_reentry`); (3) per-cycle `__ToNat_X`
  (engine→natural; the owned terminator's just unwraps its `Box`) and, for a group-ful cycle, `__FromNat_X`
  (natural→engine, **lifetime-parameterized** `<'__n>` so the borrow terminator can hold `&'__n` of the
  remainder — leaves cloned, recursive children borrowed) + the **delegated `impl`s**: `Parse`
  (`emit_delegated_parse`) registers each root's erased re-entry parser into `core::parse::vtable`, runs
  the owned engine, then `.__to_nat()`s; group-ful `Unparse`/`Spanned` (`emit_delegated_unparse`/`_spanned`)
  register each root's erased re-entry unparse/span fn, build the depth-1 borrow engine via `.__from_nat()`,
  then call the engine's impl (`gen_natural_extras`). A cycle type's `where`-clause is threaded onto the
  generated impls (`where_preds_of`); a **group-ful** cycle's `Group` uses a hand-written `Unparse<TokenTree>`
  emitting a single `TokenTree::Group` and a `Spanned` taking the span from its delimiters
  (`nested/group.rs`). The natural enum owns the name (no `pub type` alias); user inherent `impl`s land on
  the natural type verbatim. **`Parse`/`Unparse`/`Spanned` are all UNBOUNDED** despite the fixed engine
  depth: the engine's depth-floor terminator is inhabited and **re-enters the top-level natural impl at
  runtime through a type-erased fn pointer** (`Parse` erases `&mut dyn ParseStream`, keyed per `(terminator,
  atom, stream-error)`; group-ful `Unparse` erases `&mut dyn Emitter` via a `DynSink` wrapper, group-ful
  `Spanned` needs no erasure; `core::parse::vtable`; the delegated impl registers before descending)
  instead of erroring/panicking — so a tree deeper than the engine depth is handled in full (ceiling = the
  OS call stack; a *left-recursive* grammar therefore loops forever rather than being silently truncated as
  the old depth cap did). **Why these need the engine:** deriving `Parse` directly on a natural recursive
  type fails two ways — (a) per-field `field_ty: Parse` where-bounds form an infinite cycle (E0275); (b)
  backtracking `stream.dup(…)` wraps the stream in another `Dup<…>` per descent level → infinite
  stream-type monomorphization (also E0275). The fixed engine bottoms both out at compile time, and the
  erased re-entry restarts at one fixed `Dup<&mut dyn …>` layer that never grows. (Group-free
  `Unparse`/`Spanned` only hit (a) — which `#[ignore_bounds]` defuses — so they are direct; group-ful
  `Unparse`/`Spanned` additionally hit the `Fill` HRTB cycle, so they keep the engine + borrow re-entry.)
  Cycle types may carry lifetime/type/const params, possibly
  **heterogeneous** across the cycle; a back-edge to a root repeats the root's params **verbatim** (a
  non-identity arg like `Expr<Vec<S>>` is rejected — an engine constraint, kept). **Independent cycles**
  are partitioned into SCCs (`find_cycle_sccs`, Tarjan), each with its own natural+engine+conversions
  (`build_scc`). **Multi-root** cycles keep one engine depth dimension per root (`build_multiroot_tail`).
  **Finite-size precondition:** a natural recursive type must be finite-size, so a **pure by-value
  cycle** (no `Box`/`Vec`/… on any cycle edge) is rejected with a clean `abort!` (would be E0072) —
  detected via the direct-edge subgraph being acyclic (`subgraph_is_cyclic` on `direct_type_refs`). Clean
  `abort!`s also for a missing/non-identity root param and a non-acyclic rootless subcycle; passing any
  argument to `#[recurse]` is a clean compile error (`ui/recurse_takes_no_args.rs`). Code:
  `macro/recurse.rs` (root; entry + SCC partitioning) + `macro/recurse/{graph,transform,convert,names,
  emit,build,items}.rs`. Tests:
  `recurse_core.rs` (mods `basic`/`fixes`/`no_engine`/`where_clause`/`problems`),
  `recurse_visitor_cycles.rs` (mods `multi_cycle`/`multiroot`),
  `recurse_traits.rs` (mod `unparse_spanned` incl. `parse_unbounded_depth` depth-200; mod `group_ful`
  incl. `group_ful_unparse_is_unbounded`, `group_ful_spanned_folds_delimiters` depth-2000; mod
  `ignore_bounds`) + `ui/recurse_*.rs`, `ui/problem*.rs`; unbounded group-ful round-trip w/ backtracking:
  `rust/tests/rustsub_roundtrip.rs` (`deep_parens_round_trip_is_unbounded`).
- **`visitor!(…)` over a `#[recurse]` cycle** is now an **ordinary acyclic visitor** — the public types
  are natural (one type at all depths) and `Visit` methods carry no `field_ty: Visit` bounds, so there
  is no E0275 and no depth-generic machinery. `visit_xxx(&mut self, &Expr<S>)` like any acyclic type.
  **Closures** `|e: &Expr<S>|`, **tuples of closures**, inherent `.visit(closure)`, **`visit_mut`**, and
  **inheritance** `visitor!(base => New)` (via the normal supertrait — no `@recbase`) all work — this
  closes the long-deferred closure-over-recurse gap. **`#[subast]` is now required on cycle types** to
  follow cross-edges (a field is followed iff its peeled head ∈ its `#[subast(…)]` or is the type's own
  ident; e.g. `Expr` holding `Box<Stmt<S>>` needs `#[subast(crate::ast::Stmt)]`). **Heterogeneous
  concrete-fill** — a non-shared param *concrete-filled* in a cross-edge (`Box<Stmt<S, u8>>` where
  `Stmt<S,T>`'s `T` is non-root) keys the trait on the shared params and makes the non-shared param a
  per-method generic (`visit_stmt<T>`), going **struct-only** (a closure can't be `for<T>`); detected by
  `has_concrete_fill`. A non-shared *unbounded* param with **no** concrete fill (incl. a non-shared
  lifetime — works via subtyping) stays in the union+closure path. **Multi-root / multi-cycle /
  cross-crate** all work as ordinary acyclic visitors (no visitor-level depth dimensions — that's an
  internal engine `Parse` detail); cross-crate skips the inherent `.visit()` for a foreign target
  (E0116 — use `Visit::visit_*`), via `path_is_crate_local`. Tests: `visitor_recurse.rs` (mods
  `via_visitor` (+`visit_mut`, incl. `closure_over_recurse_cycle`), `disjoint_params`),
  `visitor_recurse_mixed.rs` (mods `one_visitor`/`extra_param`/`closure`/`drill`),
  `visitor_recurse_shapes.rs` (containers / container-of-tuple),
  `recurse_visitor_cycles.rs` (mod `generics` incl. `het`), `visitor_audits.rs` (mod
  `recurse_nonroot_lifetime`), `visitor_inherit.rs` (mods `over_recurse`/`over_recurse_mid`),
  `rust/tests/cross_crate_recurse.rs`.

## Known gaps / limitations

- **`#[recurse]` `Parse`/`Unparse`/`Spanned` are all UNBOUNDED** (no residual depth limit). Group-free
  `Unparse`/`Spanned` derive directly on the natural type (`#[ignore_bounds]` + the injected leaf-bound
  `#[predicate_*]` union — `recurse_traits.rs::unparse_spanned`: single-type depth-5000, multi-type
  depth-2000);
  `Parse` (always) and group-ful `Unparse`/`Spanned` delegate through a fixed-depth engine whose terminator
  **re-enters the top-level impl at runtime** via a type-erased fn pointer (`core::parse::vtable`) — `Parse`
  erases `&mut dyn ParseStream`, group-ful `Unparse` erases `&mut dyn Emitter` (via `DynSink`) on a *depth-1
  borrow* engine that borrows the remainder (no `Root: Clone`), group-ful `Spanned` likewise. Coverage:
  `recurse_traits.rs` (`parse_unbounded_depth` depth-200; mod `group_ful` round-trip + depth-2000 span),
  `rust/tests/rustsub_roundtrip.rs` (`deep_parens_round_trip_is_unbounded`, depth-60 multi-type group-ful
  w/ backtracking). **Caveat:** the runtime re-entry's only ceiling is the OS call stack, so a
  **left-recursive** cycle grammar now recurses forever instead of being silently truncated by the old
  depth cap (the honest recursive-descent behavior).
- **Two visited types sharing a last segment** (`visitor!(a::Foo, b::Foo)`): all generated names key
  off the last segment, so they collide. Now a clear build error (`visitor_diagnostics.rs`); genuine
  coexistence would need full-path-disambiguated names (the alias is one keyword — won't fix).
- **A `where`-bound naming a user trait must be in scope at the `visitor!()` site** (`use crate::Bound;`
  inside the visitor module). The generated impls repeat the bound by bare path, so an un-imported trait
  is unresolved — applies to any where-bound, shared or not. (The bounded param itself being *unshared*
  is supported — see "Shipped".) (A cycle following an **unlisted intermediate** that forms a cycle of
  unlisted intermediates is the general drill diagnostic — "list one" — incl. an omitted co-root:
  `ui/visitor_recurse_unlisted_coroot.rs`.)

---

# Drill-in (selective `visit_*` + transitive drill-through)

Two lists drive it. **`#[subast(<paths>)]`** (per `#[derive(Ast)]` type) is the *follow-list*: a field
is followed iff its container-peeled head is a listed entry or the type's own ident (self-recursion
implicit), else a leaf; the entry also gives the `$crate`-rooted path to that child's metadata macro.
**`visitor!(T, …)`** is the *method-list*: a followed head that is listed lowers to
`this.visit_<head>(field)`; a followed-but-*unlisted* head is **drilled inline** (recurse into its
`#[subast]` fields, no `visit_*` emitted for it); a cycle of unlisted intermediates errors ("list
one"). Wrappers around the head — incl. **nested** ones (`Vec<Option<_>>`) — are traversed by
per-layer `view_iter[_mut]()`/slice-`iter[_mut]()` calls (**no container name is matched**; method
resolution picks the `SeqView`/`OptView` impl — see "Containers" above). Membership + method-name
building live
in `__visitor_build` (the proc-macro), since `macro_rules!` can't compare or snake-case idents — the
metadata ping-pong only supplies each type's structure. Code: `macro/ast.rs` + `macro/visitor/lower.rs`
(`Lower`), `macro/util.rs` (`peel`/`fold_containers`); tests:
`core/tests/visitor_drill*.rs`, `visitor_diagnostics.rs`.

---

# `#[recurse]` expansion (natural types + internal engine, direct/delegated impls)

`#[recurse]` (no arguments — fixed engine depth `DEFAULT_RECURSION_DEPTH`) emits, per SCC:

1. **Natural public types** — the user's cycle types *un-renamed* (`Expr<S>`, one type at all depths),
   with the `#[derive(…)]` list rewritten: **`Parse`** is **removed** (re-supplied by delegation, below);
   **`Unparse`/`Spanned`** are **kept** for a group-free cycle (derived directly — see below) and
   **removed** for a group-ful cycle (delegated); everything else (`Ast`, `Debug`, `Default`, …) is kept.
   For a kept (group-free) `Unparse`/`Spanned` the natural type gets `#[ignore_bounds]` injected on each
   recursive-child field plus an item-level `#[predicate_unparse/spanned(<cycle leaf-type union>)]`; for a
   removed (engine-routed) derive, the field helper attrs (`#[group]`/`#[ignore_bounds]`/…) are stripped
   from the natural type (they live on the engine). `make_natural_item`.
2. **Engine types** (`pub(crate)`) — the fixed-depth `__XxxRec<…, __Rec = __XxxDefault<…>>` family
   (a back-edge to a root becomes the depth param `__Rec`, one per root) + **inhabited** terminators —
   `__XxxTerm` (newtype `Box<Root<…>>`, owned, for `Parse`) and (group-ful U/S) a **borrow** terminator
   `__XxxTermRef<'a>(&'a Root<…>)` — + `__XxxDefault` depth chains, deriving the engine-routed traits
   (`Parse` always; `Unparse`/`Spanned` only when group-ful). Each depth level is a *distinct* finite type,
   which bottoms out **both** Parse E0275 cycles (the per-field `field_ty: Parse` where-cycle **and** the
   `stream.dup(…)` `Dup<…>` stream-monomorphization cycle) and lets group-ful `Unparse`/`Spanned` derive
   normally. Each terminator **re-enters the top-level impl at runtime** through a type-erased fn pointer
   instead of erroring/panicking: `__XxxTerm::parse` erases `&mut dyn ParseStream`
   (`emit_terminator_and_reentry` + `__reentry_X`), and `__XxxTermRef`'s `Unparse`/`Spanned` erase `&mut
   dyn Emitter` (via `DynSink`) / nothing (`emit_borrow_terminator_and_reentry` + `__reentry_unparse_X`/
   `__reentry_span_X`) — so all three are **unbounded** despite the fixed type depth. Emitted only when
   `scc_needs_engine`. `make_engine_item`.
3. **Conversion + impls** — per cycle type: a private depth-generic `__ToNat_X` (engine→natural; always,
   for `Parse`; the owned terminator's `__to_nat` just unwraps its `Box`) and, for a group-ful cycle, a
   **lifetime-parameterized** `__FromNat_X<'__n>` (natural→**borrow** engine; a back-edge collapses to
   `__Rec`, a cross-edge bounds the sibling node, containers map element-wise, leaves are cloned; the
   borrow terminator's `__from_nat` just wraps `&'__n remainder` — no clone). **`Parse`**
   (`emit_delegated_parse`) **registers** each root's `__reentry_X` (keyed in `core::parse::vtable`), parses
   the owned engine, then `.__to_nat()`s; group-ful **`Unparse`/`Spanned`** (`emit_delegated_unparse`/
   `_spanned`) register each root's `__reentry_unparse_X`/`__reentry_span_X`, build the depth-1 borrow
   engine `__XRec<…, __XxxTermRef<'_, …>>` via `.__from_nat()`, then call the engine's impl. For a
   **group-free** cycle there is *no* `__FromNat`/delegated `Unparse`/`Spanned` — those are derived
   **directly** on the natural type (step 1). All paths are **unbounded** (any depth).
   `gen_natural_extras`, `conv_body`/`conv_expr`.

The natural enum owns the name (no `pub type` alias); user inherent `impl`s land on the natural type
verbatim. A **pure by-value cycle** (no heap indirection on any edge) is
rejected (`abort!`, would be E0072) — the natural type would be infinite-size; checked via the
direct-edge subgraph being acyclic (`subgraph_is_cyclic` on `direct_type_refs`).

## How `visitor!()` consumes it — ordinary acyclic metadata

The natural type's plain `#[derive(Ast)]` macro carries the visitor metadata (`@ast` + `@subast`,
re-exported under the type name) exactly like any acyclic type — there is no `#[recurse]`-specific
visitor metadata. A
`visitor!(<cycle types>)` builds a **non-depth-generic** acyclic visitor (`generate_module` in
`macro/visitor.rs`, `gen_side` in `macro/visitor/side.rs`):
`visit_xxx(&mut self, &Expr<S>)`, dispatch to listed cross-edges via `this.visit_<head>`, drill an
unlisted cross-edge inline, descend containers/tuples — closures and `visit_mut` included. The engine,
conversions, terminators, and the `__reentry_X` helpers are **fully internal** (`pub(crate)`, in no
metadata; the runtime re-entry registry is `core::parse::vtable`) — used to back the delegated `Parse`
impl (and group-ful `Unparse`/`Spanned`), which the defining crate emits (so a downstream cross-crate
visitor over the natural type has no orphan issue and parses via the upstream `Expr<S>: Parse`). A
group-free cycle's direct `Unparse`/`Spanned` impls also live on the natural type in the defining crate.

# TODOs

- [x] implement attempt() feature which requires Atom: Clone
      → `nested::Attempt<T>(pub T)` is the atomic-parse wrapper: its `Parse` parses `T` but **rewinds** the
      stream on failure (via `dup`, hence `Atom: Clone`) while still propagating the error (unlike
      `Option`, which becomes `None`). A transparent `Deref` wrapper, descended by the visitor like `Box`
      (both are single-slot `OptView<T>` layers), so
      it works as a derived AST field type. `Parse::attempt(self) -> Attempt<Self>` is the value
      constructor (sugar for `Attempt(self)`). Tests: `nested_attempt.rs`.
- [x] in #[derive(Parse)] macro, support prefix-duplicated syntax (like E | E!) without memorize or backtracking, just comparing fields in each variants
      → enum `Parse` derive now **prefix-dedups**: the longest run of leading fields shared by ALL variants
      (same member+type+attrs — `common_field_prefix_len`) is parsed ONCE, then each variant's suffix is
      tried (the shared prefix is not re-parsed). Scoped: an enum with no shared prefix (LCP 0) or <2
      variants — incl. every recurse-engine enum — keeps the per-variant-`dup` scheme byte-identical.
      Declaration order (which variant wins) is preserved. `macro/attribute/adt.rs`
      (`DataEnum::extract_parse_inner`); tests `parse_prefix_dedup.rs` (chain, divergent suffixes, named
      fields, inside a `#[recurse]` cycle, parse-count proof).

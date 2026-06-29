# Visitor system — current state

Generate `syn`-style visitors from AST type definitions.

Code: `core/src/visit.rs` (`Ast`, `Repeater` traits), `macro/ast.rs` (`#[derive(Ast)]`),
`macro/visitor.rs` (`visitor!` shim → `__visitor_entry` → `__visitor_build`). Tests:
`core/tests/visitor_*.rs`, `core/tests/ast_derive.rs`, `core/tests/ast_recurse.rs`; cross-crate:
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
- **Containers**: `Box` (deref), `Vec`/`VecDeque`/slice/array/`Punctuated` (`for …iter()/iter_mut()`),
  `Option` (`if let Some`), dereffing through a wrapping `Box`. **Nested** (`Vec<Option<T>>`,
  `Option<Vec<T>>`, …) traversed via a peeled container chain (`visitor_nested_containers.rs`). A
  **tuple** at the peeled position — top-level *or* nested behind containers/boxes (`Vec<(A,B)>`,
  `Box<(A,B)>`) — is destructured + each element lowered (`peel`'s `Head::{Path,Tuple}`;
  `visitor_container_of_tuple.rs`, recurse: `visitor_recurse_container_of_tuple.rs`).
- **Inputs**: struct visitors (`&mut`), single closures, and **tuples of closures** (2..=8) in **one**
  pass (`Hook` + `Driver` + `Chain`).
- **`visit_mut`**: full in-place mirror. Reduce/append by overriding the *parent*'s `visit_*_mut`
  (it owns the `&mut Vec`/`&mut Option`) — `visitor_reduce.rs`.
- **Inheritance `visitor!(base => New)`**: base exports a `__syan_visited` macro (visited idents +
  param union `@bg` + ancestor chain `@an`); New extends it via supertrait at the base's own arity.
  Wider new union and multi-level `base => mid => New` chains work — `visitor_inherit_arity.rs`,
  `visitor_inherit_multilevel.rs`.
- **Generics**: the trait is keyed on the **union** of visited types' params; each type uses its subset
  (`visitor_generics.rs`). Generated helper params avoid visited types' param names
  (`visitor_hygiene.rs`). Caveat: `.visit()` on a root that omits a union param may need a turbofish. A
  **`where`-bounded param not shared by all visited types** can't be a union trait param (a type lacking
  it would carry an undischargeable bound), so it becomes a **per-method generic** (`visit_bounded<S:
  Bound>`) with the trait keyed on the shared subset — going **struct-only** (a closure can't be
  `for<S>`), same machinery as the heterogeneous concrete-fill case (`method_mode`,
  `visitor_union_where_unshared_param.rs`). An *unbounded* unshared param instead stays in the union +
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
  sibling) — making them **unbounded** (any depth); a **group-ful** cycle keeps them **engine-delegated**
  (the self-recursive `#[group]` field's `for<'a> Fill<Substruct>: Unparse` HRTB forms a trait-solver
  cycle `#[ignore_bounds]` can't break), hence **depth-limited** (the one residual limitation); (2) a
  `pub(crate)` fixed-depth engine `__XxxRec<…, __Rec = __XxxDefault<…>>` family + **inhabited**
  terminators `__XxxTerm` (newtypes wrapping `Box<Root<…>>`) + `__XxxDefault` depth chains (all
  **nonce-stamped**, so a user item named e.g. `ExprTerm` never collides), deriving the engine-routed
  traits, emitted **only when needed** (`scc_needs_engine` = derives `Parse`, or is group-ful and derives
  `Unparse`/`Spanned`; a cycle deriving none of those, e.g. Ast-only, gets no engine) (`make_engine_item`,
  `emit_terminator_and_reentry`); (3) per-cycle `__ToNat_X` (engine→natural; the terminator's just unwraps
  its `Box`) and, when the cycle **engine-delegates** `Unparse`/`Spanned` (group-ful only), `__FromNat_X`
  (natural→engine) conversion traits/impls (depth-generic, `__FromNat`'s terminator arm `panic!`s) + the
  **delegated `impl`s**: group-ful `Unparse`/`Spanned` via ONE algorithm (`emit_delegated_impl`, the
  `RecTrait` enum) `.__from_nat()` then call the engine's impl, while `Parse` (`emit_delegated_parse`)
  **registers** each root's erased re-entry parser into `core::parse::vtable`, runs the engine, then
  `.__to_nat()`s (`gen_natural_extras`). A cycle type's `where`-clause is threaded onto the generated
  impls (`where_preds_of`); a **group-ful** cycle's `Group` uses a hand-written `Unparse<TokenTree>`
  emitting a single `TokenTree::Group` and a `Spanned` taking the span from its delimiters
  (`nested/group.rs`). The natural enum owns the name (no `pub type` alias); user inherent `impl`s land on
  the natural type verbatim. **`Parse` is UNBOUNDED** despite the fixed engine depth: the engine's
  depth-floor terminator is inhabited and its `Parse` **re-enters the top-level natural parser at runtime
  through a type-erased `&mut dyn ParseStream` fn pointer** (keyed per `(terminator, atom, stream-error)`
  in `core::parse::vtable`; the delegated `Parse` registers it before descending) instead of erroring — so
  a tree deeper than the engine depth parses fully (ceiling = the OS call stack; a *left-recursive* grammar
  therefore loops forever rather than being silently truncated as the old depth cap did). **Why Parse
  still needs the engine:** deriving `Parse` directly on a natural recursive type fails two ways — (a)
  per-field `field_ty: Parse` where-bounds form an infinite cycle (E0275); (b) backtracking `stream.dup(…)`
  wraps the stream in another `Dup<…>` per descent level → infinite stream-type monomorphization (also
  E0275). The fixed engine bottoms both out at compile time, and the erased re-entry restarts at one fixed
  `Dup<&mut dyn …>` layer that never grows. (`Unparse`/`Spanned` only hit (a) — which `#[ignore_bounds]`
  defuses — so they can be direct; they have no backtracking.) Cycle types may carry
  lifetime/type/const params, possibly
  **heterogeneous** across the cycle; a back-edge to a root repeats the root's params **verbatim** (a
  non-identity arg like `Expr<Vec<S>>` is rejected — an engine constraint, kept). **Independent cycles**
  are partitioned into SCCs (`find_cycle_sccs`, Tarjan), each with its own natural+engine+conversions
  (`build_scc`). **Multi-root** cycles keep one engine depth dimension per root (`build_multiroot_tail`).
  **Finite-size precondition:** a natural recursive type must be finite-size, so a **pure by-value
  cycle** (no `Box`/`Vec`/… on any cycle edge) is rejected with a clean `abort!` (would be E0072) —
  detected via the direct-edge subgraph being acyclic (`subgraph_is_cyclic` on `direct_type_refs`). Clean
  `abort!`s also for a missing/non-identity root param and a non-acyclic rootless subcycle; passing any
  argument to `#[recurse]` is a clean compile error (`ui/recurse_takes_no_args.rs`). Tests:
  `recurse_test.rs`, `recurse_multi_cycle.rs`, `recurse_multiroot.rs`, `recurse_fixes.rs`,
  `recurse_problems_test.rs` (`parse_is_unbounded`), `recurse_audit_test.rs`, `recurse_unparse_spanned.rs`
  (`parse_unbounded_depth`), `recurse_group_ful.rs`, `recurse_no_engine.rs`, `ignore_bounds.rs` +
  `ui/recurse_*.rs`, `ui/problem*.rs`; unbounded group-ful `Parse` w/ backtracking: `rust/tests/rustsub_roundtrip.rs`
  (`parse_deep_parens_is_unbounded`).
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
  (E0116 — use `Visit::visit_*`), via `path_is_crate_local`. Tests: `visitor_recurse_cycle.rs` (incl.
  `closure_over_recurse_cycle`), `visitor_recurse_via_visitor.rs` (+`visit_mut`), `…_heterogeneous`,
  `…_mixed`, `…_containers`, `…_container_of_tuple`, `…_multiroot_via_visitor`,
  `…_multicycle_via_visitor`, `visitor_recurse_drill.rs` (`unlisted`), `recurse_generics.rs` (`het`),
  `audit_visitor_recurse_nonroot_lifetime.rs`, `visitor_inherit_recurse{,_acyclic_mid}.rs`,
  `rust/tests/cross_crate_recurse.rs`.

## Known gaps / limitations

- **Group-ful `Unparse`/`Spanned` are depth-limited** — the one residual `#[recurse]` limitation now that
  `Parse` is unbounded. `Parse` (always) and a **group-free** cycle's `Unparse`/`Spanned` are unbounded:
  group-free `Unparse`/`Spanned` derive directly on the natural type (via `#[ignore_bounds]` + the injected
  leaf-bound `#[predicate_*]` union — `recurse_unparse_spanned.rs`: single-type depth-5000, multi-type
  depth-2000), and `Parse` re-enters the top-level parser at the engine terminator via a type-erased fn
  pointer (`core::parse::vtable`; `recurse_problems_test.rs` depth-8, `recurse_unparse_spanned.rs`
  depth-200, `rustsub` depth-60 group-ful w/ backtracking). Only a **group-ful** cycle's `Unparse`/`Spanned`
  stay engine-bounded (the self-recursive `#[group]` field's `for<'a> Fill<Substruct>: Unparse` HRTB forms
  a trait-solver cycle neither `#[ignore_bounds]` nor the value-level re-entry can break), so a tree deeper
  than the fixed engine depth panics at the terminator (`recurse_group_ful.rs`, `rustsub`). Lifting it would
  need a way to break the group HRTB cycle. (Caveat of the unbounded `Parse`: a **left-recursive** cycle
  grammar now recurses forever — the OS-call-stack ceiling of any recursive-descent parser — instead of
  being silently truncated by the old depth cap.)
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
one"). Containers (`Box`/`Vec`/`VecDeque`/`Option`/slice/array/`Punctuated`), incl. **nested** ones
(`Vec<Option<_>>`), are traversed via a peeled container chain. Membership + method-name building live
in `__visitor_build` (the proc-macro), since `macro_rules!` can't compare or snake-case idents — the
metadata ping-pong only supplies each type's structure. Code: `macro/ast.rs` + `macro/visitor.rs`
(`Lower`); tests:
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
   (a back-edge to a root becomes the depth param `__Rec`, one per root) + **inhabited** terminators
   `__XxxTerm` (newtypes wrapping `Box<Root<…>>`, carrying the root's param bounds) + `__XxxDefault` depth
   chains, deriving the engine-routed traits (`Parse` always; `Unparse`/`Spanned` only when group-ful).
   Each depth level is a *distinct* finite type, which bottoms out **both** Parse E0275 cycles (the
   per-field `field_ty: Parse` where-cycle **and** the `stream.dup(…)` `Dup<…>` stream-monomorphization
   cycle) and lets group-ful `Unparse`/`Spanned` derive normally. The terminator's `Parse` **re-enters**
   the top-level parser at runtime through a type-erased `&mut dyn ParseStream` fn pointer
   (`emit_terminator_and_reentry` + a per-root `__reentry_X` helper), so `Parse` is **unbounded** despite
   the fixed type depth; its `Unparse` (only reached by a group-ful engine, and even then `__from_nat`
   panics first) is a belt-and-braces `panic!`. Emitted only when `scc_needs_engine`. `make_engine_item`.
3. **Conversion + impls** — per cycle type: a private depth-generic `__ToNat_X` (engine→natural; always,
   for `Parse`; the terminator's `__to_nat` just unwraps its `Box`) and, when the cycle **engine-delegates**
   `Unparse`/`Spanned` (group-ful), `__FromNat_X` (natural→engine) trait/impl (a back-edge collapses to
   `__Rec`, a cross-edge bounds the sibling node, containers map element-wise; `__FromNat`'s terminator arm
   `panic!`s — the group-ful depth floor). **`Parse`** (`emit_delegated_parse`) **registers** each root's
   `__reentry_X` (the top-level parse monomorphized at the erased stream, keyed in `core::parse::vtable`)
   into the registry, then parses the engine and `.__to_nat()`s; group-ful **`Unparse`/`Spanned`**
   (`emit_delegated_impl`, per-trait shape from the `RecTrait` model — now just these two) `.__from_nat()`
   (cloning leaves) then call the engine's impl. For a **group-free** cycle there is *no* `__FromNat`/
   delegated `Unparse`/`Spanned` — those are derived **directly** on the natural type (step 1), so they are
   **unbounded** (any depth). `gen_natural_extras`, `conv_body`/`conv_expr` + `from_conv_body`/`from_conv_expr`.

The natural enum owns the name (no `pub type` alias); user inherent `impl`s land on the natural type
verbatim. A **pure by-value cycle** (no heap indirection on any edge) is
rejected (`abort!`, would be E0072) — the natural type would be infinite-size; checked via the
direct-edge subgraph being acyclic (`subgraph_is_cyclic` on `direct_type_refs`).

## How `visitor!()` consumes it — ordinary acyclic metadata

The natural type's plain `#[derive(Ast)]` macro carries the visitor metadata (`@ast` + `@subast`,
re-exported under the type name) exactly like any acyclic type — there is no `#[recurse]`-specific
visitor metadata. A
`visitor!(<cycle types>)` builds a **non-depth-generic** acyclic visitor (`generate_module`/`gen_side`):
`visit_xxx(&mut self, &Expr<S>)`, dispatch to listed cross-edges via `this.visit_<head>`, drill an
unlisted cross-edge inline, descend containers/tuples — closures and `visit_mut` included. The engine,
conversions, terminators, and the `__reentry_X` helpers are **fully internal** (`pub(crate)`, in no
metadata; the runtime re-entry registry is `core::parse::vtable`) — used to back the delegated `Parse`
impl (and group-ful `Unparse`/`Spanned`), which the defining crate emits (so a downstream cross-crate
visitor over the natural type has no orphan issue and parses via the upstream `Expr<S>: Parse`). A
group-free cycle's direct `Unparse`/`Spanned` impls also live on the natural type in the defining crate.

# TODOs

- [ ] implement attempt() feature which requires Atom: Clone
- [ ] in #[derive(Parse)] macro, support prefix-duplicated syntax (like E | E!) without memorize or backtracking, just comparing fields in each variants

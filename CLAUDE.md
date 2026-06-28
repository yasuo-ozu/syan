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
  (`visitor_hygiene.rs`). Caveat: `.visit()` on a root that omits a union param may need a turbofish.
- **Cross-crate**: visited types named by full path → no downstream import (`cross_crate.rs`);
  downstream drill through upstream types via `$crate`-rooted `#[subast]` (`cross_crate_drill.rs`).
  Cross-crate **inheritance** is keyed on the base **path** (supertrait, inherited `base::visit_*`, the
  `pub use`'d `__syan_visited`); multi-level incl. an *upstream* intermediate works because
  `__visitor_build` **requalifies** a relative ancestor against the direct base's full path —
  `crate::` → the host crate, `super::`/`self::` → resolved against the base module (the consumer is
  given the intermediate's path, e.g. `super::base` off `syan_rust::inherit::mid_ss` ⇒
  `syan_rust::inherit::base`), via `base_host_crate`/`requalify_ancestor`. Tests:
  `cross_crate_inherit{,_multilevel,_4level,_downstream_mid}.rs`, `cross_crate_super_self.rs`.
- **`#[recurse(limit = N)]`** (type transformer): turns a module of mutually-recursive AST types into
  **natural recursive public types** + an internal depth-limited **engine** used only for `Parse`. Per
  SCC it emits: (1) the user's cycle types as **genuine natural recursive enums/structs** — the public
  API `Expr<S>` (one type at all depths), carrying `#[derive(Ast)]` + `Debug`/`Default`/… with `Parse`
  always **routed to the engine**; `Unparse`/`Spanned` stay on the natural type (with `#[ignore_bounds]`
  injected on recursive-child fields) **only for a single self-recursive group-free cycle**, else they
  too are engine-routed (`make_natural_item`, gated by `scc_us_natural`); (2) a `pub(crate)` depth-
  limited engine `__XxxRec<…, __Rec = __XxxDefault<…>>` family + terminators `__XxxTerm` + `__XxxDefault`
  depth chains (all **nonce-stamped** — §"name hygiene"), deriving the engine-routed traits, emitted
  **only when needed** (`scc_needs_engine` — an Ast-only cycle gets none) (`make_engine_item`); (3) per-cycle
  `__ToNat_X` conversion traits/impls (engine→natural, depth-generic, terminator arm `unreachable!`) +
  a **delegated `impl Parse for X`** that parses the engine then `.__to_nat()`s (`gen_natural_extras`,
  `conv_body`/`conv_expr`). The public `pub type X = …` aliases are **gone** (the natural enum owns the
  name); user inherent `impl`s land on the natural type verbatim. **Why Parse still needs the engine:**
  deriving `Parse` directly on a natural recursive type fails two ways — (a) per-field `field_ty: Parse`
  where-bounds form an infinite cycle (E0275); (b) backtracking `stream.dup(…)` wraps the stream in
  another `Dup<…>` per descent level → infinite stream-type monomorphization (also E0275). The engine
  bottoms both out. Cycle types may carry lifetime/type/const params, possibly **heterogeneous** across
  the cycle; a back-edge to a root repeats the root's params **verbatim** (a non-identity arg like
  `Expr<Vec<S>>` is rejected — an engine constraint, kept). **Independent cycles** are partitioned into
  SCCs (`find_cycle_sccs`, Tarjan), each with its own natural+engine+conversions (`build_scc`).
  **Multi-root** cycles keep one engine depth dimension per root (`build_multiroot_tail`). **Finite-size
  precondition:** a natural recursive type must be finite-size, so a **pure by-value cycle** (no
  `Box`/`Vec`/… on any cycle edge) is rejected with a clean `abort!` (would be E0072) — detected via the
  direct-edge subgraph being acyclic (`subgraph_is_cyclic` on `direct_type_refs`). Clean `abort!`s also
  for a missing/non-identity root param (`limit = 0` still panics) and a non-acyclic rootless subcycle.
  Tests: `recurse_test.rs`, `recurse_multi_cycle.rs`, `recurse_multiroot.rs`, `recurse_fixes.rs`,
  `recurse_problems_test.rs`, `recurse_audit_test.rs`, `ignore_bounds.rs` + `ui/recurse_*.rs`,
  `ui/problem*.rs`.
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
  `…_multicycle_via_visitor`, `…_drill_unlisted`, `recurse_generics.rs` (`het`),
  `audit_visitor_recurse_nonroot_lifetime.rs`, `visitor_inherit_recurse{,_acyclic_mid}.rs`,
  `rust/tests/cross_crate_recurse.rs`.

## Known gaps / limitations

- **`Unparse`/`Spanned` on a natural former-`#[recurse]` cycle — natural everywhere except a group-ful
  cycle.** A natural `Expr<S>` is always `Parse` (delegated through the engine). `Unparse`/`Spanned` are
  emitted on the **natural** type in two ways: (a) **single self-recursive group-free** cycle — directly,
  via injected `#[ignore_bounds]` (leaf-only bounds; the recursive `.unparse()`/`.span()` resolves
  against the *same* impl, **arbitrary depth**); (b) **multi-type group-free** cycle — by **delegation
  through the engine**: a generated `__FromNat_X` bridge converts the (borrowed) natural value to the
  depth-default engine value (`Clone`ing leaves; the leaf-`Clone` bounds are *unioned* across the cycle
  so a member can build its siblings) and calls the engine's `Unparse`/`Spanned` (`gen_natural_extras`).
  Delegated `Unparse`/`Spanned` are **depth-limited** — a tree deeper than `limit` `panic!`s at the
  terminator (within the limit they succeed). Tested in `recurse_unparse_spanned.rs` (single + multi-
  type, incl. type params; `Spanned` needs `S: Span`, threaded through the conversion impls by
  `param_decls`, and a generated terminator `Spanned`). A **group-ful** cycle still keeps them on the
  `pub(crate)` engine only (the group `Fill<Substruct>: Unparse` chain isn't delegable) — there the
  natural type is `Parse` but not directly `Unparse`/`Spanned`. A cycle type's **`where`-clause** is
  threaded through the generated engine/conversion/delegated impls (`where_preds_of` in
  `gen_natural_extras`) — a param bound (`where S: Clone`) or a self-referential bound (`where Expr<S>:
  Marker`) both work (`recurse_where_clause.rs`). All generated internal **type/trait names** — engine
  `__XxxRec`, terminator `__XxxTerm`, depth default `__XxxDefault`, conversion traits
  `__ToNat`/`__FromNat` — carry a **per-expansion nonce** (`engine_name`/`term_name`/`default_name`/
  `to_nat_name`/`from_nat_name`), so a user item named e.g. `ExprTerm` never collides
  (`recurse_no_engine.rs`). (The depth *params* `__Rec` stay un-nonced — they're local type-param names
  that never escape to user scope.)
- **Two visited types sharing a last segment** (`visitor!(a::Foo, b::Foo)`): all generated names key
  off the last segment, so they collide. Now a clear build error (`visitor_diagnostics.rs`); genuine
  coexistence would need full-path-disambiguated names (the alias is one keyword — won't fix).
- **Clean `abort!` for a `where`-bounded param not shared by all visited types** (the bound would be
  undischargeable on a type lacking it — an *unbounded* unshared param is fine); `visitor_diagnostics.rs`,
  `ui/visitor_union_where_unshared_param.rs`. (A cycle following an **unlisted intermediate** that forms
  a cycle of unlisted intermediates is the general drill diagnostic — "list one" — incl. an omitted
  co-root: `ui/visitor_recurse_unlisted_coroot.rs`.)

## Closures over `#[recurse]` — now work (was deferred)

**Closed.** `#[recurse]` now emits **natural recursive public types** (`make_natural_item`), so a
former-recurse cycle is depth-*uniform* — one type at all depths. A `visitor!(…)` over it is therefore
an ordinary acyclic visitor with **no** depth-generic `visit_*<R>` and no type-level HRTB wall, so
closures, tuples-of-closures, inherent `.visit(closure)`, `visit_mut`, and inheritance all work via the
existing `Hook`/`Driver`/`Chain`. The depth-limited types survive only as an internal `pub(crate)`
engine for `Parse` (see "`#[recurse]` expansion" below). Tests: `visitor_recurse_cycle.rs`
(`closure_over_recurse_cycle`), `visitor_recurse_via_visitor.rs`.

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

# `#[recurse]` expansion (natural types + internal Parse engine)

`#[recurse(limit = N)]` emits, per SCC:

1. **Natural public types** — the user's cycle types *un-renamed* (`Expr<S>`, one type at all depths),
   with the `#[derive(…)]` list rewritten: `Parse` **removed**, everything else (`Ast`, `Debug`,
   `Default`, `Unparse`, `Spanned`, …) kept, and `#[ignore_bounds]` injected on every recursive-child
   field so the kept `Unparse`/`Spanned` derives emit **leaf-only** bounds (no E0275 where-cycle).
   `make_natural_item`.
2. **Engine types** (`pub(crate)`) — today's depth-limited `__XxxRec<…, __Rec = __XxxDefault<…>>` family
   (a back-edge to a root becomes the depth param `__Rec`, one per root) + terminators `XxxTerm` +
   `__XxxDefault` depth chains, deriving **only** `Parse`. Each depth level is a *distinct* finite type,
   which bottoms out **both** Parse E0275 cycles (the per-field `field_ty: Parse` where-cycle **and** the
   `stream.dup(…)` `Dup<…>` stream-monomorphization cycle). `make_engine_item`.
3. **Conversion + delegated Parse** — a private depth-generic `__ToNat_X` trait/impl per cycle type
   (engine→natural; a back-edge collapses to `__Rec`, a cross-edge bounds the sibling node, containers
   map element-wise; terminator arm is `unreachable!`) + a hand-emitted **`impl Parse for Expr<S>`** that
   parses the engine then `.__to_nat()`s. `gen_natural_extras`, `conv_body`/`conv_expr`. (Deep-copies
   once per top-level parse; preserves the engine's lenient depth-truncation semantics.)

The public `pub type Xxx = …` alias is **gone** (the natural enum owns the name); user inherent `impl`s
land on the natural type verbatim. A **pure by-value cycle** (no heap indirection on any edge) is
rejected (`abort!`, would be E0072) — the natural type would be infinite-size; checked via the
direct-edge subgraph being acyclic (`subgraph_is_cyclic` on `direct_type_refs`).

## How `visitor!()` consumes it — ordinary acyclic metadata

There is **no `@recurse` metadata** anymore. The natural type's plain `#[derive(Ast)]` macro carries the
visitor metadata (`@ast` + `@subast`, re-exported under the type name) exactly like any acyclic type. A
`visitor!(<cycle types>)` builds a **non-depth-generic** acyclic visitor (`generate_module`/`gen_side`):
`visit_xxx(&mut self, &Expr<S>)`, dispatch to listed cross-edges via `this.visit_<head>`, drill an
unlisted cross-edge inline, descend containers/tuples — closures and `visit_mut` included. The engine,
conversions, and terminators are **fully internal** (`pub(crate)`, in no metadata) — used only to
implement `Parse`, which the defining crate emits (so a downstream cross-crate visitor over the natural
type has no orphan issue and parses via the upstream `Expr<S>: Parse`).

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
  `Option` (`if let Some`), dereffing through a wrapping `Box`. Nested (`Vec<Option<T>>`) rejected.
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
  `__visitor_build` **requalifies** a `crate::`-relative ancestor against the base's host crate
  (`base_host_crate`/`requalify_ancestor`). Tests: `cross_crate_inherit{,_multilevel,_4level,_downstream_mid}.rs`.
  Residual hole: a `super::`/`self::`-relative ancestor from an upstream intermediate isn't requalified
  (use `crate::`-rooted entry paths).
- **`#[recurse(limit = N)]`** (type transformer + metadata — *no* visitor): turns a module of
  mutually-recursive AST types into depth-limited concrete types — renames each cycle type `Xxx` →
  `__XxxRec<…, depth>`, emits per-root terminators / `__XxxDefault` depth chains / the public `pub type
  Xxx = …` aliases, and a `@recurse` metadata macro (under each type's original name) that `visitor!()`
  consumes. Cycle types may carry lifetime/type/const params, possibly **heterogeneous** across the
  cycle; every cycle type declares the root's params and a back-edge to the root repeats them
  **verbatim** (a non-identity arg like `Expr<Vec<S>>` is rejected). **Multiple independent cycles** in
  one module are partitioned into SCCs (`find_cycle_sccs`, via `safegraph` Tarjan), each with its own
  root/chain/`XxxTerm`/aliases (`build_scc`; fixed a latent collapse-into-one-`__Rec` miscompile).
  **Multiple self-referential roots** in one cycle each keep their own depth dimension (one depth param
  per root, chains unrolled mutually, per-root `XxxTerm` — `build_multiroot_tail`); soundness guard:
  the SCC minus its roots must be acyclic (`subgraph_is_cyclic`, via `safegraph` `is_cyclic_directed`)
  else a clear `abort!` (`ui/recurse_multiroot_rootless_subcycle.rs`). Clean `abort!`s for a missing
  root param and a non-identity root arg (`limit = 0` still panics). Tests: `recurse_multi_cycle.rs`,
  `recurse_multiroot.rs`, `recurse_fixes.rs`, `recurse_audit_test.rs` + `ui/recurse_*.rs`.
- **`visitor!(…)` over a `#[recurse]` cycle** (the **only** recurse-visitor path): `visitor!()`
  consumes the `@recurse` metadata to generate a **depth-generic** visitor keyed on its own
  `Visit`/`VisitMut` traits — `visit_X(&X)` for acyclic targets, depth-generic `visit_Y<R…>(&YNode<…>)`
  for recurse targets, with `VisitRec`/`VisitRecMut` dispatch (root's depth chain drives, terminator a
  no-op) + `XxxNode` aliases, all in one `generate_module_mixed` (acyclic-only visitors keep the
  `gen_side` path). Trait/struct-based only (a closure can't be depth-generic). The trait is keyed on
  the cycle **roots'** params; a non-root cycle type's params beyond the roots' become `visit_*`
  **method generics** — so a **heterogeneous** cycle `Expr<S>` + `Stmt<S, T>` ⇒ `Visit<S>` +
  `visit_stmt<T, R>` (`visitor_recurse_heterogeneous.rs`). One `visitor!()` can **mix** acyclic +
  recurse types (the outer→inner boundary auto-crosses; acyclic params must be ⊆ the roots' params else
  a clear `abort!` — `ui/visitor_recurse_mixed_acyclic_extra_param.rs`), span **several independent
  cycles** (one unified `Visit`; each target carries its cycle's roots/depth), handle **multi-root**
  cycles (one depth param per root), traverse `Vec`/`Option`/`Box` (incl. `Box`-around-`Option`) +
  **tuples**, **drill** an *unlisted* cross-edge cycle type inline (no `visit_*` for it; back-edges
  still via the depth params — `visitor_recurse_drill_unlisted.rs`), and emit both the **shared and
  `&mut`** sides. Works **cross-crate** — a downstream
  `visitor!(upstream::Expr, …)` resolves the `$crate`-rooted `@node`/`@terms` to the defining crate;
  inherent `.visit()` is skipped for a *foreign* target (E0116 — use the `Visit::visit_*` trait
  method), via `path_is_crate_local`. Nested containers (`Vec<Option<T>>`) get a clean `abort!`. Tests:
  `visitor_recurse_via_visitor.rs` (incl. `visit_mut`), `visitor_recurse_heterogeneous.rs`,
  `visitor_recurse_mixed.rs`, `visitor_recurse_containers.rs`, `visitor_recurse_cycle.rs`,
  `recurse_generics.rs`, `visitor_recurse_multiroot_via_visitor.rs`,
  `visitor_recurse_multicycle_via_visitor.rs`, `rust/tests/cross_crate_recurse.rs`. See the
  "`#[recurse]` expansion & how `visitor!()` consumes it" section for the contract and current limits.

## Known gaps / limitations

- **`visitor!(…)` over `#[recurse]` — remaining limits** (the capability is shipped; see "Shipped &
  tested" + the expansion section): no inheritance (`base => …`) over recurse (a clean `abort!`; fix
  approach in the "Fix plan" below), and no closures (depth-generic methods can't back a closure
  `Driver`, so it's struct/`&mut`-visitor only — fundamental). An *unlisted* recurse cross-edge is now
  **drilled inline** (`visitor_recurse_drill_unlisted.rs`) — like drill-in over *acyclic* types in a
  `#[recurse]` module (`visitor_recurse_drill.rs`); back-edges still dispatch through the depth params,
  and a *cycle of unlisted* intermediates is guarded (unreachable in practice — the type-level
  rootless-sub-cycle guard already rejects it).
- **Two visited types sharing a last segment** (`visitor!(a::Foo, b::Foo)`): all generated names key
  off the last segment, so they collide. Now a clear build error (`visitor_diagnostics.rs`); genuine
  coexistence would need full-path-disambiguated names.
- **Nested containers** (`Vec<Option<T>>`) are unsupported on the `visitor!()` path (acyclic and over
  a `#[recurse]` cycle alike — clear build error); wrap the inner part in its own `#[derive(Ast)]` type.

## Fix plan — `visitor!()`-over-`#[recurse]` remaining limits

Concrete approaches for the limits above (all in `macro/visitor.rs` unless noted), ordered by
value/tractability:

1. **Inheritance (`base => …`) over recurse** (niche, larger effort). Restrict to the trait path (the
   `Driver`/closure side is already off for recurse). In `generate_module_mixed`, when `st.base` is a
   recurse visitor: add `base::Visit`/`VisitMut`/`VisitRec` as **supertraits** of the new traits (reuse
   the existing ancestor/`@an` requalification machinery — `base_host_crate`/`requalify_ancestor`),
   keep the inherited recurse types in `method_set` without re-emitting their bodies (their `VisitRec`
   impls come from `base`), and let the new types' bodies cross into them via `this.visit_<inherited>`.
   Requires the new union params ⊇ the base's, and the base's depth params to line up — feasible since
   `VisitRec`'s `visit_rec` signature is uniform.
2. **Closures over recurse — won't fix (fundamental).** A `visit_*<R: VisitRec>` method is generic
   over the remaining depth; a closure is one concrete `FnMut` and can't be depth-generic, so the
   `Driver`/`Hook`/`Chain` machinery cannot implement the unified `Visit` trait. This is inherent (the
   reason a recurse visitor has always been trait-only) — document it, don't track it as a TODO.

---

# Drill-in (selective `visit_*` + transitive drill-through)

Two lists drive it. **`#[subast(<paths>)]`** (per `#[derive(Ast)]` type) is the *follow-list*: a field
is followed iff its container-peeled head is a listed entry or the type's own ident (self-recursion
implicit), else a leaf; the entry also gives the `$crate`-rooted path to that child's metadata macro.
**`visitor!(T, …)`** is the *method-list*: a followed head that is listed lowers to
`this.visit_<head>(field)`; a followed-but-*unlisted* head is **drilled inline** (recurse into its
`#[subast]` fields, no `visit_*` emitted for it); a cycle of unlisted intermediates errors ("list
one"). Containers (`Box`/`Vec`/`VecDeque`/`Option`/slice/array/`Punctuated`) are traversed; nested ones
(`Vec<Option<_>>`) are rejected. Membership + method-name building live in `__visitor_build` (proc-
macro), since `macro_rules!` can't compare or snake-case idents — the metadata ping-pong only supplies
each type's structure. Code: `macro/ast.rs` + `macro/visitor.rs` (`Lower`); tests:
`core/tests/visitor_drill*.rs`, `visitor_diagnostics.rs`.

---

# `#[recurse]` expansion & how `visitor!()` consumes it

`#[recurse(limit = N)]` renames each cycle type `Xxx` → `__XxxRec<…, __Rec = …>` (a back-edge to a
root becomes the depth param `__Rec`; one per root), emits a terminator `XxxTerm` + depth chain
`__XxxDefault` + the public `pub type Xxx<…> = __XxxRec<…, defaults…>` (each depth level a *distinct*
type), and a `@recurse` metadata macro. Because each level is a different type, a fixed-type
`visit_xxx(&Xxx)` can't recurse into its child — the visitor must be **depth-generic**.

`visitor!(<cycle types>)` consumes `@recurse` to emit, per listed cycle type, a depth-generic
`visit_*<R: VisitRec<…>>(&__XxxRec<…, R>)`, a `VisitRec<…, V>` dispatch trait (each root's depth chain
drives its `visit_*`, terminators are no-ops), and a `XxxNode` alias. A back-edge dispatches via `R`, a
cross-edge to a listed type via `this.visit_<head>`, an unlisted one is drilled inline. An outer
(acyclic) field `Vec<Expr<S>>` lowers to `this.visit_expr(e)` and infers `R = __ExprDefault`, so one
`Visit` trait + one `.visit()` crosses the outer→inner boundary automatically. Multi-root: one depth
param per root. Trait/struct-only (a closure can't be depth-generic).

## Metadata contract (`#[recurse]` → `visitor!()`)

For each cycle type, `#[recurse]` emits (additionally, under the type's *original* name) a muncher
metadata macro that answers the visitor's fetch `X! { @ast $cb { $pre } }` by appending the type's
`@ast { <ORIGINAL def> } @subast { … }` **plus** a `@recurse { … }` section the consumer keys on:

```text
@recurse {
    @node  { $crate::ast::__ExprRec }     // depth-generic node type (path, $crate-rooted)
    @roots { Expr }                        // root idents (1 single-root; N multi-root)
    @depth { __Rec }                       // depth-param idents, PARALLEL to @roots
    @terms { $crate::ast::ExprTerm }       // terminator paths, PARALLEL to @roots
    @cycle { Expr }                        // all cycle-type idents in this SCC
}
```

An acyclic type emits **no** `@recurse` section (normal `#[derive(Ast)]` metadata). `visitor!()`'s
`build` branches on `@recurse`: a recurse type gets the depth-generic `visit_*<R…>` + the shared
`VisitRec` trait/impls above; an acyclic type is handled as today.

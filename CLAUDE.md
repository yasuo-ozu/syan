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
  `__visitor_build` **requalifies** a relative ancestor against the direct base's full path: a
  `crate::` ancestor → the base's host crate, and a `super::`/`self::` ancestor → resolved against the
  base module (the consumer is *given* the intermediate's full path, and its `visitor!()` ran inside
  that module, so `super::base` = pop `mid_ss` off `syan_rust::inherit::mid_ss` + `base`) — closing the
  former `super`/`self` hole (`base_host_crate`/`requalify_ancestor`). Tests:
  `cross_crate_inherit{,_multilevel,_4level,_downstream_mid}.rs`, `cross_crate_super_self.rs`.
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
  method), via `path_is_crate_local`. Nested containers (`Vec<Option<T>>`) are traversed. **Inheritance**
  `visitor!(base => New)` over a recurse base works (struct-only — the base's `Visit`/`VisitMut` become
  supertraits; a recurse base exports `__syan_visited` with a `@recbase` marker → the consumer drops the
  closure `Driver`): both `New(acyclic) => base(recurse)` and `New(recurse) => base(recurse)`
  (`visitor_inherit_recurse.rs`). The `@recbase` taint propagates through an **acyclic intermediate**
  too (`recurse-base => acyclic-mid => new` — each link re-exports `@recbase` iff `base_is_recurse`;
  `visitor_inherit_recurse_acyclic_mid.rs`). Tests:
  `visitor_recurse_via_visitor.rs` (incl. `visit_mut`), `visitor_recurse_heterogeneous.rs`,
  `visitor_recurse_mixed.rs`, `visitor_recurse_containers.rs`, `visitor_recurse_cycle.rs`,
  `recurse_generics.rs`, `visitor_recurse_multiroot_via_visitor.rs`,
  `visitor_recurse_multicycle_via_visitor.rs`, `rust/tests/cross_crate_recurse.rs`. See the
  "`#[recurse]` expansion & how `visitor!()` consumes it" section for the contract and current limits.

## Known gaps / limitations

- **`visitor!(…)` over `#[recurse]` — only limit: no closures.** A depth-generic `visit_*<R>` is invoked
  at *every* depth (`R` shrinks per back-edge), so a closure driver would need `for<R> FnMut(&__XxxRec<S, R>)`
  — *type*-level HRTB, which Rust lacks; a single `FnMut` is monomorphic over the full-depth alias only
  (`(self.0)(i)` → "expected `&__XxxRec<S, default>`, found `&__XxxRec<S, R>`"). So a recurse (or
  recurse-inheriting) visitor is **struct/`&mut`-only** — write a struct `impl Visit`. Options to lift
  this are **deferred** (struct visitors are the pragmatic answer); detailed designs in "Closures over
  `#[recurse]` — implementation sketches" below.
  Everything else over recurse is supported (see "Shipped & tested"): drill-through of unlisted
  cross-edges, nested containers, multi-root / multi-cycle / mixed, inheritance (`base => …`), cross-crate.
- **Two visited types sharing a last segment** (`visitor!(a::Foo, b::Foo)`): all generated names key
  off the last segment, so they collide. Now a clear build error (`visitor_diagnostics.rs`); genuine
  coexistence would need full-path-disambiguated names (the alias is one keyword — won't fix).
- **Clean `abort!`s for footguns** (all `visitor_diagnostics.rs`): an **unlisted co-root** of a
  multi-root recurse cycle (a root defines a depth dimension, can't be drilled → must be listed); a
  **`where`-bounded param not shared by all visited types** (the bound would be undischargeable on a
  type lacking it — an *unbounded* unshared param is fine); the mixed acyclic-param-not-a-root wall.

## Closures over `#[recurse]` — implementation sketches

Five approaches considered; **none implemented** (struct visitors are the default). Notation: cycle
type `Xxx`, depth-renamed `__XxxRec<S, R>`, public alias `Xxx<S> = __XxxRec<S, __XxxDefault<S>>`,
dispatch trait `VisitRec`, the depth-generic free fn `visit_xxx`.

1. **Erased view** (sound; no alloc; closure can't descend). Per cycle type emit a depth-agnostic
   `pub enum XxxView<'v, S>` mirroring `Xxx`'s variants, with leaf fields borrowed (`&'v Leaf`) and
   recursive child fields **dropped** (the driver descends into them). Emit `impl<S, R> __XxxRec<S, R>
   { fn as_view(&self) -> XxxView<'_, S> }` (a `match` per variant). Closure adapter:
   `struct XxxClo<F>(F); impl<S, F: FnMut(&XxxView<S>)> Visit<S> for XxxClo<F> { fn visit_xxx<R: VisitRec<S,Self>>(&mut self,i){ (self.0)(&i.as_view()); visit_xxx(self,i) } }`;
   inherent `.visit(f)` wraps `f` in `XxxClo`. Mut side: `XxxViewMut<'v,S>` with `&mut` leaves. Several
   types / tuples → one `View` + adapter per type, chained like the acyclic `Hook`/`Chain`. Cost: a
   `View` enum + `as_view` per type; closure sees a view (no child access); leaf borrows add lifetimes.

2. **Natural-type conversion** (sound; most ergonomic; deep-copies per visit). Per cycle emit the
   *natural* recursive `pub enum XxxNat<S>` (back-/cross-edges as `Box<XxxNat<S>>`, leaves as-is) + a
   generic `fn to_nat<R>(&__XxxRec<S, R>) -> XxxNat<S>` (a fn, **not** a closure — so it may be
   `R`-generic; recursively converts, cloning leaves → needs `Leaf: Clone`). Then run an ordinary
   recursive closure visitor over `XxxNat<S>` (one type at every depth, so `|n: &XxxNat<S>|` works and
   *can* match + descend). `e.visit(f)` = `visit_nat(&to_nat(e), f)`. Cost: a per-`.visit()` deep copy +
   `Clone` bound; `XxxNat` is another public type. Most faithful of the sound options.

3. **Cast `&__XxxRec<S, R>` → `&Xxx<S>`** (UNSOUND — rejected). All `__XxxRec<S, R>` share a layout
   (`Box<R>` is a thin pointer), so `unsafe { &*(i as *const _ as *const Xxx<S>) }` type-checks. But a
   closure handed `&Xxx<S>` that descends a child reads a shallower node's `Box<R'>` as
   `Box<__XxxDefault>` and dereferences past its terminator → UB (also violates pointer provenance /
   aliasing). Can't restrict to non-descending closures. Rejected.

4. **`dyn`-erased dispatch** (sound; dynamic; awkward API). Emit an object-safe `pub trait XxxDyn<S> {
   fn kind(&self) -> XxxKind; fn child(&self, i: usize) -> Option<&dyn XxxDyn<S>>; /* leaf accessors */ }`
   impl'd for every `__XxxRec<S, R>`. Closure takes `&dyn XxxDyn<S>`; driver `visit_xxx<R>(&mut self, i)
   { (self.0)(i as &dyn XxxDyn<S>); … }`. The closure *can* descend (children are `&dyn`). Cost:
   dynamic dispatch; object-safety forces an accessor-method API (no pattern-match; awkward leaf types).

5. **Per-depth monomorphization** (impossible). One can't emit `limit+1` `Visit` impls for a single
   `ClosureDriver<F>` (they'd redefine the same `visit_xxx<R>`), and one `FnMut` value `F` can't be
   `FnMut(&__XxxRec<S, level_k>)` for every distinct level type `k`. So no closure covers all depths —
   the type-HRTB wall restated.

**If ever lifted:** prefer #2 (faithful) or #1 (no copy); #3 is unsound, #4 awkward, #5 impossible.

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

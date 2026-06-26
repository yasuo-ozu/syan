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
- **`#[recurse(limit = N)]`** (type transformer + metadata — *no* visitor): turns a module of
  mutually-recursive AST types into depth-limited concrete types — renames each cycle type `Xxx` →
  `__XxxRec<…, depth>`, emits per-root terminators / `__XxxDefault` depth chains / public `pub type Xxx`
  aliases, and a `@recurse` metadata macro `visitor!()` consumes. Cycle types may carry
  lifetime/type/const params, possibly **heterogeneous** across the cycle; a back-edge to a root repeats
  the root's params **verbatim** (a non-identity arg like `Expr<Vec<S>>` is rejected). **Independent
  cycles** are partitioned into SCCs (`find_cycle_sccs`, Tarjan), each with its own
  root/chain/`XxxTerm`/aliases (`build_scc`). **Multi-root** cycles keep one depth dimension per root
  (`build_multiroot_tail`); soundness guard: the SCC minus its roots must be acyclic
  (`subgraph_is_cyclic`) else a clear `abort!`. Clean `abort!`s for a missing/non-identity root param
  (`limit = 0` still panics). Tests: `recurse_multi_cycle.rs`, `recurse_multiroot.rs`,
  `recurse_fixes.rs`, `recurse_audit_test.rs` + `ui/recurse_*.rs`.
- **`visitor!(…)` over a `#[recurse]` cycle** (the **only** recurse-visitor path): consumes `@recurse`
  to emit a **depth-generic** visitor (`generate_module_mixed`) — `visit_Y<R: VisitRec<…>>(&YNode<…>)`
  per listed cycle type + `VisitRec`/`VisitRecMut` dispatch (root's depth chain drives, terminator a
  no-op) + `YNode` aliases. **Struct/`&mut`-only** (a closure can't be depth-generic — see "Closures
  over `#[recurse]`"). Keyed on the cycle **roots'** params; a non-root's extra params become `visit_*`
  **method generics** (heterogeneous `Expr<S>` + `Stmt<S,T>` ⇒ `Visit<S>` + `visit_stmt<T,R>`). One
  `visitor!()` can **mix** acyclic + recurse (auto-crosses the outer→inner boundary; acyclic params
  must be ⊆ the roots' else a clear `abort!`), span **independent cycles**, handle **multi-root** (one
  depth param per root), traverse containers + **tuples**, **drill** an unlisted cross-edge inline, and
  emit shared + `&mut`. **Cross-crate**: resolves `$crate`-rooted `@node`/`@terms`; inherent `.visit()`
  skipped for a foreign target (E0116 — use `Visit::visit_*`), via `path_is_crate_local`.
  **Inheritance** `visitor!(base => New)` over a recurse base is struct-only via a `@recbase` marker
  (drops the closure `Driver`) — `New(acyclic|recurse) => base(recurse)`, incl. through an acyclic
  intermediate (`@recbase` re-exported iff `base_is_recurse`). Tests: `visitor_recurse_via_visitor.rs`
  (+`visit_mut`), `…_heterogeneous`, `…_mixed`, `…_containers`, `…_multiroot_via_visitor`,
  `…_multicycle_via_visitor`, `…_drill_unlisted`, `recurse_generics.rs`,
  `visitor_inherit_recurse{,_acyclic_mid}.rs`, `rust/tests/cross_crate_recurse.rs`. Contract + limits:
  "`#[recurse]` expansion" section below.

## Known gaps / limitations

- **`visitor!(…)` over `#[recurse]` — only limit: no closures** (struct/`&mut`-only; the depth-generic
  `visit_*<R>` would need type-level HRTB). **Deferred** — write a struct `impl Visit`; rationale +
  per-option implementation plans in "Closures over `#[recurse]`" below. Everything else over recurse is supported (see
  "Shipped & tested"): unlisted-cross-edge drill, nested containers + tuples, multi-root / multi-cycle /
  mixed, inheritance, cross-crate.
- **Two visited types sharing a last segment** (`visitor!(a::Foo, b::Foo)`): all generated names key
  off the last segment, so they collide. Now a clear build error (`visitor_diagnostics.rs`); genuine
  coexistence would need full-path-disambiguated names (the alias is one keyword — won't fix).
- **Clean `abort!`s for footguns** (all `visitor_diagnostics.rs`): an **unlisted co-root** of a
  multi-root recurse cycle (a root defines a depth dimension, can't be drilled → must be listed); a
  **`where`-bounded param not shared by all visited types** (the bound would be undischargeable on a
  type lacking it — an *unbounded* unshared param is fine); the mixed acyclic-param-not-a-root wall.

## Closures over `#[recurse]` — implementation plans (deferred)

**Why it's blocked.** A depth-generic `visit_*<R>` runs at *every* depth (`R` shrinks per back-edge),
so a closure driver would need `for<R> FnMut(&__XxxRec<S, R>)` — *type*-level HRTB, which Rust lacks
(one `FnMut` value is monomorphic over the full-depth alias: `(self.0)(i)` → "expected `&__XxxRec<S,
default>`, found `&__XxxRec<S, R>`"). So recurse (and recurse-inheriting) visitors are
**struct/`&mut`-only**. Five options below; **none implemented** (struct visitors are the pragmatic
answer). The plans are concrete enough to execute.

**Shared anchors (all implementable options).** Everything is emitted in `generate_module_mixed`
(`macro/visitor.rs`), reusing metadata the consumer already fetches — `@ast` (each cycle type's
variant/field structure) and `@recurse` (`@node`/`@roots`/`@depth`/`@terms`/`@cycle`). A field is a
**recursive child** iff its `peel`ed head ∈ `@cycle` (the existing `recurse_lower_*` classification),
else a **leaf**; reuse that split. The closure adapter impls the *already-generated* `Visit<S>` (its
`visit_*<R>` calls the existing free `visit_*` to descend), so only the per-type support type + adapter
+ inherent `.visit(f)` wiring is new. Multi-type / tuple-of-closures chains exactly like the acyclic
`Hook`/`Driver`/`Chain` (one adapter per cycle type, combined in one pass).

### Option 1 — Erased view (sound; no alloc; closure sees leaves only)

- **Emit per cycle type `Xxx`:** `pub enum XxxView<'v, S> {…}` mirroring `Xxx`'s variants, but a leaf
  field `L` → `&'v L` (a container/leaf field `Vec<L>` → `&'v Vec<L>` whole), and a recursive-child
  field is **omitted** (a variant of only children becomes unit). Plus `impl<S, R> __XxxRec<S, R> { pub
  fn as_view(&self) -> XxxView<'_, S> }` — a `match` per variant borrowing leaves, ignoring children.
- **Adapter + inherent:** `pub struct XxxClo<F>(pub F); impl<S, F: FnMut(&XxxView<'_, S>)> Visit<S> for
  XxxClo<F> { fn visit_x<R: VisitRec<S, Self>>(&mut self, i) { (self.0)(&i.as_view()); visit_x(self, i);
  } }` (pre-order: fire closure, then descend). Inherent `pub fn visit(&self, f)` on the `XxxNode`
  alias → `visit_x(&mut XxxClo(f), self)`.
- **Mut side:** `XxxViewMut<'v, S>` with `&'v mut L` leaves, `as_view_mut`, `XxxCloMut`, `.visit_mut(f)`.
- **Edge cases:** heterogeneous params → `XxxView<'v, S, T>`; multi-root → adapter `Visit` keyed on the
  roots' params; unlisted *drilled* types get no view (still descended structurally, invisible to the
  closure); a mixed leaf+child tuple field borrows only its leaf sub-parts.
- **Tests:** closure counting leaves; mut closure editing a leaf; tuple `(f_expr, f_stmt)`.
- **Cost / limit:** a `View`/`ViewMut` + `as_view*` per type; the closure **cannot read or descend
  recursive children** (leaves only) — the inherent ceiling of this option; leaf borrows add `'v`.

### Option 2 — Natural-type conversion (sound; closure can fully descend; deep-copies) — most faithful

- **Emit per cycle type `Xxx`:** the natural recursive `pub enum XxxNat<S> {…}` — a recursive child to
  cycle type `Y` → `Box<YNat<S>>` (containers preserved, `Vec<Box<YNat<S>>>`), a leaf `L` → `L`. Plus a
  **free fn** `pub fn to_xxx_nat<R>(&__XxxRec<S, R>) -> XxxNat<S>` (a fn, so it *may* be `R`-generic
  where a closure can't): `match` each variant, recurse `to_*_nat` on children (descending the depth),
  `.clone()` leaves. Requires every leaf `: Clone`.
- **Run an ordinary closure visitor over `XxxNat`:** `XxxNat` is a normal (mutually-)recursive enum —
  one type at every depth — so the **acyclic `gen_side` machinery applies directly** (emit its
  `Visit`/`Driver`/closure plumbing for the `*Nat` set, classifying `Box<YNat>` as the followed child).
  `|n: &XxxNat<S>|` can `match` *and* descend. Inherent `.visit(f)` → `visit_xxx_nat(&to_xxx_nat(self),
  &mut f)`.
- **Mut side:** convert → mutate the `Nat` → write back with `from_xxx_nat<R>(&XxxNat<S>) -> __XxxRec<S,
  R>`, which must rebuild the depth chain and so only succeeds up to `limit` (a `Nat` deeper than
  `limit` can't be rebuilt). So `visit_mut` here is awkward — make this option **shared-side only**.
- **Edge cases:** every leaf `: Clone`; `XxxNat` is another public type per cycle; heterogeneous params
  carried through.
- **Tests:** a closure that matches a variant AND descends into its child (what Option 1 can't do).
- **Cost:** a deep copy + `Clone` bound per `.visit()`; the closure sees a real, fully-traversable value.

### Option 3 — Cast `&__XxxRec<S, R>` → `&Xxx<S>` (UNSOUND — rejected; no plan)

All `__XxxRec<S, R>` share a layout (`Box<R>` is a thin pointer), so `unsafe { &*(i as *const _ as
*const Xxx<S>) }` type-checks — but a closure handed `&Xxx<S>` that descends a child reads a *shallower*
node's `Box<R'>` as `Box<__XxxDefault>` and dereferences past its terminator → UB (and violates
provenance/aliasing). No way to restrict to non-descending closures. **Not implementable.**

### Option 4 — `dyn`-erased dispatch (sound; dynamic; awkward API)

- **Emit per cycle type:** `pub enum XxxKind {…}` (variant tag) and an object-safe accessor trait. The
  heterogeneity wrinkle: a cross-edge child is a *different* node type, so a per-type `XxxDyn` can't type
  `child()` — emit one **cycle-wide** `pub trait CycleNode<S> { fn kind(&self) -> NodeKind; fn child(&self,
  i: usize) -> Option<&dyn CycleNode<S>>; /* leaf accessors */ }`, impl'd for every `__YRec<S, R>` in the
  cycle (`R`-generic): `child(i)` erases the i-th recursive child to `&dyn CycleNode`.
- **Adapter + inherent:** `struct CloDyn<F>(F); impl<S, F: FnMut(&dyn CycleNode<S>)> Visit<S> for
  CloDyn<F> { fn visit_x<R>(&mut self, i) { (self.0)(i as &dyn CycleNode<S>); visit_x(self, i); } }`. The
  closure **can** descend via `child()`. Mut side: a parallel `CycleNodeMut<S>` with `child_mut`.
- **Cost / limit:** object-safety bans generic/`Self`-returning methods → leaf access is an
  accessor API (per-leaf-type method or index + enum return), **no pattern-matching**; the cycle-wide
  supertrait must erase or fix heterogeneous params. Dynamic dispatch (a vtable per node type). Sound and
  copy-free with descent, but the least ergonomic.

### Option 5 — Per-depth monomorphization (impossible; no plan)

Can't emit `limit+1` `Visit` impls for one `ClosureDriver<F>` (each redefines the same `visit_x<R>`),
and one `FnMut` value can't be `FnMut(&__XxxRec<S, level_k>)` for every distinct level type `k`. No
closure covers all depths — the type-HRTB wall restated. **Not implementable.**

### Recommendation

If ever lifted: **#2 (natural-type)** for a faithful, fully-traversable closure API (cost: deep copy +
`Clone`, shared-side only); **#1 (erased view)** when the closure needs only leaves and a copy is
unacceptable (and `&mut` is wanted). #4 only if dynamic dispatch + an accessor API are acceptable.
#3 unsound, #5 impossible.

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

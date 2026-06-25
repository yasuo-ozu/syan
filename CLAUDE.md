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
- **`#[recurse(visit)]`**: a **depth-generic** visitor over a `#[recurse]` *cycle* — a `Visit<S…>` with
  `visit_*<R>` methods, a `VisitRec<S…,V>` dispatch (root's depth chain drives, terminator is a no-op),
  and `XxxNode` aliases. Trait-based only. Traverses `Vec`/`Option`/`Box` (incl. `Box`-around-`Option`)
  and **tuples**; cycle types may carry lifetime/type/const params, possibly heterogeneous (extras
  become `visit_*` method generics). Requirements: every cycle type declares the root's params, and a
  back-edge to the root repeats them **verbatim** (a non-identity arg like `Expr<Vec<S>>` is rejected).
  Clean `abort!`s for nested containers, a missing root param, and a non-identity root arg. Tests:
  `visitor_recurse_cycle.rs`, `visitor_recurse_containers.rs`, `recurse_generics.rs`,
  `recurse_audit_test.rs` + `ui/recurse_*.rs` (`limit = 0` still panics).
- **Multiple independent cycles in one `#[recurse]` module**: cycle types are partitioned into SCCs
  (`find_cycle_sccs`, via `safegraph` Tarjan); each cycle gets its own root/chain/`XxxTerm`/aliases/
  visitor (`build_scc`). Several cycles → root-prefixed visitor trait names (`ExprVisit`, …); a lone
  cycle keeps `Visit`/`VisitRec`. Fixed a latent collapse-into-one-`__Rec` miscompile.
  `recurse_multi_cycle.rs`.
- **Multiple self-referential roots within one cycle**: each root keeps its own depth dimension (one
  depth param per root, depth chains unrolled mutually, per-root `XxxTerm`) — `build_multiroot_tail` /
  `generate_multiroot_visitor`. Soundness guard: the SCC minus its roots must be acyclic
  (`subgraph_is_cyclic`, via `safegraph` `is_cyclic_directed`) else a clear `abort!`
  (`ui/recurse_multiroot_rootless_subcycle.rs`). Roots share params. `recurse_multiroot.rs`.
- **`visitor!(…)` over a `#[recurse]` cycle**: `#[recurse]` emits `@recurse` metadata under each cycle
  type's original name, and `visitor!()` consumes it to generate a depth-generic visitor keyed on its
  own `Visit`/`VisitMut` traits — fixed `visit_X(&X)` for acyclic targets, depth-generic
  `visit_Y<R…>(&YNode<…>)` for recurse targets, with `VisitRec`/`VisitRecMut` dispatch, in one unified
  `generate_module_mixed` (acyclic-only visitors keep the original `gen_side` path). One `visitor!()`
  can **mix** acyclic + recurse types (the outer→inner boundary auto-crosses), span **several
  independent cycles** (each recurse target carries its own cycle's roots/depth), handle **multi-root**
  cycles (one depth param per root), and emits both the **shared and `&mut`** sides. Works
  **cross-crate** too — a downstream `visitor!(upstream::Expr, …)` over an upstream `#[recurse]` cycle
  resolves the `$crate`-rooted `@node`/`@terms` back to the defining crate; inherent `.visit()` is
  skipped for a *foreign* target (an inherent impl there is E0116 — use the `Visit::visit_*` trait
  method), via `path_is_crate_local`. Tests: `visitor_recurse_via_visitor.rs` (incl. `visit_mut`),
  `visitor_recurse_mixed.rs`, `visitor_recurse_multiroot_via_visitor.rs`,
  `visitor_recurse_multicycle_via_visitor.rs`, `rust/tests/cross_crate_recurse.rs`. See the
  "`#[recurse]` expansion & how `visitor!()` consumes it" section for the contract and current limits.

## Known gaps / limitations

- **`visitor!(…)` over `#[recurse]` — remaining limits** (the capability is shipped; see "Shipped &
  tested" + the expansion section): no inheritance (`base => …`) over recurse; no closures (depth-generic
  methods can't back a closure `Driver`, so it's struct/`&mut`-visitor only); an *unlisted* recurse
  cross-edge must be listed (no inline drill). All are clean `abort!`s, with fix approaches in the "Fix
  plan" below. (Drill-in over *acyclic* types in a `#[recurse]` module does work —
  `visitor_recurse_drill.rs`.)
- **Two visited types sharing a last segment** (`visitor!(a::Foo, b::Foo)`): all generated names key
  off the last segment, so they collide. Now a clear build error (`visitor_diagnostics.rs`); genuine
  coexistence would need full-path-disambiguated names.
- **Nested containers** (`Vec<Option<T>>`) are unsupported on both the `visitor!()` and
  `#[recurse(visit)]` paths (clear build error); wrap the inner part in its own `#[derive(Ast)]` type.

## Fix plan — `visitor!()`-over-`#[recurse]` remaining limits

Concrete approaches for the limits above (all in `macro/visitor.rs` unless noted), ordered by
value/tractability:

1. **Drill an *unlisted* recurse cross-edge** (highest value, moderate effort). Today
   `recurse_lower_field` `abort!`s when a field head is in the cycle but not in `method_set`. Fix:
   inline-drill instead — the unlisted type is already fetched (it's a `#[subast]` head, so
   `followed_intermediates` enqueues it; its def + `@recurse` land in `done_by_path`), so destructure
   its node (`__YRec<S, dps>`, the **same** depth params — it's the same cycle) and recurse into its
   fields, dispatching back-edges through the in-scope `dps` exactly as for the listed types. Carry a
   stack of in-progress unlisted heads and reject a *cycle of unlisted* intermediates (mirrors the
   acyclic `Lower::visit_value` drill guard). `visit_Y` is still emitted only for listed `Y`.
2. **Inheritance (`base => …`) over recurse** (niche, larger effort). Restrict to the trait path (the
   `Driver`/closure side is already off for recurse). In `generate_module_mixed`, when `st.base` is a
   recurse visitor: add `base::Visit`/`VisitMut`/`VisitRec` as **supertraits** of the new traits (reuse
   the existing ancestor/`@an` requalification machinery — `base_host_crate`/`requalify_ancestor`),
   keep the inherited recurse types in `method_set` without re-emitting their bodies (their `VisitRec`
   impls come from `base`), and let the new types' bodies cross into them via `this.visit_<inherited>`.
   Requires the new union params ⊇ the base's, and the base's depth params to line up — feasible since
   `VisitRec`'s `visit_rec` signature is uniform.
3. **Collapse `#[recurse(visit)]` into sugar** (cleanup; large + breaking-risk). Make it expand to
   `#[recurse]` + an auto-`visitor!(<cycle types>)`, then delete `recurse.rs`'s
   `generate_recurse_visitor`/`generate_multiroot_visitor`. **Blocker:** `#[recurse(visit)]` keys its
   `Visit` trait on the *root's* params (a non-root's extras → method generics, e.g. the heterogeneous
   `recurse_generics.rs` case `Visit<S>` + `visit_stmt<T, R>`), while `generate_module_mixed` keys on
   the *union* (`Visit<S, T>`). To avoid changing the API (and breaking those tests), first switch the
   recurse path to **root-params keying** (union → roots' params; each recurse type's params beyond
   the roots' become `visit_*` method generics), then route both through one generator.
4. **Closures over recurse — won't fix (fundamental).** A `visit_*<R: VisitRec>` method is generic
   over the remaining depth; a closure is one concrete `FnMut` and can't be depth-generic, so the
   `Driver`/`Hook`/`Chain` machinery cannot implement the unified `Visit` trait. This is inherent (the
   same reason `#[recurse(visit)]` was always trait-only) — document it, don't track it as a TODO.

---

# Drill-in — as implemented (selective `visit_*` + transitive drill-through)

> **Implemented.** The design below is the rationale; it is shipped in `macro/ast.rs` +
> `macro/visitor.rs` and tested in `core/tests/visitor_drill*.rs` + `visitor_diagnostics.rs`. A few
> deltas from the plan text, chosen during implementation:
>
> - **Metadata shape (vs. "per-field records").** The metadata macro carries the *cleaned definition*
>   plus `@subast { path as matchkey, … }` — **not** per-field `@SELF`/`@leaf`/container/accessor
>   records. `__visitor_build` re-derives container/accessor/self/leaf from the def itself (`peel`)
>   and matches field heads against the `#[subast]` entries. So there is **no `@SELF` token**: a
>   visited type's own path is known from `visitor!(…)` (`path_of`), a self-referential field is a
>   plain method call, and a drilled intermediate's scrutinee is its `#[subast]`-resolved path.
> - **Effective head.** A followed field's method/drill decision uses the matched `#[subast]` entry's
>   path **last segment** (the real type name), not the field's possibly-aliased head — so
>   `#[subast(crate::Real as Aliased)]` on a *visited* `Real` dispatches to `visit_real`.
> - **Pathing.** Canonical `crate::`-rooted `#[subast]` paths are recommended; they are emitted
>   `$crate`-rooted in the metadata macro, so a visitor built **downstream** over an upstream crate's
>   `#[subast]` types resolves them (`rust/tests/cross_crate_drill.rs`). Non-`crate` paths
>   (`super`/`self`/abs/bare) are emitted verbatim.
> - **Lints.** Both the `unused entry` warning and the "this type follows nothing" lint are
>   implemented (nightly-visible, like all syan derive warnings).

## Context

The drill-in goal is to traverse *through* an Ast type the visitor has no `visit_*` for (the spec's
`Expr::Cast(cast) => this.visit_type(&cast.0)`). The model uses **two lists**:

- **`#[subast(...)]`** (per `#[derive(Ast)]` type) — the type's Ast *children*: which fields are
  *followed*. A field is followed iff its (container-peeled) head is listed there (or is the type's
  own type — self-recursion is implicit); **all other field types are ignored** (leaves: tokens,
  primitives, `PhantomData`, spans, …). The entry also supplies the resolvable path to that sub-AST's
  metadata macro (membership + pathing in one attribute). No `#[no_ast]`.
- **`visitor!(T, …)`** (per visitor) — the *visited set*: which followed types get a `visit_*`
  **method**. A followed field whose head is **listed in `visitor!(...)`** lowers to
  `this.visit_<head>(field)`; a followed field whose head is **not** listed (an *unlisted
  intermediate*, e.g. `Cast`) is **drilled through inline** — its metadata macro is invoked to reach
  the listed types nested inside it (`this.visit_type(&cast.0)`), and it gets **no** `visit_cast`.

So `visit_*` is defined only on types named in `visitor!(...)`; every `#[derive(Ast)]` type still
emits its metadata macro, but an unlisted type's macro is invoked only while processing a type whose
`#[subast]` lists it (i.e. to drill through it). The enabling fact: "is this followed head an Ast
type to invoke?" needs no macro-time existence test — `#[subast]` already declares it (and provides
its path). The `Repeater` impls (emitted by `#[derive(Ast)]`) are a fallback consumer: portable
sub-AST refs.

## `#[derive(Ast)]` (`macro/ast.rs`)

`#[subast(...)]` is the single classification + pathing source. Helper attributes:
`#[proc_macro_derive(Ast, attributes(syan, subast))]`.

- **`#[subast(<paths>)]`** — a type-level **allowlist** of this type's sub-AST types, each as a path
  resolvable **at the defining module** (forms: `crate::` / `::abs::` / `super::` / `self::` / bare
  sibling / `b::Foo as BFoo`; no generic args). A field is an Ast child **iff** its container-peeled
  head matches a listed entry (by last segment, alias-aware) — then that entry's path is its resolvable
  path. **Self-recursion is implicit**: a field whose head is the type's own type is always followed
  (path via `@SELF`, below); you do not list self. **Every other field is ignored** — bound `_`, a
  leaf, no traversal. There is no `#[no_ast]` (no per-field or per-type leaf marking).
- **Trade-off (per directive):** the allowlist is explicit and self-documenting ("these are my
  sub-ASTs and where they live") and solves pathing uniformly. The accepted cost is the
  *silent-omission* failure mode — forgetting to list a sub-AST silently stops traversal into it
  (vs. a denylist's loud "cannot find macro"). The `unused entry` warning and an optional
  "this `#[derive(Ast)]` type follows nothing" lint mitigate typos.
- The metadata macro emits, per variant → per field, a record: a **followed** field carries the
  **accessor** (tuple index / named ident), the **container**
  (`direct`/`box`/`vec`/`vecdeque`/`option`/`slice`/`punctuated`), the inner **head ident** (literal —
  for `visit_<snake(head)>` construction in the proc-macro, *never* in `macro_rules!`), and the
  **resolved path** (the entry's path; `@SELF` for a self-referential head, substituted by
  `__visitor_build` with the path it fetched the type by); an **ignored** field carries just `@leaf`.
- Diagnostics at the definition site: two `#[subast]` entries with the same last segment (a bare field
  head can't disambiguate) ⇒ **error** (hint: alias one, `b::Foo as BFoo`); an entry matching no field
  ⇒ **warning**.
- Keep the `Leaker` + `Repeater<N>` impls as a last-resort type-namer / for external metadata
  consumers; the visitor path uses `#[subast]`-resolved paths.

## Model — selective `visit_*`, drill through the rest

`visitor!(T, …)` lists the **visited set**; `__visitor_build` generates `visit_*`/`visit_*_mut` for
**those types only**. Generating a visited type's body walks its `#[subast]` fields:
- head ∈ visited set ⇒ `this.visit_<head>(access)` — a method call; recursion runs through the trait,
  handling recursive/cyclic *visited* types exactly like today's visited-type traversal.
- head a followed **intermediate** (∈ `#[subast]`, ∉ visited) ⇒ **inline drill**: invoke the
  intermediate's metadata macro, recurse into *its* `#[subast]` fields with the accessor extended
  (`&cast.0`, `&cast.0.1`, …) under the same rules — so listed types nested arbitrarily deep inside
  unlisted wrappers are reached (`Expr::Cast(c) => this.visit_type(&c.0)`), but the wrapper gets **no**
  `visit_cast`.
- head not followed (∉ `#[subast]`, not self) ⇒ leaf, bind `_`.

**Cycle guard:** inline drilling keeps a stack of intermediates being expanded; a cycle of *unlisted*
intermediates (`Cast`→`Cast`, or `A`→`B`→`A`, none visited) cannot be expanded inline ⇒ a
`__visitor_build` **error** ("list one of them in `visitor!(...)`" so a method call breaks the
recursion). Recursion through *visited* types is fine — it's a method call, not inline. A *finite*
drill subtree that bottoms out at leaves without reaching any visited type is **not** an error — it
just lowers to no `visit_*` calls; only an unlisted-intermediate *cycle* (infinite expansion) errors.

## `__visitor_build` (`macro/visitor.rs`)

- **Emitted only for listed types.** `visit_*`/`visit_*_mut` free fns + trait methods are generated
  for the `visitor!(...)`-listed types only; so are the one-shot items (`Visit`/`VisitMut` traits,
  `Driver`/`Hook`/`Chain`, `IntoVisitor`/`IntoVisitorMut`, inherent `visit`/`visit_mut`), built at the
  end from the listed-types **name-list** (ident + path + own-generics).
- **Body lowering** per `#[subast]` field of the type being emitted (peel the container, then):
  - head ∈ visited set ⇒ `this.visit_<head>(access)` — method name built in-proc from the literal head
    ident (`to_snake`; never in `macro_rules!`); the type's own match scrutinee uses `@SELF` = the path
    it was fetched by.
  - head a followed intermediate (∈ `#[subast]`, ∉ visited) ⇒ **inline drill** (recurse into its
    `#[subast]` fields with the accessor extended; its match scrutinee uses its `#[subast]`-resolved
    path; cycle guard per *Model*).
  - head ∉ `#[subast]` (`@leaf`) ⇒ bind `_`, skip.
- **Discovery / ping-pong.** Membership (visited? followed?) is decided in the proc-macro — it holds
  the visited set, and the `#[subast]` records carry followed/leaf + resolved paths. It fetches each
  listed type and each unlisted intermediate reachable for drilling, by the record's **resolved path**
  (`@SELF` is the current type — never fetched/enqueued). Fetch-dedup is on the **full resolved-path
  string**, not the last segment, so distinct types sharing a last segment (`a::Cast` vs `b::Cast`)
  are both fetched. No `path_of`, no inference.

## Resolved decisions (deep dive)

### Decision 1 — Containers: **in scope.**
A visitor that can't descend into `Vec<Stmt>` / `Option<Expr>` / `Box<Expr>` is useless on real ASTs
(blocks, item lists, optional sub-exprs). The earlier removal dropped the *seq/opt-method* machinery,
not container traversal as a goal. Recognized set: `Box`, `Vec`, `VecDeque`, `Option`, `[T]` /
`Box<[T]>`, and syan's `Punctuated` (extensible). Lowering: deref for `Box`; `for x in &…` (`&mut` on
the mut side) for `Vec`/slice/`Punctuated`; `if let Some(x) = …` for `Option`; then visit the inner
head. The inner head is followed iff its type is listed in `#[subast]` (a container of an unlisted
type, e.g. `Vec<Token>`, is ignored). Reduce/append is unchanged: override the parent's
`visit_*_mut`, which owns the `&mut Vec` / `&mut Option`.

### Decision 2 — Delegation & model: **proc-macro composes; selective drilling.**
Two hard `macro_rules!` facts settle it: (1) it **cannot compare two idents** for equality (no
`$a == $b`; reusing a metavar name in a matcher is an error) → it can't test visited-set membership;
(2) it **cannot concat / snake-case idents** (no `format_ident!`) → it can't build `visit_stmt` from
`Stmt`. So both membership *and* body generation (method names, match arms) must live in the
proc-macro. "Delegate via `macro_rules!`" is therefore the metadata macros *supplying each type's
structure* (the ping-pong fetch *is* the delegation); `__visitor_build` composes, doing what
`macro_rules!` can't: test visited-set membership, `to_snake` method names, and run the inline-drill
recursion + cycle guard. **Selective drilling** is chosen (per directive): `visit_*` only for
`visitor!(...)`-listed types, unlisted `#[subast]` types drilled through inline — a smaller,
intentional interface than visit-all (`Cast` is not visitable; you expose exactly the nodes you mean
to visit). The two lists stay distinct: `#[subast]` (per type) is the **follow-list** (+ pathing);
`visitor!(...)` is the **method-list**.

### Scale.
Only the `visitor!(...)`-listed types get methods, so the trait / `Driver` / `IntoVisitor` / inherent
items are over that (small, explicit) set, emitted once at the end from the **name-list** of listed
types (ident + path + own-generics; Rust allows items in any order, so a body may reference the trait
emitted last). A listed type's `visit_*` body is emitted once its **drill closure** (the unlisted
intermediates reachable from it until a visited type or leaf) is fetched; intermediate structures are
**used for inline drilling and dropped** — never turned into methods, never grown into the trait. Do
not re-emit all fetched structures each ping-pong bounce (that is O(N²)); carry only the name-list +
the current drill closure. The trait's **union generics = the root type's generic params** (known
early, so closures keep working); a sub-type introducing a new generic param name is an error.

### Pathing — `#[subast]` supplies every followed path.
Every followed field's resolvable path is its matching `#[subast]` entry's path — used to **fetch**
that sub-AST's metadata macro (whether it becomes a `visit_*` call or is drilled) and as the **match
scrutinee** when it is drilled. Self-recursion uses `@SELF` (the path the current type was fetched by;
match scrutinee only, never enqueued as a discovery edge). Unlisted field types are ignored, so there
is no path to infer for them. The
`#[subast]` paths are resolved at the *defining* module and republished portably — one `pub use`
carries **both** the type and metadata-macro namespaces, and `$crate`-rooting the `#[macro_export]`
half makes it resolve same-crate and downstream. No module-prefix inference, no sibling guessing: a
missing/typo'd entry is a *silently ignored* field (the directive's accepted failure mode), caught
only by the `unused entry` warning / "follows nothing" lint, not a mis-resolved path. Residual hole:
naming a sub-AST *type* when the `visitor!(...)` *entry* path is itself a non-canonical re-export —
closed by requiring canonical (`crate::`/`super::`-rooted) entry paths in `visitor!(...)`. (The
`Leaker` marker has since been dropped: `Repeater` is implemented on the AST type itself.)

## Tests (`core/tests`)

- Spec graph: `Type`, `Cast(Type)`, `Expr { Cast(Cast) }` with `#[subast(super::Cast)]` on `Expr`
  and `#[subast(super::Type)]` on `Cast`; `visitor!(super::Expr, super::Type)` (Type listed, **Cast
  not**) ⇒ `visit_expr` drills through `Cast` to `this.visit_type(&cast.0)`; `Cast` is **not**
  visitable (no `visit_cast`); a closure `|t: &Type<()>| …` fires once.
- Follow-list vs method-list: a field followed and listed (`#[subast(crate::ast::Stmt)]` + `Stmt` in
  `visitor!(...)`) lowers to `visit_stmt`; followed but unlisted ⇒ drilled; not in `#[subast]` ⇒
  silently ignored (bound `_`); an imported/aliased field (`use other::Stmt; … s: Stmt` with
  `#[subast(other::Stmt)]`) resolves (the gap `core/tests/visitor_local_types.rs` documents); a
  same-last-segment `#[subast]` collision (`a::Foo`, `b::Foo`) fails at the derive.
- Self-recursion: `Expr { Bin(Box<Expr<…>>, …) }`, `Expr` listed but *not* in its own `#[subast]`,
  recurses via the `visit_expr` method (its own scrutinee via `@SELF`).
- Cycle guard: a cycle of *unlisted* intermediates (`Cast → Cast`, none in `visitor!(...)`) ⇒
  `__visitor_build` error; listing one of them fixes it. A finite unlisted intermediate reaching only
  leaves (no visited type) ⇒ no `visit_*` calls, no error.
- Containers: a type with `Vec<Stmt>` / `Option<Expr>` / `Box<Expr>` fields (heads in `#[subast]`)
  descends into each element (visited ⇒ method, unlisted ⇒ drilled); reduce/append via overriding the
  parent's `visit_*_mut`.

---

# `#[recurse]` expansion & how `visitor!()` consumes it

> How `#[recurse]` (type transformer + metadata) and `visitor!()` (the one visitor generator) split:
> what `#[recurse]` expands to, and how a *unified* `visitor!()` builds a depth-generic visitor over
> that output, so one `visitor!(…)` can span both **outer** (acyclic) and **inner** (recurse-cycle)
> types in a single `Visit` trait. (Shipped — see the `visitor!()`-over-`#[recurse]` bullet above.)

## What `#[recurse]` expands to

Input:

```rust
#[recurse(limit = 2)]
mod ast {
    use core::marker::PhantomData;
    pub enum Expr<S> {
        Bin(Box<Expr<S>>, Box<Expr<S>>),   // self-reference: the recursion's back-edge
        Lit(PhantomData<S>),
    }
}
```

Expands (structural core) to:

```rust
mod ast {
    use core::marker::PhantomData;

    // The cycle type, RENAMED and parameterized by the depth `__Rec`. Every back-edge to the root
    // (`Box<Expr<S>>`) is rewritten to the depth param `Box<__Rec>`.
    pub enum __ExprRec<S, __Rec = __ExprDefault<S>> {
        Bin(Box<__Rec>, Box<__Rec>),
        Lit(PhantomData<S>),
    }

    // Terminator: caps the recursion. Its `Parse` errors ("recursion depth limit reached"),
    // its `Unparse` panics. (`#[recurse]` emits these impls for the terminator.)
    pub struct ExprTerm<S>(PhantomData<(S,)>);

    // Depth chain: `limit` levels of `__ExprRec`, bottoming out at the terminator.
    type __ExprDefault<S> = __ExprRec<S, ExprTerm<S>>;            // limit-1 = 1 inner level

    // The public, depth-limited type the user actually names:
    pub type Expr<S> = __ExprRec<S, __ExprDefault<S>>;
    //               = __ExprRec<S, __ExprRec<S, ExprTerm<S>>>     // 2 levels, then terminate
}
```

Pieces (general rules — see `macro/recurse.rs`):
- **`__ExprRec<S, __Rec = …>`** — the renamed cycle type, gaining one depth param per *root* (a
  directly self-referential cycle type). A **back-edge** to root `X` becomes that root's depth param;
  a **cross-edge** to another cycle type `Y` becomes `__YRec<S, __Rec…>` (Y re-expressed at the same
  depth — it threads the same depth params); a leaf is untouched.
- **`XTerm<…>`** — one terminator per root.
- **`__XDefault<…>`** — the depth chain (the multi-root case builds all roots' chains *mutually*).
- **`pub type X<…> = __XRec<…, defaults…>`** — the public alias; each *depth level is a distinct
  type*. User derives (`#[derive(Ast)]`, `Parse`, `Unparse`) apply to the renamed `__XRec`.

The crucial fact for visiting: **each depth level of `Expr<S>` is a different type** (`__ExprRec<S,
__ExprDefault<S>>`, then `__ExprRec<S, ExprTerm<S>>`, …). A *fixed-type* `visit_expr(&Expr)` therefore
cannot recurse into its own child — the visitor must be **depth-generic**.

## How `visitor!()` consumes it

`visitor!(crate::ast::Expr)` (Expr listed) generates a **depth-generic** visitor keyed on its *own*
`Visit` trait:

```rust
pub trait VisitRec<S, V> { fn visit_rec(&self, v: &mut V); }      // depth dispatch

pub trait Visit<S> {
    // depth-generic: `R` is the remaining depth (`VisitRec`-bounded)
    fn visit_expr<R: VisitRec<S, Self>>(&mut self, i: &__ExprRec<S, R>) { visit_expr(self, i) }
}

pub fn visit_expr<S, V: Visit<S>, R: VisitRec<S, V>>(v: &mut V, i: &__ExprRec<S, R>) {
    match i {
        __ExprRec::Bin(a, b) => { R::visit_rec(a, v); R::visit_rec(b, v); }   // back-edge → R
        __ExprRec::Lit(_) => {}                                              // leaf
    }
}

// the root's depth chain drives the visit; the terminator is a no-op
impl<S, R: VisitRec<S, V>, V: Visit<S>> VisitRec<S, V> for __ExprRec<S, R> {
    fn visit_rec(&self, v: &mut V) { <V as Visit<S>>::visit_expr(v, self); }
}
impl<S, V: Visit<S>> VisitRec<S, V> for ExprTerm<S> {
    fn visit_rec(&self, _v: &mut V) {}
}

pub use __ExprRec as ExprNode;   // so users can spell the method's node type
```

**Field classification** in a recurse type's body (this is `recurse_dispatch_field`'s logic, keyed on
`visitor!()`'s method set): a field head that is a **root** → back-edge → `R::visit_rec`; a head that
is another **listed** cycle type → `this.visit_<head>(field)`; an **unlisted** cycle type → drilled
inline (its fields recursed, its back-edges still via `R`); anything else → leaf. `visit_<X>` is
emitted **only** when `X` is listed in `visitor!(…)` (selective), exactly as for acyclic types.

**Why outer + inner unify in one trait.** An outer (acyclic) type `Program<S>` with a field
`body: Vec<Expr<S>>` lowers that field to `this.visit_expr(e)` — and because `Expr<S> =
__ExprRec<S, __ExprDefault<S>>`, the call infers `R = __ExprDefault<S>`. So a *fixed* `visit_program`
and a *depth-generic* `visit_expr` live in the **same `Visit<S>` trait**; one `Visit` impl + one
`.visit()` walks the whole tree and crosses the boundary into the cycle automatically (no manual
`rec::Visit::visit_expr(...)` hand-off as in `visitor_mixed_recurse.rs`). The back-edge inside the
cycle dispatches through `R::visit_rec`, so the depth `__Rec` is handled entirely by the visit traits.

**Constraints.** Depth-generic methods can't be implemented by a closure `Driver`, so a visitor that
lists any recurse type is **trait/struct-only** (no closures/tuples) — the same limitation
`#[recurse(visit)]` always had. Multi-root cycles give `visit_<X>` one `R` per root.

## Multiple AST types + multi-root example

When several cycle types are **each self-referential** (here `A` and `B` both `Box<Self>`), the cycle
has *two roots*. Every cycle type then carries **one depth param per root** (`__RecA`, `__RecB`, in
sorted-root order), a reference to root `X` becomes that root's param, and the depth chains are
unrolled **mutually**.

Input:

```rust
#[recurse(limit = 2)]
mod ast {
    use core::marker::PhantomData;
    pub enum A<S> {
        SelfA(Box<A<S>>),   // back-edge to root A
        ToB(Box<B<S>>),     // edge to root B
        Lit(PhantomData<S>),
    }
    pub enum B<S> {
        ToA(Box<A<S>>),     // edge to root A
        SelfB(Box<B<S>>),   // back-edge to root B
        Lit(PhantomData<S>),
    }
}
```

`#[recurse]` expands (structural core) to:

```rust
mod ast {
    use core::marker::PhantomData;

    // Each cycle type carries one depth param PER ROOT (sorted order A, B). A ref to root A → __RecA,
    // a ref to root B → __RecB (both A and B are roots).
    pub enum __ARec<S, __RecA = __ADefault<S>, __RecB = __BDefault<S>> {
        SelfA(Box<__RecA>),
        ToB(Box<__RecB>),
        Lit(PhantomData<S>),
    }
    pub enum __BRec<S, __RecA = __ADefault<S>, __RecB = __BDefault<S>> {
        ToA(Box<__RecA>),
        SelfB(Box<__RecB>),
        Lit(PhantomData<S>),
    }

    pub struct ATerm<S>(PhantomData<(S,)>);   // one terminator per root
    pub struct BTerm<S>(PhantomData<(S,)>);

    // Depth chains unrolled MUTUALLY: level k of each root embeds level k-1 of *all* roots
    // (limit = 2 → one inner level, then the terminators).
    type __ADefault<S> = __ARec<S, ATerm<S>, BTerm<S>>;
    type __BDefault<S> = __BRec<S, ATerm<S>, BTerm<S>>;

    pub type A<S> = __ARec<S, __ADefault<S>, __BDefault<S>>;   // one default per root
    pub type B<S> = __BRec<S, __ADefault<S>, __BDefault<S>>;
}
```

`visitor!(crate::ast::A, crate::ast::B)` then emits a visitor whose `visit_*` are generic over **all**
roots' remaining depth (`__R0` = A's, `__R1` = B's), dispatching each back-edge through the matching
param; each root's node drives its own `visit_*` via `VisitRec`:

```rust
pub trait VisitRec<S, V> { fn visit_rec(&self, v: &mut V); }

pub trait Visit<S> {
    fn visit_a<__R0: VisitRec<S, Self>, __R1: VisitRec<S, Self>>(&mut self, i: &__ARec<S, __R0, __R1>) { visit_a(self, i) }
    fn visit_b<__R0: VisitRec<S, Self>, __R1: VisitRec<S, Self>>(&mut self, i: &__BRec<S, __R0, __R1>) { visit_b(self, i) }
}

pub fn visit_a<S, V: Visit<S>, __R0: VisitRec<S, V>, __R1: VisitRec<S, V>>(v: &mut V, i: &__ARec<S, __R0, __R1>) {
    match i {
        __ARec::SelfA(a) => __R0::visit_rec(a, v),   // → root A's depth param
        __ARec::ToB(b)   => __R1::visit_rec(b, v),   // → root B's depth param
        __ARec::Lit(_)   => {}
    }
}
// visit_b is symmetric: ToA → __R0, SelfB → __R1.

// One VisitRec impl per ROOT node (drives that root's visit) + one per terminator (no-op).
impl<S, __R0: VisitRec<S, V>, __R1: VisitRec<S, V>, V: Visit<S>> VisitRec<S, V> for __ARec<S, __R0, __R1> {
    fn visit_rec(&self, v: &mut V) { <V as Visit<S>>::visit_a(v, self); }
}
impl<S, __R0: VisitRec<S, V>, __R1: VisitRec<S, V>, V: Visit<S>> VisitRec<S, V> for __BRec<S, __R0, __R1> {
    fn visit_rec(&self, v: &mut V) { <V as Visit<S>>::visit_b(v, self); }
}
impl<S, V: Visit<S>> VisitRec<S, V> for ATerm<S> { fn visit_rec(&self, _v: &mut V) {} }
impl<S, V: Visit<S>> VisitRec<S, V> for BTerm<S> { fn visit_rec(&self, _v: &mut V) {} }

pub use __ARec as ANode;
pub use __BRec as BNode;
```

(A reference to a *non-root* listed cycle type would instead lower to `this.visit_<head>(field)`, as
in the single-root case; here both `A` and `B` are roots, so every edge is a depth-param dispatch.)
This is exactly the soundness requirement under "Multiple self-referential roots": the depth only
decrements at a root, so a sub-cycle that avoids every root is rejected.

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

# Visitor system — current state

Generate `syn`-style visitors from AST type definitions.

Code: `core/src/visit.rs` (`Ast`, `Repeater` traits), `macro/ast.rs` (`#[derive(Ast)]`),
`macro/visitor.rs` (`visitor!` shim → `__visitor_entry` → `__visitor_build`). Tests:
`core/tests/visitor_*.rs`, `core/tests/ast_derive.rs`, `core/tests/ast_recurse.rs`; cross-crate:
`rust/tests/cross_crate.rs` with the AST sample in `rust/src/lib.rs`.

## Shipped & tested

- **`#[derive(Ast)]`**: empty `Ast` marker impl + a `#[macro_export]` callback "metadata" macro
  carrying a cleaned copy of the definition (re-parsed downstream as a `syn::Item`), re-exported
  under the type's own name so it's reachable as `path::to::T!{..}` (type vs. macro namespaces
  coexist). Also emits one `::syan::visit::Repeater<N>` impl **on the type itself** per
  context-dependent field type (`<T as Repeater<N>>::Type`; external-metadata fallback). `crate::`-
  rooted `#[subast]` paths are emitted `$crate`-rooted so they resolve downstream.
- **`visitor!([base =>] T, …)`** invoked **inside** an (otherwise empty) `mod` (function-like; a
  `macro_rules!` shim in `syan` captures `$crate` and forwards the syan path to `__visitor_entry`,
  so no `#[syan(..)]` needed). A metadata ping-pong through `__visitor_build` generates, per visited
  type: a `Visit`/`VisitMut` trait method, a free `visit_*`/`visit_*_mut` traversal fn, and
  **inherent** `visit`/`visit_mut` methods on the type (no trait import at the call site; the
  visitor must be generated in the types' crate). Type args are paths resolvable from inside the
  visitor module (`super::Expr`, `crate::ast::Expr`).
- **`#[subast(<path> [as Alias])]` + drill-in** (shipped): `#[derive(Ast)]` takes a type-level
  `#[subast]` allowlist of the type's Ast children (+ resolvable paths), carried in the metadata as
  `@subast { path as matchkey, … }`. A field is *followed* iff its container-peeled head is a
  `#[subast]` matchkey or the type's own ident (self-recursion implicit); everything else is a leaf.
  `visit_*` is generated **only** for `visitor!(…)`-listed types; a followed field whose head is
  listed lowers to `this.visit_<head>(…)`, a followed **unlisted intermediate** is **drilled through
  inline** (its def destructured, recursing into *its* `#[subast]` fields) and gets no `visit_*`. A
  cycle of unlisted intermediates is a build error; a finite dead-end is a no-op. Tests:
  `visitor_drill*.rs`. (See "Drill-in — as implemented" below.) Diagnostics: a same-last-segment
  `#[subast]` collision is a derive error; an `unused entry` warning fires for a matchkey matching no
  field; a **"follows nothing" lint** warns when a type with no `#[subast]` has an AST-looking field
  (heuristic; `#[subast()]` opts out). Like all syan derive warnings these are visible on nightly,
  silent on stable.
- **Containers** (shipped): `Box` (transparent; tracked as box-depth for `&**` drill deref), `Vec` /
  `VecDeque` / slice / array / `Punctuated` (`for x in …iter()/iter_mut()`), `Option`
  (`if let Some(x) = …`, dereffing through any wrapping `Box`). Nested containers (`Vec<Option<T>>`)
  are unsupported and rejected with a clear error.
- **Inputs** (`IntoVisitor`/`IntoVisitorMut` selector design): struct visitors (via `&mut`), single
  closures, and **tuples of closures** (arity 2..=8) running in **one** traversal via a shallow
  `Hook` + single-pass `Driver` + `Chain`.
- **`visit_mut`** full mirror (in-place mutation). **Reduce/append**: override the *parent* node's
  `visit_*_mut` (it owns the `&mut Vec`/`&mut Option`), then descend — see `visitor_reduce.rs`.
- **Inheritance** `visitor!(base => New)` for new→base reference DAGs (base exports a
  `__syan_visited` list macro carrying its visited idents, its generic-param union `@bg`, and its
  full ancestor chain `@an`; the new trait extends it via supertrait, referencing `base::Visit<…>`
  with the base's own arity — so a wider new union works (`visitor_inherit_arity.rs`), and
  **multi-level chains** `base => mid => New` work: `New`'s `Driver` satisfies every transitive
  supertrait (`visitor_inherit_multilevel.rs`)).
- **Generics**: the trait is parameterized by the **union** of visited types' generic params
  (`Visit<S, Tokens>`); each type uses its own subset, so mixed arities work (`visitor_generics.rs`).
  Caveat: `.visit()` on a root that doesn't use every union param may need a turbofish. Generated
  helper params avoid the visited types' param names, so a visited type may declare a param literally
  named `__V`/`__T`/`__H`/`__F`/`__A`/`__B` (`visitor_hygiene.rs`).
- **Cross-crate** validated: visited types are named by the full path given to `visitor!(...)`, so a
  downstream crate needs no import (`rust/tests/cross_crate.rs`); a downstream-built visitor can also
  drill through an upstream crate's types (`$crate`-rooted `#[subast]`; `rust/tests/cross_crate_drill.rs`).
  **Cross-crate inheritance** also works: a downstream `visitor!(upstream::base => New)` inherits an
  upstream base visitor — everything is keyed on the base **path** (supertrait `New: base::Visit`, the
  inherited `base::visit_*` fns, and the base's `#[macro_export]`+`pub use`'d `__syan_visited` macro),
  so the new visitor descends into the inherited (upstream) types. Single-level and a wider-arity
  `Visit<S>`→`Visit<S, T>` case: `rust/tests/cross_crate_inherit.rs`. Multi-level `base => mid => New`
  works **including with an upstream intermediate** (base + mid both in the library, New downstream):
  `rust/tests/cross_crate_inherit_multilevel.rs`. The subtlety there is that `mid` records its
  ancestor `base` as the `crate::inherit::base` path *relative to `mid`'s own crate*; `$crate` can't
  fix this (emitted by a proc-macro into a generated `macro_rules`, `$crate` resolves only for
  fetch/macro-invocation paths — like the `#[subast]` drill — **not** for the `base::Visit` trait path
  re-emitted into New's `Driver` supertrait impl). Instead `__visitor_build` **requalifies** a
  `crate::`-relative `@an` ancestor against the direct base's host crate — taken from the concrete
  `syan_rust::inherit::mid` path New was given (`base_host_crate` + `requalify_ancestor` in
  `macro/visitor.rs`) — making it concrete and downstream-resolvable. Same-crate and already-concrete
  ancestor chains (e.g. a downstream intermediate naming `base` by its full cross-crate path) are
  untouched. Coverage: a 4-level all-upstream chain `base => mid => upper => New` (requalify loop runs
  for **both** transitive ancestors) is `rust/tests/cross_crate_inherit_4level.rs`; the complementary
  *downstream*-intermediate shape (the requalify no-op branch — ancestor already concrete) is
  `rust/tests/cross_crate_inherit_downstream_mid.rs`; multi-level **+ arity widening** at the leaf is a
  second test in `cross_crate_inherit_multilevel.rs`. Residual hole: a `super::`/`self::`-relative
  ancestor recorded by an *upstream* intermediate is not requalified (use `crate::`-rooted entry paths).
- **`#[recurse(visit)]`** (shipped): a **depth-generic** visitor over a `#[recurse]` *cycle*.
  `#[recurse]` rewrites the cycle's back-edges to the root into the generic `__Rec` param and each
  nesting level into a distinct type, so the visitor is parameterized by the remaining depth: a
  `Visit<S…>` trait with `visit_*<R>` methods (+ free `visit_*` drive fns), and a `VisitRec<S…, V>`
  dispatch trait implemented by the root's depth chain (drives the root visit) and by the terminator
  (no-op) — turning the depth recursion into trait dispatch on the back-edge. `XxxNode` aliases name
  the depth-generic node types. Trait-based only (a closure can't be generic over the depth).
  Single-root cycles use one depth param `__Rec`; **multi-root cycles** (several independent cycles, or
  several self-referential roots in one SCC) are also supported — see the "Multiple … cycles" bullets
  below. Followed fields traverse through `Vec`/`Option`/`Box` (incl. a `Box`
  *around* an `Option`, via `cont_box`) and **tuples** (each element dispatched). Cycle types may
  carry **lifetime / type / const generic params**, and the types in a cycle may have *different*
  params (heterogeneous): each keeps its own params (threaded into its `__*Rec` node + public alias),
  the `Visit`/`VisitRec` traits are keyed on the root's params, and a type's extra params become
  generics on its `visit_*` method. The one requirement is that every cycle type declare all of the
  **root's** params (so the `__Rec` default `__RootDefault<root params>` is spellable), and that every
  **back-edge to the root repeat the root's params verbatim** (identity) — a root reference collapses
  to the single depth param `__Rec`, so a non-identity argument like `Expr<Vec<S>>` (a *non-regular*
  recursion whose param grows per level) cannot be threaded. Unsupported shapes are rejected with a
  clear `abort!` (not cryptic generated-code errors): a nested container (`Vec<Option<_>>`), a cycle
  type missing a root param, a multi-root cycle whose roots aren't a feedback vertex set (a sub-cycle
  avoids every self-referential type), and a non-identity arg on a back-edge to the root
  (was silently *dropped* → miscompile; both `#[recurse]` and `#[recurse(visit)]`). Tests:
  `visitor_recurse_cycle.rs` (root / cross-edge / back-edge /
  self-recursive root), `visitor_recurse_containers.rs` (container + tuple traversal),
  `recurse_generics.rs` (lifetime / type / const / heterogeneous params), `recurse_audit_test.rs` +
  `ui/recurse_*.rs` (the rejections; `ui/recurse_complex_root_param.rs` is the non-identity-arg case;
  `limit = 0` still panics).
- **Multiple independent cycles in one `#[recurse]` module** (shipped): the cycle types are
  partitioned into strongly-connected components (`find_cycle_sccs`, via `safegraph`'s Tarjan SCC);
  each *independent* cycle (a non-trivial SCC, or a singleton with a self-loop) is processed on its own
  by `build_scc` — its own root, depth chain, `XxxTerm`, public aliases, and (under `visit`) its own
  visitor. A field referencing another cycle's type is left as that cycle's public alias (cross-cycle
  fields are *leaves* in the visitor). When the module holds **several** cycles the visitor trait names
  are root-prefixed (`ExprVisit`/`ExprVisitRec`, `TypeVisit`/`TypeVisitRec`; the `visit_*` fns and
  `XxxNode` aliases are already per-type-unique); a **lone** cycle keeps the legacy unprefixed
  `Visit`/`VisitRec` (byte-compatible). This also fixed a latent miscompile where plain `#[recurse]`
  collapsed independent cycles into one `__Rec`. Tests: `recurse_multi_cycle.rs`.
- **Multiple self-referential roots *within one* cycle** (shipped): an SCC where several types each
  self-reference (mutually-recursive `A`/`B` that *both* `Box<Self>`). Each root keeps its **own depth
  dimension** — every cycle type carries one depth param per root (`__RecA`, `__RecB`, …; `__R0`/`__R1`
  in the visitor), a back-edge to root `i` is that root's param, the per-root depth chains are unrolled
  **mutually** (level `k` of each root embeds level `k-1` of *all* roots), and each root gets its own
  `XxxTerm`. The visitor (`build_multiroot_tail` + `generate_multiroot_visitor`) gives each cycle type
  a `visit_*` generic over *all* roots' remaining depth, and `VisitRec` is implemented by every root's
  depth chain (driving its own visit) + every terminator. Soundness guard: the depth only decrements at
  a root, so every cycle must pass through one — the SCC with the roots removed must be acyclic
  (`subgraph_is_cyclic`, via `safegraph`'s `is_cyclic_directed`), else a clear `abort!`
  (`ui/recurse_multiroot_rootless_subcycle.rs`). Roots must all declare the same generic params (extras
  allowed on non-root cycle types). Tests: `recurse_multiroot.rs`. The transform is generalized over
  the root count (`TransformCtx::{rec_params, root_rec, rec_decls}`); a single root reduces to the
  original `__Rec` machinery.

## Known gaps / limitations

- **`visitor!(…)` over a `#[recurse]` cycle is still rejected** — use `#[recurse(visit)]` (above)
  instead. The `visitor!()` path can't bridge to a recurse'd cycle: `#[derive(Ast)]` applies to the
  renamed internal type so `crate::ast::Expr!` finds no macro (`visitor_recurse_gap.rs`), and the
  rewritten fields (`__Rec`/`__StmtRec<…,__Rec>`) don't match `#[subast]` matchkeys. **Drill-in over
  *acyclic* types in a `#[recurse]` module works** (`visitor_recurse_drill.rs`). (Multi-root cycles —
  both *independent* cycles and several self-referential roots *within one* SCC — are now supported;
  see the two "Multiple … cycles" bullets above. The only remaining multi-root rejection is a sub-cycle
  that avoids every self-referential root, which can't terminate: `ui/recurse_multiroot_rootless_subcycle.rs`.)
- **Two visited types sharing a last segment** (`visitor!(a::Foo, b::Foo)`): all generated names key
  off the last segment, so they collide. Now a clear build error (`visitor_diagnostics.rs`); genuine
  coexistence would need full-path-disambiguated names.
- **Nested containers** (`Vec<Option<T>>`) are unsupported on both the `visitor!()` and
  `#[recurse(visit)]` paths (clear build error); wrap the inner part in its own `#[derive(Ast)]` type.

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
closed by requiring canonical (`crate::`/`super::`-rooted) entry paths in `visitor!(...)` (which also
lets the `Leaker` be dropped, per the standing TODO).

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

# `#[recurse]` expansion & how `visitor!()` consumes it (design)

> Reference for splitting `#[recurse]` (type transformer + metadata) from `visitor!()` (the one
> visitor generator). It explains what `#[recurse]` expands to and how a *unified* `visitor!()` builds
> a depth-generic visitor over that output, so one `visitor!(…)` can span both **outer** (acyclic)
> and **inner** (recurse-cycle) types in a single `Visit` trait.

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

# TODOs

- [x] ~~Implement `Repeater` directly on the macro target (drop the leaker struct).~~ Done.
- [x] ~~Inheritance: communicate the base's generic-param union (`@bg`) so `base => New` emits the
      base supertrait with the base's arity (fixed the opaque `E0107`).~~ Done (`visitor_inherit_arity.rs`).
- [x] ~~Hygiene: generated helper params (`__V`/`__T`/…) no longer collide with a visited type's
      params (fresh-name + `mixed_site`).~~ Done (`visitor_hygiene.rs`).
- [x] ~~`$crate`-root `crate::` `#[subast]` paths so a downstream crate can drill through an upstream
      crate's types.~~ Done (`rust/tests/cross_crate_drill.rs`).
- [x] ~~Multi-level inheritance `base => mid => New` (transitive supertrait obligations via `@an`).~~
      Done (`visitor_inherit_multilevel.rs`).
- [x] ~~"Follows nothing" lint.~~ Done — a heuristic warning (nightly-visible) when a type with no
      `#[subast]` has an AST-looking field; `#[subast()]` opts out.
- [x] ~~Visitor over a `#[recurse]` cycle.~~ Done — `#[recurse(visit)]` emits a depth-generic
      visitor (`Visit`/`VisitRec`/`visit_*`/`XxxNode`); single-root cycles, trait-based
      (`visitor_recurse_cycle.rs`). The `visitor!()` path over a recurse cycle remains unsupported
      (see "Known gaps").
- [x] ~~in recurse macro, support the case that one of the cycle type references root type giving
      complex type params (not giving just the cycle type's type params, but giving `Vec<T>`,
      `Option<T>`, ...).~~ Resolved by **rejection**, not support: a back-edge to the root collapses
      to the single depth param `__Rec`, so a non-identity argument (`Expr<Vec<S>>`) is a *non-regular*
      recursion the single-`__Rec` depth machinery cannot express. The argument used to be silently
      dropped (miscompile); `transform_type` now compares a root reference's args against the root's
      own params and `abort!`s with an actionable message (move the differing part into its own
      `#[derive(Ast)]` type, or pass the params unchanged). Fires for both `#[recurse]` and
      `#[recurse(visit)]`. Fixture: `ui/recurse_complex_root_param.rs` (in `recurse_audit_test.rs`).

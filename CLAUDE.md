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
- **`#[recurse(visit)]`** (shipped): a **depth-generic** visitor over a `#[recurse]` *cycle*.
  `#[recurse]` rewrites the cycle's back-edges to the root into the generic `__Rec` param and each
  nesting level into a distinct type, so the visitor is parameterized by the remaining depth: a
  `Visit<S…>` trait with `visit_*<R>` methods (+ free `visit_*` drive fns), and a `VisitRec<S…, V>`
  dispatch trait implemented by the root's depth chain (drives the root visit) and by the terminator
  (no-op) — turning the depth recursion into trait dispatch on the back-edge. `XxxNode` aliases name
  the depth-generic node types. Trait-based only (a closure can't be generic over the depth);
  single-root cycles only. Followed fields traverse through `Vec`/`Option`/`Box` (incl. a `Box`
  *around* an `Option`, via `cont_box`) and **tuples** (each element dispatched). Cycle types may
  carry **lifetime / type / const generic params**, and the types in a cycle may have *different*
  params (heterogeneous): each keeps its own params (threaded into its `__*Rec` node + public alias),
  the `Visit`/`VisitRec` traits are keyed on the root's params, and a type's extra params become
  generics on its `visit_*` method. The one requirement is that every cycle type declare all of the
  **root's** params (so the `__Rec` default `__RootDefault<root params>` is spellable). Unsupported
  shapes are rejected with a clear `abort!` (not cryptic generated-code errors): a nested container
  (`Vec<Option<_>>`), a cycle type missing a root param, and a multi-root cycle. Tests:
  `visitor_recurse_cycle.rs` (root / cross-edge / back-edge /
  self-recursive root), `visitor_recurse_containers.rs` (container + tuple traversal),
  `recurse_generics.rs` (lifetime / type / const / heterogeneous params), `recurse_audit_test.rs` +
  `ui/recurse_*.rs` (the rejections; `limit = 0` still panics).

## Known gaps / limitations

- **`visitor!(…)` over a `#[recurse]` cycle is still rejected** — use `#[recurse(visit)]` (above)
  instead. The `visitor!()` path can't bridge to a recurse'd cycle: `#[derive(Ast)]` applies to the
  renamed internal type so `crate::ast::Expr!` finds no macro (`visitor_recurse_gap.rs`), and the
  rewritten fields (`__Rec`/`__StmtRec<…,__Rec>`) don't match `#[subast]` matchkeys. **Drill-in over
  *acyclic* types in a `#[recurse]` module works** (`visitor_recurse_drill.rs`). Multi-root cycles
  (several self-referential types) get no `#[recurse(visit)]` visitor — back-edges collapse to one
  ambiguous `__Rec`, now a **clear `abort!`** (`ui/recurse_visit_multi_root.rs`) rather than a silent
  no-op.
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

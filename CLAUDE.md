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
  coexist). Also emits a `type_leak::Leaker` marker + one `::syan::visit::Repeater<N>` impl per
  context-dependent field type (portable refs; consumer = drill-in / external metadata users).
- **`visitor!([base =>] T, …)`** invoked **inside** an (otherwise empty) `mod` (function-like; a
  `macro_rules!` shim in `syan` captures `$crate` and forwards the syan path to `__visitor_entry`,
  so no `#[syan(..)]` needed). A metadata ping-pong through `__visitor_build` generates, per visited
  type: a `Visit`/`VisitMut` trait method, a free `visit_*`/`visit_*_mut` traversal fn, and
  **inherent** `visit`/`visit_mut` methods on the type (no trait import at the call site; the
  visitor must be generated in the types' crate). Type args are paths resolvable from inside the
  visitor module (`super::Expr`, `crate::ast::Expr`). Direct and `Box`-wrapped AST fields are
  traversed; other heads (incl. `Vec`/`Option`) are currently leaves — see the drill-in plan.
- **Inputs** (`IntoVisitor`/`IntoVisitorMut` selector design): struct visitors (via `&mut`), single
  closures, and **tuples of closures** (arity 2..=8) running in **one** traversal via a shallow
  `Hook` + single-pass `Driver` + `Chain`.
- **`visit_mut`** full mirror (in-place mutation). **Reduce/append**: override the *parent* node's
  `visit_*_mut` (it owns the `&mut Vec`/`&mut Option`), then descend — see `visitor_reduce.rs`.
- **Inheritance** `visitor!(base => New)` for new→base reference DAGs (base exports a
  `__syan_visited` list macro; the new trait extends it via supertrait).
- **Generics**: the trait is parameterized by the **union** of visited types' generic params
  (`Visit<S, Tokens>`); each type uses its own subset, so mixed arities work (`visitor_generics.rs`).
  Caveat: `.visit()` on a root that doesn't use every union param may need a turbofish.
- **Cross-crate** validated: visited types are named by the full path given to `visitor!(...)`, so a
  downstream crate needs no import (`rust/tests/cross_crate.rs`).

## Known gaps

- **Containers + drill-in** are the open feature — the next plan below.
- **Visitor over `#[recurse]` aliases**: `#[derive(Ast)]` inside `#[recurse]` applies to the
  *renamed* internal type, so its metadata macro is under that name, not the public alias. The `Ast`
  marker still holds for the alias (`ast_recurse.rs`); building a visitor over the alias needs the
  metadata macro reachable via the alias name — future.

---

# Drill-in implementation plan (resolves to: transitive auto-discovered visitor)

## Context

The original "drill-in" goal was to traverse *through* an Ast type the visitor has no `visit_*` for
(the spec's `Expr::Cast(cast) => this.visit_type(&cast.0)`). The deep dive below resolves it into
something simpler and stronger — **auto-discover the reachable AST closure and give every discovered
type a `visit_*` method** (see *Model* and *Resolved decisions*), so there is no separate "drill"
path. The enabling blocker was that "is this field-head an `Ast` type to follow?" can't be tested at
macro-expansion time. The **`#[subast(...)]` allowlist** removes it: a `#[derive(Ast)]` struct/enum
declares its sub-AST types (with resolvable paths) in a type-level `#[subast(...)]`; **a field is
followed iff its (container-peeled) head is listed there** (or is the type's own type — self-recursion
is implicit). **All other field types are ignored** (treated as leaves: tokens, primitives,
`PhantomData`, spans, …). One attribute does both jobs: membership (listed ⇒ followed) *and* pathing
(the entry supplies the resolvable path to that sub-AST's metadata macro). This also gives the
`Repeater` impls (already emitted by `#[derive(Ast)]`) a fallback consumer: portable sub-AST refs.

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

## Model — visit-all-reachable (this is what "drill-in" becomes)

`visitor!(Root, …)` lists **entry type(s)**; `__visitor_build` auto-discovers the closure reachable
through `#[subast]` edges (plus implicit self-recursion) and generates a `visit_*` method for **every
discovered type** (syn-style). Traversal is always `this.visit_<head>(field)` — a *method* call, so
recursion runs through the trait and handles recursive/cyclic ASTs exactly like today's visited-type
traversal. There is **no inline drilling and no cycle detection**: a wrapper like `Cast` (listed in
its parent's `#[subast]`) is simply *also visited* (it gets `visit_cast`, whose body reaches `Type`).
This is more uniform than the spec's invisible drill-through and matches "build a visitor without
enumerating the whole dependency set." Prune a subtree by leaving its head out of `#[subast]`.

## `__visitor_build` (`macro/visitor.rs`)

- **Auto-discovery + incremental emission** (see *Scale*): each ping-pong bounce fetches one type's
  structure, emits *that type's* free `visit_*` / `visit_*_mut` fn immediately, records its
  name/path/own-generics in a carried name-list, queues each followed field's **resolved path** —
  **except `@SELF`** (that's the current type: already recorded, so self-recursion is a method call,
  not a new fetch) — deduped on the path's last-segment **string** (a proc-macro string compare,
  allowed), and drops the structure.
- **Body lowering** per field (peel the container, then visit the inner head):
  - an **ignored** field (`@leaf` — head not in `#[subast]`, not self) ⇒ bind `_`, skip.
  - a **followed** field ⇒ `this.visit_<inner-head>(<access expr>)` — the method name is built in-proc
    from the literal head ident (`to_snake`; never in `macro_rules!`), the access expr applies the
    container lowering, and the enqueue path + enum match scrutinee use the record's **resolved path**
    (the `#[subast]` entry's path, or `@SELF`; no `path_of` / no inference). Every followed head is a
    discovered type with a method, so **no membership test is needed at lowering time**.
- **One-shot items** (final bounce, from the name-list — signatures only, no structures): the
  `Visit`/`VisitMut` traits (one method per discovered type), `Driver`/`Hook`/`Chain`, the
  `IntoVisitor`/`IntoVisitorMut` closure & tuple impls, and the inherent `visit`/`visit_mut`.

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

### Decision 2 — Delegation & model: **proc-macro composes; visit-all-reachable.**
Two hard `macro_rules!` facts settle it: (1) it **cannot compare two idents** for equality (no
`$a == $b`; reusing a metavar name in a matcher is an error) → it can't test visited-set membership;
(2) it **cannot concat / snake-case idents** (no `format_ident!`) → it can't build `visit_stmt` from
`Stmt`. So both membership *and* body generation (method names, match arms) must live in the
proc-macro. "Delegate via `macro_rules!`" is therefore the metadata macros *supplying each type's
structure* (the ping-pong fetch *is* the delegation); `__visitor_build` composes. Because the
proc-macro composes, **visit-all-reachable** is chosen over selective drilling — it eliminates
inline-drill recursion and cycle handling (recursion is via method calls). `#[subast]` is the single
**allowlist**: it declares membership (listed ⇒ followed; everything else ignored) *and* supplies the
resolvable path. "Reachable" means reachable through `#[subast]` edges (+ self); every such type gets
a method. (Membership lives in `#[subast]`, not in a `macro_rules!` test — consistent with fact (1).)

### Scale — incremental emission (avoids O(N²)).
A full AST closure can be ~100 types; accumulating every fetched structure in the ping-pong state and
re-emitting it each bounce is O(N²) tokens. Instead, structures are **used-and-dropped per bounce**
and each type's `visit_*` fn is **emitted as it is fetched**; only a small **name-list**
(ident + path + own-generics) accumulates (Rust allows items in any order, so a body may reference
the trait that is emitted last). The traits / `Driver` / `IntoVisitor` / inherent items are emitted
once at the end from that name-list. The trait's **union generics = the root type's generic params**
(known from the first fetched type, before any body is emitted, so closures keep working); a sub-type
that introduces a new generic param name is an error.

### Pathing — `#[subast]` supplies every followed path.
Every followed field's resolvable path is its matching `#[subast]` entry's path (or `@SELF` for
self-recursion — substituted with the path the type was fetched by, used only for the match
scrutinee, never enqueued as a discovery edge); unlisted field types are ignored, so there is no path
to infer for them. The
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
  and `#[subast(super::Type)]` on `Cast`; `visitor!(super::Expr)` ⇒ traversal reaches `Type` through
  `Cast` (auto-generated `visit_cast` → `visit_type`); a closure `|t: &Type<()>| …` fires once; `Cast`
  is also visitable (`visit_cast` exists — the visit-all consequence).
- Allowlist: a field whose head is listed (`#[subast(crate::ast::Stmt)]`, field `Box<Stmt<S>>`) is
  followed (`visit_stmt`); an **unlisted** field type is silently ignored (bound `_`, not traversed);
  an imported/aliased field (`use other::Stmt; … s: Stmt` with `#[subast(other::Stmt)]`) reaches
  `visit_stmt` (the gap `core/tests/visitor_local_types.rs` documents); a same-last-segment collision
  (`#[subast(a::Foo, b::Foo)]`) fails at the derive with a located error.
- Self-recursion: `Expr { Bin(Box<Expr<…>>, …) }` with `Expr` *not* in its own `#[subast]` still
  recurses via `@SELF`.
- Containers: a type with `Vec<Stmt>` / `Option<Expr>` / `Box<Expr>` fields (with `Stmt`/`Expr`
  listed) descends into each element; reduce/append via overriding the parent's `visit_*_mut`.
- Auto-discovery scale: `visitor!(super::Root)` over a multi-type graph ⇒ every type reachable
  through `#[subast]` edges gets a method and is traversed.

# TODOs

- [ ] Do not define leaker type in output of `#[derive(Ast)]`, instead implement Repeater directly for macro target.

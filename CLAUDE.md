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
something simpler and stronger — **auto-discover the whole reachable AST closure and give every type
a `visit_*` method** (see *Model* and *Resolved decisions*), so there is no separate "drill" path.
The enabling blocker was that "is this field-head an `Ast` type?" can't be tested at macro-expansion
time. The **`#[no_ast]` convention** removes it: in a `#[derive(Ast)]`
struct/enum, **every field's (head) type is itself `#[derive(Ast)]`** — so it has a metadata macro
reachable by name — **unless the field is `#[no_ast]`** (leaf: `Token![..]`, `Integer`, primitives,
`PhantomData`, spans, …). The generator never tests Ast-ness: non-`#[no_ast]` ⇒ Ast by contract; a
forgotten `#[no_ast]` surfaces as a clear "cannot find macro `Foo`" error. This also gives the
`Repeater` impls (already emitted by `#[derive(Ast)]`) their consumer: portable sub-AST references.

## `#[derive(Ast)]` (`macro/ast.rs`)

- Add `no_ast` to the helper attributes: `#[proc_macro_derive(Ast, attributes(syan, no_ast))]`;
  read `#[no_ast]` per field (strip it from the embedded def, like other helper attrs).
- The metadata macro emits, per variant → per field, a record carrying: the **accessor** (tuple
  index or named ident), the **container** (`direct` / `box` / `vec` / `option` / … — see Decision 1),
  and for non-`#[no_ast]` fields the inner **head ident** plus a **portable macro path** to that
  field type's metadata macro (so it is callable from the visitor's context). `#[no_ast]` fields
  carry just `@no_ast`. (Equivalently, the derive could keep embedding the cleaned def and let
  `__visitor_build` re-derive these per field — but emitting explicit records lets the derive own
  container/`#[no_ast]` classification.)
- Keep the `Leaker` + `Repeater<N>` impls; use the `Referrer` to make the per-field type/macro path
  portable (the consumer that justifies the type-leak work).

## Model — visit-all-reachable (this is what "drill-in" becomes)

`visitor!(Root, …)` lists **entry type(s)**; `__visitor_build` auto-discovers the reachable closure
by following non-`#[no_ast]` fields and generates a `visit_*` method for **every discovered type**
(syn-style). Traversal is always `this.visit_<head>(field)` — a *method* call, so recursion runs
through the trait and handles recursive/cyclic ASTs exactly like today's visited-type traversal.
There is **no inline drilling and no cycle detection**: a wrapper like `Cast` is simply *also
visited* (it gets `visit_cast`, whose body reaches `Type`). This is more uniform than the spec's
invisible drill-through and matches "build a visitor without enumerating the whole dependency set."
Prune a subtree by `#[no_ast]`-ing the field that leads into it.

## `__visitor_build` (`macro/visitor.rs`)

- **Auto-discovery + incremental emission** (see *Scale*): each ping-pong bounce fetches one type's
  structure, emits *that type's* free `visit_*` / `visit_*_mut` fn immediately, records its
  name/path/own-generics in a carried name-list, queues its undiscovered non-`#[no_ast]`
  field-head types, and drops the structure.
- **Body lowering** per field (peel the container, then visit the inner head):
  - `#[no_ast]` ⇒ bind `_`, skip.
  - otherwise ⇒ `this.visit_<inner-head>(<access expr>)`, where the access expr applies the
    container lowering. Every inner head is a discovered type with a method, so **no membership
    test is needed**.
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
head. The inner head is Ast by the `#[no_ast]` convention (a container of leaves, e.g. `Vec<Token>`,
is `#[no_ast]`). Reduce/append is unchanged: override the parent's `visit_*_mut`, which owns the
`&mut Vec` / `&mut Option`.

### Decision 2 — Delegation & model: **proc-macro composes; visit-all-reachable.**
Two hard `macro_rules!` facts settle it: (1) it **cannot compare two idents** for equality (no
`$a == $b`; reusing a metavar name in a matcher is an error) → it can't test visited-set membership;
(2) it **cannot concat / snake-case idents** (no `format_ident!`) → it can't build `visit_stmt` from
`Stmt`. So both membership *and* body generation (method names, match arms) must live in the
proc-macro. "Delegate via `macro_rules!`" is therefore the metadata macros *supplying each type's
structure* (the ping-pong fetch *is* the delegation); `__visitor_build` composes. Because the
proc-macro composes, **visit-all-reachable** is chosen over selective drilling — it eliminates
inline-drill recursion and cycle handling (recursion is via method calls).

### Scale — incremental emission (avoids O(N²)).
A full AST closure can be ~100 types; accumulating every fetched structure in the ping-pong state and
re-emitting it each bounce is O(N²) tokens. Instead, structures are **used-and-dropped per bounce**
and each type's `visit_*` fn is **emitted as it is fetched**; only a small **name-list**
(ident + path + own-generics) accumulates (Rust allows items in any order, so a body may reference
the trait that is emitted last). The traits / `Driver` / `IntoVisitor` / inherent items are emitted
once at the end from that name-list. The trait's **union generics = the root type's generic params**
(known from the first fetched type, before any body is emitted, so closures keep working); a sub-type
that introduces a new generic param name is an error.

### Pathing.
A discovered sub-type's macro/type path = the field type's path as written, made absolute by
prefixing the *defining* type's module when relative (`Stmt` in `Expr @ super::ast` ⇒
`super::ast::Stmt`); explicit paths (`crate::other::Stmt`) are used as-is. Imported-relative paths
(`use other::Stmt; … Stmt`) are the residual gap that the `Leaker`/`Repeater` + `$crate` machinery
closes (canonical identity / portable path).

## Tests (`core/tests`)

- Spec graph: `Type`, `Cast(Type)`, `Expr { Cast(Cast) }`; `visitor!(super::Expr)` (only the entry
  listed) ⇒ traversal reaches `Type` through `Cast` (via the auto-generated `visit_cast` →
  `visit_type`); a closure `|t: &Type<()>| …` fires once; `Cast` is also visitable (`visit_cast`
  exists — the visit-all consequence).
- Containers: a type with `Vec<Stmt>` / `Option<Expr>` / `Box<Expr>` fields descends into each
  element; reduce/append via overriding the parent's `visit_*_mut`.
- `#[no_ast]`: a leaf/container-of-leaf field is skipped; a non-Ast field without `#[no_ast]` gives a
  clear "cannot find macro" error.
- Auto-discovery scale: `visitor!(super::Root)` over a multi-type graph ⇒ every reachable non-leaf
  type gets a method and is traversed.

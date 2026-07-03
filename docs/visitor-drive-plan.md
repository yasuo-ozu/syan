# Plan: unify the container descent onto `SeqView`/`OptView`

## Status

- **IMPLEMENTED** (branch `visitor-descent-views`). `SeqView`/`OptView` are bare-element
  (`impl<T> SeqView<T> for Vec<T>`/`VecDeque`/`Punctuated`; `impl<T> OptView<T> for Option<T>`);
  `Box<T>`/`Attempt<T>` are single-slot `OptView<T>` (descent-real, `take` unreachable); `Wrap` and the
  `Box` forwarders are gone. The macro (`peel` → name-free `View`/`Raw` walk; `lower_field` → per-level
  `field.view_iter_mut()` **method-call** descent, head via `#[subast]` match) matches **no container
  type name** (only `#[subast]` keys + syn `Array`/`Slice`/`Tuple` shapes). Descent is automatic and
  behavior-preserving; nested containers, `Vec<Box<_>>`, and container-of-tuple all descend for free.
  Whole workspace green, clippy/doc clean.
  - **Edit-view casualties** (bare-element cost): `#[seq]`/`#[opt]` edit views now require a **bare
    single container** (`Vec<Head>` / `Option<Head>`). Box-wrapped (`Vec<Box>`, `Box<Vec>`), nested
    (`Vec<Option>`, `Vec<Vec>`), and self-recursive `Option<Box<_>>` are no longer edit targets — they
    still *descend*, just can't be structurally edited. Tests migrated: `rec`/`rec_cross` → bare
    `Vec<Head>`; `boxed_container` → bare `Punctuated`/`Option`; `rec_opt`/`nested` → descent tests.
  - **Residual left as-is:** a `#[seq]` on a single wrapper (`Box<Head>`) or a kind-mismatch (`#[seq]`
    on an `Option`) is now caught by the `SeqView<Head>`/`OptView<Head>` trait bound (E0277) rather
    than a macro abort — the marker + bound self-validate. (Conversely `#[opt] Box<Head>` *satisfies*
    the bound — `Box` is a single-slot `OptView` — so it is a valid always-full edit view: `set`/
    `get_mut` work, `take`/`clear` would panic; `visitor_edit.rs::boxed_opt`.)
  - **Follow-up refactor (done):** `ContLayer` collapsed to `Vec<LayerKind>`; macro helpers deduped
    (`marker_word`/`base_tokens`/`strip_param_defaults`), dead code + induced comments removed; every
    `abort!` in `visitor.rs` points at the offending field type / head / path (not `Span::call_site()`).
    The visitor **test suite was consolidated 71 → 35 files** (thematic groups merged into
    `visitor_recurse{,_shapes,_mixed}.rs`, `visitor_drill.rs`, `visitor_inherit.rs`, `visitor_core.rs`,
    `visitor_containers.rs`, `visitor_audits.rs`, `recurse_{core,traits,visitor_cycles}.rs`), with
    redundant tests dropped and comments cut. `gen_side()`'s emission was reduced: the free
    `visit_*_seq`/`_opt` fns were removed (their trivial descent is inlined into the trait-method
    defaults) and the emitted `#[doc]` strings trimmed to one-liners. The oversized sources were split
    into submodules (`macro/visitor/{entry,build_input,discover,lower,params,side}.rs`,
    `macro/recurse/{graph,transform,convert,names,emit,build,items}.rs`,
    `macro/attribute/{find,substruct,adt}.rs`, `core/src/source/proc_macro2/literal/*.rs`) — every
    source file is now <1000 lines.
  - `CLAUDE.md`'s "Containers" / "Structural edits" sections are refreshed to this model.


- **Goal.** Stop **hard-coding container type names** in the macro. `peel` used to recognize containers
  by matching `"Vec"`/`"Option"`/`"Box"`/… literally; that name list was the target.
- **Chosen direction.** The head comes from the **`#[subast]` search** (walk the field type to the
  matching sub-type); container descent reuses the shipped **`SeqView`/`OptView`** (made **bare-element**)
  via a per-layer `view_iter[_mut]()` **method call** — **method resolution** picks the view, so no
  container name is matched. Descent is automatic; `#[seq]`/`#[opt]` stays an **additive edit view**
  (not needed for descent). No new trait family, no per-type leaf impl.
- **Scope decisions.** Descent **recurses per layer**, so nested containers, `Vec<Box<T>>`, and
  container-of-tuple all descend; arrays/slices/tuples are handled structurally; `#[seq]`/`#[opt]` edit
  views require a bare single container; the `type`-alias blind spot stays.
- **Result.** `peel` loses its container-name arms + the `ContLayer` chain / box-counting (head
  extraction + the followed/drill/leaf decision stay); a user container descends for free by
  `impl SeqView`/`OptView` — no annotation needed for descent.
- **Set aside:** a type-system-recursive `AstDrive`/`AstDriveMut` trait family that would *also* have
  fixed the alias blind spot. Proven viable (3 rustc spikes + a cross-crate prototype) but not chosen
  — it adds a second trait family + a per-type leaf `impl` on every AST type. Kept as **Alternatives
  considered** below.

## Motivation

Before this change the *same* container shapes were traversed by **two** code paths:

- `#[seq]`/`#[opt]` field → `SeqView`/`OptView` (structural edit views, `core::visit`, shipped).
- unmarked field → `fold_containers` (`macro/util.rs`) emits raw `.iter_mut()` / `if let Some(..)`.

The **real goal is to stop hard-coding container type names.** `peel` recognized containers by
matching literal segment names — `"Box" | "Attempt"`, `"Vec" | "VecDeque" | "Punctuated"`,
`"Option"`, plus slice/array — which was the bulk of its complexity and was brittle (a user-defined
container was invisible to it). Two operations replace that identification, **neither hard-coding a
container name**, and both simpler than the old `peel`:

1. **Get the head type** — walk the field's `syn::Type` for the sub-type whose head path-segment name
   matches a `#[subast]` key (or the type's own ident), **verbatim with its generic args**
   (`Stmt<S, u8>`). Names the `visit_*` target *and* its exact type.
2. **Everything wrapping the head is a container**, traversed through a **trait recognized by impl,
   not by name** — `Vec`/`Option`/`Box`/`Punctuated`/user containers alike.

The `type`-alias blind spot is *not* fixed — operation 1 matches a literal segment name, so
`type Exprs = Vec<…>` stays opaque (acceptable).

## Design

The macro finds the **head** by walking the field's `syn::Type` to the `#[subast]`-matching (or
self-ident) sub-type. Every wrapper *around* the head is a descent **layer**, classified without any
container name, and descent **recurses per layer** (so it needs no annotation):

- **`View` layer** — any path that is neither the head nor `Array`/`Slice`/`Tuple` (i.e. `Vec`,
  `Option`, `Box`, `VecDeque`, `Punctuated`, `Attempt`, user wrappers). Descended by a
  `view_iter[_mut]()` **method call**; **method resolution** picks `SeqView` or `OptView` — a container
  type impls exactly one — so **no container name is matched and no UFCS is needed**. `Vec<T>` etc. are
  `SeqView<T>`; `Box<T>`/`Attempt<T>` are single-slot `OptView<T>` (one node).
- **`Raw` layer** — `[T]` / `[T; N]` (a syntax shape, not a name). Descended by the slice
  `iter[_mut]()` (arrays/slices have no view impl).
- **Tuple** — `Type::Tuple`, destructured; each element lowered.
- **Head dispatch** — the innermost call is `self.visit_<head>_mut(x)`.

Because each layer recurses, **nested containers, `Vec<Box<T>>`, and container-of-tuple all descend
for free** — e.g. `Vec<Box<Head>>` is a `SeqView` layer yielding `Box<Head>`, then an `OptView` layer
yielding `Head`. Descent is automatic; a field is a leaf only when no `#[subast]` head is reachable.

`#[seq]`/`#[opt]` is an **additive edit view** on top: a marked field *also* gets a
`visit_<head>_seq/_opt(&mut impl SeqView/OptView<Head>)` the visitor can override to restructure the
container in place. It requires a **bare single container** (`Vec<Head>` / `Option<Head>`), so the
`SeqView<Head>` / `OptView<Head>` bound self-validates — a box-wrapped / nested / kind-mismatched marked
field is a clean compile error (pointing at the field type). Such fields still *descend*; they just
aren't edit targets.

### No hard-coded container names — how each concern is met

| Concern | Resolved by (no name-match) |
|---|---|
| Which node to visit (head) | `#[subast]` search — walk the type to the matching sub-type, verbatim with args |
| `View` vs `Raw` layer | `syn::Type` shape: a `Path` (not head/tuple) is `View`, `Array`/`Slice` is `Raw` |
| Seq vs Opt view (descent) | **method resolution** — a `View` type impls exactly one of `SeqView`/`OptView` |
| nested / boxed / wrapper layers | per-layer recursion (`Box`/`Attempt` are single-slot `OptView`) |
| tuples | structural `Type::Tuple` destructure |
| user containers | `impl SeqView`/`OptView` — descended for free, no annotation |

## Tradeoffs (as implemented)

- **Descent is automatic and unchanged in reach.** Any container of a followed head descends —
  unannotated — via per-level `view_iter_mut` (method resolution picks `SeqView`/`OptView`). Nested
  containers (`Vec<Option<T>>`), boxed elements (`Vec<Box<T>>`), and container-of-tuple all descend
  for free (each wrapper is one `View` level; `Box`/`Attempt` are single-slot `OptView`). `#[seq]`/
  `#[opt]` are **not** required for descent — they are purely the additive structural-edit views.
- **Edit views require a bare single container** — `#[seq] Vec<Head>` / `#[opt] Option<Head>`. A
  box-wrapped (`Vec<Box>`, `Box<Vec>`), nested (`Vec<Option>`, `Vec<Vec>`), or self-recursive
  `Option<Box<_>>` field still *descends* but cannot be a `#[seq]`/`#[opt]` edit target; the
  `SeqView<Head>`/`OptView<Head>` bound on the generated method self-validates (a mismatch is a clean
  compile error, no name-matching).
- **Alias blind spot stays** — the head is found by matching a literal `#[subast]` segment name, so a
  `type` alias for a container/head is opaque.
- **Net:** **zero hard-coded container names** — head via `#[subast]` search, view via per-layer
  `view_iter[_mut]()` method resolution (`Box`/`Attempt` are single-slot `OptView`), arrays/tuples
  structural, user containers free. `peel`'s name arms + `ContLayer` chain + box-counting are gone.

## The `SeqView`/`OptView` impls (`core::visit`)

`SeqView`/`OptView` are the single container vocabulary — descent uses their `view_iter[_mut]`, and a
`#[seq]`/`#[opt]` edit view uses their structural half (`insert`/`remove`/`push`/`set`/`take`). Two
impl shapes back everything, both **bare-element** and name-free (`Wrap` and the container forwarders
of the intermediate design were removed):

- **Sequence containers** — `impl<T> SeqView<T> for Vec<T>` (and `VecDeque`/`Punctuated`). The element
  *is* the head `T`, so `Vec<T>: SeqView<T>` is the **only** impl — no `SeqView<T>`/`SeqView<Box<T>>`
  ambiguity, hence plain `view_iter_mut()` method calls (no UFCS).
- **Single-slot views** — `impl<T> OptView<T> for Option<T>`, and for the transparent wrappers
  `impl<T> OptView<T> for Box<T>` / `for Attempt<T>` (always one node: `is_some`=true, `get`/`get_mut`
  via deref, `set` replaces, `take` unreachable — a single wrapper is descent-only, never an edit
  target).

Everything else composes by **per-layer recursion**: `Vec<Box<Head>>` is a `SeqView<Box<Head>>` layer
whose elements are then each an `OptView<Head>` single-slot view; `Box<Vec<Head>>` is an
`OptView<Vec<Head>>` layer over a `SeqView<Head>` layer. A user container works by implementing
`SeqView`/`OptView` — no forwarder or `Wrap` needed.

## `#[seq]`/`#[opt]` tuple rule

A `#[seq]`/`#[opt]` container's element must be a **single bare head** (`Vec<Head>`, not a container
of wrapped things). So a **tuple element is rejected at any arity** — `#[seq] Vec<(A, B)>` *and*
`#[seq] Vec<(T,)>` are clean compile errors at the field: a tuple isn't a bare head, and (elements
being bare) there is no `Wrap` on elements to make `(T,)` transparent.

A **bare tuple *field*** `(A, B)` (not inside a container) is unaffected — it is destructured
structurally and each element lowered, as usual.

(This supersedes the earlier `Wrap<(T,)>` one-item-tuple idea: with bare elements there is no element
adapter, so `(T,)`-as-element is neither needed nor possible. `(T,)` only ever appears as a standalone
tuple field, handled by destructuring.)

## As built

*(The intermediate plan explored an annotation-*required* descent with `Box`/`Bracketed` forwarders;
the shipped design is simpler — descent is automatic and per-level, `#[seq]`/`#[opt]` stay edit-only.)*

1. **`core::visit`:** `SeqView`/`OptView` made **bare-element** (`impl<T> SeqView<T> for Vec<T>` /
   `VecDeque` / `Punctuated`; `impl<T> OptView<T> for Option<T>`). `Box<T>`/`Attempt<T>` are
   **single-slot `OptView<T>`** (`is_some`=true, `get`/`get_mut` via deref, `set` replaces, `take`
   unreachable). `Wrap` and the `Box`/`Attempt` **forwarders were removed** — a boxed/wrapped container
   descends per level instead (`Box<Vec<T>>` = an `OptView` layer over a `SeqView` layer).
2. **`peel` (`macro/util.rs`) is name-free:** walk the `syn::Type`; the sub-type whose head segment is a
   `#[subast]` key (or self ident) is the head; any other path is a `View` layer, arrays/slices are
   `Raw` layers, a followed tuple is `Head::Tuple`. No container name is matched. (`ContLayer` was later
   collapsed to `Vec<LayerKind>`.)
3. **Descent (`Lower::lower_field`) is automatic:** for a followed head, emit one `view_iter[_mut]()`
   **method call** per `View` layer (method resolution picks `SeqView`/`OptView`, brought into scope
   unnamed), a slice `iter[_mut]()` per `Raw` layer, and destructure a tuple — recursing to the head.
   **No annotation is needed to descend.**
4. **Edit views:** a `#[seq]`/`#[opt]` field additionally dispatches `this.visit_<head>_seq/_opt(field)`;
   the macro requires a single bare `View` container, else a clean `abort!` pointing at the field type.
5. **Tests:** all descent behavior preserved (nested / `Vec<Box>` / container-of-tuple still descend);
   only edit tests using box-wrapped/nested containers moved to bare forms. The suite was later
   consolidated 71 → 35 files (see Status).

No new trait, no `#[derive(Ast)]` change, no per-type leaf impl; `#[seq]`/`#[opt]` remain **optional**
(edit views only).

## Alternatives considered (rejected): recursive `AstDrive`/`AstDriveMut`

The more ambitious design — a compositional trait resolved by the **type system**, which *would*
have deleted `peel`'s container decomposition and fixed the alias bug. Set aside because it adds a
second trait family plus a per-type leaf `impl` on every AST type; the view-reuse plan above gets the
"one vehicle" win without that. Recorded here because the analysis is the reason the trade was made.

```rust
pub trait AstDrive<T>    { fn drive(&self, f: &mut dyn FnMut(&T)); }
pub trait AstDriveMut<T> { fn drive_mut(&mut self, f: &mut dyn FnMut(&mut T)); }
// recursive container impls in core: Vec<W>, Option<W>, Box<W>, [W], [W; N], Punctuated<W,P>, &W (no-op mut)
// CONCRETE per-type leaf, emitted by #[derive(Ast)] in the defining crate:
impl<S> AstDriveMut<Expr<S>> for Expr<S> { fn drive_mut(&mut self, f) { f(self) } }
```

The descent would collapse to one call: `field.drive_mut(&mut |x: &mut Expr<S>| self.visit_expr_mut(x))`,
composing arbitrary nesting (`Vec<Option<Box<T>>>`), slices/arrays, and **aliases** (the compiler sees
`Vec<Box<Leaf>>: AstDriveMut<Leaf>` through a `type` alias) with no layer chain.

Why it was viable but not chosen:

- **Coherence crux.** The leaf terminator **cannot** be a blanket `impl<T> AstDriveMut<T> for T`: it
  conflicts with the recursive `Vec` impl at the fixpoint `T = Vec<W>` (`E0119: conflicting
  implementations of AstDriveMut<Vec<_>> for Vec<_>`). (Contrast `Wrap<T> for T` + `Wrap<T> for
  Box<T>`, which is fine because `Box` shifts `Self` one constructor off the trait param, so overlap
  needs an infinite type.) Leaves must therefore be **concrete per-type** impls — the extra
  per-AST-type `impl` the chosen plan avoids.
- **Tuples still don't fit.** A single-leaf-param `AstDriveMut<T>` is monomorphic in `T`; a
  heterogeneous `(Cast, Type)` needs two leaf types. The visitor-threaded variant that would fit
  tuples hits the cross-crate orphan rule. So tuples would stay macro-destructured anyway.
- **Cross-crate: clean** (the one real risk). A three-crate prototype (`/tmp/drive_proto`) had a
  downstream crate drive an upstream type's `Vec<Option<Box<Leaf>>>` (and an alias over it) with **no
  trait impl for any upstream type** — only the upstream-derived concrete leaf + `core` container
  impls; leaves coexist with no coherence conflict; `&W` is a mut-side no-op; the blanket-leaf E0119
  was reproduced as a negative control. 6/6 tests passed. So the approach *works*; it was a
  cost/benefit call, not a feasibility one.

## Open questions

- *(Resolved — descends for free)* **Nested containers** (`Vec<Option<T>>`, `Vec<Vec<T>>`): each
  wrapper is one `View`/`Raw` layer, so per-level descent reaches every node with no special handling.
  They are not `#[seq]`/`#[opt]` *edit* targets (edit needs a bare single container), but they descend.
- *(Resolved — descends automatically)* An **unannotated** `Vec`/`Option`/custom container of a
  followed head **descends** (per-level `view_iter`); it is *not* a leaf. `#[seq]`/`#[opt]` only add the
  edit views. (Supersedes the earlier "unannotated = leaf" idea from the abandoned
  annotation-required-descent design.)
- *(Resolved)* **Trait shape:** reuse `SeqView`/`OptView` made **bare-element**; descend by per-level
  `view_iter[_mut]()` method resolution — no new trait, no per-type leaf impls, and no hard-coded
  container names.

# Plan (v2): structural edits in `visitor!` via a **container view passed as an argument**

## Status

- **v1 (replaced):** `fn visit_t_edit(&mut self, &mut T) -> Edit<T>` — the node returned an
  `Edit` (`Keep`/`Replace`/`Remove`) and the parent applied it. Removed (`Edit`, `SeqEdit`,
  `visit_opt_edit`, `visit_fixed_edit`, `macro/visitor.rs::edit_apply`). **v2 below replaced it.**
- **v2 (IMPLEMENTED, Design B):** the edit is **not a return value**. The node's edit method
  receives a **collection-like view of its own slot in the parent** as an argument, and mutates the parent
  **in place** through it (no cloning of existing nodes). The view is **split by container kind** —
  `SeqView<T>` (Vec-like, unbounded) and `OptView<T>` (Option-like, ≤1) — each a dedicated interface. An
  edit method exists **only for an AST type that is actually held inside another AST in Vec-like or
  Option-like form** (discovered through `visitor!(..)` membership *and* drill-in).

## Requirements (verbatim intent)

1. The edit method does **not return** `Edit`; the **edit view is an argument**.
2. The view is **separated** for **Option-like (≤1)** and **Vec-like (unbounded)** — dedicated interfaces.
3. Edit methods are emitted **only for AST types held inside another AST** (listed or drilled) in
   Option-like or Vec-like form.
4. **No clone cost** — edit the actual AST value in place (the view exposes `&mut T`; existing nodes are
   moved, never cloned).
5. (Standing, from the prior turn) **do not break `visit_*` / `visit_*_mut`** — those stay exactly as they
   are; the views are additive.

## Public API (`core::visit`)

`SeqView` / `OptView` expose the **same edit method set** (below) under **two candidate designs, both
fully specified** in "Backend abstraction — two designs" further down:

- **Design A:** `SeqView<T>` / `OptView<T>` are **wrapper structs** (`&mut dyn _Backend<T>`); the methods
  below are *inherent* methods; the descent wraps the field (`SeqView::new(&mut field)`).
- **Design B:** `SeqView<T>` / `OptView<T>` are **public traits implemented on the containers themselves**
  (`Vec`/`Punctuated`/…, no wrapper); the methods below are *trait* methods (a small required core +
  provided methods); the descent passes the field directly (`&mut field`).

Either way the method surface a visitor calls is identical — the list below (shown as Design A's inherent
`impl`; under B read each `pub fn` as a trait method on `SeqView<T>` for `Self`):

```rust
// ── Vec-like (unbounded) ────────────────────────────────────────────────────────────────────────
pub struct SeqView<'a, T> { backend: &'a mut dyn SeqBackend<T> }
impl<'a, T> SeqView<'a, T> {
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn get(&self, i: usize) -> Option<&T>;
    pub fn get_mut(&mut self, i: usize) -> Option<&mut T>;     // edit one node in place — no clone
    pub fn for_each_mut(&mut self, f: impl FnMut(&mut T));      // in-place over every node
    pub fn push(&mut self, x: T);
    pub fn insert(&mut self, i: usize, x: T);
    pub fn remove(&mut self, i: usize) -> T;                    // moved out, not cloned
    pub fn retain_mut(&mut self, f: impl FnMut(&mut T) -> bool);
    pub fn edit_each(&mut self, f: impl FnMut(&mut SeqCursor<'_, T>)); // structural per-element walk
}
// A transient per-element cursor handed out by `edit_each` (index-managed by core::visit):
pub struct SeqCursor<'a, T> { /* &mut backend + current index + recorded action */ }
impl<'a, T> SeqCursor<'a, T> {
    pub fn get(&self) -> &T;
    pub fn get_mut(&mut self) -> &mut T;        // edit in place — no clone
    pub fn replace(&mut self, x: T);            // swap this node
    pub fn remove(&mut self);                   // drop this node (don't advance)
    pub fn insert_before(&mut self, x: T);
    pub fn insert_after(&mut self, x: T);
}

// ── Option-like (≤1) ────────────────────────────────────────────────────────────────────────────
pub struct OptView<'a, T> { backend: &'a mut dyn OptBackend<T> }
impl<'a, T> OptView<'a, T> {
    pub fn is_some(&self) -> bool;
    pub fn get(&self) -> Option<&T>;
    pub fn get_mut(&mut self) -> Option<&mut T>;   // edit the node in place — no clone
    pub fn set(&mut self, x: T);                    // fill / replace
    pub fn take(&mut self) -> Option<T>;            // remove → None (moved out)
    pub fn clear(&mut self);
}
```

The element access is provided by a per-container trait — **hidden `SeqBackend<T>`/`OptBackend<T>` under
Design A** (the wrapper erases it; `SeqBackend` adds `for_each_mut(&mut self, &mut dyn FnMut(&mut T))` so
the default descent is one call with a monomorphic inner loop), or the **public `SeqView<T>`/`OptView<T>`
traits themselves under Design B** (implemented directly on the containers). Either way the **same impl
table** applies:

| backend impl | notes |
|---|---|
| `Vec<T>`, `VecDeque<T>` | direct |
| `Punctuated<T, P>` | `get` via `iter().nth`, `get_mut`/`remove` direct; `insert`/`push` synthesize the separator → requires `P: Default` |
| `Vec<Box<T>>` / `VecDeque<Box<T>>` / `Punctuated<Box<T>, P>` | **box-transparent**: `get_mut` derefs the box, `remove` unwraps, `insert` re-boxes — so `SeqView<T>` (not `SeqView<Box<T>>`); no clone. Same for the `Attempt<T>` element forms. |
| `Box<T>` where `T: SeqView`/`OptView` | **transparent forwarder**: `impl<E, T: SeqView<E>> SeqView<E> for Box<T>` (and `OptView` mirror) delegates through the box, so a `Box`-around-a-container field (`#[seq] Box<Vec<T>>` / `#[opt] Box<Option<T>>`) views the inner collection/Option. A bare `Box<Leaf>` (leaf not a view) is *not* a view. |
| `Option<T>`, `Option<Box<T>>`, `Option<Attempt<T>>` | box-transparent like above (`OptView`) |

Arrays `[T; N]` are **fixed-size** → not a Vec-like editable form; an array-of-`T` field stays an ordinary
fixed descent (`visit_t_mut` per element, no structural edit). Same for `Box<T>` / a direct field.

## Backend abstraction — two designs (final pick pending)

Both keep the **same user-facing methods**; they differ only in how the view is parameterized and in the
trait-method signature, with a clear performance/ergonomics trade-off.

### Design A — `dyn`-erased view (`SeqView<T>`)

```rust
pub struct SeqView<'a, T> { backend: &'a mut dyn SeqBackend<T> }
impl<'a, T> SeqView<'a, T> {
    pub fn new(b: &'a mut (impl SeqBackend<T> + 'a)) -> Self { Self { backend: b } }
    // … the method list above …
}
// trait method (clean, monomorphic in T):
fn visit_t_seq(&mut self, v: &mut SeqView<T>) { … }
// override the user writes (clean):
fn visit_stmt_seq(&mut self, v: &mut SeqView<Stmt<S>>) { … }
```

- **Signatures: clean.** The view is `SeqView<T>`; an override names exactly `&mut SeqView<Stmt<S>>`.
- **Descent cost:** the *default* `visit_t_seq` is `v.for_each_mut(|e| self.visit_t_mut(e))`. With
  `SeqBackend::for_each_mut(&mut self, &mut dyn FnMut(&mut T))`, that is **one** virtual call into the
  concrete backend, whose inner `for e in self.iter_mut()` loop is monomorphic; the per-element cost is an
  **indirect `FnMut` call** (not a vtable lookup). So: 1 vtable call + N indirect calls per Vec descent —
  a small, bounded overhead vs. today's fully-static loop. Edit ops (`remove`/`insert`/…) are one vtable
  call each (rare).
- **Object-safe** `VisitMut` (no generic methods) — preserved (in case `dyn VisitMut` is ever wanted; not
  currently used).

### Design B — `SeqView<T>` is a **public trait implemented on the actual containers** (no wrapper)

There is **no wrapper struct and no separate hidden backend**: `SeqView<T>` / `OptView<T>` *are* the public
view traits, implemented directly on `Vec<T>`, `VecDeque<T>`, `Punctuated<T, P>`, `Option<T>`, … (and the
box-wrapped forms). The generated method is generic over the implementor, and the descent passes the field
**directly** — zero wrapping.

```rust
pub trait SeqView<T> {                       // element type is a TYPE PARAM (not associated) — see box note
    // required core (object-safe):
    fn len(&self) -> usize;
    fn get(&self, i: usize) -> Option<&T>;
    fn get_mut(&mut self, i: usize) -> Option<&mut T>;
    fn insert(&mut self, i: usize, x: T);
    fn remove(&mut self, i: usize) -> T;
    // provided (built on the core; `Self: Sized` so the trait stays object-safe):
    fn is_empty(&self) -> bool { self.len() == 0 }
    fn push(&mut self, x: T) where Self: Sized { let n = self.len(); self.insert(n, x); }
    fn for_each_mut(&mut self, f: impl FnMut(&mut T)) where Self: Sized { … }
    fn retain_mut(&mut self, f: impl FnMut(&mut T) -> bool) where Self: Sized { … }
    fn edit_each(&mut self, f: impl FnMut(&mut SeqCursor<'_, T>)) where Self: Sized { … }
}
impl<T> SeqView<T> for Vec<T> { … }
impl<T> SeqView<T> for Vec<Box<T>> { … }     // box-transparent: get_mut→&mut**, insert→Box::new, remove unboxes
impl<T> SeqView<T> for VecDeque<T> { … }
impl<T> SeqView<T> for VecDeque<Box<T>> { … }
impl<T, P: Default> SeqView<T> for Punctuated<T, P> { … }   // insert/push need P: Default

pub trait OptView<T> { /* is_some/get/get_mut/set/take, same pattern */ }
impl<T> OptView<T> for Option<T> { … }
impl<T> OptView<T> for Option<Box<T>> { … }

// trait method (generic over the implementor; no ?Sized — the field is always a concrete container):
fn visit_t_seq<V: SeqView<T>>(&mut self, v: &mut V) { v.for_each_mut(|e| self.visit_t_mut(e)); }
// override the user writes (the generic leaks, but over a PUBLIC trait):
fn visit_stmt_seq<V: SeqView<Stmt<S>>>(&mut self, v: &mut V) { v.retain_mut(|s| keep(s)); … }
// descent passes the field directly — no wrapper:
this.visit_stmt_seq(&mut self.stmts);
```

- **Why the element type is a type param (`SeqView<T>`), not an associated `Item`:** box-transparency.
  `impl<T> SeqView<T> for Vec<T>` and `impl<T> SeqView<T> for Vec<Box<T>>` are **non-overlapping** (a
  `Vec<Box<U>>` implements *both* `SeqView<Box<U>>` and `SeqView<U>` — distinct trait params), so the
  generated `visit_t_seq::<Vec<Box<T>>>` selects `SeqView<T>` (the unboxed view) by the method's `T`. With
  an associated `Item` the two interpretations would collide (one `Item` per type) — coherence error. So a
  type param is required; this matches how Design A's hidden `SeqBackend<T>` is parameterized.
- **Descent cost: zero overhead** — everything monomorphizes per concrete container; `for_each_mut` inlines
  to today's static `iter_mut` loop. `edit_each`'s `SeqCursor<'a, T>` erases to `&mut dyn SeqView<T>`
  *internally* (the core trait is object-safe), so its closure stays clean `FnMut(&mut SeqCursor<T>)` while
  the dyn is confined to the (rare) structural walk.
- **Signatures: a generic leaks** (`<V: SeqView<Stmt<S>>>`), but `SeqView` is now a documented public trait,
  not a hidden backend — the leak is "your container, viewed as a sequence", which is intelligible.
- **Not object-safe `VisitMut`** (generic method) → no `dyn VisitMut`; monomorphization grows code per
  (visitor × container-type).

### Trade-off / recommendation

| | A `dyn` wrapper | B trait-on-container |
|---|---|---|
| view is | a wrapper `SeqView<T>(&mut dyn SeqBackend<T>)` | a public trait `SeqView<T>` impl'd on `Vec`/`Punctuated`/… |
| override signature | `&mut SeqView<Stmt<S>>` (clean, non-generic) | `<V: SeqView<Stmt<S>>>(&mut self, &mut V)` (generic, public trait) |
| descent | `this.visit_t_seq(&mut SeqView::new(&mut field))` | `this.visit_t_seq(&mut field)` (no wrapping) |
| descent cost | 1 vtable + N indirect calls / Vec | fully static (inlines) |
| object-safe `VisitMut` | yes | no |
| codegen size | smaller | larger (monomorphized) |

**Recommend A** (clean, non-generic override signatures dominate; the indirect-call cost is negligible for
an AST walk). Choose **B** if zero-overhead descent and a public, reusable `SeqView`/`OptView` trait
surface are wanted, accepting the generic in overrides. *Final pick deferred to you* — the `gen_side`
difference is localized to the `visit_t_seq` signature + the descent call (`SeqView::new(&mut field)` vs
`&mut field`).

## Generated trait shape

For each visited type `T`, on the **`VisitMut` side only**:

```rust
trait VisitMut<…> {
    // (unchanged) the per-node in-place hook — ALWAYS emitted, for every visited type:
    fn visit_t_mut(&mut self, i: &mut T) { visit_t_mut(self, i) /* free fn: descend */ }

    // (new) emitted ONLY iff some visited/drilled AST holds T in a Vec-like field:
    fn visit_t_seq(&mut self, v: &mut SeqView<T>) {
        v.for_each_mut(|e| self.visit_t_mut(e));    // default: descend into every element, keep all
    }
    // (new) emitted ONLY iff some visited/drilled AST holds T in an Option-like field:
    fn visit_t_opt(&mut self, v: &mut OptView<T>) {
        if let Some(e) = v.get_mut() { self.visit_t_mut(e); } // default: descend the present node
    }
}
```

- `visit_t_mut` is **untouched** (returns `()`), so every existing visitor keeps working unchanged.
- A visitor opts into structural editing by overriding `visit_t_seq` / `visit_t_opt` and calling view
  methods (`get_mut` for in-place, `remove`/`insert`/`push`/`set`/`take`/`edit_each` for structure). The
  default bridges to `visit_t_mut`, so overriding only `visit_t_mut` still descends normally.
- The free fns `visit_t_seq(this, v)` / `visit_t_opt(this, v)` hold the default bodies (parity with
  `visit_t_mut`); the `&mut V` blanket forwards the new methods; `Driver` does **not** override them
  (closures don't edit — see below), relying on the default.

Naming is bikeshed-able (`visit_t_seq` vs `visit_t_in_seq` vs `edit_t_seq`); `_seq`/`_opt` chosen to sit
next to `_mut` and read as "the t inside a seq/opt".

## Which methods are emitted — explicit `#[seq]` / `#[opt]` markers (requirement 3)

**No container-kind auto-detection.** A field gets a container-edit view *iff* it is explicitly annotated
`#[seq]` or `#[opt]` (a `#[derive(Ast)]` helper attribute). The macro then:

- **`#[derive(Ast)]`** declares `seq`/`opt` as helper attrs (`macro/lib.rs`) and `cleaned_definition`
  **preserves them on fields** in the emitted metadata (everything else is stripped), so `visitor!` sees
  them in the re-parsed def.
- **`visitor!` (`Lower::lower_field`)**: `field_view(&field.attrs)` → `Option<Container>`. If the field is
  marked **and** its `peel`ed head `T` is a visited (method-set) type, dispatch to `visit_<t>_seq`/`_opt`
  (recording the usage in `seq_used`/`opt_used`). An **unmarked** field — even a `Vec`/`Option` — takes
  the ordinary (non-structural) descent and produces no view method. Tuple elements carry no attrs, so no
  view. This is collected through the **same follow/drill walk** as the descent, so a marked field inside
  a **drilled** intermediate counts too.
- Emit `visit_t_seq` iff some marked `#[seq]` field's head is `t`; `visit_t_opt` iff some `#[opt]`. A type
  whose fields are never marked gets **neither** — just `visit_t_mut`.

## Codegen changes (`macro/visitor.rs`)

1. **Usage sets** `seq_used`/`opt_used` `: RefCell<HashSet<String>>` — populated by the mut `Lower` walk as
   it emits view dispatches (`view_dispatch`); then handed to `gen_side(true, ..)`.
2. **Descent dispatch (`Lower::lower_field`):** at a directly-followed method head whose field is marked,
   `this.visit_<head>_seq(&mut #field);` / `_opt(&mut #field)` — the field's own type `impl SeqView<head>`
   / `OptView<head>` (no wrapper; box-transparency and `Box<T>`/`Punctuated`/`VecDeque` are picked by type
   inference from the field type + the head `T`). An unmarked field falls through to the ordinary descent
   (`this.visit_<head>_mut(..)` wrapped in the container loop).
3. **`gen_side`:** add the `visit_t_seq`/`visit_t_opt` trait methods + free fns + `&mut V` forwards, each
   guarded by `seq_used`/`opt_used` membership. Mirror the heterogeneous (`method_params`) /
   `struct_only` handling the per-type `visit_t_mut` already does. The immutable side is unchanged.

## `core::visit` additions / removals

- **Add:** `SeqView`, `OptView`, `SeqCursor`, and (Design A only) the hidden `SeqBackend`/`OptBackend`
  (under Design B those two collapse into the public `SeqView`/`OptView` traits) + the container impls. The
  index/cursor arithmetic (advance vs. stay on remove/insert in `edit_each`) lives entirely here,
  unit-tested directly.
- **Remove (v1):** `Edit<T>`, `SeqEdit` trait + impls, `visit_opt_edit`, `visit_fixed_edit`. Keep
  `Punctuated::get_mut` (now used by the `Punctuated` view/backend impl).
- **Remove (v1 macro):** `edit_apply`, `edit_method_ident`, the `visit_*_edit` trait method and its
  `&mut V` forward.

## Interactions

- **`visit_*_mut` / `visit_*` (immutable):** untouched. Immutable side gets no views (can't edit through `&`).
- **Closures / `Hook` / `Driver` / `Chain`:** stay non-editing in this design — a closure `FnMut(&mut T)`
  only sees a node, not its container, so it routes through `visit_t_mut` via `Driver` exactly as today.
  (A future phase could add a `FnMut(&mut SeqView<T>)` hook; out of scope.)
- **`#[recurse]` cycles:** the natural types are ordinary acyclic AST to `visitor!`, so a `Vec<Box<Expr>>`
  inside a cycle is just `(Expr, Seq)` — `visit_expr_seq` is emitted and edits apply in place. No engine
  interaction.
- **Inheritance (`base => New`):** `visit_t_seq/opt` for a base-owned type come from the base trait
  (supertrait); New only adds methods for its own newly-held container usages. Needs the usage pre-pass to
  consider inherited types — detail to verify, mirrors how `method_set` already unions inherited idents.
- **`struct_only` / heterogeneous mode:** the new methods carry the same `method_params` + `where Self:
  Sized` the per-type `visit_t_mut` carries; closure machinery already off in that mode.
- **Cross-crate:** a foreign target gets no inherent `.visit_mut`, and its `visit_t_seq/opt` come via the
  upstream trait — same `path_is_crate_local` gating as today.

## Constraints / footguns (documented)

- **`Punctuated` insert/push** needs `Sep: Default`; for a non-`Default` separator those ops `panic!` with a
  clear message (read/remove/in-place still fine). Could later gate behind a `Sep: Default` bound on the
  method.
- **Empty containers:** `visit_t_seq`/`_opt` is invoked once per *field occurrence* (even when the `Vec` is
  empty / the `Option` is `None`), so `push`/`set` can fill an empty slot. (The default descent just does
  nothing when empty.) This is strictly more capable than v1.
- **No clone:** existing nodes are reached by `&mut` (`get_mut`) or moved (`remove`/`take`); only *new*
  nodes the visitor supplies are owned values it constructs.

## Migration from the shipped v1

Files to change: `core/src/visit.rs` (swap `Edit`/`SeqEdit`/… for the views), `macro/visitor.rs`
(usage pre-pass + view dispatch, drop `edit_apply`), `core/tests/visitor_reduce.rs` +
`core/tests/visitor_edit.rs` (rewrite to the view API), `CLAUDE.md` bullet. Net: the per-node
`visit_*_mut` overrides in the other test files are unaffected (they never used `Edit`).

## Phasing

1. **MVP (shipped):** `SeqView`/`OptView` with `get_mut`/`for_each_mut`/`push`/`insert`/`remove`/
   `retain_mut`/`set`/`take` **and** `SeqCursor::edit_each`; struct visitors; single-layer
   `Vec`/`VecDeque`/`Option` incl. box-transparent; the usage pre-pass.
2. **`Punctuated` inserts** (`Sep: Default`) — shipped; **nested-container** views — shipped (`#[seq]`/
   `#[opt]` names the innermost container; outer layers are iterated; a marker/innermost mismatch errors —
   `visitor_edit.rs::nested`, `ui/visitor_edit_marker_mismatch.rs`).
3. **Editing closures — assessed, deferred (low value / high cost).** A *plain* editing closure
   (`visit_mut(|v: &mut dyn SeqView<T>| …)`) is **infeasible**: adding
   `impl<F: FnMut(&mut dyn SeqView<T>)> IntoHookMut<…, T> for F` alongside the existing
   `impl<F: FnMut(&mut T)> IntoHookMut<…, T> for F` is an **E0119 conflict** (both blanket over bare `F`,
   same trailing marker `T`, and the two `FnMut` bounds aren't provably disjoint) — confirmed empirically.
   It is feasible only via an **explicit wrapper type** (`SeqEdit(f)` / `OptEdit(f)`, a distinct `Self` that
   sidesteps the overlap) **plus** a parallel copy of the Hook/Driver machinery that routes the wrapped
   closure through `visit_<t>_seq`/`_opt` (today `Driver`/`Hook` only descend via `visit_<t>_mut`), and
   matching tuple combinators. That is a ~150-line additive feature whose only benefit is sugar over the
   already-working struct-`VisitMut` path (`impl VisitMut { fn visit_t_seq(..) {..} }`), so it is **left
   unbuilt**. `SeqView`/`OptView` are already object-safe (`SeqCursor` stores `&mut dyn SeqView<T>`), so the
   wrapper route would compile — the blocker is purely value/cost, not feasibility.

## Tests

- `SeqView`: in-place `for_each_mut` / `get_mut` (no realloc), `remove`/`insert`/`push`, `retain_mut`,
  `edit_each` (remove-current, insert-after), fill an empty `Vec`.
- `OptView`: `get_mut` in place, `set` (fill `None` and replace `Some`), `take` (→ `None`).
- Box-transparent `Vec<Box<T>>` and `Option<Box<T>>` (no clone, box managed by backend).
- Edit a `Vec<Box<Expr>>` inside a `#[recurse]` cycle (self-recursive `(Expr, Seq)`).
- Regression: a `visit_*_mut`-only visitor still mutates every node and structurally changes nothing.
- `core::visit` unit tests of the `SeqCursor`/`edit_each` index arithmetic.
- Negative: a type only in a fixed `Box`/field position generates **no** `visit_t_seq`/`_opt` (compile
  check that the method name is absent), proving requirement 3.

## Decisions

- **Method names — DECIDED:** `visit_t_seq` / `visit_t_opt`.
- **MVP scope — DECIDED:** views + index ops **and** `SeqCursor`/`edit_each` (single pass).
- **View backend — DECIDED: Design B (trait on the containers), IMPLEMENTED.** `SeqView<T>`/`OptView<T>`
  are public `core::visit` traits; the per-method generic (`visit_t_seq<V: SeqView<T>>`) is accepted. The
  descent passes `&mut field` directly. Box-/`Attempt`-wrapped element forms have box-transparent impls.
  Shipped on branch `feat-visitor-edit-view` (replaces the v1 `Edit<T>` return); tests `visitor_reduce.rs`,
  `visitor_edit.rs`.

# Plan: remove the depth `limit` of `#[recurse]` via a fn-pointer re-entry vtable

## Goal

Today `#[recurse]` builds a **fixed-depth engine** (`DEFAULT_RECURSION_DEPTH`; the old `limit = N`
argument has been removed) so the cycle's traits can be derived; a tree deeper than that depth fails
(`Parse` errors at the terminator). **`Unparse`/`Spanned` no longer need this for a group-free cycle** —
they are now derived directly on the natural type (`#[ignore_bounds]` + the injected leaf-bound union)
and are unbounded; only **`Parse`** (always) and a **group-ful** cycle's `Unparse`/`Spanned` remain
engine-bounded. This plan targets the remaining bounded case — making **`Parse`** unbounded (any runtime
tree depth) while keeping the type recursion finite, using the user-proposed mechanism: a global
**fn-pointer registry** whose terminator newtype re-enters the top-level parser at runtime instead of
bottoming out.

## Why the limit exists (two independent E0275 causes)

A natural recursive type (`Expr<S>` referencing itself through `Box`/`Vec`/…) cannot derive `Parse`
directly:

1. **Where-bound cycle.** The derive adds a `field_ty: Parse` predicate per field, so
   `Expr: Parse ⇐ Stmt: Parse ⇐ Expr: Parse …` is an infinite `where`-clause (E0275).
2. **Backtracking stream-type monomorphization (Parse only).** A derived enum `Parse` backtracks via
   `stream.dup(|s| …)`, which wraps the stream in another `Dup<&mut …>` per descent level. Recursive
   descent then monomorphizes `parse::<Dup<&mut Dup<&mut …>>>` with a strictly-growing stream type →
   E0275 again.

The current engine `__XxxRec<S, __Rec = __XxxDefault<S>>` makes each depth level a *distinct finite
type*, bottoming at `__XxxTerm` after `N` levels — finite type recursion, so both cycles terminate. The
cost is the runtime depth cap.

**Key observation:** only **`Parse`** is blocked by cause (2). `Unparse`/`Spanned` have no `Dup` growth —
their limit is incidental (they delegate through the same engine for code uniformity). So the hard,
mechanism-requiring case is `Parse`; `Unparse`/`Spanned` have a cheaper escape (see §7).

## The mechanism (runtime re-entry, finite type)

Replace "go one type-level deeper" with "call the same parser again at runtime, through a fn pointer
whose type is erased." Concretely:

- The engine keeps a **fixed, small type depth** (1 level is enough): `__XxxRec<S, __XxxTerm<S>>`. The
  `limit` argument is dropped (or becomes a no-op).
- **`__XxxTerm<S>` becomes a real, inhabited newtype** wrapping the *pure natural* AST:
  `struct __XxxTerm<S>(Box<Xxx<S>>)` (today it is uninhabited `PhantomData`). It implements the cycle
  traits **with no `Xxx: Trait` bounds** — so no where-bound cycle (cause 1).
- `__XxxTerm`'s trait bodies **call a registered fn pointer** that points back at the *natural type's*
  own (top-level) trait impl. That re-enters the full parser at runtime; the fn-pointer's signature uses
  an **erased stream** (see §3), so the call does not grow the stream type (cause 2).
- The natural type's delegated `impl Parse for Xxx` (in `gen_natural_extras`) **registers** its own
  top-level parse fn into the registry on entry, *before* descending, so the terminator finds it set.

Runtime: parse the depth-1 engine → a recursive field has type `Box<__XxxTerm<S>>` → `__XxxTerm::parse`
reads the saved fn ptr and calls it → it parses a complete `Xxx<S>` subtree (which itself goes
engine→Term→fn-ptr, recursing at runtime). Unbounded runtime depth; depth-1 type; both E0275 causes cut
at the erased fn-ptr boundary.

## §3 The hard sub-problem: erasing the stream (the `fn(&mut usize)` placeholder)

The registered fn must have a **fixed, non-generic** signature (a fn pointer can't be generic over the
stream). `Parse::parse` is generic (`parse(impl IntoParseStream)`), so we must pin a concrete erased
stream type. Two viable routes:

- **(Recommended) `&mut dyn ParseStream` trait object.** Make `ParseStream` object-safe and erase to
  `&mut dyn ParseStream<Atom = A, Error = E>`. The registered fn is
  `fn(&mut dyn ParseStream<Atom = A, Error = E>) -> Result<Xxx<S>, ParseError>`. At the terminator,
  coerce the incoming concrete stream (`&mut Dup<…>` or whatever) to `&mut dyn ParseStream` and call the
  fn ptr. Inside the re-entered parse the stream is `&mut dyn ParseStream`; its `dup` makes
  `Dup<&mut dyn ParseStream>` — a **fixed** type; the next terminator re-erases `&mut Dup<&mut dyn …>`
  back to `&mut dyn ParseStream`. So the stream type is pinned at one `Dup` layer, never grows.
  **Blocker (SOLVED):** `ParseStream` was not object-safe — `dup<F>` and `validate_spacing<S: Span>` are
  generic methods. Add `where Self: Sized` to both (no separate trait): a `Self: Sized` method is excluded
  from the vtable, so the trait becomes object-safe while `dup`/`validate_spacing` stay callable on every
  real (sized) stream (`Stream`, `Dup<…>`, `&mut T`) — they're never needed on `dyn` itself (the re-entry
  always calls `dup` on a sized `&mut dyn …`, where `Self = &mut dyn …` is Sized). Then `dyn ParseStream:
  ParseStream` (auto) and `&mut dyn ParseStream: ParseStream` (existing `&mut T` blanket), and `&mut dyn
  ParseStream: IntoParseStream` (blanket over `T: ParseStream`), so `<Xxx<S> as Parse<A>>::parse` coerces
  to the fixed fn-ptr at that monomorphization. (Earlier draft used a `ParseStreamExt` blanket; the
  `Self: Sized` form is simpler — no extra trait, no `use`/UFCS churn.)

- **(Fallback) reified cursor stream.** A concrete owned stream `Cursor<Atom> { buf: Rc<[Atom]>, pos }`
  built once at the top; the fn ptr is `fn(&mut Cursor<Atom>) -> …`. Avoids `dyn`, but requires
  integrating a cursor stream into the existing `next`/`peek`/`push` + `Dup` design, and care that
  backtracking restores `pos`. More invasive than (Recommended).

**Backtracking correctness:** with route (Recommended), `Dup<&mut dyn ParseStream>` keeps the existing
take/push-buf snapshot semantics, so an outer `dup` that fails still rewinds across a terminator
re-entry. This must be verified with a deep-backtracking test (an ambiguous grammar that backtracks
*through* several terminator boundaries).

## §4 The registry (the `GetVTableKey` snippet) — keying & soundness

The per-type cell is found by a per-monomorphization key. The original snippet keyed on
`Self::get_vtable_key as usize` (a fn *address*) — that is **unsound under ICF** (identical-code-folding /
`-Zshare-generics` / linker `--icf=all`): `get_vtable_key::<T>` had the same empty body for every `T`, so
the linker could collapse them to one address → all types share one cell → wrong fn ptrs, silently.

**Fix (chosen): key on the `type_name` string pointer.** `get_vtable_key` returns the *data pointer* of
`core::any::type_name::<Self>()`, as `*const c_void`, and the key is that pointer:

```rust
trait GetVTableKey {
    /// A per-type-stable, ICF-safe identity: the data pointer of `type_name::<Self>()`. The string's
    /// *content* differs per type, so the statics are never merged; the same type returns the same
    /// interned `&'static str` (hence the same pointer) on every call. `?Sized`-ok (no `'static` bound,
    /// unlike `TypeId`), which matters because an engine type may carry a lifetime param.
    fn get_vtable_key() -> *const core::ffi::c_void {
        core::any::type_name::<Self>().as_ptr() as *const core::ffi::c_void
    }
    fn get_cell() -> &'static OnceLock<fn(&mut usize) -> Self> {
        let map = VTABLE_MAP_PARSE.get_or_init(|| Mutex::new(HashMap::new()));
        let mut map = map.lock().unwrap();
        let cell = map.entry(Self::get_vtable_key() as usize).or_insert(OnceLock::new());
        unsafe { core::mem::transmute(cell) }
    }
}
impl<T: ?Sized> GetVTableKey for T {}
```

Why this is ICF-safe (vs. the fn-address): the key is now the fn's *return value*, derived from a
per-`T` string static whose content differs by type — so neither the strings nor the
`get_vtable_key::<T>` bodies (which reference different statics) can be merged, and even if the fns *were*
merged the returned pointer is still per-`T`-correct. (`type_name` is "not guaranteed unique" by the std
docs but is unique in practice for distinct monomorphizations; if paranoid, key on the `&'static str`
*content* in the `HashMap` instead of its pointer — robustly per-type and independent of pointer
interning — at the cost of a string hash per lookup. With `S: 'static`, `TypeId::of::<T>()` is the
textbook key.)
- **The `transmute` of `&OnceLock<usize>` → `&OnceLock<fn…>` is `unsafe`** and relies on
  `OnceLock<usize>` and `OnceLock<fnptr>` having identical layout (both pointer-sized, non-Drop). True
  today but unguaranteed; isolate it in one audited `unsafe` block with a `static_assert` on sizes, and
  a comment. Per-type the cell is only ever transmuted to ONE concrete fn-ptr type (the key is
  per-monomorph), so reads/writes stay type-consistent.
- **Thread-safety / timing.** `OnceLock::set` is first-wins and the value is the same fn for a given
  type, so concurrent top-level parses are fine. `__XxxTerm::…` does `get().unwrap()` — it panics if the
  cell is unset, which can only happen if the terminator is reached without the delegated top-level
  having registered first. The generated entry points always register before descending; still, prefer
  a clear `expect("…internal: terminator reached before registration")`.

## §5 Codegen changes (`macro/recurse.rs`)

1. **Drop the depth chain.** In `build_scc` / `build_multiroot_tail`, set the engine type depth to a
   fixed small constant (1); stop expanding `__XxxDefault` to `recursion_depth - 1` levels. `RecurseArgs`
   keeps accepting `limit` for back-compat but ignores it (warn that it's deprecated/no-op), or remove
   the arg.
2. **Terminator newtype.** `make`-the-terminator emits `pub struct __XxxTerm<S>(Box<Xxx<S>>)` (was
   `PhantomData`). Its `__to_nat` returns the wrapped `*Box<Xxx<S>>` directly (was `unreachable!`); its
   `__from_nat` wraps a clone of the natural value (was `panic!`).
3. **Terminator trait impls call the registry** (replace the `Err("recursion depth limit reached")` /
   `panic!` bodies):
   - `Parse`: `let f = <__XxxTerm<S> as Reg>::cell().get().expect(…); Ok(__XxxTerm(Box::new(f(&mut stream.as_dyn())?)))`.
   - `Unparse`: read the saved unparse fn (separate registry `VTABLE_MAP_UNPARSE`), call it on `&*self.0`.
   - `Spanned`: read the saved span fn (`VTABLE_MAP_SPANNED`), call it on `&*self.0`.
4. **Registration in the delegated impls** (`gen_natural_extras` / `emit_delegated_impl`). Each delegated
   `impl Parse for Xxx` first does
   `<__XxxTerm<S> as Reg>::cell().set(<Xxx<S> as Parse<A>>::parse as fn(&mut dyn ParseStream<…>) -> _);`
   then parses the engine + `__to_nat`s as today. Same for the `Unparse`/`Spanned` delegated impls into
   their registries.
5. **The `RecTrait` model** already abstracts the three delegated impls — thread a per-trait "registry
   name + erased-boundary type" through `emit_delegated_impl` so one algorithm still emits all three.

## §6 Library support (`core/src/parse`, `core/src/span`)

- Make `ParseStream` object-safe by adding `where Self: Sized` to `dup` + `validate_spacing` (they stay on
  the base trait, just vtable-excluded). **DONE** — no separate trait; the existing method-syntax /
  `ParseStream::dup` UFCS call sites are unchanged. Optionally add a `fn as_dyn(&mut self) -> &mut (dyn
  ParseStream<…> + '_)` convenience (or just coerce at call sites).
- Add `impl<Atom, T: ?Sized + Emitter<Atom>> Emitter<Atom> for &mut T` so `&mut dyn Emitter` works
  (needed only if `Unparse` uses the vtable — see §7).
- The `GetVTableKey`/registry trait + the three `static` maps live in a new `core::parse::vtable` module
  (one `OnceLock<Mutex<HashMap<Key, OnceLock<usize>>>>` per trait). `#[recurse]` references them
  `$crate`-rooted.

## §7 Per-trait scope (do `Parse` first; reconsider U/S)

- **`Parse`** genuinely needs the vtable (cause 2). Implement it fully.
- **`Unparse` / `Spanned`** have *no* stream growth — only the where-bound cycle (cause 1), which a
  bound-free terminator already solves. Two cheaper options than a runtime vtable:
  - **(a) Direct natural impl via `#[ignore_bounds]`** — arbitrary depth, no indirection, no global
    state. This is what the *old* direct path did; it works for single-self-recursive group-free cycles
    but not uniformly (multi-type cross-member leaf bounds; group-ful `Fill`). 
  - **(b) The same vtable** as `Parse` (uniform, but adds global state + the `&mut dyn Emitter` erase for
    `Unparse`). 
  Recommendation: ship `Parse` via vtable (unbounded); keep `Unparse`/`Spanned` **delegated &
  depth-limited as today** in v1 (they already pass their tests), and decide (a) vs (b) for them
  separately. I.e. "remove the limit" first means **remove the *parse* limit**; document that
  `Unparse`/`Spanned` stay depth-limited until a follow-up. (If full uniformity is required, use (b).)

## §8 Risks / open questions (decide before building)

1. **ICF on the key** — RESOLVED by §4: key on `type_name::<T>()`'s data pointer (its fn *return value*),
   not the fn address. Was the highest-priority soundness item; the fn-address form must not be used.
2. **`type_name` uniqueness / pointer interning** — std doesn't *guarantee* `type_name` is unique, and
   keying on its pointer assumes the same `&'static str` (same address) is returned per type per call
   (true in practice). Document; if paranoid, key on the string *content* (§4).
3. **`OnceLock` transmute layout** — isolate + assert (§4).
4. **`HashMap` entry stability + per-node locking.** Two coupled hazards: (a) `get_cell` returns a `&`
   into a `HashMap` bucket transmuted to `&'static` — but a later `insert` can rehash and **move**
   buckets, dangling the reference; (b) re-locking the global `Mutex<HashMap>` at *every* terminator
   re-entry is a per-node mutex (perf disaster). Both are fixed the same way: resolve the
   `&'static OnceLock` **once** (e.g. cache it in a generic `static`/thread-local keyed by `T`, or use a
   map that never moves entries — `boxed` values, or a leaked `Box<OnceLock>`), then the hot path is a
   lock-free `OnceLock::get`. Must be designed in, not bolted on.
5. **Backtracking through the erased boundary** — needs the deep-ambiguous-grammar test (§3).
6. **Object-safety refactor of `ParseStream`** — a public-API change (moves `dup` to an ext trait);
   audit all `stream.dup(..)` call sites still resolve via the ext trait in scope.
7. **Error/span quality** — re-entry erases spans into `&mut dyn`; confirm `ParseError` spans survive.
8. **Multi-root / multi-cycle / cross-crate** — each cycle type needs its own registry cell; cross-crate
   re-entry must register in the *defining* crate's statics (the `$crate`-rooted registry handles this,
   but verify the key is stable across crates).

## §9 Sequencing

1. **Spike — ✅ DONE (`docs/spike_unbounded_recurse.rs`, standalone, no syan deps).** A toy grammar
   `Expr = "(" Expr ")" | Int` with a depth-1 engine + inhabited `Term` + `type_name`-pointer registry +
   `&mut dyn Stream` re-entry. Confirmed:
   - **(a) compiles** — finite type recursion, no E0275 (the successful compile *is* the proof the
     erasure cuts the `Dup<…>` growth);
   - **(b) unbounded depth** — parses 0/1/5/50/500/**2000** nested levels (old type-`limit` ≈ 4–12); the
     only ceiling is the OS call stack (runtime recursion, like any recursive-descent parser);
   - **(c) backtracking through terminators** — a top-level `Expr "!" | Expr` rewinds the *entire* deep
     `Expr` (D=200 term boundaries) in one backtrack, then re-parses — `dup` snapshot/restore propagates
     across the erased re-entry;
   - **(d) confirmed as the open issue** — the naive `lookup` locks the global `Mutex` on *every*
     re-entry (per-node). Fine for the spike; must be fixed per §8.4 before production.

   Caveats: the spike used `Mutex<HashMap<usize, usize>>` (fn-ptr-as-`usize`), sidestepping the
   `OnceLock`-transmute and `HashMap`-entry-stability concerns — those (§4, §8.3–8.4) still need solving.
   Its toy `Dup` mirrors syan's but isn't identical.

1b. **Second spike — ✅ DONE, against syan's REAL types (`core/tests/spike_real_parsestream.rs`).**
   First made `ParseStream` object-safe by adding `where Self: Sized` to `dup`/`validate_spacing` (kept on
   the trait, vtable-excluded — no separate `Ext` trait, so all existing call sites are untouched) — the
   whole suite stays green (syan 319, syan-rust 15, clippy 0), so the riskiest *library* change is proven
   safe and minimal. Then a grammar `Expr = "<" Expr ">" | Int` over real `TokenTree` tokens, hand-built
   with the actual `ParseStream`/`Dup`/`ParseStream::dup`,
   erasing to `&mut (dyn ParseStream<Atom = TokenTree, Error = Infallible> + '_)`. Confirmed: **(a)**
   compiles — finite monomorphization with syan's real `Dup` (no E0275); **(b)** depth 500; **(c)**
   backtracks through 200 erased boundaries. **New finding:** the erased dyn needs an explicit
   non-`'static` lifetime (`+ '_`) because `Dup<&mut …>` borrows; `&'b mut S → &'b mut (dyn … + 'b)`
   is WF-sound (the `&'b mut S` borrow gives `S: 'b`) with no `S: 'static` bound. The fn-ptr type is
   `for<'r,'s> fn(&'r mut (dyn … + 's))`. **Feasibility is now fully proven on the real types** — the
   remaining work is engineering (§8.4 hot-path/entry-stability, §4 unsafe registry, codegen).
2. **Library:** the `ParseStream` object-safety split is **done** (above); still TODO: the `vtable`
   module (§6) with the production registry (cached `&'static OnceLock`, entry-stable).
3. **Codegen:** terminator newtype + registry registration + depth-1 engine, `Parse` only (§5).
4. **Tests:** convert `unparse_past_limit_panics`-style depth pins; add a deep-parse test (depth ≫ old
   limit); the backtracking test; cross-crate. Update `recurse_test.rs`, `rustsub_roundtrip.rs`
   (drop the `recursion_limit`/`limit = 12` workarounds).
5. **Decide U/S** (§7) and either leave them depth-limited (document) or extend the vtable.
6. **Docs:** rewrite the `#[recurse]` sections of CLAUDE.md; this file records the rationale.

## §10 Recommendation

The mechanism is **feasibility-proven** (§9.1b, on syan's real types) and would close the last
`#[recurse]` limitation, but it trades the current clean, all-compile-time design for **global mutable
state + one audited `unsafe` transmute + a per-type registry**. Settled: the keying (§4: `type_name`
pointer, ICF-safe), the **object-safety split** (done, suite green), and **backtracking through the
erased boundary** (spike (c)). Remaining hazards are pure engineering: **`HashMap` entry stability +
per-node hot-path locking** (§8.4) and the registry's `unsafe`/`OnceLock` details (§4, §8.3). Proceed
`Parse`-first (§7); keep `Unparse`/`Spanned` depth-limited until a follow-up.

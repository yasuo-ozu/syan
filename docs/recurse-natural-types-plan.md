# Plan: natural public AST types + internal depth-limited engine for `#[recurse]`

> **STATUS: IMPLEMENTED** (the core; see "Implementation outcome" at the bottom). The headline win —
> closures over `#[recurse]` cycles — works. Full suite green (syan 301, syan-rust 11). The one
> deferred piece is natural `Unparse`/`Spanned` for *group-ful* cycles (§5 / §11 note).


**Goal.** Stop exporting `pub type Expr<…> = __ExprRec<…, defaults>`. Instead emit the user's cycle
types as **genuine natural recursive types** (the public API + the type closures/visitors see) and keep
the depth-limited `__ExprRec<…, R>` family as an **internal engine** used only to satisfy `Parse`.
This makes the public types depth-*uniform*, which collapses the visitor onto the existing acyclic path
— so **closures, tuples-of-closures, inheritance, and `visit_mut` over `#[recurse]` cycles all work**,
closing the long-deferred "Closures over `#[recurse]`" gap.

---

## 0. The one-paragraph why

`#[derive(Parse/Unparse/Spanned)]` on a natural recursive type fails with **E0275 "overflow evaluating
the requirement"** because the derive adds a `field_ty: Trait` where-bound for *every* field, so
`Expr: Parse ⇐ Vec<Stmt>: Parse ⇐ Stmt: Parse ⇐ Expr: Parse` is an infinite where-bound cycle.
`#[recurse]` breaks that cycle today by renaming cycle types into a finite chain of *distinct*
depth-limited types — but that also makes each tree level a different type, which is exactly why a
closure can't drive a recurse visitor (`for<R> FnMut(&__XxxRec<S,R>)` is type-level HRTB Rust lacks).
The fix keeps depth-limiting **only** as a private parse engine, exposes natural types, and bridges them
with generated conversions. The visitor never sees the engine, so it has no depth parameter and closures
work. (`type Error` on the Parse derive is hardcoded `ParseError`, so bounds are the *only* cycle —
verified empirically.)

---

## 1. Trait-by-trait strategy

Derives applied to cycle types in the test corpus: `Ast`, `Parse`, `Unparse`, `Spanned`, `Debug`,
`Default`. They split into three groups by whether they hit E0275 on a natural type:

| Trait(s) | E0275 on natural? | Strategy |
|---|---|---|
| `Ast` | No (`impl Ast for T` — only the type's own where-clause) | Derive **on the natural type** (also carries the visitor metadata). |
| `Debug`, `Default`, `Clone`, … (std/per-param derives) | No (std bounds per generic *param*, not per field) | Derive **on the natural type**, unchanged. |
| `Parse` | **Yes** | **Delegate**: derive on the *engine*, hand-emit `impl Parse for Natural` that parses the engine then converts engine→natural. |
| `Unparse`, `Spanned` | **Yes** | **Direct leaf-only impl on the natural type** via re-enabled `#[ignore_bounds]` on recursive-child fields (no engine, no depth limit). |

Why the asymmetry (`Parse` delegates, `Unparse`/`Spanned` direct):

- `Unparse`/`Spanned` are `&self`, purely structural emission; a leaf-only-bound impl recurses into
  children via sibling impls and handles **arbitrary depth** — so they have **no deep-tree problem**.
  The natural→engine direction would be lossy (a tree deeper than `limit` can't be rebuilt into the
  engine's finite chain), so we *avoid* converting in that direction entirely.
- `Parse` constructs values and uses the `#[group(…)]` substruct **`Fill`** machinery + backtracking.
  Delegating to the engine reuses the already-working derive on finite types and **preserves today's
  "lenient truncation" parse semantics** (the engine caps depth). It also keeps the leaf-only-bound
  substruct-threading (the riskier part) out of scope. (§10 discusses dropping the engine for Parse too.)

---

## 2. What `#[recurse]` emits, after the change

For each SCC (independent cycle), in this order inside the module:

1. **Natural types** — the user's *original* enum/struct definitions, **un-renamed**, with their
   `#[derive(…)]` list **rewritten**: `Parse` removed; everything else kept; `#[ignore_bounds]`
   injected on every **recursive-child field** (a field whose container-peeled head ∈ the SCC). These
   are the public `Expr<S>`, `Stmt<S>`.
2. **Engine types** — today's transformed `__ExprRec<S, __Rec = __ExprDefault<S>>` family, emitted
   `pub(crate)` (no longer `pub`, no public alias), carrying **only** `#[derive(Parse)]`.
3. **Terminators** — `ExprTerm` (+ its `Parse` impl that errors, as today) **+ a new `__ToNatExpr`
   impl that is `unreachable!()`** (see §4).
4. **Depth-chain aliases** — `__ExprDefault<…>` etc., as today (drive the engine's default depth).
   The public `pub type Expr = …` alias is **deleted** (the natural enum owns that name now).
5. **Conversion impls** — `engine → natural`, depth-generic (§4).
6. **Delegated `impl Parse for Natural`** — parse engine, then `to_nat()` (§4).
7. **Direct `impl Unparse`/`impl Spanned` for Natural** — produced by the rewritten derive in step 1
   (via `#[ignore_bounds]`); nothing extra to hand-emit beyond honoring the attribute (§5).
8. **Visitor metadata** — `#[derive(Ast)]` on the natural type provides it; the `@recurse` metadata
   macro is **no longer emitted** (§6, §7).

Inherent `impl<S> Expr<S> { … }` blocks the user wrote stay **verbatim** on the natural type — the
`transform_item` `Item::Impl` rewriting branch (recurse.rs ~363–432) is **deleted** (§9).

---

## 3. The hard precondition: natural types must be finite-size (E0072)

`#[recurse]` today supports **pure by-value cycles** — e.g. `enum Expr<S> { Lit(Integer),
Nested(Expr<S>) }` (no `Box`/`Vec`) — which are valid *only* because the engine is depth-limited. The
**natural** form of such a cycle is **infinite-size (E0072)** and cannot be emitted. (This is exactly
what `ui/problem1_trait_impl.rs` and `ui/problem3_pub_crate.rs` show.)

**Detection is nearly free with existing helpers.** A natural SCC is finite-size-safe iff every simple
cycle passes through at least one *heap-indirected* edge (`Box`/`Vec`/`VecDeque`/`Option<Box>`/
`Punctuated`/`Rc`/`Arc`/`&`/`[_]`). The existing `collect_direct_refs_item` already computes the
**non-indirected (outermost-constructor)** edges, and `subgraph_is_cyclic` already checks acyclicity.
So:

```
finite_size_safe(scc) := ! subgraph_is_cyclic(scc, /*remove nothing*/ ∅, direct_type_refs|scc)
```

i.e. the **direct-edge** subgraph restricted to the SCC must be **acyclic**.

- **Safe (acyclic direct-edge subgraph):** emit natural types + engine + conversions (closures enabled).
  All current real tests (`minimal`, `shallow`, `visitor_recurse_closure`, `rust/src/lib.rs`)
  satisfy this (a `Vec`/`Box` sits on the cycle edge).
- **Unsafe (a pure value-cycle):** **fall back to today's representation** — emit the engine + the
  `pub type Expr = …` alias, *no* natural type, *no* closure support for that cycle. Emit a one-line
  note: "cycle `…` has no indirection; closures over it are unavailable — add a `Box<…>` on a cycle
  edge to enable them." This keeps `problem1`/`problem3`-style code compiling exactly as today.

---

## 4. Conversion machinery (engine → natural), detailed

Running example — `minimal` (root = `Expr`, single depth param `__Rec`):

```rust
// natural (public)
pub enum Expr<S> { Lit(Integer), Block { brace: GroupBrace<(),S>, stmts: Vec<Stmt<S>> } }
pub enum Stmt<S> { Semi { expr: Expr<S>, semi: Token![S=>;] }, Expr(Expr<S>) }
// engine (pub(crate))
pub(crate) enum __ExprRec<S, __Rec = __ExprDefault<S>> {
    Lit(Integer), Block { brace: GroupBrace<(),S>, stmts: Vec<__StmtRec<S, __Rec>> } }
pub(crate) enum __StmtRec<S, __Rec = __ExprDefault<S>> {
    Semi { expr: __Rec, semi: Token![S=>;] }, Expr(__Rec) }
```

**One conversion trait per cycle type** (private to the module), implemented depth-generically:

```rust
trait __ToNatExpr<S> { fn to_nat(self) -> Expr<S>; }
trait __ToNatStmt<S> { fn to_nat(self) -> Stmt<S>; }

impl<S, __Rec> __ToNatExpr<S> for __ExprRec<S, __Rec>
where __Rec: __ToNatExpr<S>,                       // back-edge to root collapses to __Rec
      __StmtRec<S, __Rec>: __ToNatStmt<S>,         // cross-edge
{
    fn to_nat(self) -> Expr<S> {
        match self {
            __ExprRec::Lit(x0) => Expr::Lit(x0),                       // leaf: move as-is
            __ExprRec::Block { brace, stmts } => Expr::Block {         // container of cross-edge
                brace,                                                 // leaf
                stmts: stmts.into_iter().map(__ToNatStmt::to_nat).collect(),
            },
        }
    }
}
impl<S, __Rec> __ToNatStmt<S> for __StmtRec<S, __Rec>
where __Rec: __ToNatExpr<S>,
{
    fn to_nat(self) -> Stmt<S> {
        match self {
            __StmtRec::Semi { expr, semi } => Stmt::Semi { expr: expr.to_nat(), semi },
            __StmtRec::Expr(e) => Stmt::Expr(e.to_nat()),
        }
    }
}
// terminator: never constructed at runtime (its Parse always errors) — the arm is dead
impl<S> __ToNatExpr<S> for ExprTerm /* <S> if the cycle is generic */ {
    fn to_nat(self) -> Expr<S> {
        unreachable!("#[recurse]: depth-limit terminator can never be parsed")
    }
}
```

**Per-field-kind lowering** (reuse the visitor's `peel` + `recurse_lower`-style classification — do
**not** invent per-container blanket traits; lower the conversion *expression* inline):

| Field shape (engine) | Conversion expression |
|---|---|
| leaf (head ∉ SCC) | move as-is: `x` |
| back-edge `__Rec` / cross-edge `__YRec<…,R>` | `x.to_nat()` |
| `Box<C>` | `Box::new((*x).to_nat_inner())` (peel the box, lower `C`) |
| `Vec<C>` / `VecDeque<C>` / `[C]` / `[C; N]` | `x.into_iter().map(\|e\| e.to_nat_inner()).collect()` |
| `Option<C>` | `x.map(\|e\| e.to_nat_inner())` |
| `Punctuated<C, P>` | map values, preserve `P` separators (`Punctuated::from_iter` of mapped pairs) |
| tuple `(C0, C1, …)` | destructure, lower each element |
| nested containers `Vec<Box<C>>` | compose the above recursively |

Codegen walks the **original (pre-transform) item's** fields (so it sees `Vec<Stmt<S>>`, classifying
`Stmt` as a cross-edge), and emits **explicit, exhaustive** struct/tuple construction — every field
named, **no `..`** — so a missed field is a compile error, not silent data loss. Wrap the match in
`let r: Expr<S> = match … ;` for a type-ascription guard.

**Groups are irrelevant to conversion.** `#[group(…)]` only affects *parsing* (the `Fill`/`unfill`
substruct dance happens inside the engine's `Parse`). By the time conversion runs, the engine value's
field already holds `Vec<__StmtRec<…>>` — conversion is purely structural. (Dissolves a "blocking"
critique.)

**No E0275.** Proving `__ExprRec<S, __ExprDefault<S>>: __ToNatExpr<S>` needs
`__ExprDefault<S>: __ToNatExpr<S>` and `__StmtRec<S, __ExprDefault<S>>: __ToNatStmt<S>`; both reduce to
`__ExprDefault<S>: __ToNatExpr<S>`, and `__ExprDefault = __ExprRec<S, __ExprRec<S, … ExprTerm>>` is a
**strictly shrinking concrete chain** bottoming at the concrete `ExprTerm: __ToNatExpr<S>` impl. Solver
recursion depth = `limit` (≤ a few), identical in shape to the engine's already-working `Parse`
derivation. ✔

**Delegated Parse:**

```rust
impl<S, __Atom> ::syan::parse::Parse<__Atom> for Expr<S>
where __Atom: ::syan::span::Spanned + ::core::clone::Clone,
      __ExprRec<S, __ExprDefault<S>>: ::syan::parse::Parse<__Atom, Error = ::syan::error::ParseError>,
      __ExprRec<S, __ExprDefault<S>>: __ToNatExpr<S>,
{
    type Error = ::syan::error::ParseError;
    fn parse(stream: impl ::syan::parse::IntoParseStream<Atom = __Atom>) -> Result<Self, Self::Error> {
        Ok(<__ExprRec<S, __ExprDefault<S>> as ::syan::parse::Parse<__Atom>>::parse(stream)?.to_nat())
    }
}
```

### Multi-root / heterogeneous / const / lifetime generalization

- **Multi-root** (N self-referential roots ⇒ N depth params `__RecA,__RecB,…`): the engine carries all
  N; the conversion impl is generic over all N and bounds each root param + every cross-edge node type:
  `impl<S, __RecA, __RecB> __ToNatExpr<S> for __ExprRec<S, __RecA, __RecB> where __RecA: __ToNatExpr<S>,
  __RecB: __ToNatStmt<S>, __StmtRec<S,__RecA,__RecB>: __ToNatStmt<S> { … }`. A back-edge to root `X`
  lowers via that root's param; cross-edges thread all N. (Mirrors `build_multiroot_tail`.)
- **Heterogeneous params** (root `Expr<S>`, non-root `Stmt<S,T>`): natural types keep their own params
  verbatim; `__ToNatStmt<S, T>` carries `T`; the conversion signature copies each type's full generic
  declaration (`generic_tokens`) and appends the depth params on the engine side only.
- **const / lifetime params**: copied verbatim into the conversion `impl<…>` header (const params are
  fine in where-clauses; lifetimes thread normally). Phantom handling mirrors the terminator's existing
  `phantom_elems`.
- **Identity back-edge rule** (`root_ident_args`, the `Expr<Vec<S>>` rejection) is **kept** — it
  constrains the *engine*, which is unchanged. The natural type can *spell* `Box<Expr<Vec<S>>>`, but the
  engine still can't represent non-regular recursion, so the existing `abort!` stands.

---

## 5. Direct `Unparse`/`Spanned` on the natural type (re-enable `#[ignore_bounds]`)

`#[ignore_bounds]` is currently a **documented no-op** (the honoring code is commented out at
attribute.rs ~361 / ~539; `audit_ignore_bounds_noop.rs` pins this). It was never broken — just
unfinished. Re-enable it:

1. **Honor it in `extract_parse`/`extract_unparse`/`extract_spanned`**: skip pushing
   `field_ty: Trait` when the field has `#[ignore_bounds]`.
2. **`#[recurse]` injects `#[ignore_bounds]`** on every recursive-child field of the natural type
   before the derives run.
3. **Propagate through substructs**: `generate_substruct` already clones the field (preserving attrs),
   so a grouped recursive field keeps `#[ignore_bounds]`; the substruct's re-derived `Unparse`/`Spanned`
   then also emits leaf-only bounds. Verify the clone path keeps the attr (and the `to_parse_ty`/`Fill`
   bound for the group is on the *substruct*, which is itself leaf-only).

Result: `impl Unparse for Expr<S> where <leaf bounds only>` whose body calls `child.unparse(sink)`,
resolved via the **sibling** `Stmt: Unparse` impl (also leaf-only) — no where-bound cycle, **arbitrary
depth**. Same for `Spanned` (its `span()` is depth-independent, so it's sound and not lossy).

The engine does **not** need `Unparse`/`Spanned` (only `Parse`), so route those derives to the natural
type exclusively.

---

## 6. Visitor subsystem: collapse to the acyclic path

With natural public types, a `visitor!(Expr, Stmt)` over a former-recurse cycle is **byte-for-byte an
acyclic visitor**: `gen_side` never emits `field_ty: Visit` bounds (all dispatch is via `Self: Visit`
methods), so there is **no E0275** and **no need for depth-generic methods**. Concretely:

- `visit_expr<R: VisitRec<S,Self>>(&mut self, &ExprNode<S,R>)` → **`visit_expr(&mut self, &Expr<S>)`**.
- **Closures** `|e: &Expr<S>|` and **tuples of closures** work via the existing `Hook`/`Driver`/`Chain`.
- **Inheritance** `visitor!(base => New)` over former-recurse cycles works via the **normal supertrait**
  path — the `@recbase` marker and the `base_is_recurse`/`struct_only` branch are gone.
- **`visit_mut`** works via the existing in-place mirror.

**Delete (dead code):** `generate_module_mixed`, `VisitRec`/`VisitRecMut` emission, the `@recbase`
plumbing (`base_is_recurse`, `recbasecarry`), the `struct_only` parameter and all `#(if !struct_only)`
gates in `gen_side`, the `recurse_lower_*` helpers, the depth-param naming logic, and the `*Node`
aliases. `build()` no longer branches on `@recurse`; it always calls `generate_module`.

**Drill-in/`#[subast]` is unaffected**: the natural type's `#[derive(Ast)]` metadata already carries the
natural definition (`cleaned_item`) and `@subast` keyed by natural idents (`Stmt`), and drill-in matches
by **key name** — exactly the acyclic mechanism. (Dissolves the "drill-in assumes engine fields"
critique.)

---

## 7. Metadata simplification

- **Drop `@recurse` emission** (`recurse_metadata_macros`, `@node/@roots/@depth/@terms/@cycle`): only
  the depth-generic visitor consumed it, and that's gone. Delete `parse_recurse`/`emit_recurse`/
  `RecurseMeta` consumption in visitor.rs.
- The visitor metadata for a cycle type is now just its ordinary **`#[derive(Ast)]`** macro
  (re-exported under the type name; type+macro namespaces coexist, unchanged).
- **Engine types are `pub(crate)`** and appear in *no* metadata — fully internal.
- **Cross-crate**: downstream `visitor!(other::Expr)` fetches `Expr!{@ast …}` (the natural Ast metadata)
  and builds an acyclic visitor over the natural `Expr`. Parsing downstream uses `Expr<S>: Parse`, which
  is implemented in the **defining** crate (conversion impls + engine are emitted there — no orphan
  issue). ✔

---

## 8. Behavior changes & migration

- **Parse truncation preserved** — delegation keeps the engine depth cap, so `recurse_problems_test.rs`
  "lenient truncation" assertions still hold (but the inner field is now natural `Expr<S>`, so the
  `recurse_problems_test.rs:76` "cannot inspect inner.len()" note *changes* — inner is now inspectable;
  update that comment/assertion).
- **`ui/problem1_trait_impl.rs` now compiles** (trait impls on cycle types work once the type is real
  *and* finite-size). It currently *uses* a pure value-cycle (`Nested(Expr<S>)`), so it falls under §3
  fallback unless a `Box` is added. **Action:** repurpose it into a passing test
  (`Nested(Box<Expr<S>>)` + `impl Display`) demonstrating the new capability, and likewise audit
  `problem2_free_fn`, `problem7_multiseg_path`, `problem8_qself`.
- **Visitor signature migration** (breaking, but it's the desired simplification): update manual
  `impl Visit<S>` blocks in ~10 files — `visitor_recurse_{cycle,via_visitor,mixed,heterogeneous,
  container_of_tuple,multiroot_via_visitor,multicycle_via_visitor,drill_unlisted}.rs`,
  `visitor_inherit_recurse{,_acyclic_mid}.rs`, `recurse_generics.rs`, and
  `rust/tests/cross_crate_recurse.rs` (drop `<R>`, replace `ExprNode<S,R>` → `Expr<S>`,
  `v::ExprNode::Lit(…)` → `ast::Expr::Lit(…)`). Provide a sed/regex recipe.
- **`transform_item` `Item::Impl` branch deleted** — user inherent impls land on natural types as
  written (`recurse_test.rs` `is_literal`/`stmt_count`/`semi_expr` unchanged; they're cleaner now).
- **Performance**: delegated Parse **deep-copies** the tree (engine→natural) once per top-level parse.
  Acceptable for typical ASTs; note it. (§10's alternative removes the copy.)
- **Inherent methods lose the type-level depth guarantee** — `stmt_count()` on a user-built deep tree
  can recurse arbitrarily. This matches normal recursive-AST expectations; document it.

---

## 9. Edge-case checklist (from the adversarial panel)

- **`limit = 0`**: keep today's behavior (panic/abort); **`limit = 1`**: engine bottoms straight at the
  terminator — conversion's terminator arm is `unreachable!()` (correct, dead).
- **Terminator safety**: engine + terminator are `pub(crate)` → users can't construct an `ExprTerm`, so
  the `unreachable!()` is truly unreachable; give it a clear message.
- **Multi-cycle modules**: each SCC independently gets natural+engine+conversions (the §3 finite-size
  test is per-SCC); `internal_names`/naming already isolates them.
- **Punctuated separators** survive the round-trip: conversion preserves `P` (map values only); Parse
  goes through the engine which already handles `Punctuated`.
- **Nested containers** (`Vec<Box<Expr>>`, `Option<Vec<Stmt>>`): inline lowering composes; no
  ordering/coherence issue (no blanket container traits).
- **Foreign last-segment collision** (`recurse_fixes.rs`): unaffected — conversion keys on SCC
  membership, not bare idents.
- **`where`-clauses on cycle types** (`problem6`): still gated by the derive's existing limitation;
  out of scope here.

---

## 10. Ruled out: dropping the engine (leaf-only direct Parse) — SPIKED, IMPOSSIBLE

Hypothesis: if leaf-only-bound **`Parse`** compiled directly on the natural type, the engine +
conversion + depth limit would all be unnecessary. **Spiked (2026-06) and disproved.** Re-enabling
`#[ignore_bounds]` does break the where-bound E0275 cycle (good — that's why Unparse/Spanned work
direct), but a natural recursive-descent `Parse` then hits a **second, independent** overflow:
`Parse::parse(stream: impl IntoParseStream)` backtracks via `stream.dup(…)`, wrapping the stream in
another `Dup<…>` per level, so `Expr::parse::<S0>` → `Stmt::parse::<S1>` → `Expr::parse::<S2>` …
monomorphizes a strictly-growing stream type `Dup<&mut Dup<&mut Dup<…>>>` → `E0275: …: ParseStream`
overflow. The depth-limited engine is exactly what bottoms this out (finite distinct types). So:

- **Parse MUST go through the engine** (parse engine → convert to natural). Confirmed necessary.
- **Unparse/Spanned** thread a *fixed* emitter `&mut E` (no `Dup`), monomorphize finitely, and were
  spiked working direct on a depth-6 natural tree. So they stay direct leaf-only (no engine).

The §1 hybrid is therefore not a preference but a requirement. (Recorded in memory: `recurse-parse-needs-engine`.)

---

## 11. Implementation order

1. **Re-enable `#[ignore_bounds]`** (honor in the three `extract_*`; keep `audit_ignore_bounds_noop` →
   flip to a passing test). Independent, low-risk.
2. **Spike §10** (leaf-only Parse through substructs) on `minimal`/`shallow`. Decide engine vs. no-engine.
3. **`#[recurse]` natural emission**: stop renaming/aliasing; emit natural types verbatim; route derives
   (`Parse`→engine or direct per step 2; `Unparse`/`Spanned`/`Ast`/rest→natural + injected
   `#[ignore_bounds]`); add §3 finite-size detection + fallback.
4. **Conversion + delegated Parse** (only if the engine is retained).
5. **Visitor collapse** (§6): delete the depth-generic subsystem; `build()` → always `generate_module`.
6. **Metadata** (§7): drop `@recurse`.
7. **Migrate tests** (§8) + add new ones: closure over a former-recurse cycle, tuple-of-closures,
   `visit_mut`, inheritance over a cycle, trait impl on a cycle type, deep-tree `Unparse`, round-trip
   with `#[group]`, multi-root + heterogeneous conversion, finite-size fallback diagnostic.
8. **Docs**: rewrite the CLAUDE.md `#[recurse]`/"Closures over `#[recurse]`" sections — the gap is closed.

---

## Implementation outcome (as landed)

**Done & green** (syan 301 / syan-rust 11, 0 failed):
- `#[recurse]` emits natural public types + `pub(crate)` engine + `__ToNat_*` conversion + delegated
  `impl Parse` (`macro/recurse.rs`: `make_natural_item`/`make_engine_item`/`gen_natural_extras`/
  `conv_body`/`conv_expr`). Public `pub type X` aliases removed.
- **Parse-needs-engine confirmed by spike**: leaf-only direct Parse is impossible (infinite
  `Dup<&mut Dup<…>>` stream-type monomorphization), so Parse delegates through the engine. Recorded in
  memory `recurse-parse-needs-engine`.
- `#[ignore_bounds]` honored (`macro/attribute.rs`); test `ignore_bounds.rs`.
- Finite-size precondition (clean abort on a pure value-cycle); `ui/problem1`,`ui/problem3`.
- Visitor collapsed to the acyclic path: deleted `generate_module_mixed`, `VisitRec`/`VisitRecMut`,
  `@recbase`/`base_is_recurse`, `recurse_lower_*`, `*Node` aliases, `@recurse` metadata + parsing.
- **Closures over recurse cycles work** (`visitor_recurse_cycle.rs::closure_over_recurse_cycle`).
- Heterogeneous concrete-fill → per-method-generic struct-only mode (`has_concrete_fill` in
  `macro/visitor.rs`); union+closure path untouched (`visitor_generics.rs`). Non-shared *lifetime* fills
  stay in union mode (subtyping) — `audit_visitor_recurse_nonroot_lifetime.rs`.
- Cross-crate: foreign inherent `.visit()` skipped (`path_is_crate_local`) — `cross_crate_recurse.rs`.
- Former diagnostics that are **now supported** were converted to passing tests:
  `visitor_mixed_recurse_extra_param.rs`, `visitor_multicycle_disjoint_params.rs`.

**Deferred (one limitation):** `Unparse`/`Spanned` stay on the `pub(crate)` engine; a natural cycle
type is `Parse` (delegated) but not directly `Unparse`/`Spanned`. Group-FREE natural `Unparse` works via
`#[ignore_bounds]` (the mechanism is in place — `ignore_bounds.rs`); group-FUL needs the cycle-wide
union bounds of §5 (the group `Fill<Substruct>: Unparse` bound references derive-internal substruct
names, so each member's impl can't name its siblings' Fill bounds without the derive processing the
whole cycle, or a depth-generic `from_nat` delegation + `Clone`). No test requires it; the engine
retains `Unparse`. Implementation path: §5 (recurse-emitted impls with unioned, shared-nonce
where-predicates) or a `from_nat` mirror of `__ToNat_*`.

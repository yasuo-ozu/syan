# recurse-via-decycle plan — replace syan's engine with the decycle crate

Status: **PLANNED** (nothing implemented; this document is the deliverable). Companion:
`docs/recurse-removal-inventory.md` (exhaustive machinery/test inventory, written separately).
Spikes referenced below live in `scratchpad/recurse-decycle-spikes/{spike_a,spike_b}` (session
scratch; the programs are reproduced in §2.1).

## 0. Motivation & prior art

syan's `#[recurse]` and decycle solve the same problem with the same two-part shape: a
compile-time finite chain that bottoms out a mutually-recursive obligation, plus a runtime
re-entry at the chain's floor that makes real recursion unbounded. decycle 0.4.0's unbounded
shim **is** syan's technique — it was ported from `core/src/parse/vtable.rs` +
`macro/recurse/emit.rs` and then hardened beyond the original (thread-local registry, layout
fingerprint, fail-closed floors; see decycle `docs/unbounded-reentry-plan.md`). Two parallel
implementations of one idea is one too many: this plan removes syan's engine and drives the
generalized implementation instead.

Difference in kind, not technique: decycle ranks **trait obligations** on user-shaped impls;
syan's engine ranks **types** (a `__XxxRec<…, __Rec>` family) because its cycles arise inside
derive-generated impls. The migration therefore keeps syan's derive as the impl *generator* and
hands the generated impls to decycle for cycle-breaking.

## 1. Engine today vs decycle 0.4.0

| Concern | syan engine (`macro/recurse/*`, `core/src/parse/vtable.rs`) | decycle 0.4.0 |
|---|---|---|
| Finite chain | per-SCC engine type family, depth = `DEFAULT_RECURSION_DEPTH` = 4 (`macro/recurse.rs:33`) | one generic ranked impl per (trait, impl); nested-tuple rank, default depth 10, any ≥ 1 |
| Floor | inhabited terminator types re-entering via `vtable::lookup` (`emit.rs:82-86,168-208`) | leaf ranked impl re-entering via `__reentry::lookup` |
| Registry key | `ReKey<terminator, atom, error>`, nonce-stamped terminator (both Parse and, since the audit fix, group-ful U/S — `emit.rs:262-267`) | `type_name` of per-(trait, method, instantiation) marker + layout fingerprint |
| Registry storage | global `Mutex<HashMap>`, copy-out, poison-tolerant (post-audit `vtable.rs`) | **thread-local** `RefCell<HashMap>` — no lock at all, cross-thread contamination impossible |
| Generic-method floors | erased by construction (`&mut dyn ParseStream` / `DynSink`) | fail-closed panic (documented residual) — no erasure hook |
| Registration | all SCC roots, in the delegated impl, before descent (`emit.rs:432-442`) | rule 1 + rule 2, per inductive frame, bound-graph-gated |
| Stream-type growth (`Dup`) | cut at the boundary: re-entry restarts at one fixed `Dup<&mut dyn …>` layer | **not addressed** — see §2.1 (the load-bearing gap) |
| Conversions | `__ToNat`/`__FromNat` engine↔natural (`convert.rs`, 176 lines) | none needed — decycle ranks impls *on the natural types directly* |

Remaining hardening delta after syan's audit backports: thread-local isolation and the layout
fingerprint (syan's key components are all nameable types, so the fingerprint is moot for syan;
thread-locality is a real, small win). The delta the *other* way: decycle has no erasure story
for generic methods — which is exactly what Parse needs — resolved in §2.1 **on the syan side**.

## 2. Gap analysis

### 2.1 The generic-stream gap — spike-settled

`Parse::parse(stream: impl IntoParseStream<Atom = A>)` (`core/src/parse/parse.rs:7`) is generic
over the stream, and every backtracking level wraps it: `dup` yields `Dup<&'a mut Self, Atom>`
(`core/src/parse/parse_stream.rs:57-64`) — a *growing type*, a monomorphization cycle, not a
trait-obligation cycle.

**Spike A (raw decycle on that shape): hard compile failure.** A two-type cycle whose trait
method takes `&mut impl St` with a per-level generic `Dup<S>` wrapper, under
`#[decycle(recurse_level = 2)]`, fails with E0562/E0658 (the generated `__Fp` fn-pointer alias
cannot carry an `impl Trait` parameter) and E0283 at the delegate. Raw decycle cannot express
this trait at all — the question "would the floor's per-instantiation key miss?" is never even
reached. Any migration **must** erase the stream before decycle sees the method.

**Spike B (syan-side erasure adapter): works, zero decycle changes.** The cycle is carried by a
*non-generic* method — `fn parse_dyn(s: &mut dyn St) -> Result<Self, ()>` — with:
- a generic facade erasing once at the top (`parse<S: St>` → `parse_dyn(s as &mut dyn St)`);
- backtracking through a **concrete** `Dup<'a>` over `&mut dyn St`, re-erased at each level
  (mirroring the engine's fixed `Dup<&mut dyn ParseStream>` layer — the tower never grows);
- container edges **peeled inline** by the generated body (`E::Bang(Box::new(F::parse_dyn(d)?))`
  — the bound is `F: Pd`, a decycle-trait bound on a module type, *not* `Box<F>: Pd`, which
  would dangle: `Box`'s blanket impl lives outside the module and gets no ranked chain).

Result: `#[decycle(recurse_level = 1)]`, depth 200 (≈200 floor crossings), plus a whole-tree
deep backtrack — arm 1 parses the entire 200-level spine through re-entry frames, fails on the
final token, rewinds completely, arm 2 re-parses — `SPIKE_B_OK depth=200`. This is the same
property syan's own `deep_backtrack_rewinds_past_reentry_boundaries` test pins on the engine.

**Decision: option (ii), the syan-side adapter.** Option (i) — teaching decycle an
`#[decycle(erase(S = &mut dyn ParseStream))]` hook — is rejected: Spike A shows the generic
method can't even reach decycle's floor machinery, so the hook would have to rewrite the trait
itself (a decycle-side re-implementation of exactly the adapter syan's derive can emit in one
place, with knowledge decycle lacks: which param erases to what object type, how to re-wrap).

Consequences for syan's derive (sized in §4):
- `ParseStream` is already object-safe by design (`parse_stream.rs:1-8`) — no library change.
- `Self::Error` in `parse_dyn`'s signature exercises decycle's alias-projection fix
  (0.4.0, `complex_cycles.rs` regression) — supported.
- The per-field error conversions the derive already emits stay in the generated bodies.

### 2.2 Container/wrapper cycle edges

The cycle edge `Expr → Vec<Expr> / Box<Stmt> / Option<…> / Attempt<…>` must not surface as a
`Container<T>: Parse` where-bound (unsatisfiable through the ranked chain — the std/`nested`
impls live outside the module). The derive's cycle mode lowers container fields **inline**,
reusing the visitor's `peel`/`LayerKind` machinery (`macro/util.rs`) that already classifies
these shapes name-free. Non-cycle fields (leaves) keep ordinary `field_ty: Parse<A>` bounds —
decycle preserves non-cyclic bounds (0.4.0 fix; heterogeneous side-bounds are the fail-closed
residual to watch, see §5 risk R3).

### 2.3 Group-ful Unparse/Spanned (HRTB) — Phase 2, gated

The group cycle is `for<'a> Fill<Substruct<'a>>: Unparse` (HRTB). Code-read of decycle:
`cyclic_where_bounds` (`finalize.rs:702-727`) matches `WherePredicate::Type` without inspecting
`pt.lifetimes`, so a `for<'a>` predicate is *detected* like a plain one, and the ranked rewrite
preserves the binder — mechanically plausible, **unspiked**. The harder problem is the same as
§2.2 in worse form: `Fill`'s `Unparse` impls are generated by the substruct machinery *outside*
any would-be decycle module, so the ranked chain can't thread through them unless syan routes
those impls into the module. Phase 2 begins with a dedicated spike (§5 step 6); until it passes,
group-ful U/S **keeps the borrow engine** (it is only ~2 of the engine's emission paths).

### 2.4 Integration mechanics

- **Driving decycle programmatically.** syan's macro generates impls; it should not round-trip
  through `#[decycle]`-attribute parsing or the carrier-macro ping-pong (Parse's definition is
  statically known to syan). decycle already exports the bridge: `FinalizeArgs` is a plain
  `pub` struct (`finalize.rs:1877-1886`: `working_list`, `traits`, `contents`, `recurse_level`,
  `support_infinite_cycle`, `renames`) and `pub fn finalize(FinalizeArgs) -> TokenStream`
  (`finalize.rs:2202`, re-exported from `decycle::finalize`, currently `#[doc(hidden)]`).
  Wrinkle: `finalize` recovers the `decycle` crate path from a trailing
  `<path>::__finalize` entry in `working_list` — workable but implicit (§3 D1).
- **Dependency shape.** Generated code references `#decycle::__reentry::…`, so the *user* crate
  needs decycle in its graph: syan core re-exports it (`pub use decycle as __decycle;`,
  doc-hidden) and syan passes `::syan::__decycle` as the path. Version pin `=0.4.x` in syan
  (finalize's version handshake is string-equality — irrelevant when bypassing the carrier, but
  keep the pin for coherence).
- **Cross-crate.** The ranked machinery is crate-local to the defining crate, exactly like the
  engine today ("fully internal, in no metadata" — CLAUDE.md); downstream visitors keep seeing
  natural types + the upstream `Parse` impl. The carrier ping-pong is unused, so syan's own
  metadata ping-pong cannot conflict with it.
- **Nonce.** decycle's `name!` suffix is a fixed FNV; scoping is per generated
  `shadowing_module`, and the carrier (per-item discriminant) is unused here. Two `#[recurse]`
  modules in one crate get two shadowing modules — no collision. syan's per-compilation random
  nonce is no longer needed for engine names (they cease to exist).
- **Compile cost.** Engine today, per SCC: a type *family* (root × depth-4 defaults ×
  terminators) + full derive machinery on each engine type + `__ToNat`/`__FromNat` + delegated
  impls. decycle, per (trait × impl): 3 impls (leaf/inductive/final) + per method one marker +
  alias + re-entry fn + prologue; the rank chain is *solver-instantiated*, not emitted. For a
  realistic 5-type single-trait SCC: engine ≈ 5×(4 engine items + conversions + delegation);
  decycle ≈ 5×3 impls + 5 method-item triples. Same order of magnitude, decycle likely smaller
  in emitted tokens; trait-solver work goes up (rank descent × field bounds). Measure at step 3.
- **MSRV.** decycle declares `rust-version = "1.87"` — driven solely by `type-leak → gotgraph`,
  not by anything the migration uses (the marker interning is inert; decycle L-M4). §3 D2
  feature-gates it, bringing decycle to ~1.70 for syan's use.

## 3. Required decycle modifications (all small; none block Phase 1 correctness)

| ID | Size | Change |
|---|---|---|
| D1 | S | Stabilize the programmatic bridge: un-hide `finalize`/`FinalizeArgs`, add an explicit `decycle_path: Option<Path>` field (deprecating the trailing-`working_list` convention), document semver commitment for the bridge surface. |
| D2 | M | Feature-gate `type-leak` (default off for the bridge use): the marker-interning path is inert (L-M4) and is the sole reason for MSRV 1.87. Gated build drops to decycle's own floor (~1.70). |
| D3 | S | Suppress carrier-macro/`#[macro_export]` emission when driven via `finalize` directly (avoid polluting the user crate's macro namespace). Verify nothing is emitted today on this path; if clean, D3 is a test, not a change. |
| D4 | S | Regression tests in decycle mirroring Spike A (compile-fail: generic-stream method gets a clean "erase the generic parameter first" diagnostic instead of E0562 internals) and Spike B (the adapter pattern, as a documented recipe in decycle's docs). |
| D5 | M (Phase 2 only) | Whatever the HRTB/`Fill` spike (§5 step 6) surfaces; expected: none-to-small if impls are routed into the module, per §2.3 code-read. |

## 4. Migration design (syan side)

Per SCC, `#[recurse]` emits after the swap:

1. **Natural types** — unchanged (`make_natural_item`), except `Parse` is no longer removed
   from the derive list; it is redirected to cycle mode.
2. **A cycle module** (nonce-named, `#[doc(hidden)]`) containing: the `parse_dyn`-carrying
   trait (or reuse of a new `syan::parse::ParseDyn<Atom>` library trait — preferred: one shared
   object-safe trait, defined once in core, marked `#[decycle]`-compatible), the generated
   cycle-mode impls (inline-peeled containers, `&mut dyn ParseStream` throughout, per-level
   concrete `Dup`), and the `finalize`-produced ranked output spliced in via
   `decycle::finalize(FinalizeArgs { traits: vec![PARSE_DYN_DEF], contents: generated_impls,
   recurse_level: 4, support_infinite_cycle: true, .. })`.
3. **The public `Parse` facade** per root: `impl Parse<A> for Expr<S> { fn parse(stream: impl
   IntoParseStream<Atom = A>) … { Self::parse_dyn(&mut stream.into_parse_stream() as &mut dyn
   ParseStream<…>) } }` — non-cyclic (one erased call), no decycle involvement.
4. **Deletions** (Phase 1): `emit_terminator_and_reentry` + `emit_delegated_parse` + the engine
   type family for Parse-only SCCs (`transform.rs` 290 lines, `convert.rs` 176, most of
   `emit.rs` ≈ 500 of 738, engine parts of `items.rs`/`build.rs`/`names.rs`);
   `core/src/parse/vtable.rs` (91 lines) **after Phase 2** — its only users are the recurse
   emissions (grep-verified: `parse.rs:8` re-export + `macro/recurse/emit.rs`); Phase 1 keeps
   it for group-ful U/S. Docs/memory to update: CLAUDE.md engine sections, the
   `recurse-parse-needs-engine` memory entry, `docs/recurse-unbounded-plan.md` status note.
5. **Unchanged regardless**: SCC partitioning (`graph.rs`), the by-value finite-size abort,
   group-free `Unparse`/`Spanned` (`#[ignore_bounds]` + `#[predicate_*]` — already engine-free),
   all visitor integration (already engine-agnostic), metadata/`macro_derive` plumbing,
   `Attempt` semantics (its `dup` becomes the concrete erased `Dup`, same as every other
   backtrack point).

## 5. Rollout, gates, risks, GO/NO-GO

Steps (each gated by the named tests in the inventory doc's matrix):

1. decycle D1–D4 land (decycle suite stays green: 65+ tests, trybuild 10/10, clippy 0).
2. syan core: object-safe `ParseDyn` trait + facade plumbing; no consumer yet
   (`cargo test --workspace` unchanged).
3. Derive cycle mode behind an internal flag; one pilot SCC (`recurse_core.rs::basic`) swapped;
   compare emitted-token counts and build time vs engine (gate: `recurse_core.rs`,
   `recurse_traits.rs::parse_unbounded_depth` depth-200).
4. Swap all Parse SCC emission; delete engine-Parse paths (gates: whole `recurse_*.rs` family,
   `visitor_recurse*.rs`, `rust/tests/rustsub_roundtrip.rs::deep_parens_round_trip_is_unbounded`,
   `rust/tests/cross_crate_recurse.rs`, `ui/problem*.rs` + `ui/recurse_*.rs` with regenerated
   `.stderr` where diagnostics legitimately move).
5. CLAUDE.md/docs/memory sync.
6. **Phase 2 gate**: the HRTB/`Fill` spike (group-ful U/S through decycle with substruct impls
   routed in-module). Pass → swap U/S, delete the borrow engine + `vtable.rs`; fail → keep the
   borrow engine permanently (it is small) and close the plan at Phase 1.

Risks:
- **R1** Derived cyclic impls hit decycle's heterogeneous-side-bounds fail-closed skip (leaf
  fields with `where`-bounded params differing across SCC members) → runtime panic instead of
  parse. Mitigation: derive emits the *union* of leaf bounds on every cycle impl (syan already
  computes exactly this union for `#[predicate_*]` — reuse it); step-3 pilot asserts no
  registration was skipped (decycle could expose a `deny_residual` flag — fold into D1).
- **R2** Trait-solver time regression on deep rank descent × wide ASTs. Mitigation: step-3
  measurement; `recurse_level` is tunable per module (engine depth was fixed at 4).
- **R3** `dyn ParseStream` indirect-call overhead per atom vs today's monomorphized fast paths
  (the engine erases only at re-entry; the adapter erases *every* level). Mitigation: step-3
  parse benchmark on the `rustsub` corpus; if measurable, keep monomorphized bodies and erase
  only at the recursion boundary (hybrid: generic inherent fn + `parse_dyn` thin wrapper —
  exactly the engine's cost profile, still decycle-compatible since only `parse_dyn` is ranked).
- **R4** decycle version coupling (bridge API). Mitigation: D1 semver commitment + `=0.4.x` pin.

**GO/NO-GO: GO for Phase 1 (Parse), phased.** The keystone spike (B) proves the mechanism
end-to-end at depth 200 with deep backtracking and needs *zero* decycle core changes; the raw
path (A) is conclusively dead, so the adapter is not optional complexity but the only design.
Phase 2 (group-ful U/S) is genuinely uncertain (HRTB + external `Fill` impls) and is gated on
its own spike — the honest fallback, keeping the small borrow engine, is acceptable
indefinitely. The migration's value is consolidation (~1.5–2k lines of engine machinery deleted,
one hardened cycle-breaking implementation maintained in one place), not new capability; if
step-3 measurements show R2/R3 regressions that the hybrid mitigation can't recover, stopping
after step 3 and keeping the engine is a legitimate outcome this plan explicitly permits.

---

## Outcome (implementation record, post-plan)

Executed on branch `recurse-via-decycle` against decycle 0.4.0 (snapshot worktree). Status:

- **Step 2 — DONE.** Feature-gated scaffolding (`recurse-decycle` on core + macro, forwarded
  `syan-macro/recurse-decycle`); default build/tests byte-identical with the feature off.
- **Step 3 — DONE (green), with a structural narrowing.** `macro/recurse/decycle.rs` +
  `Adt::extract_parse_dyn` emit, per *eligible* SCC, the `#[decycle]` module wrapping, a
  non-generic `__ParseDyn` trait (method-generic atom — a trait-level atom panics at the decycle
  floor; validated), ranked impls reusing the derive's arm/dup skeleton, and the `Parse<A>`
  facade, at `recurse_level = max(1, SCC width)`. Gates: feature-off 311/0 byte-green;
  feature-on 308 functional tests green incl. `parse_unbounded_depth` depth-200 **via decycle**
  (runtime identical, 0.01s; compile +7s one-time decycle dep, no codegen blowup); 3 trybuild
  runners drift by rustc path-qualification only (decycle in the crate graph), goldens not
  regenerated.
- **Eligibility — the structural finding:** only **S-free, group-free** cycles with direct/`Box`
  recursive edges can migrate. Two harness-confirmed blockers: (1) a `#[group]` cross-edge's
  `Fill<Substruct>: Parse` obligation re-enters the cycle *indirectly* through the facade —
  invisible to decycle's direct-bound ranking (the same wall as the Phase-2 Unparse NO-GO, E0275);
  (2) span-tying leaves need `A: Spanned<Span = S>`, unexpressible when the atom is a method
  generic (mandatory) — not on the shared method (names `S`), not on the impl (names `A`), and a
  trait assoc type fails decycle's rank rewrite (E0271/E0433).
- **Phase 2 — NO-GO** (see `docs/recurse-phase2-spike.md`): the borrow engine for group-ful
  Unparse/Spanned is permanent.
- **Step 4 recommendation — NO-GO for engine deletion.** Group-ful and span-tying cycles — the
  majority of real syan ASTs — must keep the engine, so deletion is unachievable and the end
  state would be a permanent hybrid: two Parse codepaths instead of one, negative consolidation
  value. The branch stands as a validated pilot: decycle-backed Parse for the S-free class,
  behind the off-by-default feature, at zero runtime cost. Revisit only if decycle later gains
  (a) indirect/projection-routed obligation ranking and (b) an atom↔span constraint story.

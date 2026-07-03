# recurse-via-decycle Phase-2 gate spike — group-ful Unparse/Spanned through decycle

Gate (plan §5 step 6): can decycle 0.4.0's ranked-trait rewrite break the group-ful obligation
cycle `for<'a> <_ as EmptyGroup>::Fill<Substruct>: Unparse` (syan `macro/attribute/adt.rs:257`),
under the hard constraint **no `Root: Clone`** (borrow the recursive children), replacing syan's
depth-1 borrow engine?

## Verdict (3 lines)

- **NO-GO** for a decycle-only Phase 2. The no-Clone requirement forces the group cross-edge to be
  a **borrowed-substruct HRTB** bound, which hits two independent decycle structural boundaries; an
  **owned-substruct (Clone) variant recurses unbounded** — so the crux is precisely no-Clone/HRTB.
- Kill points: (1) realistic external-`Fill` shape → **E0277** (projection normalizes to an
  out-of-module type with no ranked impl); (2) even routed in-module → **runtime fail-closed panic**
  at depth (bare-type-param / projection-target registration skip); (3) concrete non-projection HRTB
  → **E0261** (decycle's re-entry registration drops the `for<'a>` binder).
- **Recommendation: permanent fallback — keep syan's depth-1 borrow engine** for group-ful U/S. It
  already works, is small (~2 emission paths + `core/src/parse/vtable.rs` 91 LOC), handles the
  no-Clone HRTB re-entry by construction, and the plan (§2.3, §5 step 6) explicitly permits it.

## Setup

- decycle path dep: `/home/yasuo/ghq/github.com/yasuo-ozu/decycle-snapshot` @ `51f6a74` (v0.4.0,
  "Rework unbounded re-entry; … release 0.4.0").
- Spikes: `scratchpad/migrate-phase2-spike/src/bin/{spike1,s1_control,s1_bounded,spike2,s2b_routed,
  s2c_concrete,s2d_noproj,s3_owned}.rs`; `decycle` used via `#[decycle]` attribute (default
  `support_infinite_cycle = true` unless noted).
- Toolchain: `rustc 1.90.0` (build/run); macro expansion via `nightly 1.97.0 -Zunpretty=expanded`.
- Model fidelity: `ext::{EmptyGroupLike, G, Filled, impl B for Filled<Slot>}` mirrors
  `core/src/nested/group.rs` (`EmptyGroup::Fill<Slot>` :80; `impl EmptyGroup for Group<(),O,C> {
  type Fill<Slot> = Group<Slot,O,C> }` :106; `impl<T,S> Unparse for Group<T,..> where T: Unparse`
  :18) — all in `core`, **outside** any decycle module, exactly as the migration would face. `B` =
  Unparse; `Expr` = the group-ful cycle root; `Sub<'a>(&'a [Expr])` = the borrowed substruct; the
  bound `for<'a> <G as EmptyGroupLike>::Fill<Sub<'a>>: B` = adt.rs:257 verbatim in shape.

## Status matrix

| Spike | Shape | Result |
|---|---|---|
| `s1_control` | HRTB cycle, **no decycle** | **E0275** (cycle is genuine; needs decycle) |
| `spike1` | HRTB cycle, bounded type = **in-module** `W<'a>`, unbounded mode | **E0261** (compile) |
| `s1_bounded` | same, `support_infinite_cycle = false` | **compiles + runs shallow** (`(())`) |
| `spike2` | HRTB **projection**, `Fill` impl **external** (realistic) | **E0277 ×20** (compile) |
| `s2b_routed` | route `Filled` + generic `impl<Slot:B> B for Filled<Slot>` **in** | compiles; **PANIC** at depth |
| `s2c_concrete` | route + **concrete** `impl<'a> B for Filled<Sub<'a>>`, projection bound | compiles; **PANIC** at depth |
| `s2d_noproj` | concrete `impl B for Filled<Sub<'a>>`, **non-projection** HRTB bound | **E0261** (compile) |
| `s3_owned` | **owned** substruct (Clone), **non-HRTB** bound | **compiles + UNBOUNDED** (depth 1500, len 3002) |

## Spike-by-spike

### s1_control — the cycle is real
`for<'a> W<'a>: B ⇐ Node: A ⇐ for<'a> W<'a>: B` with no decycle:
```
error[E0275]: overflow evaluating the requirement `for<'a> W<'a>: B`
```
So decycle is genuinely required; not a strawman.

### Spike 1 — HRTB cycle, in-module bounded type → E0261 in re-entry registration
Bounded type `W<'a>` is a module type decycle *can* rank. Default (unbounded) mode:
```
error[E0261]: use of undeclared lifetime name `'a`
  --> spike1.rs:29:19   for<'a> W<'a>: B,
   |   lifetime `'a` is missing in item created through this procedural macro
```
Expansion (`nightly -Zunpretty=expanded`) pins it: the **rank rewrite preserves the binder** —
the inductive impl carries `where for<'a> W<'a>: BRanked<Rank>` intact — but the **Rule-2 re-entry
registration does not**:
```rust
fn __dcl_register_once_A_0…()  {                        // no <'a> declared
    ::decycle::__reentry::register::<…__Mk_B_b…<W<'a>>>( // 'a is FREE here
        …fp_fold(…, size_of::<W<'a>>(), align_of::<W<'a>>()),
        …__Re_B_b…::<W<'a>> as usize);
}
```
Mechanism, pinned:
- `cyclic_where_bounds` (`finalize.rs:702-745`) matches `WherePredicate::Type` but the tuple it
  pushes (`:735-739`) is `(pt.bounded_ty, trait_ident, args)` — **`pt.lifetimes` (the `for<'a>`
  binder) is discarded**, so the returned target `W<'a>` has a now-free `'a`.
- `build_shared_registrations` (`:1735-1774`) emits that target verbatim (`target_tokens =
  quote!(#target)`, `:1749`) into the shared register-once fn, whose generics are the **impl's own**
  (`fn … #{stripped.impl_generics()} ()`, `:2444`, from `remove_cyclic_bounds(&impl_.generics)`
  `:2437`). `impl A for Node` has no `'a`, so `'a` is undeclared → E0261.
- **`s1_bounded` isolates it**: `support_infinite_cycle = false` emits no re-entry machinery
  (`:2426` gates `build_shared_registrations`), and **compiles + runs** (`(())`). So decycle's core
  cycle-breaking handles HRTB; the failure is confined to the unbounded re-entry path — which the
  migration *requires* (bounded mode caps depth at `recurse_level`, the exact thing being removed).

### Spike 2 — realistic projection (external `Fill`) → E0277
Bound `for<'a> <G as EmptyGroupLike>::Fill<Sub<'a>>: B`, `Fill`/`Filled`/`impl B for Filled` in
`ext` (out of module). 20× (rank chain × impls):
```
error[E0277]: the trait bound `Filled<Sub<'a>>: BRanked…<…>` is not satisfied
  = help: the trait `for<'a> …BRanked…<…>` is not implemented for `ext::Filled<cyc::Sub<'a>>`
  = help: the following other types implement …BRanked…:
            `cyc::Expr` implements …    `cyc::Sub<'a>` implements …
note: required for `cyc::Expr` to implement `…BRanked…<…>`
```
The rewrite turns the trait `B`→`BRanked` in place; the projection **normalizes to the external
`Filled<Sub<'a>>`**, for which decycle generated no ranked impl (only in-module `Expr`/`Sub` get
one). This is the plan's §2.3 flagged killer, confirmed: *"`Fill`'s Unparse impls are generated …
outside any would-be decycle module, so the ranked chain can't thread through them."* This is the
**realistic** shape — `Group` lives in `core`.

### s2b_routed / s2c_concrete — route the group impl in → runtime fail-closed panic
Best case the plan offers (D5, §2.3): move `Filled` + its `B` impl into the module so decycle ranks
it. `s2b_routed` (generic `impl<Slot:B> B for Filled<Slot>`) and `s2c_concrete` (concrete
`impl<'a> B for Filled<Sub<'a>>`, projection bound retained) **both compile and run shallow**
(`{{{}}}`, within `recurse_level=10`), then at depth 3000:
```
thread panicked at decycle-snapshot/lib.rs:228:15:
decycle: re-entry fn not registered before the floor was reached. … an impl whose cyclic bound
targets a bare type parameter. Increase recurse_level; …
```
Not unbounded — **fail-closed** (`lib.rs:220-236`, documented `lib.rs:128-133`). Two causes, both
structural:
- the generic group impl `impl<Slot> B for Filled<Slot> where Slot: B` is decycle's documented
  bare-type-param shape → never registers;
- even concrete (`s2c`), the **cross-edge from `Expr` is a projection** `<G …>::Fill<Sub<'a>>`;
  `reachable_side_bounds_ok` → `unify_type_pattern` (`:1199`) can't match a projection self against
  any in-module impl's `self_ty` → `false` (`:1745`) → registration for that edge **silently
  skipped** → floor unregistered → panic. Raising `recurse_level` only moves the cliff; it never
  becomes unbounded (the hard requirement is depth-2000/60, `recurse_traits.rs`).

### s2d_noproj — concrete, non-projection HRTB → E0261 again
Strip the projection entirely (`for<'a> Filled<Sub<'a>>: B`, concrete in-module `Filled` impl):
```
error[E0261]: use of undeclared lifetime name `'a`
   lifetime `'a` is missing in item created through this procedural macro
```
Same Spike-1 mechanism: the HRTB target `Filled<Sub<'a>>` mentions the bound `'a`, dropped by
`cyclic_where_bounds` (`:735-739`), re-emitted into the lifetime-free register-once fn (`:2444`).
(The projection forms 2b/2c dodge E0261 only because their edge is *silently unregistered* instead —
trading a compile error for a runtime panic. Neither reaches unbounded.)

### s3_owned — the crux, isolated: owned (Clone) recurses unbounded
Owned substruct `SubOwned(Vec<Expr>)` (children **cloned**), so the group bound is **non-HRTB**
`Filled<SubOwned>: B` — concrete, in-module, no `for<'a>`:
```
S3 shallow: {{{}}}
S3_OK deep len=3002 (UNBOUNDED with Clone)
```
Depth 1500 crosses `recurse_level=10` ~150×, re-entry works. **decycle CAN break group-ful cycles
and reach unbounded depth — iff the cross-edge is a concrete, in-module, non-HRTB bound, i.e. iff
the substruct is owned = `Root: Clone`.** The delta from s3 (works) to s2d (E0261) is exactly the
`for<'a>` borrowed substruct. The no-Clone constraint is the sole, decisive blocker.

## Root causes (two decycle structural boundaries)

- **(A) HRTB binder dropped in re-entry registration.** `cyclic_where_bounds` discards
  `pt.lifetimes` (`finalize.rs:702-745`, tuple `:735-739`); the register-once fn takes only the
  impl's own generics (`:2437-2444`). Any HRTB cyclic-bound whose target mentions the bound lifetime
  is then either E0261 (concrete target: spikes 1, 2d) or silently unregistered (projection target:
  2b, 2c). The rank *rewrite* preserves the binder (expansion, spike1) — only the *registration*
  path is broken. The plan's §2.3 code-read ("ranked rewrite preserves the binder — plausible") was
  half-right and missed the registration path.
- **(B) Re-entry needs a concrete, in-module, non-projection, non-bare-param self type.** The
  group's `Fill` cross-edge is external (E0277, spike 2), a projection (registration-skip, 2c), and
  its natural impl is generic-over-`Slot` (bare-param skip, 2b). syan's engine sidesteps all of this
  by re-entering the **concrete root** `Expr` through a type-erased fn pointer + a borrow terminator
  (`emit_borrow_terminator_and_reentry`, sink erased to `&mut dyn Emitter` via `DynSink`); decycle's
  registry (keyed by `type_name` + size/align fingerprint, storing one concrete `fn`) has no
  equivalent for a `for<'a>`-quantified / projected / generic re-entry target.

## Required decycle modifications to unblock (not minimal)

| Fix | Size | Effect |
|---|---|---|
| (A) thread HRTB binders through registration: return `pt.lifetimes` from `cyclic_where_bounds`; re-introduce them on the register-once fn / marker / alias / `__Re` fn generics when the target mentions them | **M** | removes E0261 (spikes 1, 2d) — *necessary but not proven sufficient*: unverified whether the lifetime-erased fn pointer then re-enters correctly for a `for<'a>` target (untestable without patching decycle, which is out of scope here) |
| (B1) route the group `Fill` impl into the module **and** have syan emit a **concrete monomorphic** `impl Unparse for Group<Sub,O,C>` per expansion (replace the `EmptyGroup::Fill` HRTB with a direct `Group<Sub,..>` bound) | **L** (syan-side redesign) | needed to escape E0277 (spike 2) and the bare-param skip (2b); still depends on (A) and on the projection→concrete rewrite |
| (B2) alternative: decycle escape hatch `#[decycle(also_rank(…))]` to rank a foreign trait's impl + support higher-ranked re-entry fn pointers | **L** | essentially re-implements syan's borrow-engine re-entry inside decycle |

The minimal viable path is (A:M) + (B1:L) — a decycle patch of unproven sufficiency stacked on a
syan-side group-lowering redesign. That is strictly larger and riskier than the fallback, for a
Phase whose only value is consolidation (plan §5), not capability.

## Recommendation

**Keep syan's depth-1 borrow engine for group-ful Unparse/Spanned, permanently; close the migration
at Phase 1 (Parse).** The evidence is decisive: the no-Clone requirement (the borrow engine's entire
reason to exist) forces a borrowed-substruct HRTB cross-edge, which collides head-on with decycle's
two structural boundaries — E0277 in the realistic external-`Fill` shape, E0261 / fail-closed panic
in every routed variant — while the only configuration decycle handles unbounded (s3_owned) requires
`Root: Clone`, the one thing forbidden. The borrow engine already delivers exactly this (unbounded,
no-Clone, `recurse_traits.rs::group_ful` depth-2000/60, `rustsub_roundtrip.rs` depth-60 w/
backtracking) and is small — the plan scopes it as ~2 of `emit.rs`'s paths plus `vtable.rs` (91
LOC). Phase 1 (Parse) is unaffected: its keystone Spike B stands, Parse is non-HRTB, and `vtable.rs`
simply survives to back the retained group-ful U/S. Proceed with Phase 1; do not attempt Phase 2.

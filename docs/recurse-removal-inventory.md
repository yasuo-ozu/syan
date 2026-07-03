# #[recurse] removal inventory

Inventory (branch `visitor-descent-views` @ 049effd) for replacing syan2's `#[recurse]` depth-engine
with `decycle`-based trait-cycle breaking. Of the 2,383 LOC under `macro/recurse*`, ~1,744 LOC exist
only to defeat the E0275/monomorphization cycles (engine types, terminators, vtable re-entry, `__ToNat`/
`__FromNat`, delegated impls) plus 93 LOC in `core/src/parse` (the vtable); ~457 LOC (SCC partitioning,
natural-type generation, guards, `#[ignore_bounds]`/`#[predicate_*]` injection) are strategy-independent
and ~160 LOC are shared. Test surface: ~260 LOC engine-implementation-specific; ~2,900 LOC of
recurse-touching tests pin observable behavior and must survive a strategy swap verbatim.

---

## 1. Machinery map

Classification key: **(a)** CYCLE-BREAKING — exists only to defeat E0275/monomorphization cycles;
**(b)** INDEPENDENT of the cycle-breaking strategy; **(c)** SHARED/AMBIGUOUS.

### 1.1 `macro/recurse.rs` (297 LOC) — entry + orchestration

| Lines | Item | What it does | LOC | Class |
|---|---|---|---|---|
| 28–33 | `DEFAULT_RECURSION_DEPTH = 4` | fixed engine type-depth | 6 | (a) |
| 35–70 | `recurse()` head | no-args guard (clean error 38–49), module parse, `set_dummy` re-emitting the original module on abort (57–61) | 36 | (b) |
| 71–111 | ref-map build | `pub_types`, `type_refs` (all refs) + `direct_type_refs` (outermost-ctor refs), `find_cycle_sccs`, no-cycle short-circuit | 41 | (b) |
| 113–130 | finite-size guard | pure by-value cycle → `abort!` (would be E0072 on the natural type) | 18 | (b) |
| 132–153 | `type_to_scc`, `item_in_scc`, `item_attrs` | SCC index plumbing | 22 | (b) |
| 154–168 | `scc_has_group` | detects a `#[group]` field in the SCC → decides direct vs. delegated U/S | 15 | (c) — exists because the group `Fill` HRTB defeats `#[ignore_bounds]`; if the replacement breaks that cycle, the distinction vanishes |
| 170–183 | `scc_needs_engine` | engine iff derives `Parse`, or group-ful and derives `Unparse`/`Spanned` | 14 | (a) |
| 185–215 | `parse_types` + `delegated_us` + `DelegSets` init | which types get delegated impls | 31 | (a) |
| 217–232 | `plans` loop | one `build_scc` per SCC → `(TransformCtx, tail tokens)` | 16 | (a) |
| 234–251 | `scc_union_leaf` | union of leaf field types per SCC → feeds `#[predicate_unparse/spanned]` injection | 18 | (b) |
| 253–281 | emission loop | natural item always (`make_natural_item`); engine item + tail only when `scc_needs_engine` (270–277 is the (a) branch, ~8 LOC); non-cycle items (incl. user `impl` blocks) pass through verbatim (279) | 29 | (b) except 270–277 |
| 282–297 | tails + module reassembly | engine tails filtered by `scc_needs_engine` | 16 | (a) tails / (b) reassembly |

Split: ≈ 85 (a) / 195 (b) / 15 (c).

### 1.2 `macro/recurse/graph.rs` (165 LOC) — reference graph + SCC

| Lines | Item | LOC | Class |
|---|---|---|---|
| 3–47 | `collect_refs` / `collect_refs_fields` / `collect_refs_item` — all (nested) type references, keyed on the **first** path segment | 45 | (b) |
| 49–78 | `collect_direct_refs_item` — outermost-ctor (by-value) refs | 30 | (c) — feeds the by-value guard (b) *and* engine root selection (a) |
| 80–129 | `find_cycle_sccs` — Tarjan via `safegraph`, deterministic ordering | 50 | (b) |
| 131–165 | `subgraph_is_cyclic` — induced-subgraph cyclicity | 35 | (c) — by-value guard w/ empty root set (b, recurse.rs:119) *and* the rootless-subcycle engine-termination guards (a, build.rs:238, 338) |

### 1.3 `macro/recurse/names.rs` (69 LOC) — nonce-stamped name builders

| Lines | Item | LOC | Class |
|---|---|---|---|
| 1–48 | `engine_name` (`__XRec_<nonce>`), `term_name` (`__XTerm_`), `default_name` (`__XDefault_`), `to_nat_name`, `from_nat_name`, `reentry_name`, `reentry_fn_alias` (`__ReFn_`), `term_ref_name` (`__XTermRef_`), `reentry_unparse_name`, `reentry_span_name` | 48 | (a) |
| 50–69 | `item_generics`, `where_preds` — generic lookups | 20 | (c) — generic helpers, today called only from emit.rs |

### 1.4 `macro/recurse/transform.rs` (290 LOC) — engine-type rewriting

| Lines | Item | LOC | Class |
|---|---|---|---|
| 3–23 | `TransformCtx` (cycle/root sets, `internal_names`, `rec_params`, `root_rec`, `rec_decls`, `root_ident_args`) | 21 | (a) |
| 25–117 | `transform_type` — back-edge → `__Rec` collapse, cross-edge rename + depth-param append; **non-identity back-edge `abort!`** (40–56, e.g. `Expr<Vec<S>>`) | 93 | (a) |
| 119–133 | `transform_fields` | 15 | (a) |
| 135–146 | `param_key` — kind+name identity for root-param checks | 12 | (a) |
| 148–160 | `generic_tokens` — (decl, use) param token lists | 13 | (c) |
| 162–187 | `param_decls` — bound-preserving impl-header params | 26 | (c) |
| 189–290 | `transform_item` — rename enum/struct to engine name, append depth params. **NOTE:** the `Item::Impl` branch (220–287, ~68 LOC) appears **dead** under the natural-types design: user impl blocks pass through unchanged in recurse.rs:279, and `transform_item` is only called from `make_engine_item` (items.rs:182) on Enum/Struct items | 102 | (a) |

### 1.5 `macro/recurse/convert.rs` (176 LOC) — engine↔natural bridges

Entirely **(a)**: `ConvDir` (`ToNat` by-value / `FromNat` by-ref+`Clone`-leaves, 9–58), `conv_expr`
(container/tuple-recursive field conversion, 60–107), `conv_body` (per-variant match bodies, 109–176).

### 1.6 `macro/recurse/emit.rs` (738 LOC) — terminators, re-entry, delegated impls

| Lines | Item | LOC | Class |
|---|---|---|---|
| 9–97 | `emit_terminator_and_reentry` — owned terminator `__XTerm(Box<Root>)`; `__ReFn` fn-ptr alias over `&mut dyn ParseStream`; `__reentry_X` (top-level parse monomorphized at the dyn stream); terminator `Parse` that `vtable::lookup`s + `transmute`s + calls (the audited `unsafe`, 86–88) | 89 | (a) |
| 99–218 | `emit_borrow_terminator_and_reentry` — borrow terminator `__XTermRef<'a>(&'a Root)`, its `__from_nat`, `__reentry_unparse_X` (erases sink via `DynSink`), `__reentry_span_X` + terminator `Unparse`/`Spanned` via vtable | 120 | (a) |
| 220–292 | `emit_delegated_unparse` — registers re-entries, builds depth-1 borrow engine via `__from_nat`, calls engine `Unparse` | 73 | (a) |
| 294–361 | `emit_delegated_spanned` — analogue, `SpanReentry`-keyed, no erasure | 68 | (a) |
| 363–381 | `leaf_field_types` — leaf-ness probe via `conv_expr` | 19 | (c) — feeds `#[predicate_*]` injection (b, recurse.rs:240–251) *and* `from_leaf_clones` (a, emit.rs:570) |
| 383–503 | `DelegTarget`, `RootReentry`, `DelegSets`, `RootData` structs | 121 | (a) |
| 505–738 | `gen_natural_extras` — per cycle type: `__ToNat_X` trait+impl, `__FromNat_X<'__n>` (group-ful), delegated `Parse`/`Unparse`/`Spanned`, `where_preds_of` threading, terminator `__to_nat` (Box unwrap, 705–725), span-param = first type param convention (647–650) | 234 | (a) |
| 407–483 | `emit_delegated_parse` — registers all roots' re-entries into `vtable`, inner `__run` naming `__St` so `__St::Error` is spellable, parses `engine_default`, `.__to_nat()` | (counted above) | (a) |

### 1.7 `macro/recurse/build.rs` (420 LOC) — per-SCC engine construction

Entirely **(a)**:
- `build_scc` (9–308): root selection (self-referential first, else most-directly-referenced,
  20–74); root-generics extraction + **missing-root-param `abort!`** (124–153); `effective_roots` +
  **identity back-edge arg table** (155–186); per-root depth params `__Rec`/`__Rec<Root>` + defaults
  (188–216); single-root **rootless-subcycle guard** (232–247); depth chain `__XRec<…,__XRec<…,Term>>`
  (249–263); `gen_natural_extras` call (290–305).
- `build_multiroot_tail` (310–420): multiroot rootless-subcycle guard (338–346), **roots-must-share-
  exact-params `abort!`** (351–369), per-root terminators, mutual depth chains (388–412).

### 1.8 `macro/recurse/items.rs` (228 LOC) — natural/engine item construction

| Lines | Item | LOC | Class |
|---|---|---|---|
| 3–15 | `derive_attr_name` — `#[derive]` / `#[macro_derive]` recognition | 13 | (b) |
| 17–54 | `split_cycle_derives` — derive-list rewriting (kept-on-natural vs engine-routed paths) | 38 | (b) — the *routing target* is strategy-dependent, the split mechanism is not |
| 56–65 | `derives_any` | 10 | (b) |
| 67–81 | `strip_field_helper_attrs` — drop `#[group]`/`#[ignore_bounds]`/… from a natural type carrying no structural derive | 15 | (b) |
| 83–156 | `make_natural_item` — un-renamed public type; engine-routes `Parse` (always) + U/S (group-ful); injects `#[predicate_unparse/spanned(<leaf union>)]` (115–121) and `#[ignore_bounds]` (via 158–174) for group-free direct U/S | 74 | (b) |
| 158–174 | `inject_ignore_bounds` — mark recursive-child fields | 17 | (b) |
| 176–228 | `make_engine_item` — `transform_item` + `pub(crate)` + engine derives; strips `#[ignore_bounds]`/`#[seq]`/`#[opt]` from engine fields (198–211) | 53 | (a) |

### 1.9 `core/src` — runtime support

| File | Item | LOC | Class |
|---|---|---|---|
| `core/src/parse/vtable.rs` | whole file: `ReKey<T,A,E>` (type_name-keyed marker, 47), `SpanReentry` (51), `DynSink` (58–68), `REG: OnceLock<Mutex<HashMap<&str,usize>>>` (70), `register`/`lookup` (75–91) | 91 | (a) |
| `core/src/parse.rs:7–8` | `#[doc(hidden)] pub mod vtable;` | 2 | (a) |
| `core/src/parse/parse_stream.rs:3–7, 29–36, 56–63, 128` | `ParseStream` object-safety accommodations: doc states object safety exists "to type-erase the stream at the unbounded-`#[recurse]` re-entry boundary"; `dup`/`validate_spacing` carry `where Self: Sized`; `impl<T: ?Sized> ParseStream for &mut T` enables `&mut dyn` | ~10 (of 146) | (c) |
| `core/src/parse/parse_stream.rs:56–126` | `dup` + `Dup<Slot, Atom>` — the backtracking wrapper itself | ~70 | (b) — the backtracking primitive is independent; its *per-level type growth* is one of the cycles the engine breaks |
| `core/src/parse/into_parse_stream.rs:10–20` | blanket `impl<T: ParseStream> IntoParseStream for T` — lets the re-entry fn feed `&mut dyn ParseStream` back into `Root::parse` | 11 | (c) — generally used, but load-bearing for re-entry |

### 1.10 Adjacent proc-macro code (not in `macro/recurse/`)

| File | Item | Class |
|---|---|---|
| `macro/lib.rs:196–198` | `#[proc_macro_attribute] recurse` wiring + nonce (`random()`, 14–19, shared) | (b) wiring; docs 127–195 describe the (a) strategy |
| `macro/attribute/find.rs:84–101` | `strip_param_defaults` — documented reason: the engine's `__Rec = __ExprDefault<S>` default must be stripped from derive impl headers; generically correct behavior | (c) |
| `macro/attribute/adt.rs:510–524` | enum-`Parse` prefix-dedup deliberately scoped so "every recurse-engine enum — keeps the per-variant-`dup` scheme byte-identical" | (c) comment-level coupling |
| `macro/attribute/*` (`ignore_bounds`, `predicate_unparse`/`predicate_spanned` handling; declared in `macro/lib.rs:39,72,112`) | consumed by the group-free direct path; the attributes are general-purpose derive features | (b) |

### 1.11 Docs (strategy documentation, superseded by a swap)

`docs/recurse-unbounded-plan.md` (279), `docs/recurse-natural-types-plan.md` (403 — mixed: natural-type
design is (b), engine parts (a)), `docs/recurse-deferred-fixes-plan.md` (184),
`docs/spike_unbounded_recurse.rs` (294 — standalone spike of the vtable mechanism). ≈1,160 lines.

---

## 2. The three delegated traits — the obligation cycles a replacement must break

### 2.1 `Parse` (always delegated) — TWO independent cycles

**(i) Per-field where-bound cycle (E0275).** The `Parse` derive adds a `field_ty: Parse` predicate per
field. `docs/recurse-unbounded-plan.md` ("Why the limit exists"): *"The derive adds a `field_ty: Parse`
predicate per field, so `Expr: Parse ⇐ Stmt: Parse ⇐ Expr: Parse …` is an infinite `where`-clause
(E0275)."* (Same statement: `macro/lib.rs:138–140`, CLAUDE.md `#[recurse]` section item (a).)

**(ii) Backtracking stream-type monomorphization (Parse-only, NOT a where-clause cycle).** Derived enum
`Parse` backtracks via `stream.dup(|s| …)` (`macro/attribute/adt.rs:524`), and `dup` wraps the stream in
a fresh `Dup<&'a mut Self, Atom>` (`core/src/parse/parse_stream.rs:56–89`). CLAUDE.md: *"backtracking
`stream.dup(…)` wraps the stream in another `Dup<…>` per descent level → infinite stream-type
monomorphization (also E0275)."* `docs/recurse-unbounded-plan.md`: *"Recursive descent then monomorphizes
`parse::<Dup<&mut Dup<&mut …>>>` with a strictly-growing stream type."*
⚠ **A trait-obligation cycle-breaker alone (ignore_bounds-style, or decycle's) does not touch (ii)** —
it is monomorphization growth, not a solver cycle. Today it is cut by the erased re-entry restarting at
one fixed `Dup<&mut dyn ParseStream>` layer (CLAUDE.md: *"the erased re-entry restarts at one fixed
`Dup<&mut dyn …>` layer that never grows"*).

**Today's defusal:** fixed-depth engine (each level a distinct finite type — bottoms out both cycles at
compile time) + inhabited terminator `__XTerm(Box<Root>)` whose `Parse` carries **no `Root: Parse`
bound** (emit.rs:7–8: *"that would re-form the E0275 where-cycle the engine exists to break"*) and
re-enters the top-level natural `Parse` at runtime via `vtable::lookup::<ReKey<Term, Atom, St::Error>>()`
+ `transmute` (emit.rs:64–95); the delegated impl registers all roots' re-entries first
(emit.rs:441–451, 474).

**Replacement requirements (Parse):**
1. Break the per-field where-bound cycle for mutually-recursive types (multi-type SCCs, multi-root SCCs).
2. Cap the `Dup<…>` stream-type growth — some erasure/fixed-point of the stream type at recursion
   boundaries, or an alternative backtracking scheme.
3. Preserve `type Error = ParseError` on the public impl (emit.rs:460, 69) and the
   `impl IntoParseStream` entry signature.
4. Preserve backtracking correctness **across** recursion boundaries: a late failure after a deep
   successful sub-parse must rewind the whole descent (`recurse_traits.rs:274–334`
   `deep_backtrack_rewinds_past_reentry_boundaries`, 120-deep; `rustsub_roundtrip.rs:48`).
5. Unbounded runtime depth (`recurse_traits.rs:38–53` depth-200), left recursion allowed to recurse to
   OS-stack exhaustion (documented semantics).
6. Cross-crate: the impl must land in the defining crate so downstream `Expr<S>: Parse` just works
   (`rust/tests/cross_crate_recurse.rs`); if any runtime registry survives, keys must stay unique per
   expansion across two linked versions of one crate (vtable.rs:19–26 nonce-stamping rationale).

### 2.2 Group-ful `Unparse` (delegated only when the SCC has a `#[group]` field)

**The cycle.** The structural derive on a type with `#[group(self.x)]` synthesizes, per group field
(`macro/attribute/adt.rs:257`):
```rust
for<'syan_substruct_ref> <#field_ty as #syan::nested::group::EmptyGroup>::Fill<#fill_ty>: #trait_fullpath
```
For a self-recursive group field the `Fill<Substruct>` projection contains the cycle, so this HRTB forms
a trait-solver cycle. `macro/recurse.rs:154–158`: *"a self-recursive group field's derive-generated
`for<'a> Fill<Substruct>: Unparse` bound forms a trait-solver cycle that `#[ignore_bounds]` can't break
(it only suppresses the per-field bound, not the group `Fill` bound)."* (Group-free cycles hit only the
per-field bound cycle and are already solved WITHOUT the engine — see §2.4.)

**Today's defusal:** depth-1 **borrow** engine — `__XRec<…, __XTermRef<'_,…>>` built by `__FromNat_X<'__n>`
(leaves `Clone`d, recursive children borrowed — no `Root: Clone`; emit.rs:127–136, gen_natural_extras
672–693); the engine's distinct finite types let the derive's HRTB discharge; the borrow terminator's
`Unparse` re-enters the top-level natural impl via vtable with the sink erased to `&mut dyn Emitter`
re-wrapped by `DynSink` (emit.rs:138–183; vtable.rs:53–68).

**Replacement requirements:** break the `Fill` HRTB cycle (not just per-field bounds); no `Root: Clone`
requirement; unbounded depth (`recurse_traits.rs:216–230` depth-60 round-trip); output identical
(a brace group is ONE `TokenTree::Group` — `recurse_traits.rs:204–213`); generic `unparse<E: Emitter>`
signature unchanged (`core/src/parse/unparse.rs:3–5`).

### 2.3 Group-ful `Spanned`

**The cycle:** same shape as 2.2 — the per-field `field_ty: Spanned<Span = _>` bound cycle plus the group
`Fill` HRTB for `Spanned` (same `adt.rs` predicate with `trait_fullpath = Spanned`); additionally the
dropped per-field predicate is what pins the associated `Span` type (`macro/lib.rs:101–104`).

**Today's defusal:** same depth-1 borrow engine; `Spanned` needs no erasure — a plain
`fn(&Root<…>) -> S` fn pointer keyed by `ReKey<TermRef, SpanReentry, S>` (emit.rs:185–216, 297–361).
The span type is pinned to the cycle's **first type param** by convention (emit.rs:647–650:
*"The cycle's span type is its first type param (recurse convention)"*) so the private engine type never
leaks into the public assoc type (E0446).

**Replacement requirements:** break both cycles; keep `type Span = S` public (no private-type leak);
unbounded (`recurse_traits.rs:250–272` depth-2000); the delimiters-provide-the-span behavior for empty
group slots (`nested/group.rs` — a library feature, untouched).

### 2.4 For contrast: what already works WITHOUT the engine (the in-repo prior art)

Group-free `Unparse`/`Spanned` are derived directly on the natural type: `#[ignore_bounds]` injected on
recursive-child fields (items.rs:158–174) drops the per-field bound; the injected item-level
`#[predicate_unparse/spanned(<SCC leaf-type union>)]` (items.rs:115–121; union computed
recurse.rs:234–251) re-adds exactly the leaf bounds each member's body needs. This is the existing
proof that a bound-suppression strategy suffices when the only cycle is the per-field where-bound —
i.e. the decycle question reduces to §2.1(ii) and the §2.2/§2.3 `Fill` HRTB.

---

## 3. Public/observable surface that must not change

1. **Natural public types, one type at all depths.** The user's enums/structs own their names (no
   `pub type` alias); `Ast`/`Debug`/`Default`/docs/`#[subast]` land on them; user inherent/trait `impl`
   blocks pass through verbatim (recurse.rs:279; pinned by `ui/problem1_trait_impl.rs` and
   `recurse_core.rs::basic` methods).
2. **Unbounded depth for all three traits** — Parse (depth-200 test), group-free U/S (5000/2000),
   group-ful U/S (60/2000), cross-crate group-ful w/ backtracking (depth-60).
3. **Clean `abort!`s with the original module re-emitted** (`set_dummy`, recurse.rs:57–61 — no
   cascading "cannot find type" errors):
   - by-value cycle (recurse.rs:122–129; `ui/problem1`, `ui/problem5`) — **inherent to natural types, must stay**;
   - `#[recurse]` takes no args (recurse.rs:38–49; `ui/recurse_takes_no_args.rs`) — must stay;
   - pub-only cycle detection (recurse.rs:74–79; `ui/problem3`) and first-segment path keying
     (graph.rs:6–10; `ui/problem7`) — current documented scope;
   - missing-root-param (build.rs:142–152), non-identity root back-edge (transform.rs:40–56),
     rootless sub-cycle (build.rs:238–247, 338–346), multiroot exact-params (build.rs:360–368) —
     **engine constraints**; a replacement may lift them (design decision), but if kept the messages
     change (all four stderr goldens name "depth machinery"/"`__Rec`"/"depth recursion").
4. **`visitor!(…)` over a cycle is an ordinary acyclic visitor.** Nothing in `macro/visitor*` or
   `core/src/visit.rs` references recurse internals (verified by grep; only comments). Metadata is the
   natural type's plain `#[derive(Ast)]` macro. Closures, tuples, `visit_mut`, inheritance,
   drill-in, containers all pinned by `visitor_recurse*.rs` / `recurse_visitor_cycles.rs`.
5. **Cross-crate:** the delegated `Parse` (and group-ful U/S) impls are emitted in the **defining**
   crate, so a downstream crate parses via upstream `Expr<S>: Parse` with no orphan issue
   (`rust/tests/cross_crate_recurse.rs`, `rust/tests/rustsub_roundtrip.rs`); generated helpers are
   `pub(crate)`/`#[doc(hidden)]` and appear in no metadata.
6. **Name hygiene:** all generated items nonce-stamped (names.rs:3–8) — a user type named `ExprTerm`
   coexists (`recurse_core.rs::no_engine`). A no-engine outcome satisfies this vacuously.
7. **Left recursion recurses forever** (OS-stack ceiling) rather than being silently truncated —
   documented at `macro/lib.rs:144–146`, CLAUDE.md, `recurse_core.rs:313–315` comment.
8. **`#[macro_derive]` recognition** (items.rs:7–15) for cycles with `Token![…]` type-macro fields
   (`rust/src/rustsub.rs:76–78`).
9. **Root-param / identity-back-edge restrictions are *currently documented* user-facing constraints**
   (`macro/lib.rs:157–175`) — observable, but engine-caused; removal would be a compatible relaxation.
10. Misc: `recurse_core.rs:431–432` comments claim a "non-conventional span param warning"; no warning
    is emitted anywhere in `macro/recurse` (stale comment — the first-type-param convention is silent).

---

## 4. Test coverage matrix

"Verbatim" = survives a strategy swap unchanged (behavioral pin). "Engine-specific" = pins the current
mechanism; rewrite/delete under a swap. Stale comments noted where the test body still survives.

| File (LOC) | Module / test | Pins | Verdict |
|---|---|---|---|
| `core/tests/recurse_core.rs` (457) | `basic` (7–194) | natural types, Parse round-trips, user inherent impls, multi-param threading | **verbatim** (comments at 150–152 name `__ExprRec`) |
| | `fixes` (198–278) | generic/non-generic cycles compile (bug6); foreign type sharing a cycle last segment is a leaf (bug7, visitor) | **verbatim** (bug6's *intent* was terminator generics; assertion is natural-type construction) |
| | `no_engine` (282–339) | Ast-only cycle gets no engine; nonce-stamped names don't collide with user `ExprTerm` | **engine-specific intent** (~58 LOC) — assertions become vacuous but still pass; keep or fold |
| | `where_clause` (343–400) | where-clause threading (param + self-referential bounds) through generated impls | **verbatim** |
| | `problems` (403–457) | trybuild driver + non-conventional span param compiles | driver survives; see ui rows |
| `core/tests/recurse_traits.rs` (379) | `unparse_spanned` (6–179) | group-free direct U/S; **`parse_unbounded_depth` (depth-200)**; direct unparse depth-5000; multi-type leaf-union; multi-type Spanned | **verbatim** — these are the requirement pins |
| | `group_ful` (182–335) | single-`TokenTree::Group` unparse; depth-60 unbounded round-trip; depth-2000 span; **`deep_backtrack_rewinds_past_reentry_boundaries`** (120-deep late-failure rewind) | **verbatim** (name/comments reference re-entry; behavior is strategy-independent) |
| | `ignore_bounds` (339–379) | the raw `#[ignore_bounds]` primitive, no `#[recurse]` | **verbatim** — independent |
| `core/tests/recurse_visitor_cycles.rs` (349) | `generics` (lt/ct/ct_char/multi/het/base), `multi_cycle`, `multiroot` | visitor over natural types w/ lifetime/const/heterogeneous params; independent SCCs; multiroot | **verbatim** (test name `each_root_keeps_its_own_depth` (332) is stale flavor — it counts visits) |
| `core/tests/visitor_recurse.rs` (174) | `via_visitor`, `disjoint_params` | struct/closure/mut visitors, disjoint-param union | **verbatim** |
| `core/tests/visitor_recurse_mixed.rs` (353) | `one_visitor`, `extra_param`, `closure`, `drill` | mixed acyclic+cycle visitors, drill-in | **verbatim** |
| `core/tests/visitor_recurse_shapes.rs` (147) | `containers`, `container_of_tuple` | container/tuple descent through a cycle | **verbatim** |
| `core/tests/visitor_audits.rs` (56 of 120) | `recurse_helper_hygiene` (65–87), `recurse_nonroot_lifetime` (91–120) | helper-param hygiene; non-root lifetime | **verbatim** |
| `core/tests/visitor_inherit.rs` (~190 of 465) | `over_recurse` (276–387), `over_recurse_mid` (388–465) | inheritance over a cycle base | **verbatim** |
| `ui/problem1_trait_impl.rs` (26 + 97 stderr) | by-value guard + impl passthrough + rustc cascade on dummy | **verbatim** (stderr = rustc output on the re-emitted module; fragile to message-wording only) |
| `ui/problem3_pub_crate.rs` (19 + 83) | pub-only detection | **verbatim** |
| `ui/problem5_multiple_roots.rs` (28 + 171) | by-value guard, two-type | **verbatim** |
| `ui/problem7_multiseg_path.rs` (26 + 83) | first-segment ref keying | **verbatim** |
| `ui/recurse_takes_no_args.rs` (20 + 5) | args rejected | **verbatim** |
| `ui/recurse_missing_root_param.rs` (32 + 5) | root-param superset requirement; stderr: "…so the **depth machinery** can thread them through" | **engine-specific** |
| `ui/recurse_complex_root_param.rs` (24 + 37) | identity back-edge; stderr names "`__Rec`"; header states it fires only "when the cycle needs the engine" | **engine-specific** |
| `ui/recurse_multiroot_rootless_subcycle.rs` (48 + 5) | feedback-vertex-set guard; stderr: "the **depth recursion** … would not terminate" | **engine-specific** |
| `ui/recurse_rootless_subcycle_single_root.rs` (45 + 5) | same guard on the single-root path | **engine-specific** |
| `ui/visitor_recurse_unlisted_coroot.rs` (36 + 10) | omitted co-root → clean drill diagnostic | **verbatim behavior**; header comment (VisitRec depth dimensions) is stale |
| `rust/tests/cross_crate_recurse.rs` (39) | downstream visitor over upstream cycle | **verbatim** |
| `rust/tests/rustsub_roundtrip.rs` (111) | full-stack group-ful round-trip; `deep_parens_round_trip_is_unbounded` (depth-60, multi-type, w/ backtracking); visitor + closure | **verbatim** |
| `rust/src/rustsub.rs` (130) + `rust/src/lib.rs::recursed` (~20) | sample sources (`#[macro_derive]`, group-ful cycle, upstream no-visitor cycle) | **verbatim** (comments at rustsub.rs:66–78 & lib.rs:63–66 are engine/stale-`@recurse`-flavored) |

**Totals:** engine-specific ≈ 149 LOC ui `.rs` + 52 stderr + ~58 (`no_engine`) ≈ **260 LOC**; surviving
behavior pins ≈ **2,900 LOC** (2,105 core test + 155 surviving ui `.rs` + 439 surviving stderr + 150
rust tests + 150 rust sample src). Numerous stale *comments* to sweep even in surviving files.

---

## 5. Dependency edges

### 5.1 Outside → recurse internals

- **`core/src/parse/vtable.rs` (whole file)** — exists solely for generated re-entry code; exported
  `#[doc(hidden)]` at `core/src/parse.rs:8`. Its only consumers are tokens emitted by
  `macro/recurse/emit.rs` (`::syan::parse::vtable::{register, lookup, ReKey, DynSink, SpanReentry}` at
  emit.rs:82–84, 168–170, 204–206, 267–269, 337–339, 446–448). **No hand-written code in any crate
  calls it.** Removable with class (a).
- **`core/src/parse/parse_stream.rs:3–7`** — `ParseStream` object safety is documented as existing for
  the re-entry erasure; `dup`/`validate_spacing` carry `where Self: Sized` for it;
  `impl<T: ?Sized> ParseStream for &mut T` (:128) enables `&mut dyn`. Harmless to keep.
- **`macro/attribute/find.rs:84–101` `strip_param_defaults`** — doc cites the engine's
  `__Rec = __ExprDefault<S>` default as the motivating case; generically correct (keep).
- **`macro/attribute/adt.rs:510–524`** — prefix-dedup scoping comment promises recurse-engine enums keep
  the per-variant-`dup` codegen byte-identical (comment-level only).
- **`macro/lib.rs`** — wiring (196–198) + ~70 lines of `#[recurse]` docs (127–195) describing the (a)
  strategy; `parse_derive`/`unparse_derive`/`spanned_derive` doc references (33–36, 66–68, 107–109).
- **Tests** touch internals only behaviorally (no test names an engine type; `recurse_core.rs::no_engine`
  probes hygiene via a user `ExprTerm`).
- CLAUDE.md, `docs/recurse-*.md`, memory file `recurse-parse-needs-engine.md` — documentation.

### 5.2 Recurse ← rest of workspace

- `macro/util.rs`: `first_ty_arg` (:112, used by convert.rs:77–81), `param_tokens` (:56, via
  transform.rs:151–160) — shared utils, stay.
- **`safegraph`** (`macro/Cargo.toml:29`): `tarjan_scc` + `is_cyclic_directed` (graph.rs:87–89,
  140–142) — used by class (b)/(c) code; **stays**.
- **Structural derives** (`macro/attribute/*`): the engine items carry `#[derive(Parse, Unparse,
  Spanned)]` / `#[macro_derive(…)]` (make_engine_item); the group-free direct path relies on
  `#[ignore_bounds]` + `#[predicate_unparse/spanned]` (declared `macro/lib.rs:39,72,112`; stripped list
  find.rs:56). A decycle-based path presumably leans harder on these.
- `type-macro-derive-tricks` (`core/Cargo.toml:25`; items.rs:7–15) for `Token![…]` fields.
- **Core runtime types named by generated code** (emit.rs): `syan::parse::{Parse, IntoParseStream,
  ParseStream}`, `syan::parse::unparse::Emitter`, `syan::error::ParseError`,
  `syan::span::{Span, Spanned}`, `syan::parse::vtable::*`.
- `proc_macro_error` (`abort!`, `set_dummy`), `template_quote`, `syn`; the per-expansion `nonce` from
  `macro/lib.rs::random()` (also used by other derives).

---

## 6. Line-count summary

| Bucket | LOC | Breakdown |
|---|---|---|
| **(a) removable under an ideal swap** | **≈ 1,837** | macro/recurse: build.rs 420 + emit.rs 719 + convert.rs 176 + transform.rs 243 + names.rs 48 + items.rs 53 + recurse.rs 85 ≈ **1,744**; core: vtable.rs 91 + parse.rs decl 2 = **93**. (Includes ~68 LOC already dead: `transform_item`'s `Item::Impl` branch.) |
| **(b) retained (strategy-independent)** | **≈ 457** | recurse.rs orchestration/guards 195 + graph.rs 95 + items.rs natural-type generation 167 |
| **(c) shared — re-home case-by-case** | **≈ 160** | `subgraph_is_cyclic` 35 + `collect_direct_refs_item` 30 + `generic_tokens`/`param_decls` 39 + `leaf_field_types` 19 + `scc_has_group` 15 + names.rs helpers 20 (+ ~10 `parse_stream.rs` accommodations + 18 `strip_param_defaults` outside the tree) |
| **Test LOC affected (rewrite/delete)** | **≈ 260** | ui: `recurse_missing_root_param` 32, `recurse_complex_root_param` 24, `recurse_multiroot_rootless_subcycle` 48, `recurse_rootless_subcycle_single_root` 45 (+52 stderr); `recurse_core.rs::no_engine` ~58 |
| **Test LOC that must survive verbatim** | **≈ 2,900** | all other recurse-touching tests (behavior pins), incl. the unbounded-depth and deep-backtrack requirement tests |
| **Docs superseded** | ≈ 1,160 lines | `docs/recurse-{unbounded,natural-types,deferred-fixes}-plan.md` + `docs/spike_unbounded_recurse.rs` |

Sanity: (a)+(b)+(c) within `macro/recurse*` = 1,744 + 457 + ~158 ≈ 2,359 of 2,383 (remainder: imports/
mod decls, recurse.rs:1–26).

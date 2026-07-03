# syan reduction plan — verified findings

Produced by a 49-agent survey + adversarial-verify workflow over the worktree `../syan2-reduce`
(snapshot of `visitor-descent-views` incl. uncommitted work). Every finding below either passed
an adversarial verification agent (`verified`) or is in the low-risk comment/dedup class
(`unverified`). Refuted proposals are listed at the end with the refutation — do NOT apply them.
The three largest dead-weight claims were independently re-checked by the coordinating agent
(noted inline as `Spot-checked`).

**Kept findings: 78 — estimated ~7884 lines saved. Refuted: 3.**

Execution: in the `../syan2-reduce` worktree, per slice — `dead-code` and test merges first
(largest, most isolated), then `dup-code`/`emit-reduction`, `comment` sweeps last. After each
slice: `cargo build && cargo test --workspace && cargo clippy` must stay green; trybuild
`.stderr` regeneration (`TRYBUILD=overwrite`) only where a UI test's message text was
intentionally touched.


## Slice `repo-dead-weight` — 4 findings, ~5263 lines

### rust_old : entire tree (18 .rs files + Cargo.toml + spec.md)  [dead-code] (~3732 lines, risk low, verified)
  Delete the whole rust_old/ directory. Companion edit: drop or reword the archival comment at
  rust/src/lib.rs:3 ("previous contents are preserved in rust_old/ at the repo root") to point at
  git history instead (commit ea51aa6).
  - evidence: Root Cargo.toml: members = ["core", "macro", "rust"] — rust_old is not a member. Grep for 'rust_old' across all .toml/.rs/.lock outside the tree hits only the comment rust/src/lib.rs:3. No path-dep (core deps: syan-macro; macro: none local; rust deps: ../core, ../macro), no include!/#[path] references (grep exit 1). Cargo.lock's single 'syan-rust' entry (line 524) is the member rust/ crate. Tree is git-tracked (created by ea51aa6, 2026-06-22, when rust/ was archived) so deletion is fully recoverable from history. The old rationale for keeping it (CLAUDE.md.old: rust_old/src/visit.rs as the hand-written visitor prototype) is spent — the visitor system is shipped per CLAUDE.md.
  - verifier: Confirmed dead: rust_old/ is not a workspace member (cargo metadata --locked lists only core/macro/rust; the Cargo.lock syan-rust entry maps to rust/Cargo.toml despite the duplicate package name), whole-tree grep for rust_old hits only rust/src/lib.rs:3 (the comment the proposal amends) and the archived CLAUDE.md.old, no include!/#[path]/path-dep/symlink/CI references, tree is git-tracked and recoverable (added 8ce2e04, archived at ea51aa6), and the 3732-line/18-file size claim is exact. Minor evidence imprecision only: CLAUDE.md.old also greps for rust_old (7 mentions) — acceptable dangling refs in an archival doc, optionally reworded in the same companion edit.
  - Spot-checked: root workspace members are exactly [core, macro, rust]; rust_old/ is not a member and its only inbound reference is the doc-comment rust/src/lib.rs:3 — update that line on deletion.

### haskell : entire tree (5 src .rs + 2 test .rs + Cargo.toml)  [dead-code] (~778 lines, risk low, verified)
  Delete the whole haskell/ directory (legacy demo crate from the predecessor 'syan' project
  generation).
  - evidence: Not a workspace member (root Cargo.toml members = core/macro/rust). Grep for 'haskell' across every .toml/.rs/.lock outside the tree: zero hits. 'syan-haskell' absent from Cargo.lock, so it has never been built as part of this workspace. Last touched 2025-08-08 (git log -1 -- haskell/ → 81907ec), ~11 months stale; its Cargo.toml repository field still points at github.com/yasuo-ozu/syan (the old repo). Git-tracked → recoverable.
  - verifier: Attempted refutation failed on every axis: haskell/ is absent from workspace members (core/macro/rust only), 'haskell'/'syan-haskell' has zero grep hits outside the directory (checked all .rs/.toml/.md/.lock incl. CLAUDE.md, docs/, rust_old/), it is missing from Cargo.lock (never built in this workspace), no crate path-depends on it (dependency direction is haskell -> core/macro only), its Cargo.toml still points at the old github.com/yasuo-ozu/syan repo, last commit touching it is 81907ec (2025-08-08), all 8 files are git-tracked (recoverable), and the 778-line count is exact. Pure orphan deletion; cannot affect any workspace build or test.
  - Spot-checked: haskell/ is a standalone crate (syan-haskell), not a workspace member; no inbound references.

### CLAUDE.md.old : 1-459  [dead-code] (~459 lines, risk low, verified)
  Delete CLAUDE.md.old, the pre-rewrite snapshot of the project instructions ('# Implementation
  plan (visitor system)' vs the current '# Visitor system — current state').
  - evidence: Grep for 'CLAUDE.md.old' across the worktree: zero hits (exit 1). It was snapshotted by commit ea51aa6 (2026-06-22) alongside the rust_old/ archive and fully superseded by the rewritten CLAUDE.md (diff shows a wholesale rewrite from line 1). Git-tracked → recoverable from history. It also contains the now-obsolete claim that rust_old/src/visit.rs is the prototype to generate from — stale guidance that could mislead future agents.
  - verifier: Verified: CLAUDE.md.old (459 lines) has zero references anywhere in the worktree (grep exit 1, docs/ included) and in the user's memory dir; Claude Code never auto-loads a *.md.old file; it is git-tracked, added in exactly one commit (ea51aa6, alongside the rust_old/ archive) and unmodified vs HEAD, so it is fully recoverable from history. Content is a superseded implementation plan (stale guidance: rust_old/src/visit.rs as prototype, #[visitor(...)] attribute form, IntoVisitor — all replaced by the shipped visitor!/Hook/Driver design documented in the rewritten CLAUDE.md), so no live non-derivable constraint is lost. Deletion is safe.
  - Spot-checked: exists at repo root (29,823 bytes), superseded by CLAUDE.md.

### docs/spike_unbounded_recurse.rs : 1-294  [dead-code] (~294 lines, risk low, verified)
  Delete the standalone spike file. If docs/recurse-unbounded-plan.md is kept, update its §9.1
  line 231 reference; if the plan docs are also removed (see separate finding) nothing needs
  updating.
  - evidence: docs/ is not a crate (no Cargo.toml, not a member, no [[bin]]/[[example]]/include! anywhere referencing it), so the file is never compiled. Sole references: docs/recurse-unbounded-plan.md:231 ('Spike — ✅ DONE') and a cross-mention in core/tests/spike_real_parsestream.rs:2. The mechanism it proved (erased fn-ptr re-entry, backtracking through terminators, unbounded depth) is shipped and tested against the real generated code: core/tests/recurse_traits.rs:38 parse_unbounded_depth (depth-200 via core::parse::vtable) and rust/tests/rustsub_roundtrip.rs:48 deep_parens_round_trip_is_unbounded. Note docs/ is untracked (?? in git status), so deletion is unrecoverable — but this is a served-its-purpose scratch spike.
  - verifier: Verified: docs/ is not a workspace member (members = core/macro/rust), has no Cargo.toml, and no include!/#[path] references the spike — it is never compiled. Exact reference census matches the finding: only docs/recurse-unbounded-plan.md:231 and a doc-comment aside in core/tests/spike_real_parsestream.rs:2 (becomes a harmless stale mention; optionally tweak it too). Coverage is even stronger than claimed: besides recurse_traits.rs::parse_unbounded_depth (depth-200) and rustsub_roundtrip.rs::deep_parens_round_trip_is_unbounded (depth-60 w/ backtracking), the compiled test core/tests/spike_real_parsestream.rs reproduces all three toy-spike properties (finite monomorphization, unbounded depth, c_backtracks_through_terminators at D=200) against the real ParseStream/Dup — the toy is strictly subsumed. Unrecoverability confirmed (git ls-files docs/ empty), already disclosed in the finding; content is a zero-unique-value scratch proof, so risk stays low.


## Slice `tests-core` — detailed design

All 25 findings re-verified against the worktree `../syan2-reduce` (line numbers current as of this design); **0 rejected**, ~1,530 lines saved (+~35 optional in step 24). Execution order: (A) standalone file deletions + `visitor_diagnostics.rs` consolidation (steps 1–7), (B) in-file test trims (8–14), (C) the `visitor_recurse*` family, bottom-up per file (15–18), (D) the `visitor_edit`/`visitor_containers`/`visitor_recurse_shapes` triangle (19–22), (E) audit/header comment work with the trybuild-sensitive step last (23–25), then one consolidated CLAUDE.md/docs sync pass. Risk adjustments from the plan: step 9 (symbol.rs rewrite), step 22 (visitor_reduce merge) and step 25 (problem headers → **mandatory** `.stderr` regen) are **medium**, everything else low. Per-step gate: `cargo test --workspace` in `../syan2-reduce`; `TRYBUILD=overwrite` is legitimately needed **only** in step 25 (for `problem{1,3,5,7}*.stderr`) and only if step 6's Option B is chosen (for `visitor_edit_marker_boxed.stderr` — Option A, recommended, needs none).

### 1. Delete `core/tests/attribute_macro.rs` (whole file, 24 lines) [dead-code]

Evidence (condensed): 24 lines, every one `//`-commented, zero `#[test]`; whole-worktree grep for `attribute_macro` hits nothing outside the file; no `[[test]]` entry — it builds as an empty auto-discovered test binary.

**What changes**: `git rm core/tests/attribute_macro.rs`. Nothing moves; there were never any tests, so no covering-test map is needed. For the record, the commented shapes are live elsewhere: `where_clause_attribute.rs::where_clause_and_composite_span_derives_compile` (WithSpan + where-clause Parse/Unparse/Spanned) and `visitor_edit_group.rs::{edit_grouped_seq_and_opt, tail_kept_when_not_seven, empty_groups_edit_is_noop}` (`#[group(self.X)]` on GroupParen/GroupBrace).

**Risk: low.** Verify: `cargo test --workspace`.

### 2. Delete `core/tests/spike_real_parsestream.rs` (whole file, 183 lines) [test-unneeded]

Evidence: hand-rolled prototype of the vtable re-entry mechanism (own registry, own `term_parse`, toy `< Expr >` grammar) — exercises no shipped engine/vtable code; its `//!` header (line 2) references `spike_unbounded_recurse.rs`, which exists only under `docs/`.

**What changes**: delete the file. The three properties it proved are pinned against the real engine: (a) finite monomorphization — every `#[recurse]` Parse test compiling; (b) unbounded depth — `recurse_traits.rs::unparse_spanned::parse_unbounded_depth` (line 38, depth-200); (c) backtracking through the erased re-entry — `rust/tests/rustsub_roundtrip.rs::deep_parens_round_trip_is_unbounded` (line 48). Residue: `docs/recurse-unbounded-plan.md:248` and `docs/spike_unbounded_recurse.rs:91` name the deleted file — both are docs-only (and the latter is itself deleted by the `repo-dead-weight` slice); fold into the final doc-sync pass.

**Risk: low.** Verify: `cargo test --workspace`.

### 3. Delete `core/tests/ast_recurse.rs` (whole file, 44 lines) [test-unneeded]

Evidence: its single assertion (`assert_is_ast::<Expr<()>>/<Stmt<()>>` on a `#[recurse]` cycle) is a strict subset of surviving coverage; its `//!` header describes the removed rename/depth-alias architecture ("renames the cyclic types (e.g. Expr → __ExprRec) … depth-limited public aliases").

**What changes**: delete the file. Covering: `visitor_recurse_mixed.rs::drill::ast_markers_hold_for_both_recurse_aliases_and_acyclic_types` (fn body 256–261; asserts `Ast` on a structurally identical Expr/Stmt Box-cycle + acyclic types). The one delta — no explicit `assert_is_ast::<Stmt<..>>` there — is immaterial: `#[derive(Ast)]` emits an unconditional empty marker impl, so any compiling cycle with `#[derive(Ast)]` on `Stmt` proves it (e.g. `recurse_core.rs::fixes`, lines 302–334, whose `visitor!(…Expr, …Stmt)` requires Stmt's derive expansion). Doc-sync: `CLAUDE.md:11` and `docs/recurse-natural-types-plan.md:103` name `ast_recurse` — final pass.

**Risk: low.** Verify: `cargo test --workspace`.

### 4. Delete `core/tests/ui/problem2_free_fn.{rs,stderr}` + `problem8_qself.{rs,stderr}` (30+100+39+99 = 268 lines) [test-unneeded]

Evidence: both fixtures use by-value `Nested(Expr<S>)`, so the macro aborts on the by-value-cycle guard before the nominal concerns (free-fn transform, qself) are ever reached; both `.stderr` files open with the identical single-type abort pinned by the kept `problem1_trait_impl.stderr` ("the cycle (Expr) has no heap indirection…"), and the remaining ~90 lines each are brittle rustc cascades.

**What changes**: delete the four files, and delete the two runner lines in `core/tests/recurse_core.rs::problems::compile_fail_problems`:
- line 468: `t.compile_fail("tests/ui/problem2_free_fn.rs");`
- line 472: `t.compile_fail("tests/ui/problem8_qself.rs");`

Covering: the single-type by-value abort stays pinned by `recurse_core.rs::problems` via `problem1_trait_impl.stderr:1`; the multi-type form by `problem5_multiple_roots.stderr:1`. Residue: `docs/recurse-natural-types-plan.md:303` (historical plan doc, itself flags these for audit) — final doc-sync pass. No `.stderr` regeneration: the remaining problem-file `.stderr`s are untouched (step 25 handles their headers separately — do **this** step first so step 25 regenerates only 4 files, not 6).

**Risk: low.** Verify: `cargo test --workspace`.

### 5. Delete `core/tests/visitor_drill_cycle.rs` (8 lines); move its compile_fail into `visitor_diagnostics.rs` [test-dup]

Evidence: the file is one `#[test]` wrapping a single `t.compile_fail("tests/ui/drill_cycle.rs")`; `visitor_diagnostics.rs` already hosts the same "list one" diagnostic family (`visitor_recurse_unlisted_coroot.rs`, line 15); trybuild snapshots key on the ui path, not the runner, so `ui/drill_cycle.{rs,stderr}` (kept) are unaffected.

**What changes**: delete `core/tests/visitor_drill_cycle.rs`; in `core/tests/visitor_diagnostics.rs::visitor_diagnostics`, insert directly after line 15 (`t.compile_fail("tests/ui/visitor_recurse_unlisted_coroot.rs");`):

```rust
    // Drilling that would loop through a cycle of unlisted intermediates is a compile error ("list one").
    t.compile_fail("tests/ui/drill_cycle.rs");
```

No other reference to `visitor_drill_cycle` exists; CLAUDE.md's `visitor_drill*.rs` glob still matches the remaining `visitor_drill.rs`. Sequence with steps 6–7 (same file); apply 5 → 6 → 7 or as one combined edit.

**Risk: low.** Verify: `cargo test --workspace` (no `.stderr` regen — the snapshot file is unchanged).

### 6. Delete `core/tests/ui/visitor_edit_marker_mismatch.{rs,stderr}` (24+10 = 34 lines) + its runner line [test-unneeded]

Evidence: `peel` (`macro/util.rs:200-210`) matches no container names, so `Box<Vec<T>>` (pinned by the kept `visitor_edit_marker_boxed.rs`) and `Vec<Option<T>>` (this file) both peel to two `View` layers and hit the single `_ =>` catch-all abort at `macro/visitor/lower.rs:358-369`; the two `.stderr`s are the same template differing only in the interpolated head ident — one code path tested twice.

**What changes — Option A (recommended, plain delete)**: delete both files and `visitor_diagnostics.rs:21` (`t.compile_fail("tests/ui/visitor_edit_marker_mismatch.rs");`) together with its lead comment line 20 (`// `#[seq]`/`#[opt]` must name the field's innermost container…`). Covering: `visitor_diagnostics.rs` → `ui/visitor_edit_marker_boxed.{rs,stderr}` (same abort arm, same message template). No `.stderr` regen.

**Option B (only if the nested-inner shape must stay literally pinned)**: append a second `Item`/`Holder`/`vis2` struct+visitor pair with `#[seq] pub grid: Vec<Option<Item<S>>>` to `visitor_edit_marker_boxed.rs`, then delete the mismatch files + runner line. This forces `TRYBUILD=overwrite` regeneration of `visitor_edit_marker_boxed.stderr` (now two errors in one snapshot) plus a manual diff review — **risk medium**; the error message's own text already promises both shapes, which is why the plan preferred it, but the abort arm is provably shared, so A loses no code-path coverage.

Doc-sync: `CLAUDE.md:66` and `docs/visitor-edit-plan.md:312` name the deleted ui test — final pass.

**Risk: low (Option A) / medium (Option B).** Verify: `cargo test --workspace`; Option B additionally `TRYBUILD=overwrite cargo test -p syan --test visitor_diagnostics` then re-run clean.

### 7. Trim historical comments in `core/tests/visitor_diagnostics.rs` (lines 9–12, 31–33; ~7 lines) [comment]

Evidence: both blocks are self-contained "(… is now SUPPORTED — see `<file>.rs`)" parentheticals whose referenced files (`visitor_nested_containers.rs`, `visitor_mixed_recurse_extra_param.rs`, `visitor_union_where_unshared_param.rs`) no longer exist (content now in `visitor_containers.rs`, `visitor_recurse_mixed.rs::extra_param` (L85), `visitor_core.rs::union_where_unshared` (L359)).

**What changes**: delete whole lines
- 9: `// (Nested containers `Vec<Option<T>>` are now supported — see …`
- 10–12: `// (A former-`#[recurse]` cycle mixed with an acyclic type …` through `…visitor_mixed_recurse_extra_param.rs`.)`
- 31–33: `// (A `where`-bounded generic param not shared by all visited types is now SUPPORTED …` through `…visitor_union_where_unshared_param.rs`.)`

(Line numbers are pre-steps-5/6; if applied after them, locate by the quoted text.) All surrounding `t.compile_fail(...)` lines stay.

**Risk: low.** Verify: `cargo test --workspace`.

### 8. Trim `core/tests/rec_group.rs` to `test_case_1..4` + `test_simple_container_basic` (~171 lines) [test-dup]

Evidence: `GroupParen/Brace/Bracket` are type aliases over one generic `Group<T,O,C>` with a single delimiter-agnostic `Parse` impl (`core/src/nested/group.rs:59-78`), so every deleted test is a data-only recombination of the kept ones.

**What changes** (all in-file; ranges current): delete lines **74–112** (`test_case_5_complex_nested_structure` … `test_case_8_large_numbers`), **134–156** (`test_simple_container_single_element`, `_empty`), **158–192** (`TwoLevelContainer` struct + `test_two_level_container{,_single_value,_empty_inner}`), **194–261** (`test_data_access_outer_content`, `test_punctuated_lengths_comprehensive`, `test_final_element_variations`, `test_stress_test_large_structure`), and the stale line-1 comment `// TokenStream and TokenTree imports removed as they're unused`. Remaining layout: imports, `NestedContainer`, `test_case_1..4`, `SimpleContainer`, `test_simple_container_basic` (~90 lines).

Dying → covering (all `rec_group.rs::` unless noted): case_5 → `test_case_3_multiple_outer_elements` + `test_case_2_with_final_element`; case_6 → `test_case_4_empty_groups` + `test_case_2…`; case_7 → `test_case_1` (byte-identical 1/1/1/none); case_8 → `test_case_2…` (digit magnitude is data); `test_simple_container_single_element/_empty` → `test_simple_container_basic`; TwoLevelContainer trio → `test_case_1..4` (3-level superset nesting); `test_data_access_outer_content` (its `.iter().count()` extra) → `punctuated.rs::test_insert`/`test_remove_multiple_elements` + parsed-Punctuated iteration in `visitor_edit_containers.rs`/`visitor_edit_group.rs`; `test_punctuated_lengths_comprehensive` → `test_case_1..4` (same four length combos); `test_final_element_variations` → `test_case_1`/`test_case_2…`; stress → `test_case_3…` (same path, larger N).

**Risk: low.** Verify: `cargo test --workspace`.

### 9. Rewrite `core/tests/symbol.rs` to 5 tests (~140 lines) [test-dup]

Evidence: all 18 tests exercise only Default+Display over 4 token-parse branches (`macro/symbol.rs:15-42`: LitInt/LitChar/Ident/Punct), one shared `char_to_type_path` arm for `a-z|A-Z|0-9`, and `create_joint_type` chunking at `MAX_TUPLE_SIZE = 12` (the tests' "fourteen character limit" comments are simply wrong, harmlessly).

**What changes**: replace the file body with exactly 5 `#[test]` fns — `test_symbol_idents` (short + single-char `x` + `test_123` + `hello_world` + `MyStruct`), `test_symbol_literals` (int, `1 2 3` concat, char, `'a' 'b'` concat), `test_symbol_puncts` (`+`, `::`, `->`, one mixed `test :: 42 'a' +` sequence), `test_symbol_joint_chunking` (`very_long_identifier`, `very_long_function_name_42`), and `test_symbol_very_long_sequences` **kept verbatim** (current lines 224–229 — the only multi-level Joint recursion, and the sole carrier of `= ! & |`). Concrete shape (assertion strings must be transcribed exactly from the current file):

```rust
use syan::symbol::Symbol;

#[test]
fn test_symbol_idents() {
    assert_eq!(&<Symbol![hello]>::default().to_string(), "hello");
    assert_eq!(&<Symbol![x]>::default().to_string(), "x");
    assert_eq!(&<Symbol![test_123]>::default().to_string(), "test_123");
    assert_eq!(&<Symbol![hello_world]>::default().to_string(), "hello_world");
    assert_eq!(&<Symbol![MyStruct]>::default().to_string(), "MyStruct");
}
// … test_symbol_literals / test_symbol_puncts / test_symbol_joint_chunking as above …
// test_symbol_very_long_sequences: current lines 224-229, byte-identical.
```

Dying → covering (all `symbol.rs::`, post-rewrite names): `_underscores`/`_single_char`/`_mixed_case` → carried into `test_symbol_idents`; `_longer`/`_rust_keywords` → same single a-z/A-Z arm, no carry; `_complex_mixed`/`_alternating_patterns`/`_mixed_long_patterns` → `test_symbol_puncts` (+ chunking test for length); `_long_mixed_tokens` → carried into `test_symbol_joint_chunking`; `_long_token_sequences` → same 2-chunk depth as the kept chunking asserts. After editing, run a char-coverage diff (`grep -o` each punct char old vs new) — every punct char in a deleted test must appear in a kept assert.

**Risk: medium** (active rewrite; a transcription slip silently drops coverage with no compiler signal — hence the char-diff check). Verify: `cargo test --workspace`.

### 10. Trim `core/tests/source_string.rs` (~100 lines) [test-dup]

Evidence: `test_span_basic` is a tautological struct-literal read-back; `Stream::next` (`core/src/source/string.rs:66-76`) has no per-char or length-1 special case; the 7 char-class tests all exercise one `impl_parse_for_char!`-generated body.

**What changes**: delete lines **6–17** (`test_span_basic`), **79–90** (`test_stream_single_char`), **117–150** (`test_stream_newlines`), **218–229** (`test_into_parse_stream_for_string`); replace block **231–317** (the 7 char-class tests + `test_parse_multiple_chars`) with one merged `test_parse_char_classes` — 1 ok + 1 err assert per class (lowercase/uppercase/digit/punct/delimiter/underscore) + two empty-string err asserts (shape as sketched in the verification: `Symbol::<chars::_a>::parse("a".to_string()).is_ok()` etc., copying the exact `chars::` type names from the current file).

Dying → covering: `test_span_basic` → none needed (tested nothing); `test_stream_single_char` → `source_string.rs::test_stream_multiple_chars`; `test_stream_newlines` → `source_string.rs::test_multiline_position_tracking` (strict span/line/col superset incl. the empty line; the `.slot=='\n'` asserts are textually unique but `slot` assignment is char-uniform, pinned by `test_stream_multiple_chars`); `test_into_parse_stream_for_string` → `source_string.rs::test_complex_parsing_sequence` (same `into_parse_stream` entry + terminal `None`); `test_parse_multiple_chars` → `test_parse_pushback_functionality` + `test_complex_parsing_sequence`; the 7 char-class tests → the new merged test.

**Risk: low** (merge is mechanical; the macro body is verified uniform). Verify: `cargo test --workspace`.

### 11. Trim `core/tests/punctuated.rs` (~66 lines) [test-dup]

Evidence: `TestSpan` (+ `Span`/`Spanned` impls + the line-2 import) appears only at lines 2–19; `core/src/nested/punctuated.rs` is all-safe fully generic code (no specialization), so the `String`-element test adds no path.

**What changes**: delete lines **2–19** (dead `TestSpan` + impls + `use syan::span::{Span, Spanned};`; keep the line-1 `Punctuated` import), **24–37** (`test_len`), **103–121** (`test_first_and_last`), **224–242** (`test_iterator_after_operations`).

Dying → covering (all `punctuated.rs::`): `test_len` → `test_push` (len after each of 3 pushes, lines 44–56) + `test_default_construction` (empty, 245–249); `test_first_and_last` → `test_default_construction` (None/None) + `test_push` (first/last after pushes, byte-identical final state); `test_iterator_after_operations` → `test_insert` (middle-insert + full `.iter()` order) + `test_remove_multiple_elements` (middle-remove + order).

**Risk: low.** Verify: `cargo test --workspace`.

### 12. Delete 5 tests in `core/tests/recurse_core.rs` (~51 lines) [test-unneeded]

Evidence: each deleted assertion is a strict subset of survivors; `#![allow(unused_imports)]` confirmed at line 4, so the `Integer` import left behind in `mod basic` stays silent.

**What changes**: delete, **bottom-up** to keep ranges stable: **216–224** (`test_multi_param_lit`), **163–179** (`test_direct_init_expr_lit`), **117–126** (`test_get_expr`), **109–115** (`test_stmt_count`), **78–86** (`test_block_with_stmts`).

Dying → covering: `test_block_with_stmts` and `test_get_expr` → `recurse_core.rs::basic::test_mixed_stmts` (182–195; same `{ 1 ; 2 }` parse, len, `get_expr().is_literal()` on both stmts); `test_stmt_count` → `basic::test_empty_block` (128–134, N=0) + `basic::test_semi_contains_block` (150–161, N=2); `test_direct_init_expr_lit` → `fixes::bug6_generic_limit1_compiles` (288–296, direct natural-type construction) + `proc_macro2_literal.rs::test_integer_parse_plain` (86–92, Integer value/suffix); `test_multi_param_lit` → `basic::test_multi_param_parse_lit` (226–237, same variant via Parse).

**Risk: low.** Verify: `cargo test --workspace`.

### 13. Delete `recurse_visitor_cycles.rs::multi_cycle::plain` + `two_independent_cycles_build` (lines 231–257, ~27 lines) [test-unneeded]

Evidence: `mod plain` (234–251) is byte-identical to `mod vis` (260–277) modulo the mod name; `two_independent_cycles_build` (253–257) only constructs `Lit`/`Unit` with no assertions — a compile-smoke strictly implied by `vis` compiling and by `independent_visitors_are_separate` constructing the superset `Nest(Lit)`/`Arrow(Unit)` **and** visiting it.

**What changes**: delete lines 231–257. Relocate the rationale comment currently at 231–232 ("Expr and Type are disjoint self-referential cycles; each must regenerate against its OWN depth default…") above the surviving `mod vis` — note the verifier found this comment historically inaccurate for an engine-less Ast-only mod, so prefer trimming it to just "Expr and Type are disjoint self-referential cycles (independent SCCs)". Dying → covering: `multi_cycle::two_independent_cycles_build` → `recurse_visitor_cycles.rs::multi_cycle::vis::independent_visitors_are_separate` (intra-file; independent of step 16's deletion of `visitor_recurse.rs::multicycle` — that third copy was bonus evidence, not load-bearing). Sequencing: this deletion shifts `multiroot` (currently 311–376) up ~27 lines; steps 15–17 cite that mod **by name**.

**Risk: low.** Verify: `cargo test --workspace`.

### 14. Delete stale debug comments in `core/tests/rust_ast.rs` (lines 206, 217; 2 lines) [comment]

Evidence: both are leftover debug scribbles inside now-green plain `#[test]`s (no `#[ignore]`/`#[should_panic]`).

**What changes**: delete line 206 (`    // failed`, in `test_expression_literal`) and line 217 (`    // inf loop`, in `test_expression_block`).

**Risk: low.** Verify: `cargo test --workspace`.

### 15. Delete `core/tests/visitor_recurse.rs::multicycle` (lines 260–315, ~56 lines) [test-unneeded]

Evidence: fixture and assertions byte-equivalent to `recurse_visitor_cycles.rs::multi_cycle::vis::independent_visitors_are_separate` (same two disjoint self-cycles `Expr{Nest,Lit}`/`Type{Arrow,Unit}`, one `visitor!`, stronger per-cycle counts there).

**What changes**: delete lines 260–315 (comment + `mod multicycle`). The genuine delta — inherent `.visit(&mut struct)` instead of `Visit::visit_*` trait-path — is covered by `visitor_recurse.rs::via_visitor::walks_the_cycle_mut` (inherent `.visit_mut`, struct visitor, recurse type), `rust/tests/rustsub_roundtrip.rs::visitor_walks_the_tree` (95–102, inherent `.visit` + struct visitor on a recurse cycle root), and `visitor_recurse.rs::disjoint_params` (inherent `.visit` on both roots of a two-SCC visitor). Dying → covering: `multicycle::two_independent_cycles_one_visitor` → `recurse_visitor_cycles.rs::multi_cycle::vis::independent_visitors_are_separate` (+ the inherent-visit pins above). Apply **before** step 16 (it sits below `multiroot`; bottom-up keeps ranges stable).

**Risk: low.** Verify: `cargo test --workspace`.

### 16. Delete `core/tests/visitor_recurse.rs::multiroot` (lines 202–258, ~57 lines) [test-unneeded]

Evidence: same A/B mutually-and-self-referential fixture as `recurse_visitor_cycles.rs::multiroot`, which asserts strictly more; root handling in the macro is position-independent (verified directly: `macro/recurse/build.rs:191-203` sorts roots alphabetically and maps them uniformly; `macro/recurse/transform.rs:25-61` lowers any back-edge via one hashmap lookup — no first-root special case).

**What changes**: delete lines 202–258 (comment + `mod multiroot`). Dying → covering: `multiroot::multiroot_via_visitor` (single (3,1) assertion over A→B, B→A, A-self edges) → `recurse_visitor_cycles.rs::multiroot::each_root_keeps_its_own_depth` (cross-edges, two-counter struct visitor) + `recurse_visitor_cycles.rs::multiroot::visit_from_either_root` (self-edge `B::Me`, second-root entry) + `visitor_recurse.rs::via_visitor::closure_over_self_recursive_root` (self edge on a sole root — survives step 17's merge). Note the residual "struct visitor over a self edge" is pinned by `visit_from_either_root`.

**Risk: low.** Verify: `cargo test --workspace`.

### 17. Merge `visitor_recurse.rs::cycle` into `::via_visitor` (lines 96–200 → ~62 lines saved) [test-dup]

Evidence: `cycle::ast` (101–123) is byte-identical to `via_visitor::ast` (11–29) modulo module path; `cycle::Counter` + its `Visit` impl (~124–139) is instantiated by no test; `visits_self_recursive_root` + `Nodes` duplicate `closure_over_self_recursive_root` (same `tree` fixture, same count 3) plus `via_visitor::walks_the_cycle`'s struct-visitor descent. The merge was empirically applied in a scratch copy by the verifier: compiles, 8/8 tests pass.

**What changes**: delete `mod cycle` entirely; inside `mod via_visitor`, after `walks_the_cycle_mut`, append (a) `closure_over_recurse_cycle` moved with its fixture paths retargeted to `via_visitor::ast` (body otherwise verbatim — it only uses inherent `e.visit(closure)`, generated by via_visitor's `mod v`); (b) the `#[recurse] mod tree { … }` block moved verbatim (its `#[subast()]` is empty — no paths to fix); (c) `mod v_tree` with its `visitor!` path updated `crate::cycle::tree::Expr` → `crate::via_visitor::tree::Expr`; (d) `closure_over_self_recursive_root` moved verbatim. Module layout after: `via_visitor { mod ast; mod v; Counter (+Visit/+VisitMut); walks_the_cycle; leaf_only; walks_the_cycle_mut; closure_over_recurse_cycle; mod tree; mod v_tree; closure_over_self_recursive_root }` — then `disjoint_params` (the only other surviving mod after steps 15–16). Dies without move: `cycle::ast`, `cycle::v_ast`, `cycle::Counter`+impl, `Nodes`+impl, `visits_self_recursive_root`. Dying → covering: `cycle::visits_self_recursive_root` → `via_visitor::closure_over_self_recursive_root` (same fixture/count) + `via_visitor::walks_the_cycle` (struct-visitor descent) + `recurse_visitor_cycles.rs::multiroot::visit_from_either_root` (struct visitor over a self edge). File `//!` header (line 1): drop "multi-root, multi-cycle," (steps 15–16) — do it in this step's edit.

**Risk: low** (verbatim moves + path retargets; empirically pre-validated). Verify: `cargo test --workspace`.

### 18. Delete `core/tests/visitor_recurse_mixed.rs::heterogeneous` (lines 355–407, ~53 lines) [test-unneeded]

Evidence: byte-for-byte the same fixture, visitor, `visit_stmt<T>` method-generic shape, tree, and assert message ("Expr + Stmt (extra param T=u8) + inner Expr", `c.0 == 3`) as `recurse_visitor_cycles.rs::generics::het` (mod at 159–201) — which no other finding deletes.

**What changes**: delete lines 355–407 (clean tail-of-file deletion). Dying → covering: `heterogeneous::heterogeneous_cycle_via_visitor` → `recurse_visitor_cycles.rs::generics::het::heterogeneous_generics_visitor`. Also trim "heterogeneous concrete-fill" from the file's `//!` header (lines 1–2) in the same edit.

**Risk: low.** Verify: `cargo test --workspace`.

### 19. Trim narration comments in `core/tests/visitor_edit.rs` (~10 lines) [comment]

Evidence: the flagged comments restate adjacent assert messages; two of the plan's cited lines were found on re-verification to be mechanism comments with **no** assert-message fallback and are excluded.

**What changes** (apply before steps 20/22 so line numbers hold; all cited lines are < 415, no overlap with step 20's deletion):
- **Delete** L108 `// Drop 0s, replace 2 -> 102, and append a 7 sentinel; …` (dup of L142–143 messages), L118 trailing `// insert into the (now-shorter) collection`, L123 trailing `// fill an empty slot`, L242 banner `// Plain descent through the boxed-Option cycle: …`, L249 trailing `// default descent into the Opt child` (dup of the **kept** L230–232 constraint note), L374 banner `// Drop \`Nop(0)\` statements anywhere in the cycle; …` (dup of L411's message), and the L403–407 trailing per-element comments (`// removed`, `// kept`, `// removed (nested, via the back-edge)`, `// kept`).
- **Keep (excluded from the finding)**: L184 `// dropped by retain below` and L186 `// descend nested \`Many\`` — control-flow mechanism comments; the test's assert (L213) has no message string. Likewise keep **one** of {L178 banner, L203–209 per-element annotations} so `vec_of_box_edits_in_cycle` retains prose — recommend keeping L203–209 (they map elements to the unlabeled `vec![1, 99, 5, 99]`) and deleting the L178 banner.
- **Keep** all section banners (`// ── … ──`) and the constraint notes (L230–232 rec_opt; L468–471 boxed_opt).

**Risk: low.** Verify: `cargo test --workspace`.

### 20. Delete `visitor_edit.rs::nested` (lines 415–466, ~48 lines); extend `visitor_containers.rs::nested::nested_containers_visit_mut` [test-dup]

Evidence: `visitor_containers.rs::nested` already covers `Vec<Option<T>>`/`Option<Vec<T>>`/`Vec<Vec<T>>` shared-side (count 7) plus `Vec<Option<T>>` mut-side; `visitor_edit::nested`'s only unique assertion is mut-side `Vec<Vec<Item>>` element reach (the covering mut fixture currently has `vv: vec![]`).

**What changes**: (a) in `core/tests/visitor_edit.rs`, delete lines 415–466 (`// ── nested containers …` banner + `mod nested`); the "not edit-view targets" constraint it narrated stays enforced by `ui/visitor_edit_marker_mismatch`-family coverage (post step 6: `ui/visitor_edit_marker_boxed.rs`) and CLAUDE.md. (b) in `core/tests/visitor_containers.rs::nested::nested_containers_visit_mut` (lines 44–50), change the fixture and count:

```rust
let mut h: Holder<()> = Holder { vo: vec![Some(leaf())], ov: None, vv: vec![vec![leaf()]] };
let mut n = 0usize;
h.visit_mut(|_: &mut Leaf<()>| n += 1);
assert_eq!(n, 2, "Vec<Option<_>> and Vec<Vec<_>> both descended on the mut side");
```

Dying → covering: `visitor_edit.rs::nested::nested_containers_descend` → `visitor_containers.rs::nested::nested_containers_are_traversed` (shared-side, all three shapes) + the extended `visitor_containers.rs::nested::nested_containers_visit_mut` (mut-side Vec<Option> + Vec<Vec>). Mut-persistence-with-values stays pinned by `visitor_edit.rs::plain_mut` and `::rec_opt`. Residual accepted: mut-side `Vec<Option<_>>` with an interleaved `None` drops to shared-side-only coverage (second-order). Sequence with step 21 (same `visitor_containers.rs`): this step edits lines 44–50, step 21 deletes 52–88 — apply 20 then 21 (or one combined edit); doing 20 first keeps both cited ranges valid.

**Risk: low.** Verify: `cargo test --workspace`.

### 21. Move `visitor_containers.rs` recurse block (lines 52–88, ~24 lines net) into `visitor_recurse_shapes.rs::containers` [test-dup]

Evidence: `visitor_recurse_shapes.rs::containers` (5–77) owns the recurse container-shape fixture (Counter + `count()` helper; Box<Option<Box>>, tuples, Vec<Box>, Option<Box>); `Vec<Option<_>>` is the one missing nesting, and the Counter/`rv` machinery is duplicated across both files. Empirically pre-applied by the verifier: compiles, 13/13 tests pass.

**What changes**: (a) in `core/tests/visitor_containers.rs`, delete lines 52–88 (`#[syan::parse::recurse] mod rec`, `mod rv`, `Counter`, `recurse_nested_container_is_traversed`). (b) in `core/tests/visitor_recurse_shapes.rs::containers::ast::Expr`, add a variant `ManyOpt(Vec<Option<Expr<S>>>),` (after `Many(Vec<Box<Expr<S>>>),`), and add one test using the existing `count()` helper:

```rust
#[test]
fn vec_of_option_descends() {
    let e: ast::Expr<()> = ast::Expr::ManyOpt(vec![
        Some(ast::Expr::Lit(PhantomData)),
        None,
        Some(ast::Expr::Lit(PhantomData)),
    ]);
    assert_eq!(count(&e), 3, "outer Expr + 2 back-edges; None skipped");
}
```

Dying → covering: `visitor_containers.rs::nested::recurse_nested_container_is_traversed` → the new `visitor_recurse_shapes.rs::containers::vec_of_option_descends` (identical property, count==3). The deleted fixture's implicit "Vec-as-sole-indirection accepted by `#[recurse]`" property stays pinned by `visitor_edit.rs` (seq-in-cycle fixture), `recurse_traits.rs`, `recurse_core.rs`.

**Risk: low.** Verify: `cargo test --workspace`.

### 22. Merge `core/tests/visitor_reduce.rs` (93 lines) into `visitor_edit.rs::views`; delete the file [test-dup]

Evidence: identical fixture (`Stmt<S>(i64, PhantomData<S>)` / `Block{#[seq] stmts, #[opt] tail}` — `visitor_edit.rs::views` lines 88–153); `child_level_edits_on_vec_and_option` ⊂ `views::seq_edits_and_push` + `views::opt_take_clears`; the two unique bits (opt set-on-Some; parent-override style) are carried over. Verified: `views::Editor::visit_stmt_opt` currently has **no** `Some(2)` arm, and its seq hook unconditionally pushes `Stmt(7,..)` — so the carried opt-replace test **cannot** be a verbatim copy.

**What changes** (destination `core/tests/visitor_edit.rs`, inside `mod views`; then delete `core/tests/visitor_reduce.rs`):
1. Add an arm to `views::Editor::visit_stmt_opt` (currently ~L120–126): `Some(2) => v.set(Stmt(102, PhantomData)),` between the `Some(0) => v.clear(),` and `None => v.set(Stmt(5, PhantomData)),` arms. (Verified non-disruptive: `seq_edits_and_push`'s tail starts `None`; `opt_take_clears`'s starts `Some(Stmt(0))` — neither hits value 2.)
2. Add a new test after `opt_take_clears`, **accounting for the seq-side push**:

```rust
#[test]
fn opt_replace_on_two() {
    let mut b: Block<()> = Block { stmts: vec![], tail: Some(Stmt(2, PhantomData)) };
    b.visit_mut(&mut Editor);
    assert_eq!(b.stmts.iter().map(|s| s.0).collect::<Vec<_>>(), vec![7], "7 pushed into the empty Vec");
    assert_eq!(b.tail.as_ref().map(|s| s.0), Some(102), "the Option tail (a 2) was replaced");
}
```

3. Move `ParentEditor` + `parent_override_still_works` from `visitor_reduce.rs` verbatim except `visit::` → `v::` (two sites: the impl'd trait path and the `v::visit_block_mut(self, b)` re-entry), appended at the end of `mod views`.

Module layout after: `views { Stmt; Block; mod v; Editor (seq+opt hooks, opt now 3-armed); seq_edits_and_push; opt_take_clears; opt_replace_on_two; ParentEditor; parent_override_still_works }`. Dying → covering: `visitor_reduce.rs::child_level_edits_on_vec_and_option` → `visitor_edit.rs::views::seq_edits_and_push` (view_iter_mut replace + retain_mut, superset) + `views::opt_take_clears` (identical `Some(0)`→clear); `visitor_reduce.rs::replace_then_keep_in_option` → new `views::opt_replace_on_two` (adapted); `visitor_reduce.rs::parent_override_still_works` → moved verbatim. Doc-sync: `CLAUDE.md:47` (retarget the `visitor_reduce.rs` citation to `visitor_edit.rs::views::parent_override_still_works`) and `CLAUDE.md:85` (drop the `visitor_reduce.rs (…)` clause, fold "parent-override still works, opt-replace" into the `visitor_edit.rs` parenthetical); `docs/visitor-edit-plan.md:299,345` optional — final pass.

**Risk: medium** (only step writing new logic — a new match arm on a shared `Editor` used by two pre-existing tests, plus an adapted assertion; review `views::` test output line-by-line, not just the pass count). Verify: `cargo test --workspace`.

### 23. Trim fixed-bug narration in `core/tests/macro_audit_test.rs` + `macro_audit_runtime_test.rs` (~16 lines) [comment]

Evidence: the referenced regression files (`visitor_tuple_field.rs`, `visitor_where_clause.rs`, `recurse_fixes.rs`, `ignore_bounds.rs`, `recurse_where_clause.rs`, `recurse_no_engine.rs`, `recurse_group_ful.rs`) no longer exist (content in `where_clause_attribute.rs`, `recurse_core.rs::{fixes,no_engine,where_clause}`, `recurse_traits.rs::{group_ful,ignore_bounds}`). One evidence correction: `where_clause_attribute.rs` **does** exist — the L34–36 deletion stands on the narration-trim rationale, not "dead reference".

**What changes** in `macro_audit_test.rs`:
- L6–9: partial-line surgery, not whole-line deletion — the stale parenthetical starts mid-L7 (`… (Audit findings #1–#8 have since been FIXED — see`) and runs through L9's `…the symbol! abort below;`. Replace L6–9 with: `//! These are *known limitations*, captured so a fix has a regression target and the failure modes` / `//! are documented rather than surprising. The entries registered below remain open.`
- L34–36: delete the self-contained `// (#1 Parse where-clause panic, #4 … are now FIXED — positive regression tests in / where_clause_attribute.rs.)` block.
- L39–40: delete `// (#[ignore_bounds] is now HONORED … regression test lives in \`ignore_bounds.rs\`.)`.
- L60–68: delete the four self-contained `(#N … now FIXED — see recurse_{fixes,where_clause,no_engine,group_ful}.rs)` parentheticals.
- **Keep** the "Also found, but NOT encoded" block (L11–27) — otherwise-untracked known bugs.

In `macro_audit_runtime_test.rs` L8–9: delete only the first sentence (`(The visitor!() tuple-skip finding that lived here is now FIXED — see \`visitor_tuple_field.rs\`.`); keep `The two symbol! encoding bugs below remain open.` as a standalone doc line (drop the orphaned closing paren).

**Risk: low** (driver files, no paired `.stderr`). Verify: `cargo test --workspace`.

### 24. Trim stale cross-references in `core/tests/recurse_audit_test.rs` (~11 lines; optional +35) [comment]

Evidence: none of `recurse_problems_test.rs`, `visitor_recurse_containers.rs`, `recurse_generics.rs`, `recurse_multiroot.rs`, `audit_visitor_recurse_*.rs`, `visitor_multicycle_disjoint_params.rs` exist (homes: `recurse_core.rs::problems`, `visitor_recurse_shapes.rs`, `recurse_visitor_cycles.rs::{generics,multiroot}`, `visitor_audits.rs`, `visitor_recurse.rs::disjoint_params`). Two range corrections vs the plan: the stale header is L5–7 (not 5–10 — **L8–10 are accurate and must stay**), and L31's parenthetical opens mid-L30.

**What changes**:
- Delete L4–7 (blank `//!` spacer + `//! Companion to \`recurse_problems_test.rs\` …` through `…recurse_generics.rs.`); keep L9–10.
- Delete L20–21 (`// (2. nested containers … visitor_nested_containers.rs.)`).
- Delete L25 only (`//    (Lifetime / type / const params … — see recurse_generics.rs.)`); keep L23–24.
- Edit L30 to strip the trailing `(Multi-root` and delete L31 (`cycles where every cycle passes through a root ARE supported — see recurse_multiroot.rs.)`).
- Delete L41–44 (the `audit_visitor_recurse_*.rs` / `visitor_multicycle_disjoint_params.rs` parenthetical).
- **Optional (+~35 lines, recommended)**: move the file's five `t.compile_fail(...)` lines (L18, 26, 32, 37, 45) into `recurse_core.rs::problems::compile_fail_problems` (keeping their one-line lead comments, rewritten current-tense) and delete the file. No `.stderr` impact — trybuild keys on the ui path. Companion (out of the strict finding scope, same pattern): the stale names also linger at `ui/recurse_missing_root_param.rs:7-8` and `ui/recurse_multiroot_rootless_subcycle.rs:6`; **do not** touch those in this step — editing a ui fixture's header shifts its `.stderr` line pins (same trap as step 25). Leave them or fold them into step 25's regen batch deliberately.

**Risk: low** (the driver file itself has no `.stderr`). Verify: `cargo test --workspace`.

### 25. Rewrite `ui/problem{1,3,5,7}_*.rs` headers to current behavior (~10 lines net) + regen their `.stderr` [comment]

Evidence: `problem1`/`problem5` headers narrate the removed `__ExprRec`-renaming/`effective_roots` architecture while their `.stderr` actually opens with the natural-type by-value-cycle abort; `problem3`/`problem7` headers were re-verified as **still accurate** (the `Visibility::Public` gate and first-segment-only `collect_refs` both still exist), so those two get optional tightening only. Correction to the plan: problem3's actual pass-through error is **E0072** ("recursive type has infinite size" family), not E0392 — the replacement text below reflects the real diagnostic.

**What changes** (headers only; content per file):
- `problem1_trait_impl.rs` L1–5 → 2 lines: `// Problem 1: \`Expr<S>\` self-references by value (no Box), so #[recurse]'s by-value-cycle guard aborts` / `// before any transform; rustc's E0072/type-param/derive errors then cascade on the untransformed module.` Also fix the stale inline L21 (`// Trait impl targets \`Expr<S>\` — after #[recurse] this name is gone.` → `// The trait impl passes through verbatim onto the natural type.`).
- `problem5_multiple_roots.rs` L1–4 → 2 lines: same by-value-cycle-abort statement, multi-type form (`the cycle (Forest, Tree)`); rewrite the inline block L13–15 likewise.
- `problem3_pub_crate.rs`: header L1–3 already accurate — keep; fold the duplicate inline block L12–13 into it (delete the inline block).
- `problem7_multiseg_path.rs`: header L1–5 accurate — optionally compress to 2 lines; inline block L18–20 may stay.

**Mandatory companion**: all four files are trybuild fixtures whose `.stderr` pins literal `line:col` positions — any header line-count change shifts them. After editing, run `TRYBUILD=overwrite cargo test -p syan --test recurse_core problems` to regenerate **exactly** `problem1_trait_impl.stderr`, `problem3_pub_crate.stderr`, `problem5_multiple_roots.stderr`, `problem7_multiseg_path.stderr`; then diff-review that **only** line/col numbers shifted (any diagnostic-text change indicates an unrelated regression — abort and investigate); then re-run clean. Do this step **after** step 4 (problem2/8 already deleted → only 4 files to regen) and last in the slice.

**Risk: medium** (raised from the plan's low — "comment-only" in intent but `.stderr`-shifting in effect). Verify: `TRYBUILD=overwrite cargo test -p syan --test recurse_core problems` (regen, diff-review) then `cargo test --workspace`.

---

**Final doc-sync pass (companion, not a finding)**: one consolidated edit of `CLAUDE.md` in the worktree — L11 (drop `ast_recurse.rs`), L47 + L85 (retarget/drop `visitor_reduce.rs`), L66 (drop `ui/visitor_edit_marker_mismatch.rs`), L201–207 (drop `multiroot`/`multicycle` from the `visitor_recurse.rs` mod list, move `closure_over_recurse_cycle` under `via_visitor`, drop `/heterogeneous` from `visitor_recurse_mixed.rs`) — plus the optional historical-doc touch-ups (`docs/recurse-natural-types-plan.md:103,303`, `docs/recurse-unbounded-plan.md:248`, `docs/visitor-edit-plan.md:299,312,345`). Then a final full `cargo test --workspace`.

## Slice `macro-visitor` — detailed design

All 14 findings verified against `/home/yasuo/ghq/github.com/yasuo-ozu/syan2-reduce` (commit `e5d0576`
+ uncommitted split). Line citations below are the worktree's *current* numbers — most match the plan
exactly; a few comment findings drift by ±1 line from the plan's citation (noted inline) with no effect
on the finding's validity. Execution order below follows the plan's own listing order, which already
sequences correctly: dead-code (1–2) → dup-code (3–8) → emit-reduction (9) → comment (10–14). Estimated
~74 lines saved, matching the plan header. **0 rejected** — every finding holds on close reading; two
(#4, #6) have real emission-visible side effects, both shown safe below. Two findings (#6/#7/#8/#13) share
`macro/visitor/build_input.rs` at disjoint line ranges — apply in listed order, no conflicts.

### 1. `macro/visitor/build_input.rs:47` — drop `#[derive(Clone)]` on `AncIn` [dead-code]

- Evidence (condensed): `AncIn` is only constructed (`build_input.rs:77`, `visitor.rs:252,271`),
  iterated by reference (`emit_ancestors`, `build_input.rs:87`), or field-cloned (`a.path.clone()`,
  `a.names.clone()`); grep for a whole-value `.clone()` on `AncIn`/`Vec<AncIn>` is empty.
- **Code shape**: one-line deletion.
  ```diff
  -#[derive(Clone)]
   pub(crate) struct AncIn {
       pub(crate) path: Path,
       pub(crate) names: Vec<Ident>,
   }
  ```
- **Emission**: unaffected — `AncIn` is a proc-macro-internal struct, never in generated tokens.
- **Ordering**: independent of every other finding in this slice (no shared lines).
- **Risk**: low. **Verify**: `cargo build -p syan-macro` (a stray `.clone()` on a whole `AncIn` would be
  the only possible break, and grep confirms none exists).

### 2. `macro/visitor/util.rs:143,153` — drop `PartialEq` from `Container`/`LayerKind` [dead-code]

- Evidence (condensed): both enums are consumed only by pattern match (`lower.rs:84-89,342`,
  `util.rs:250`) or move (`view: Option<Container>` in `lower_field`); grep for `==`/`!=`/`.contains`
  on either type across `macro/` is empty; both are `pub(crate)` so no downstream impact.
- **Code shape**:
  ```diff
  -#[derive(Clone, Copy, PartialEq)]
  +#[derive(Clone, Copy)]
   pub(crate) enum Container { Seq, Opt }
   ...
  -#[derive(Clone, Copy, PartialEq)]
  +#[derive(Clone, Copy)]
   pub(crate) enum LayerKind { View, Raw }
  ```
  `Copy`/`Clone` stay (both are moved-out-of-`Option` in `lower_field`, `lower.rs:289,330`).
- **Emission**: unaffected — same as #1.
- **Ordering**: independent; touches `util.rs`, which no other finding in this slice touches.
- **Risk**: low. **Verify**: `cargo build -p syan-macro`.

### 3. `macro/visitor/side.rs` — fold the seq/opt emission pair into `Vec<ViewSpec>` [dup-code]

- Evidence (condensed): the `has_seq`/`has_opt` blocks in `trait_def` (`side.rs:259-280`) and
  `blanket_ref_impl` (`side.rs:292-301`) are token-for-token parallel (`SeqView`/`OptView`,
  `view_iter_mut`/`get_mut`, `#p_vw`/`#p_ow`, `seq_method`/`opt_method`); `seq_doc`/`opt_doc`
  (`side.rs:135-143`) differ only in wording; none of `has_seq/has_opt/seq_method/opt_method/
  seq_doc/opt_doc` is referenced outside `side.rs` (confirmed by grep — safe to restructure).
- **Concrete code shape.** Add one struct, remove six `S` fields for one:
  ```rust
  /// One container-edit view (`#[seq]` or `#[opt]`) generated for a visited type. Replaces the
  /// paired `has_seq`/`has_opt` bools + `seq_method`/`opt_method` + `seq_doc`/`opt_doc` fields on `S`
  /// with a single `Vec` so `trait_def`/`blanket_ref_impl` iterate once instead of two parallel
  /// `#(if ..)` blocks.
  struct ViewSpec {
      /// `visit_<name>_seq` / `visit_<name>_opt`.
      method: Ident,
      /// Doc string for the trait method (was `S::seq_doc`/`S::opt_doc`).
      doc: String,
      /// `::syan::visit::SeqView` / `::syan::visit::OptView` — bound root, sans `<Ty>`.
      view_trait: TokenStream,
      /// The per-method generic naming the view type: `p_vw` for seq, `p_ow` for opt (the *same*
      /// idents `gen_side` already mints once via `fresh_ident` — not reminted per type/kind).
      view_param: Ident,
      /// Trait-method default body. NOT further folded: the seq body is a `for .. in
      /// view_iter_mut(v)` loop, the opt body an `if let Some(..) = get_mut(v)` — genuinely
      /// different shapes, only the *emission site* (trait_def/blanket_ref_impl) is shared.
      default_body: TokenStream,
  }
  ```
  `struct S` loses `seq_doc: String, opt_doc: String, seq_method: Ident, opt_method: Ident,
  has_seq: bool, has_opt: bool` and gains `views: Vec<ViewSpec>` (6 fields → 1).
- **Builder** (inside the existing `.map(|t| {...})` closure that builds `sides`): hoist the
  already-duplicated `method_ident_m(&ident, mutable)` call into one `let method = ...;` (currently
  called twice — once for the `mname` doc string, once for the `S.method` field — reusing it is a
  free-standing minor simplification, not required by the finding but adjacent and harmless), then:
  ```rust
  let mut views = Vec::new();
  if mutable && seq_used.contains(&name) {
      views.push(ViewSpec {
          method: Ident::new(&format!("visit_{}_seq", to_snake(&ident)), Span::call_site()),
          doc: seq_doc,           // unchanged string, built exactly as today
          view_trait: quote!(::syan::visit::SeqView),
          view_param: p_vw.clone(),
          default_body: quote! {
              for __syan_e in ::syan::visit::SeqView::view_iter_mut(v) {
                  self.#method(__syan_e);
              }
          },
      });
  }
  if mutable && opt_used.contains(&name) {
      views.push(ViewSpec {
          method: Ident::new(&format!("visit_{}_opt", to_snake(&ident)), Span::call_site()),
          doc: opt_doc,
          view_trait: quote!(::syan::visit::OptView),
          view_param: p_ow.clone(),
          default_body: quote! {
              if let ::core::option::Option::Some(__syan_e) = ::syan::visit::OptView::get_mut(v) {
                  self.#method(__syan_e);
              }
          },
      });
  }
  ```
- **`trait_def` rewrite** (`side.rs:245-283`), replacing the two `#(if s.has_seq)`/`#(if s.has_opt)`
  blocks with one loop (loop var named `spec`, not `v`, to avoid reading confusion with the emitted
  parameter literal `v:` inside the quote body — `template_quote` only substitutes `#{...}`
  interpolations, so a bare `v` token in the quote is unaffected either way):
  ```rust
  #(for spec in &s.views) {
      #[doc = #{&spec.doc}]
      fn #{&spec.method}< #(for mp in &s.method_params) { #mp, } #{&spec.view_param}: #{&spec.view_trait}< #{&s.ty} > >(
          &mut self,
          v: &mut #{&spec.view_param},
      ) #{&s.trait_where} {
          #{&spec.default_body}
      }
  }
  ```
- **`blanket_ref_impl` rewrite** (`side.rs:285-305`), same fold, forwarder body (no `default_body`
  needed here — a distinct UFCS forward, reusing only `method`/`view_param`/`view_trait`):
  ```rust
  #(for spec in &s.views) {
      fn #{&spec.method}< #{&spec.view_param}: #{&spec.view_trait}< #{&s.ty} > >(&mut self, v: &mut #{&spec.view_param}) {
          <#p_v as #visit_tr #g_use>::#{&spec.method}(self, v)
      }
  }
  ```
- **Byte-identical emission**: the resulting token stream for a type with both `#[seq]` and `#[opt]`
  usage is the same two `fn` items in the same order (seq pushed before opt, matching the original
  `has_seq` block preceding `has_opt`); for a type with only one or neither, the loop emits exactly
  that many items — identical to the original `#(if ..)` gating. `view_trait`/`view_param` splice to
  the same tokens (`::syan::visit::SeqView< Ty >` etc.) as the inline original. Not literally
  byte-for-byte on whitespace (template-quote token trees don't preserve source spacing either way),
  but the *token stream* — what rustc and every existing test observes — is unchanged.
- **Ordering/conflicts**: touches `side.rs` lines ~55-171 (struct + builder) and ~245-305
  (`trait_def`/`blanket_ref_impl`). Finding #11 (comment cleanup) also touches the "Opt-in
  container-edit hooks…" comment that sits immediately above the old `has_seq`/`has_opt` blocks
  (`side.rs:257-258`) — apply #3 first (it restructures that region into the `views` loop), then #11
  removes the now-adjacent redundant comment. Finding #9 (use hoist) touches a disjoint region
  (`free_fns`, `side.rs:307-326`) — no conflict, order between #3 and #9 doesn't matter, but #9 is
  listed after per the plan's dup-code-before-emit-reduction convention.
- **Risk**: low (no `.stderr`-visible surface — `#[seq]`/`#[opt]` methods are exercised only by
  compiling/running real visitors, e.g. `visitor_edit.rs`, `visitor_reduce.rs`,
  `visitor_edit_containers.rs`, `visitor_edit_group.rs`, `rust/tests/cross_crate_edit.rs`).
  **Verify**: `cargo build -p syan-macro && cargo test -p syan --test visitor_edit --test
  visitor_edit_containers --test visitor_edit_group && cargo test --workspace`.

### 4. `macro/visitor/lower.rs:84-89,~286-299,~329-341` — merge the `#[seq]`/`#[opt]` marker aborts [dup-code]

- Evidence (condensed): the two abort bodies differ by exactly the substring `"single "`; message A
  (`peel` returns `None` — no followed head at all) is pinned verbatim by
  `core/tests/ui/visitor_edit_marker_unvisited.stderr`; message B ( `peel` succeeds but the resolved
  head isn't in `method_set` — an unlisted-intermediate or tuple head) is grepped **zero** times
  across every `core/tests/ui/*.stderr` (`grep -rn "single container of a type listed"
  core/tests/ui/` → empty) — genuinely unpinned by any UI fixture (confirmed: none of the 5
  `visitor_edit_marker_*.rs` fixtures reach message B; `_unvisited.rs` hits message A via a plain
  `Vec<String>` field with no followed head).
- **Exact diverging helper.** `marker_word` (`lower.rs:84-89`) is folded in (its only two callers are
  the two abort sites being merged):
  ```rust
  /// Abort: a `#[seq]`/`#[opt]`-marked field's element isn't a visited type. `single` distinguishes
  /// the two call sites: `false` when `peel` found no followed head at all (a leaf field, e.g.
  /// `Vec<String>`); `true` when a head *was* found but it resolves to an unlisted intermediate or a
  /// tuple, not a `visitor!(..)`-listed type. Only the `false` (first) wording is UI-pinned
  /// (`core/tests/ui/visitor_edit_marker_unvisited.stderr`) — the `true` wording is unpinned
  /// anywhere in the trybuild suite, so this merge is free to standardize on the pinned text plus one
  /// inserted word.
  fn abort_marker_not_visited(ty: &Type, kind: &Container, single: bool) -> ! {
      let marker = marker_word(kind);
      let extra = if single { "single " } else { "" };
      abort!(
          ty,
          "a `#[{}]` field's element type is not a visited type — mark only a field whose element is \
           a {}container of a type listed in `visitor!(..)` (or reached via `#[subast]`)",
          marker,
          extra
      );
  }
  ```
  (`marker_word` itself — `fn marker_word(kind: &Container) -> &'static str` — stays as a private
  helper called once inside `abort_marker_not_visited`, replacing its two direct call sites.)
- **Call-site rewrite**:
  ```diff
  // site A (peel == None, lower.rs ~286-299)
  -if let Some(kind) = view {
  -    let marker = marker_word(&kind);
  -    abort!(ty, "a `#[{}]` field's element type is not a visited type — mark only a field whose \
  -                element is a container of a type listed in `visitor!(..)` (or reached via \
  -                `#[subast]`)", marker);
  -}
  -return None;
  +if let Some(kind) = view {
  +    abort_marker_not_visited(ty, &kind, false);
  +}
  +return None;

  // site B (mutable marker branch, lower.rs ~329-341)
  -_ => abort!(ty, "a `#[{}]` field's element type is not a visited type — mark only a field whose \
  -              element is a single container of a type listed in `visitor!(..)` (or reached via \
  -              `#[subast]`)", marker),
  +_ => abort_marker_not_visited(ty, &kind, true),
  ```
- **Emission-visible change**: message B's wording changes (loses nothing pinned — it's unpinned) but
  its *text* stays character-identical to today's (the `abort_marker_not_visited(_, _, true)` call
  reproduces the exact current site-B string); this is a pure refactor, not a wording change, despite
  the task's request to double-check for "abort-message merge" risk — the two messages are **not**
  merged into one text, they're merged into one *generator* that reproduces both texts unchanged.
  So this finding, correctly implemented as specified above (site B keeps `single`), changes **zero**
  characters of emitted diagnostic text. (A more aggressive merge that also dropped `single` from site
  B, as one misreading of the plan's phrasing might suggest, would be safe too per the empty-grep
  check above — but is not necessary and not what "keep the wording of the first variant" implies:
  that phrase means "when in doubt about which of the two to standardize on, keep A's" — here both are
  kept via the `single` flag, so no doubt exists.)
- **Ordering/conflicts**: touches `lower.rs` lines ~84-89 (delete `marker_word`'s two direct call
  sites, keep the fn), ~286-299 and ~329-341. Finding #12 (comment cleanup) touches `lower.rs:145`
  (unrelated fn `visit_value`) and `~307-309`/`428` (between and after the two abort sites, but not
  overlapping them) — apply in either order relative to #4, no line conflicts.
- **Risk**: low. **Verify**: `cargo build -p syan-macro && cargo test -p syan --test
  visitor_diagnostics -- --include-ignored` then `git diff core/tests/ui/*.stderr` — must be **empty**
  (no `TRYBUILD=overwrite` needed for this slice; message B has no existing pin to break).

### 5. `macro/visitor.rs:308-321,400-407` — dedup `lower`/`lower_mut` and the two `gen_side` calls [dup-code]

- Evidence (condensed): the two `Lower { .. }` struct literals (`visitor.rs:308-321`) are
  field-for-field identical except `mutable: false`/`true`; the two `gen_side(..)` calls
  (`visitor.rs:400-407`) are identical except the leading bool — both pure parameterization.
- **Code shape** — closure form for `Lower` (keeps two named bindings `lower`/`lower_mut` since both
  are used by name later at `visitor.rs:347,349-350`, so a `[..].map()` array form would need
  destructuring back into two names anyway; a closure is the smaller diff):
  ```rust
  let mk_lower = |mutable: bool| Lower {
      method_set: &method_set,
      done_by_path: &done_by_path,
      mutable,
      seq_used: &seq_used,
      opt_used: &opt_used,
  };
  let lower = mk_lower(false);
  let lower_mut = mk_lower(true);
  ```
  and for the `gen_side` pair — array-map form works cleanly here since both results are consumed only
  by name (`shared`, `mutable`) in the final `quote!`:
  ```rust
  let [shared, mutable] = [false, true].map(|m| {
      gen_side(
          m, &vtypes, &g_params, &g_args, &g_def, &g_use, &base_g_use, &ancestors, &st.base,
          &union_where, struct_only, &seq_used, &opt_used,
      )
  });
  ```
- **Emission**: unaffected — `mk_lower(false)`/`mk_lower(true)` and the array-map produce the exact
  same two `Lower` values / `gen_side` outputs as the two inlined call sites; no token-level change.
- **Ordering/conflicts**: touches `visitor.rs:308-321` (disjoint from #7's target `~152-159`) and
  `~400-407` (disjoint from #9's target `~415-427` and #10's targets `83,160,342-343,382`). Apply
  after #7 and #10 if doing a single combined `visitor.rs` pass (see slice-level ordering note below),
  but no line ranges actually overlap so order among {5,7,9,10} within `visitor.rs` is flexible.
- **Risk**: low. **Verify**: `cargo build -p syan-macro && cargo test --workspace`.

### 6. `macro/visitor/entry.rs:87-98` + `build_input.rs` rest-bounce block — shared `state_tokens` serializer [dup-code, risk medium]

- Evidence (condensed): `entry.rs`'s `make_state` closure (87-98) and `build_input.rs`'s rest-bounce
  `quote!` (in `pub fn build`, the `if !st.rest.is_empty() { .. }` block) emit the same `@`-section
  skeleton; entry's is a strict subset (omits `@baseg`/`@anc`, which `BuildInput::parse` defaults to
  empty when the section is absent).
- **Exact diverging helper**:
  ```rust
  /// Serialize one `__visitor_build` ping-pong bounce's full state payload. Shared by `entry` (the
  /// first bounce — `inherited`/`base_generics`/`anc`/`done` are always empty, nothing fetched yet)
  /// and `build` (every later bounce, carrying the accumulated state). Content pieces that need
  /// their own rendering (`@base`, `@anc`, `@done`) are passed pre-rendered so this fn stays a pure
  /// section-list assembler.
  fn state_tokens(
      base: &TokenStream,             // base_tokens(&base_path) or quote!()
      build: &Path,
      nonce: &TokenStream,
      visited: &[Path],
      inherited: &[Ident],
      base_generics: &[GenericParam],
      anc: &TokenStream,              // emit_ancestors(&base_ancestors) or quote!()
      fetching: &TokenStream,
      done: &TokenStream,             // emit_done(&done) or quote!()
      rest: &[Path],
  ) -> TokenStream {
      quote! {
          @base { #base }
          @build { #build }
          @nonce { #nonce }
          @visited { #(#visited),* }
          @inherited { #(#inherited)* }
          @baseg { #(#base_generics),* }
          @anc { #anc }
          @fetching { #fetching }
          @done { #done }
          @rest { #(#rest),* }
      }
  }
  ```
- **`entry.rs` call site** (replaces `make_state`):
  ```rust
  let base_ts = base_tokens(&args.base);
  let make_state = |fetching: TokenStream, rest: &[Path]| {
      state_tokens(&base_ts, &build, &nonce, all_types, &[], &[], &quote!(), &fetching, &quote!(), rest)
  };
  ```
- **`build_input.rs` call site** (replaces the inline rest-bounce `quote!`):
  ```rust
  let base_ts = base_tokens(base);
  let done_ts = emit_done(done);
  let anc_ts = emit_ancestors(base_ancestors);
  let state = state_tokens(
      &base_ts, build, nonce, visited, inherited, base_generics, &anc_ts, &quote!(#next), &done_ts, rest,
  );
  return quote! { #next ! { @ast #build { #state } } };
  ```
- **Emission-visible change (the reason for "medium risk"):** `entry.rs`'s first-bounce payload today
  **omits** the `@baseg`/`@anc` sections entirely; after this merge it emits them as `@baseg { }`
  `@anc { }` (present, empty). This is a real token-stream change to an **intermediate**
  macro-to-macro payload — but it is not user-visible generated code: it is the private
  `__visitor_build` ping-pong argument, never part of any expanded item, doc comment, or public API,
  and no test snapshots it (confirmed: whole-worktree grep finds no consumer/test of this token shape
  other than `BuildInput::parse` itself). Safety: `BuildInput::parse`'s `"baseg" | "bg"` arm is
  guarded (`if !content.is_empty() { base_generics = .. }`) so an empty `@baseg { }` leaves
  `base_generics` at its already-empty default — a no-op; the `"anc" | "an"` arm is **unguarded**
  (`base_ancestors = parse_ancestors(content)?`) but `parse_ancestors` on empty input hits its
  `while !input.is_empty()` loop's false branch immediately and returns `Ok(vec![])` — also a no-op,
  and this exact empty-content path is already exercised today by `entry.rs`'s existing `@done { }`
  (also always present-but-empty on the first bounce) and by `emit_ancestors(&[])` on every no-base
  bounce elsewhere in the pipeline. So the change is behaviorally inert, just newly-present-but-empty
  sections in a private wire format.
- **Ordering/conflicts**: `build_input.rs` is also touched by #7 (`~377-382`), #8's caller
  (`~383,391`), and #13 (`~204`, `~365-366`) — all at line ranges strictly before this finding's
  target (the rest-bounce block is near the end of `pub fn build`, after the `just_def`-handling block
  #7/#8/#13 touch). No overlap; apply in the plan's listed order (6 before 7/8/13, per the raw
  finding list) or reorder freely — line ranges don't intersect either way.
- **Risk**: medium (per plan — the only non-mechanical risk in this slice, from the `@baseg`/`@anc`
  presence change above; verified safe). **Verify**: `cargo build -p syan-macro && cargo test
  --workspace`, with extra attention to `core/tests/visitor_inherit.rs` (mods `basic`/`arity`/
  `multilevel` — exercises multi-level `base => mid => New` inheritance, the only path that actually
  populates `@baseg`/`@anc` with non-empty content and round-trips through both `entry` and `build`)
  and `rust/tests/cross_crate_inherit*.rs`.

### 7. `macro/visitor/build_input.rs:377-382` — extract `BuildInput::method_set()` [dup-code]

- Evidence (condensed): `build_input.rs:377-382` and `visitor.rs:155-159` (confirmed exact line
  numbers via grep) both compute `HashSet<String>` = `st.visited` mapped through
  `last_ident(..).to_string()` chained with `st.inherited` idents-as-strings.
- **Code shape**:
  ```rust
  impl BuildInput {
      /// Visited types' last-idents ∪ inherited idents — the set of heads that dispatch via a
      /// `visit_*` method call rather than being drilled/leaf.
      fn method_set(&self) -> HashSet<String> {
          self.visited
              .iter()
              .map(|p| last_ident(p).to_string())
              .chain(self.inherited.iter().map(|i| i.to_string()))
              .collect()
      }
  }
  ```
- **`build_input.rs:377-382` call site**: `let method_set = st.method_set();` (was the 5-line
  expression, now reading `self.visited`/`self.inherited` instead of `st.visited`/`st.inherited` — same
  fields, same set).
- **`visitor.rs:155-159` call site**: currently builds from a *different* intermediate
  (`visited: HashSet<String> = path_of.keys().cloned().collect()`, itself derived from `st.visited`)
  chained with `st.inherited`. Since `path_of`'s keys are exactly `last_ident(p).to_string()` for each
  `p` in `st.visited` (built two lines earlier as `path_of: HashMap<String, &Path> = st.visited.iter()
  .map(|p| (last_ident(p).to_string(), p)).collect()`), `visited.iter().cloned().chain(..)` and
  `st.method_set()` are the same set — replace with `let method_set = st.method_set();`.
- **Emission**: unaffected (both sites already produce the identical `HashSet<String>`; the extraction
  changes no downstream consumer, e.g. `Lower { method_set: &method_set, .. }` at `visitor.rs:309,316`
  is untouched).
- **Ordering/conflicts**: `build_input.rs` region (~377-382) is immediately followed by #8's caller
  edit (~383,391 — `self_ident`) in the same `if let Some(def) = ..` block; apply #7 first so #8's
  diff lands cleanly against the already-shrunk block. `visitor.rs`'s half touches ~155-159, adjacent
  to but not overlapping #10's comment deletion at line 160 (which follows immediately after and can
  be deleted in the same pass without conflict, since it's the *next* line, not a shared line).
- **Risk**: low. **Verify**: `cargo build -p syan-macro && cargo test --workspace`.

### 8. `macro/visitor/discover.rs:12-15` — reuse `self_and_subast_keys` [dup-code]

- Evidence (condensed): `discover.rs:12-15`'s inline `HashSet` build is the same construction as
  `self_and_subast_keys` (`params.rs:34-40`, confirmed exact line match) modulo `&str` vs `&Ident`;
  `self_and_subast_keys` has exactly one existing caller (`lower.rs:283`, confirmed via grep).
- **Code shape**: change `followed_intermediates`'s and `discover_followed`'s `self_ident` parameter
  from `Option<&str>` to `Option<&Ident>`:
  ```diff
   pub(crate) fn followed_intermediates(
       def: &Item,
       subast: &[SubEntry],
       method_set: &HashSet<String>,
  -    self_ident: Option<&str>,
  +    self_ident: Option<&Ident>,
   ) -> Vec<Path> {
  -    let mut user_types: HashSet<String> = subast.iter().map(|e| e.key.to_string()).collect();
  -    if let Some(s) = self_ident {
  -        user_types.insert(s.to_string());
  -    }
  +    let user_types = self_and_subast_keys(self_ident, subast);
       ...
   }
  ```
  and in `discover_followed` (signature `self_ident: Option<&Ident>` too), the compare at
  `discover.rs:44` becomes an `Ident` compare:
  ```diff
  -let hs = head.to_string();
  -if Some(hs.as_str()) == self_ident {
  +if Some(head) == self_ident {
       return; // self -> already in `done`
   }
  ```
  (`hs` is dropped entirely — its only other use, `subast.iter().find(|e| &e.key == head)`, already
  uses `head` directly, not `hs`.)
- **Caller update** (`build_input.rs:383,391`):
  ```diff
  -let self_ident = item_ident(&def).map(|i| i.to_string());
  +let self_ident = item_ident(&def);
   ...
  -followed_intermediates(&def, &subast, &method_set, self_ident.as_deref())
  +followed_intermediates(&def, &subast, &method_set, self_ident)
  ```
  (`item_ident(&def)` already returns `Option<&Ident>` — the `.map(|i| i.to_string())`/`.as_deref()`
  pair was only there to satisfy the old `Option<&str>` signature.)
- **Emission**: unaffected — pure internal set-construction refactor, no generated tokens involved.
- **Ordering/conflicts**: `discover.rs` is touched by no other finding in this slice. The
  `build_input.rs` caller edit sits at `~383,391`, immediately after #7's `~377-382` — apply #7 then
  #8 as one contiguous pass over that block.
- **Risk**: low. **Verify**: `cargo build -p syan-macro && cargo test --workspace` (drill-in behavior
  specifically covered by `core/tests/visitor_drill*.rs`).

### 9. `macro/visitor/side.rs:317-320` (+ `visitor.rs` final quote) — hoist the `use ::syan::visit::{OptView as _, SeqView as _}` [emit-reduction]

- Evidence (condensed): only the free-fn bodies (`side.rs:307-326`, embedding `s.body`/`s.body_mut`
  from `Lower::destructure`, which calls `fold_containers` → bare `.view_iter()`/`.view_iter_mut()`
  method calls) need the traits nameable in scope; `trait_def`'s seq/opt defaults and
  `blanket_ref_impl`'s forwarders use fully-qualified UFCS (`::syan::visit::SeqView::view_iter_mut(v)`
  at the former `side.rs:265`, now inside `ViewSpec::default_body` per finding #3; and
  `<#p_v as #visit_tr #g_use>::method(..)` in `blanket_ref_impl`) — confirmed by reading every call
  site in `side.rs`. `visitor!(..)` is documented as invoked inside an (initially empty) `mod`, and
  `generate_module`'s final `quote!` (`visitor.rs:415-427`) already emits ancestor-trait `use`s
  directly into that module.
- **Code shape** — delete from `free_fns` (`side.rs:317-320`):
  ```diff
   pub fn #{&s.method}< .. >(this: &mut #p_v, i: #amp #{&s.ty}) #{&s.free_where} {
  -    #[allow(unused_imports)]
  -    use ::syan::visit::{OptView as _, SeqView as _};
       #{&s.body}
   }
  ```
  add once in `generate_module`'s final `quote!` (`visitor.rs`, next to the ancestor `use` loop):
  ```diff
   quote! {
       #visited_macro

       #(for a in &ancestors) {
           #[allow(unused_imports)]
           use #{&a.path}::{Visit as _, VisitMut as _};
       }

  +    #[allow(unused_imports)]
  +    use ::syan::visit::{OptView as _, SeqView as _};
  +
       #shared
       #mutable
   }
  ```
- **Why this is safe despite changing emission**: `#shared`/`#mutable` (the `gen_side` outputs) are
  spliced as *direct items of the same enclosing module* — not nested in a sub-`mod` or a `fn` body —
  so every free fn generated inside them sits in the same lexical scope as the hoisted `use`. Rust
  resolves an unqualified trait method call (`.view_iter_mut()`) by scanning traits in scope at *any*
  lexically enclosing level, function-local or module-level; a module-level `use Trait as _` is visible
  to every function textually inside that module exactly as if each function repeated it locally. Since
  `as _` binds no name, one copy vs. `2 × (#visited types)` copies cannot conflict or shadow anything.
  The **only** call sites that need the trait nameable (`view_iter`/`view_iter_mut`, unqualified) are
  the free-fn bodies — confirmed no other generated item calls these methods unqualified (`trait_def`'s
  seq/opt defaults and `blanket_ref_impl`'s forwarders are fully UFCS/associated-fn calls, immune to
  scope).
- **Net emission delta**: expansion shrinks by `2 × (#visited types) − 1` `use` lines net (was `2 ×
  N`, now `1`) plus one `#[allow(unused_imports)]`; this is an intentional, stated emission change
  (unlike #1–#8, which are token-identical).
- **Ordering/conflicts**: `side.rs` half (~307-326) is disjoint from #3's target (~55-171,245-305) and
  #11's targets (178,244,257-258) — apply after #3 (dup-code-before-emit-reduction convention; no
  actual line dependency). `visitor.rs` half (~415-427) is disjoint from #5 (~400-407) and #10
  (83,160,342-343,382) — safe in any order.
- **Risk**: low (no `.stderr` surface; `use ... as _` never appears in a diagnostic). **Verify**:
  `cargo build -p syan-macro && cargo test --workspace`, plus a spot check that
  `-D unused-imports`/clippy stays clean: `cargo clippy -p syan-macro -p syan -- -D warnings` (per the
  plan's overall post-slice gate).

### 10. `macro/visitor.rs:83,160,342-343,382` — delete narration comments [comment]

- Evidence (condensed): all four restate the adjacent line or a field doc elsewhere, confirmed by
  direct read: L83 "Recurse into every type argument…" sits directly above the `for arg in &ab.args`
  loop it restates; L160 "Fetched types keyed by full path…" duplicates `Lower::done_by_path`'s doc
  (`lower.rs:98-99`); L342-343 restates the `let scrut_path = path_of.get(..).unwrap_or(&d.path);`
  line immediately below (confirmed at current line 344, one line off the plan's citation — same
  content); L382 "The mut walk has finished…" narrates `seq_used.into_inner()`/`opt_used.into_inner()`.
- **Code shape**: four straight deletions, no logic change.
- **Emission**: unaffected — doc/line comments in macro source never appear in generated tokens.
- **Ordering/conflicts**: L160's deletion is adjacent to #7's `method_set` extraction (which ends at
  what becomes the new line ~156) — apply #7 first so #10 deletes the correct (shifted) line. L342-343
  is untouched by any other finding. L83/L382 are in `has_concrete_fill`/`generate_module`, regions no
  other finding here touches.
- **Risk**: low. **Verify**: `cargo build -p syan-macro` (comment-only; `cargo test --workspace` as
  the slice-level gate, not specifically required per-finding).

### 11. `macro/visitor/side.rs:178,~244,257-258` — delete stale/duplicate comments [comment]

- Evidence (condensed): "// Generated API docs." (current line **178**, plan cites 179 — 1-line
  drift, same content) is a bare section header adding nothing; "the final `quote!` splices them
  verbatim…" (**244**, part of a 2-line comment starting at 243) narrates a past refactor (the
  named-token-block-assembly style itself), not a live constraint; the in-quote comment "Opt-in
  container-edit hooks… Default: descend each held node via `visit_*_mut`" (**257-258**) duplicates
  the `seq_doc`/`opt_doc` text generated two lines below it (and, after finding #3, becomes `ViewSpec`
  doc text one level further down).
- **Code shape**: three deletions. Rationale comments explicitly kept per the plan (struct_only doc at
  16-19, `?Sized` note at 310-311, ancestor `Driver` impls note at 356-358) are untouched.
- **Emission**: unaffected.
- **Ordering/conflicts**: L257-258 sits directly above the `has_seq`/`has_opt` blocks that #3 replaces
  with the `views` loop — **apply #3 before #11** so #11's deletion targets the post-restructure
  location (the comment would otherwise sit above `#(for spec in &s.views)` — still applicable to
  delete, just at a shifted position). L178 and L243-244 are outside #3's/#9's touched regions — safe
  in any order relative to those.
- **Risk**: low. **Verify**: `cargo build -p syan-macro`.

### 12. `macro/visitor/lower.rs:145,~307-309,428` — delete/trim narration comments [comment]

- Evidence (condensed): L145 "Unlisted intermediate -> inline drill." (confirmed exact) restates what
  `Lower`'s struct doc already says (`lower.rs:91-94`); L307-309 (plan cites 308-310 — 1-line drift)
  "Dispatch at the innermost… and now `Vec<(A, B)>`" mixes a live one-sentence fact ("An empty body …
  ⇒ the whole field is a leaf") with historical narration ("and now") — trim to keep only the live
  sentence; L428 trailing `// tuple of only leaves -> leaf (empty body)` restates the early return and
  `lower_tuple`'s doc (`391-392`).
- **Code shape**:
  ```diff
  -        // Unlisted intermediate -> inline drill.
           let key = norm_path(drill_path);
  ```
  ```diff
  -        // Dispatch at the innermost (container-peeled) accessor, then wrap the container chain
  -        // (handles nested containers like `Vec<Option<T>>`, and now `Vec<(A, B)>`). An empty body
  -        // (a leaf head, or a finite drill reaching nothing) ⇒ the whole field is a leaf.
  +        // An empty body (a leaf head, or a finite drill reaching nothing) ⇒ the whole field is a leaf.
           let acc = innermost_acc(&p.conts, binding);
  ```
  ```diff
  -            return quote!(); // tuple of only leaves -> leaf (empty body)
  +            return quote!();
  ```
- **Emission**: unaffected.
- **Ordering/conflicts**: L145 sits in `visit_value` (untouched by #4). L307-309 sits between #4's two
  abort sites (~286-299 and ~329-341) but does not overlap either. L428 is in `lower_tuple`, after
  every other finding's target. No conflicts with #4; apply in either order.
- **Risk**: low. **Verify**: `cargo build -p syan-macro`.

### 13. `macro/visitor/build_input.rs:204,365-366` — delete duplicate-rationale comments [comment]

- Evidence (condensed): L204 (plan cites 203 — 1-line drift) `// crate::REST -> <host>::REST (replace
  only the leading segment)` duplicates the rule already spelled out in `requalify_ancestor`'s doc
  comment (`170-182`, specifically the bullet at 175); L365-366 `// Record the just-fetched type (def +
  subast) under the path it was fetched by, then discover the followed-but-unvisited intermediates it
  references and enqueue them for drilling.` narrates the visibly-named code immediately below
  (`st.done.push(..)`, the `followed_intermediates(..)` loop pushing to `st.rest`) without adding a
  constraint.
- **Code shape**: two deletions (L204: one line inside the `"crate" => { .. }` arm of
  `requalify_ancestor`; L365-366: the two-line comment immediately preceding `if let Some(def) =
  st.just_def.take() { .. }` in `pub fn build`).
- **Emission**: unaffected.
- **Ordering/conflicts**: L204 is in `requalify_ancestor`, untouched by #6/#7/#8. L365-366 is the very
  first thing inside the block that #7 (`~377-382`) and #8's caller (`~383,391`) also touch — apply
  #13's comment deletion first (or last; it's a pure comment removal at the top of the block, doesn't
  shift the meaning of the code #7/#8 edit below it) — no functional conflict either way, only a minor
  textual-diff-ordering convenience in doing it first.
- **Risk**: low. **Verify**: `cargo build -p syan-macro`.

### 14. `macro/visitor/params.rs:6` — trim stale half-sentence from `param_union`'s doc [comment]

- Evidence (condensed): confirmed exact line match. `grep -rn "param_union"` across the whole worktree
  finds exactly two hits: the definition (`params.rs:7`) and its one caller (`visitor.rs:179`) — no
  recurse-path code (`macro/recurse*.rs`) calls it, so "the recurse path additionally filters this to
  the cycle roots' params" is stale (recurse-over-visitor is now an ordinary acyclic visitor per
  CLAUDE.md — no depth/param-filtering machinery survives).
- **Code shape**:
  ```diff
   /// The deduped union of every target's generic params (first declaration wins), followed by the
   /// base's params (for inheritance — the new trait must declare them to name `base::Visit<base params>`
  -/// as a supertrait, so the new union must ⊇ the base's). The caller normalizes order with
  -/// `sort_lifetimes_first`; the recurse path additionally filters this to the cycle roots' params.
  +/// as a supertrait, so the new union must ⊇ the base's). The caller normalizes order with
  +/// `sort_lifetimes_first`.
   pub(crate) fn param_union(targets: &[&DoneType], base_generics: &[GenericParam]) -> Vec<GenericParam> {
  ```
- **Emission**: unaffected.
- **Ordering/conflicts**: `params.rs` is touched by no other finding in this slice.
- **Risk**: low. **Verify**: `cargo build -p syan-macro`.

---

**Slice-level verification.** After applying all 14 in the order above:
`cargo build -p syan-macro && cargo test --workspace && cargo clippy --workspace -- -D warnings`.
Only finding **#4** touches trybuild-adjacent code and only finding **#6** touches an
emission-affecting-but-unsnapshotted wire format; run `git diff core/tests/ui/*.stderr` after the
`visitor_diagnostics` test — it must come back **empty**. No `.stderr` file in this slice legitimately
needs `TRYBUILD=overwrite`.

## Slice `macro-recurse` — 12 findings, ~185 lines

### macro/recurse/build.rs : 277, 324  [dead-code] (~2 lines, risk low, verified)
  Delete the `_root_generics: &Generics` parameter from `build_multiroot_tail` and the
  `&root_generics` argument at the single call site (build.rs:277).
  - evidence: The parameter is underscore-prefixed and its name appears nowhere in the fn body (grep for `root_generics` in build.rs: only lines 87/108/111/277/301/324; 301 is the separate `RootData` field). `build_multiroot_tail` already receives the derived `gen_decl`/`gen_use`/`root_keys` it actually uses.
  - verifier: Confirmed dead parameter: `_root_generics` appears exactly once in the whole worktree (its declaration at macro/recurse/build.rs:324); the fn body (317–420) reads generics from `items` (`e.generics`/`s.generics`) and validates via the separately-passed `root_keys`. Exactly one call site (build.rs:270, arg at 277); all other mentions of `build_multiroot_tail` are comments/docs naming the fn, not its signature. The local `root_generics` in `build` stays live (lines 108/111/301 — the RootData field at 301 is a distinct data path into emit.rs), so no unused-binding/unused-import fallout. Deletion of the param + argument is semantically inert.

### macro/recurse/build.rs : 87-102, 165-186, 351-357  [dup-code] (~30 lines, risk low, unverified (low-risk class))
  Replace three inline 'find the enum/struct named X and clone its Generics' blocks with the
  existing names.rs helper `item_generics(items, name)`: (1) the `root_generics` lookup (16 lines
  -> `let root_generics = item_generics(items, &root_name);`), (2) the `root_ident_args` map
  (build per-root via `generic_tokens(&item_generics(items, r)).1`), (3) the multiroot per-root
  generics loop (the `if let Some(g)` None case is unreachable since every root name comes from an
  item in `items`). The Visibility::Public guard in these blocks is redundant: root/scc names
  originate from `pub_types` (macro/recurse.rs:72-79), and two same-named items in one module
  would not compile anyway.
  - evidence: Helper exists at macro/recurse/names.rs:51-60 and is already used for exactly this purpose at macro/recurse/emit.rs:19 and emit.rs:116; grep shows no other build.rs use. All three build.rs blocks perform an identical items-scan keyed on ident equality.

### macro/recurse.rs : 72-79, 92-100, 142-153, 186-193, 200-210, 254-260  [dup-code] (~30 lines, risk low, unverified (low-risk class))
  Add one helper, e.g. `fn adt_parts(item: &Item) -> Option<(&Ident, &Generics, &[Attribute])>`
  (pub items only), and use it at the ~8 sites that currently spell out the `Item::Enum(e) if
  matches!(e.vis, Visibility::Public(_)) => ... / Item::Struct(s) ... => ...` match: recurse.rs
  pub_types (72-79), the type_refs loop (92-100), `item_in_scc` + the local `item_attrs` fn
  (142-153), `parse_types` (186-193), `delegated_us` (200-210), the `cycle_name` match (254-260),
  plus build.rs:124-137 and emit.rs:583-592. Note: parse_types/delegated_us currently omit the vis
  check, but their results are only ever consulted for SCC members (build.rs:230,
  emit.rs:533-534/654/685/688), and SCCs contain only pub types (type_refs is built from pub items
  only), so adding the check is behavior-preserving.
  - evidence: The same 6-10 line two-arm match appears verbatim (modulo the extracted field) at all listed sites; e.g. recurse.rs:74-77 vs 94-95 vs 256-258 vs build.rs:126-135 vs emit.rs:585-590 differ only in which of ident/generics/attrs they return.

### macro/recurse/names.rs : 9-48  [dup-code] (~26 lines, risk low, unverified (low-risk class))
  Collapse the ten identical 3-line name-builder fns (engine_name, term_name, default_name,
  to_nat_name, from_nat_name, reentry_name, reentry_fn_alias, term_ref_name, reentry_unparse_name,
  reentry_span_name) into one `macro_rules!` table: `name_fns! { engine_name =>
  "__{name}Rec_{nonce}", term_name => "__{name}Term_{nonce}", ... }` expanding to `pub(crate) fn
  $f(name: &str, nonce: u64) -> Ident { Ident::new(&format!($fmt), Span::call_site()) }`. The
  format literals themselves document each name shape, so the per-fn doc comments (which just
  restate the format string) fold into the existing module-level nonce comment (lines 3-7).
  - evidence: All ten fns are byte-identical except the format string (names.rs:10-48); each body is `Ident::new(&format!(...), Span::call_site())` with the same `(&str, u64) -> Ident` signature. Grep confirms all ten are live (build.rs, convert.rs, emit.rs), so a macro-generated set is a drop-in.

### macro/recurse/emit.rs : 229-243 and 300-311; 246-253 and 313-321; 254-265 and 322-333  [dup-code] (~18 lines, risk low, unverified (low-risk class))
  Hoist the shared prologue of `emit_delegated_unparse` and `emit_delegated_spanned` into one
  helper: the `tr_args` closure producing `tr_b`/`tr_anon` is duplicated verbatim (233-243 vs
  302-310), and the other-roots bound builders (`from_bounds` vs `span_bounds`) and per-root
  `registrations` builders have the identical iterate-roots shape differing only in the quoted
  bound/key — parameterize with a `impl Fn(&RootReentry) -> TokenStream`. e.g. `fn
  borrow_engine_prologue(roots, nonce, root_use) -> (Vec<TokenStream>, Vec<TokenStream>)` plus `fn
  other_root_bounds(roots, self_name, f)`.
  - evidence: emit.rs:233-243 and emit.rs:302-310 are character-identical closures; emit.rs:246-253 vs 314-321 and 254-265 vs 322-333 differ only inside the innermost quote! (Unparse vs Spanned bound; ReKey<_, __Atom, __E::Error> vs ReKey<_, SpanReentry, #sp>).

### macro/recurse/graph.rs : 94-114, 144-163  [dup-code] (~14 lines, risk low, unverified (low-risk class))
  Extract a shared u32-keyed graph builder, e.g. `fn build_graph<'a>(nodes: Vec<&'a String>, adj:
  &HashMap<String, HashSet<String>>) -> (BTreeGraph<u32,u32>, Vec<&'a String>)`: both
  `find_cycle_sccs` and `subgraph_is_cyclic` build `id_of`, insert one node per name, and push an
  edge per adjacency pair filtered through `id_of.get(to)` with a running `edge_id` counter, in
  identical code.
  - evidence: graph.rs:94-114 and graph.rs:144-163 are the same ~20-line construction; the only differences are the node set (all keys vs `scc \ root_types`) and the adjacency source (`graph` vs `type_refs`), both of which become the two helper arguments.

### macro/recurse.rs : 240-251  [dup-code] (~12 lines, risk low, unverified (low-risk class))
  Extract a shared `fn scc_leaf_union(items: &[Item], scc: &HashSet<String>) -> Vec<Type>`
  (natural home: emit.rs next to `leaf_field_types`) and use it both for `scc_union_leaf` here and
  for `from_leaf_clones` in `gen_natural_extras` (emit.rs:561-576, which maps the same union to
  `#t: Clone` bounds).
  - evidence: Both sites run the identical pipeline: filter items whose ident is in the scc, flat_map `leaf_field_types(it, scc)`, dedup via `seen.insert(quote!(#t).to_string())` — recurse.rs:243-249 vs emit.rs:562-572. The comment at emit.rs:557-560 even says it is 'the UNION of every SCC member's leaf field types', i.e. the same value as recurse.rs:236-239 describes.

### macro/recurse/emit.rs : 416-431  [dup-code] (~8 lines, risk low, unverified (low-risk class))
  In `emit_delegated_parse`, build `run_root_bounds` and `impl_root_bounds` from one shared
  closure: `let bound = |r: &RootReentry| { let rid = &r.root_id; quote!(#rid #root_targs:
  ::syan::parse::Parse<__Atom, Error = ::syan::error::ParseError>) };` then
  `roots.iter().map(bound)` and `roots.iter().filter(|r| r.name != self_name).map(bound)`. The
  two-line comment explaining the split (415-416) stays.
  - evidence: emit.rs:417-423 and emit.rs:424-431 are identical map bodies; the only difference is the `.filter(|r| r.name != self_name)` on the second.

### macro/recurse.rs : 36-37, 71, 81-85, 102-103, 113-117, 154-158, 170-172, 185, 195-196, 234-236, 266-267, 275, 282  [comment] (~24 lines, risk low, unverified (low-risk class))
  Delete/trim in-body comments that restate error messages, callee docs, or the group-free/group-
  ful routing prose whose canonical home is the `make_natural_item` doc (items.rs:83-91) and
  CLAUDE.md: 36-37 (restates the const doc at 28-33 and the error string at line 45, plus the
  historical 'former limit = N is gone' — also trim that parenthetical from the const doc); 71 and
  81-85 (narrate what pub_types/type_refs/direct_type_refs are — the collector fn names say it;
  KEEP 86-89, the non-obvious safegraph-vs-HashMap rationale); 102-103 (restates the
  find_cycle_sccs doc, graph.rs:80-85); 113-117 (restates the abort message directly below at
  124-126 — keep only the 'would be E0072' clause); 154-158, 195-196, 266-267 (three restatements
  of the same group-ful routing rationale — keep one pointer to make_natural_item); 170-172, 185,
  275, 282 (narrate the directly-readable expressions below them); 234-236 (narrates the emission
  loop; keep 236-239, which says what the leaf union is for).
  - evidence: Each listed range duplicates text available elsewhere: e.g. lines 154-158 vs 195-196 vs 266-267 vs items.rs:87-91 all state 'group-free derives Unparse/Spanned directly; group-ful delegates because the Fill HRTB cycle'; lines 113-117 vs the abort! string at 124-126 both say 'no heap indirection -> infinite-size natural type'.

### macro/recurse/emit.rs : 651-653, 658-662, 664-665, 694-695, 717  [comment] (~9 lines, risk low, unverified (low-risk class))
  Trim the `gen_natural_extras` in-body narration that restates the doc comments of the functions
  being called: 651-653 restates `emit_delegated_parse`'s doc (398-403); 658-662 and 664-665
  restate the `__FromNat`/borrow-terminator mechanics already in this fn's own doc (499-503) and
  `emit_borrow_terminator_and_reentry`'s doc (99-105); 694-695 restates 'terminator __to_nat
  unwraps its Box' from the doc at 497-498; 717 restates the borrow-terminator doc. Keep the
  genuinely local notes (601-605 E0277 rationale, 635-637 span-param convention, SAFETY comments).
  - evidence: Each range duplicates a callee's doc: e.g. emit.rs:664-665 'clone leaves ... bottoming at the borrow terminator ... so only leaves copy (no Root: Clone)' vs emit.rs:100-101 'borrows the natural remainder — no clone, no Root: Clone' and 501-502 'Clone-ing leaves, borrowing recursive children'.

### macro/recurse/items.rs : 98-105  [comment] (~6 lines, risk low, unverified (low-risk class))
  Merge the in-body comment of `make_natural_item` into its doc comment (83-91) and delete the
  body block: both explain the identical Parse-always-engine-routed / group-free-direct / group-
  ful-delegated split. The doc even says 'see the body' — instead move the two mechanism-specific
  sentences (ignore_bounds drops the per-field bound; predicate_unparse/spanned re-adds leaf
  bounds) up into the doc and drop the rest.
  - evidence: items.rs:83-91 (doc) and items.rs:98-105 (body) restate each other nearly sentence-for-sentence ('Parse is always routed to the depth-limited engine', 'a group-ful cycle engine-routes them too', the Fill<Substruct> where-cycle rationale).

### macro/recurse/build.rs : 233-236, 259-263  [comment] (~6 lines, risk low, unverified (low-risk class))
  Trim two comment blocks: at 233-236 delete the historical parenthetical '(it was previously
  skipped whenever <=1 cycle type self-references, silently leaving such a sub-cycle un-depth-
  limited)' and the sentence restating the abort! message at 240-246 (keep the one-line soundness
  statement); at 259-263 delete lines 259-261 (restates the same 'no public alias — natural types
  own the names' NOTE that already appears at 414-415 in this file) and 262-263 (restates the
  `emit_terminator_and_reentry` doc, emit.rs:3-8).
  - evidence: 'was previously skipped' is a historical note (policy: delete); build.rs:259-261 vs build.rs:414-415 are duplicate statements of the same design fact within one file; build.rs:262-263 vs emit.rs:3-8 duplicate the re-entry/unbounded explanation.


## Slice `macro-rest` — detailed design

All 8 findings verified against `/home/yasuo/ghq/github.com/yasuo-ozu/syan2-reduce` (HEAD `e5d0576`,
plus uncommitted split of `macro/attribute.rs` into `macro/attribute/{find,substruct,adt}.rs`); zero
rejected. Estimated ~71 lines saved, matching the plan header. Execution order (dead-code, then
dup-code, then comment, matching the repo-wide policy; within `find.rs` the dup-code step depends on
the dead-code step run first): **(1) symbol.rs dead-code → (2) find.rs dead-code → (3) adt.rs
preamble+substruct helper → (4) adt.rs `fields_pattern` helper → (5) find.rs array collapse → (6)
ast.rs `param_name` reuse → (7) adt.rs comment trim → (8) attribute.rs + ast.rs comment trim**. None
of the 8 changes emitted tokens or diagnostic text — every step is either unreachable code, a
byte-identical mechanical refactor, or a comment-only edit — so no trybuild `.stderr` is expected to
move.

### 1. `macro/symbol.rs:7,125-147` — dead code in `create_joint_type` + unused `Debug` derive

Verified as described. `create_joint_type`'s `else` branch only runs when `char_types.len() >
MAX_TUPLE_SIZE` (12), so `char_types.chunks(12)` always yields `≥2` chunks, so `joint_types.len() ==
1` (lines 141-143) can never hold — confirmed by re-reading the recursion (line 128 `if len <=
MAX_TUPLE_SIZE` takes the `if`; only overflow reaches the `else`). `SymbolToken` (line 7
`#[derive(Debug)]`) is never `{:?}`-formatted anywhere in `macro/`: `grep -n '{:?}' macro/symbol.rs`
hits only line 159, which formats a `char`, not `SymbolToken`.

**Code shape** — collapse the intermediate `Vec<Vec<TokenStream>>` and drop the unreachable arm:

```rust
fn create_joint_type(char_types: Vec<TokenStream>, syan_path: &Ident) -> TokenStream {
    const MAX_TUPLE_SIZE: usize = 12;
    if char_types.len() <= MAX_TUPLE_SIZE {
        quote! { #syan_path::nested::Joint<(#(#char_types,)*)> }
    } else {
        let joint_types: Vec<TokenStream> = char_types
            .chunks(MAX_TUPLE_SIZE)
            .map(|chunk| create_joint_type(chunk.to_vec(), syan_path))
            .collect();
        quote! { #syan_path::nested::Joint<(#(#joint_types),*)> }
    }
}
```

Drop `#[derive(Debug)]` from `SymbolToken` (line 7).

**Invariant**: emission unchanged — the deleted branch never executed, and `Debug` on a proc-macro-
internal parse type has no effect on emitted tokens.

**Risk**: low. **Verify**: `cargo build -p syan-macro && cargo test --workspace` (also
`cargo build -p syan-macro 2>&1 | grep -i dead_code` to confirm no new dead-code lint appears from the
collapsed `else`).

### 2. `macro/attribute/find.rs:61-63` — dead `is_derive_helper_attr` arms

Verified: `fundamental_tys` occurs nowhere else in the worktree; `predicate`/`predicate_parse` occur
only in this strip list and in `macro/recurse/items.rs`'s separate, out-of-scope
`strip_field_helper_attrs` list. None of the four `#[proc_macro_derive(..., attributes(...))]`
registrations in `macro/lib.rs` (Parse: `group,syan,joint,alone,ignore_bounds`; Unparse: `+
predicate_unparse`; Spanned: `+ predicate_spanned`; Ast: `syan,subast,seq,opt`) registers these three
names, so a field bearing them fails to compile before `strip_derive_helper_attrs` ever runs — the
arms are unreachable. Confirmed drift noted by the plan: `predicate_spanned` (registered on `Spanned`)
is *not* in this list — out of scope for this finding (not proposed for addition), left as-is.

**Code shape** — delete 3 of the (then 12-, becoming 9-) arms:

```rust
fn is_derive_helper_attr(attr: &Attribute) -> bool {
    attr.path().is_ident("group")
        || attr.path().is_ident("syan")
        || attr.path().is_ident("joint")
        || attr.path().is_ident("alone")
        || attr.path().is_ident("ignore_bounds")
        || attr.path().is_ident("default")
        || attr.path().is_ident("predicate_unparse")
        // `#[derive(Ast)]`'s view markers: … (comment kept verbatim)
        || attr.path().is_ident("seq")
        || attr.path().is_ident("opt")
}
```

**Invariant**: emission unchanged — the removed arms never matched any field reaching this function.

**Risk**: low. **Verify**: `cargo build -p syan-macro && cargo test --workspace`.

### 3. `macro/attribute/adt.rs:29-35,133-151,155-161` vs `186-192,268-287,295-301` — `extract_parse`/`extract_unparse` preamble + substruct-emission dedup

Verified byte-identical: `29-35` (7 lines: `tp_atom`, `trait_path_owned`, `trait_fullpath`,
`generic_params` clone+strip+push, `ty_generics`) equals `186-192` verbatim; `136-140`
(`DataStruct{struct_token,fields,semi_token}` construction) equals `271-275`; `150-151`
(`substructs_for_emit` map) equals `286-287`; the `quote!` substruct loops `156-161` equal `296-301`.
Only the inner call differs (`data_struct.extract_parse(syan, generics, ident, nonce, trait_path)` vs
`data_struct.extract_unparse(syan, generics, ident, &[], nonce, &trait_path_owned)`).

**Code shape** — two small helpers on `impl Adt` (or free fns in `adt.rs`):

```rust
fn atom_impl_header<'g>(
    generics: &'g Generics,
    trait_path: &Path,
) -> (Ident, Path, Punctuated<GenericParam, Token![,]>, syn::TypeGenerics<'g>) {
    let tp_atom: Ident = parse_quote!(__SyanMacro_Atom);
    let trait_fullpath: Path = parse_quote!(#trait_path<#tp_atom>);
    let mut generic_params = generics.params.clone();
    strip_param_defaults(&mut generic_params);
    generic_params.push(parse_quote!(#tp_atom));
    let ty_generics = generics.split_for_impl().1;
    (tp_atom, trait_fullpath, generic_params, ty_generics)
}

fn substruct_items(
    substructs: &[ItemStruct],
    mut derive: impl FnMut(&DataStruct, &Generics, &Ident) -> TokenStream,
) -> TokenStream {
    let substructs_for_emit: Vec<ItemStruct> =
        substructs.iter().map(strip_derive_helper_attrs).collect();
    let substruct_impls: Vec<TokenStream> = substructs
        .iter()
        .map(|substruct| {
            let data_struct = DataStruct {
                struct_token: Default::default(),
                fields: substruct.fields.clone(),
                semi_token: substruct.semi_token,
            };
            derive(&data_struct, &substruct.generics, &substruct.ident)
        })
        .collect();
    quote! {
        #(for substruct in &substructs_for_emit) { #substruct }
        #(for substruct_impl in &substruct_impls) { #substruct_impl }
    }
}
```

Call sites — `extract_parse` (replaces 29-35 and 133-161):

```rust
let (tp_atom, trait_fullpath, generic_params, ty_generics) = atom_impl_header(generics, trait_path);
// …
let substruct_defs = substruct_items(&substructs, |data_struct, generics, ident| {
    data_struct.extract_parse(syan, generics, ident, nonce, trait_path)
});
// then `quote! { #substruct_defs #[automatically_derived] impl … }`
```

`extract_unparse` (replaces 186-192 and 268-287/295-301) is identical except
`data_struct.extract_unparse(syan, generics, ident, &[], nonce, trait_path)`. Note this drops the
local `trait_path_owned` clone at the old line 282 call site in favor of the original `trait_path: &Path`
parameter already in scope — `syn::Path::clone()` preserves spans exactly, so `trait_path` and
`&trait_path_owned` denote token-identical values; this is required by extracting the preamble (which
no longer exposes a `trait_path_owned` local) and is token-neutral.

**Invariant**: emission unchanged — pure extract-helper refactor; token order and content identical,
including spans (verified the one behavioral wrinkle: dropping `trait_path_owned` in favor of
`trait_path` at the substruct-recursion call site).

**Risk**: low. **Verify**: `cargo build -p syan-macro && cargo test --workspace` (pay particular
attention to `core/tests/where_clause_attribute.rs` and `visitor_edit_group.rs`, which exercise
`#[group]` substruct emission through both `extract_parse` and `extract_unparse`).

### 4. `macro/attribute/adt.rs:466-508,530-540,655-669` — `fields_pattern` helper for the four Named/Unnamed template blocks

Verified: `DataStruct::extract_parse_inner`'s `Fields::Unnamed`/`Fields::Named` template (lines
480-485), `DataStruct::extract_inner`'s (500-504), `DataEnum::extract_parse_inner`'s `construct_of`
closure (533-538), and `DataEnum::extract_inner`'s match-arm head (659-663) all emit the identical
`#(if let Fields::Unnamed(_) = ..) { (..) } #(if let Fields::Named(_) = ..) { {..} }` shape, differing
only in the `Fields` source (`&self.fields`/`&variant.fields`) and the per-item binding name
(`field_ident`/`id`) — confirmed by re-reading all four sites; `grep -rn 'Fields::Unnamed\|Fields::Named'
macro/` shows no other quote-template use of this shape anywhere in the crate (the other hits are
plain `match`/`if let` on `Fields`, not token templates).

**Code shape**:

```rust
/// `(a, b,)` for tuple fields, `{a, b,}` for named fields, or nothing for a unit shape.
fn fields_pattern(shape: &Fields, fields: &[MappedField]) -> TokenStream {
    quote! {
        #(if let Fields::Unnamed(_) = shape) {
            (#(for (_, id, _) in fields) {#id,})
        }
        #(if let Fields::Named(_) = shape) {
            {#(for (_, id, _) in fields) {#id,}}
        }
    }
}
```

Call sites (all four, byte-identical composition since `#{expr}` splices a `TokenStream` verbatim):
`DataStruct::extract_parse_inner`: `#ident #{fields_pattern(&self.fields, &fields)}`;
`DataStruct::extract_inner`: `let #ident #{fields_pattern(&self.fields, &fields)} = #v_self;`;
`DataEnum::extract_parse_inner`'s `construct_of`: `#ident :: #{&variant.ident} #{fields_pattern(&variant.fields, fields)}`;
`DataEnum::extract_inner`: `#ident :: #{&variant.ident} #{fields_pattern(&variant.fields, &fields)} => { #inner }`.
`MappedField<'a>` (the existing `type MappedField<'a> = (Member, Ident, &'a Field);` alias at line 432)
is already module-private and in scope for all four sites.

**Invariant**: emission unchanged — same conditional structure and same iteration source per call
site, only the enclosing variable names differ (irrelevant to emitted tokens).

**Risk**: low. **Verify**: `cargo build -p syan-macro && cargo test --workspace` (covers both tuple-
and named-field structs/enums via `core/tests/ast_derive.rs`, `recurse_core.rs`, and the
`parse_prefix_dedup.rs` enum-construction paths).

### 5. `macro/attribute/find.rs:54-70` — collapse `is_derive_helper_attr` to an array + `.any`

Verified: after step 2, the chain has 9 identical-shape arms (`attr.path().is_ident("…")`). The
array-based style is already proven working in this exact crate:
`macro/recurse/items.rs:70-74`'s `is_struct_helper` uses `[…].iter().any(|n| attr.path().is_ident(n))`
on a `&[&str]`, the same pattern proposed here.

**Code shape**:

```rust
fn is_derive_helper_attr(attr: &Attribute) -> bool {
    [
        "group", "syan", "joint", "alone", "ignore_bounds", "default", "predicate_unparse",
        // `#[derive(Ast)]`'s view markers: strip them off a `#[group]`-cloned substruct (which carries no
        // `Ast` derive to register them), else `#[group] #[seq] Punctuated<..>` fails with "cannot find
        // attribute `seq`".
        "seq", "opt",
    ]
    .iter()
    .any(|n| attr.path().is_ident(n))
}
```

**Invariant**: emission unchanged — same predicate, same matched name set.

**Risk**: low. **Verify**: `cargo build -p syan-macro && cargo test --workspace`.

### 6. `macro/ast.rs:331-340` — reuse `crate::util::param_name`

Verified: `macro/util.rs:27-33`'s `param_name(p: &GenericParam) -> String` has an arm-for-arm identical
body to the inline closure at `ast.rs:335-339` (`GenericParam::Type => t.ident.to_string()`, `Const =>
c.ident.to_string()`, `Lifetime => l.lifetime.ident.to_string()`). `ast.rs` already imports from
`crate::util` (line 1: `angle, gargs, gparams, to_snake`) and already calls it fully qualified
elsewhere (`crate::util::peel` at line 274), so either an added import or an inline fully-qualified
call works.

**Code shape**:

```rust
let param_names: std::collections::HashSet<String> = input
    .generics
    .params
    .iter()
    .map(crate::util::param_name)
    .collect();
```

(`.iter()` over `Punctuated<GenericParam,_>` yields `&GenericParam`, matching `param_name`'s
`&GenericParam` parameter — direct fn-pointer coercion, no closure needed.)

**Invariant**: emission unchanged — this only feeds the nightly "follows nothing" lint's suspect-name
filter; the computed `HashSet<String>` contents are identical, so the lint fires on the same fields
with the same message.

**Risk**: low. **Verify**: `cargo build -p syan-macro && cargo test --workspace` (nightly-gated lint
path is exercised by the `ui/` diagnostics suite per CLAUDE.md; re-run
`TRYBUILD=overwrite cargo test --test visitor_diagnostics -- --ignored` only if a `.stderr` drifts,
which is not expected here).

### 7. `macro/attribute/adt.rs` comments — trim restatements of helper docs

Verified all eleven locations exactly as claimed, re-reading each: line 60 and its twins at 211, 351,
374 all read `// Skip fields with #[default] attribute — …` immediately above `field.has_default()`/
`attrs.has_default()` checks; 152-153 and 292-293 and 391-392 all restate
`append_user_where_predicates`'s own doc (`find.rs:129-131`); 288 and 387 both restate
`predicate_tys`'s doc (`find.rs:143-148`, which already states the `Ty: Unparse<atom>` /
`Ty: Spanned<Span=span>` mapping for both `predicate_unparse` and `predicate_spanned`); 329 restates
`add_spanned_param_predicates`'s doc (`find.rs:109-112`); 529 restates the `construct_of` closure's own
name/purpose immediately below it. The constraint comments the plan says to keep (355-367 E0207/E0308
rationale, 542-546, 575-586, 594, 603) are untouched by this deletion list.

**Code shape**: delete the 11 listed single/double-line `//` comments verbatim; no code lines change.

**Invariant**: emission unchanged — comment-only edit.

**Risk**: low. **Verify**: `cargo build -p syan-macro && cargo test --workspace` (a comment-only diff
cannot itself change behavior; the build simply confirms no stray line was caught in the edit).

### 8. `macro/attribute.rs:16` + `macro/ast.rs:43-44,306-307` — stale/duplicate comments

Verified: `attribute.rs:16` (`// FindAttribute is imported directly by lib.rs.`) sits directly above
`pub(crate) use find::FindAttribute;` (line 17), and `macro/lib.rs:12` does indeed
`use crate::attribute::FindAttribute;` — but the comment only restates the next line, adding nothing
`grep` can't already show. `ast.rs:43-44`'s "Shared by `#[derive(Ast)]` and `#[recurse]`'s per-cycle-
type metadata macros" is stale: `grep -rn 'subast_tokens\|crate_rooted_tokens\|cleaned_definition\|parse_subast'
macro/recurse.rs macro/recurse/` returns zero hits — all four callers of these functions are in
`ast.rs` itself (self-calls at lines 366/368/308), confirming `#[recurse]` cycle types now carry a
plain `#[derive(Ast)]` with no `#[recurse]`-specific metadata path, per CLAUDE.md. `ast.rs:306-307`
narrates the `#[subast(..)]` allowlist immediately above `parse_subast(&input.attrs)`, restating
`parse_subast`'s own doc at `ast.rs:91-93`.

**Code shape**: delete `attribute.rs:16` entirely; in `ast.rs:43-44` delete the trailing sentence
"Shared by `#[derive(Ast)]` and `#[recurse]`'s per-cycle-type metadata macros." from `subast_tokens`'s
doc comment (keep the rest of the doc); delete `ast.rs:306-307` entirely.

**Invariant**: emission unchanged — comment-only edit.

**Risk**: low. **Verify**: `cargo build -p syan-macro && cargo test --workspace`.

## Slice `core-src` — detailed design

Execution order: 8 dead-code removals first (steps 1–8, each a pub-API removal — verify against the whole
workspace incl. `rust/` and doc-tests), then the literal-family dup-code folds (steps 9–11: parse fold,
display dedup, unparse dedup — display before unparse, since the unparse fold relies on Display parity),
then the formatting fix (step 12) and comment/doc trims (steps 13–15). All 15 findings verified against the
worktree `../syan2-reduce`; **0 rejected**, 3 revised in detail (step 9: 8/11 impls fold, savings revised
UP to ~225; step 10: 96 idents not 100; step 11: Str/ByteStr in the cited range are untouched).
**Total estimated savings: ~804 lines** (plan said ~778; the delta is the macro-fold revision).
Baseline verification after every step: `cargo build -p syan && cargo test --workspace` in the worktree
(`cargo test --workspace` covers the `rust/` crate's tests and all doc-tests, which is required for the
pub-API steps 1–8).

### 1. Delete `core/src/nested/choice.rs` (whole module) — ~143 lines [dead-code]

Delete the file (141 lines: `Choice<HList>` type, the `_Choice`/`Choice!` macro, five `unsafe transmute`s)
plus the two hookup lines in `core/src/nested.rs`: line 2 `pub mod choice;` and line 9
`pub use choice::Choice;`.

Verified (grep re-run, whole worktree incl. tests, macro crate, `rust/`, doc fences): the only `Choice`
references outside `choice.rs` are `nested.rs:2` and `nested.rs:9`. The `Choice!` macro is unusable by
construction — its transcriber (choice.rs:15) contains `$($t:ty),*`; a fragment specifier in transcriber
position emits literal `: ty` tokens, so `Choice!(T, U)` expands to `@impl T : ty , U : ty`, which no
`@impl` rule matches (independently reproduced by the plan's verifier with a compiled mimic). Only the
useless zero-arg form expands. Fields of `Choice` are private, so no working downstream usage can exist.
No other companion edits: no doc-comment elsewhere links `choice`/`Choice`.

- Evidence (condensed): worktree-wide grep hits only choice.rs itself + nested.rs:2,9; macro front door is
  a guaranteed compile error for any non-empty invocation.
- Risk: **low** (pub-API removal, but provably unusable; semver-irrelevant for this unpublished 0.1.0
  workspace).
- Verify: `cargo build -p syan && cargo test --workspace`

### 2. Delete the `Map<S>` trait from `core/src/span.rs` — ~95 lines [dead-code]

Delete `pub trait Map<S>` (span.rs:26–33, method `fn map(self, replacement: impl FnMut(Self::Span) -> S)
-> Self::Output`) and all seven impl sites, keeping the `Spanned` halves of each: `WithSpan` (80–89),
`Vec` (146–161), `VecDeque` (163–177), `Option` (190–202), `Result` (218–230), `[T; N]` (256–268), and the
Map half of the `impl_for_tup!` macro body (287–297: the `impl<S: Span$(,$A: Map<S,...>)*> Map<S> for
($($A,)*)` block and its `type Output`).

Verified (grep re-run): zero references to `Map` outside span.rs anywhere in core/tests/macro/rust except
two prose comments using "Map" as a verb (`macro/recurse.rs:132`, `macro/visitor.rs:145`). No
`use ...span::Map` or `span::*` glob import exists, so no `.map()` call site can resolve to the trait;
the macro crate never emits a `Map` ident. The plan's verifier additionally compile-checked the exact
deletion (`cargo check --workspace --all-targets` clean).

- Evidence (condensed): trait + 7 impls only in span.rs; no import path by which the trait method is
  reachable anywhere in the workspace.
- Risk: **medium** (pub trait — possibly a planned span-remapping feature; owner call, as the plan notes).
- Verify: `cargo build -p syan && cargo test --workspace` (pub-API removal — workspace run covers `rust/`
  + doc-tests)

### 3. Delete `impl_for_map!` from `core/src/parse.rs` — ~64 lines [dead-code]

Delete the `impl_for_map!` macro definition (parse.rs:75+) and its single invocation for
`HashMap<K, V>` / `BTreeMap<K, V>` (ends at EOF, ~line 139) — the Parse ("repeat k,v pairs"), Unparse,
and Spanned impls for std map types. Leave the sibling `HashSet`/`BTreeSet` entries in
`impl_for_collection!` (lines 71–72) alone, per the plan (2 lines, not worth the churn).

Verified (grep re-run): every `HashMap`/`BTreeMap` mention in core/tests, `rust/src`, `rust/tests` is
unrelated — `core/tests/symbol.rs:207` uses `HashMap` as ident *tokens* inside a `Symbol![...]` doctest
string, and `spike_real_parsestream.rs` uses a test-local registry. No AST field anywhere is map-typed;
the macro crate never emits map types. Plan's verifier compile-checked the deletion clean.

- Evidence (condensed): no map-typed AST field, test, or macro emission in the workspace; range 75–139 is
  self-contained.
- Risk: **low** (pub trait impls on std types — unpublished workspace).
- Verify: `cargo build -p syan && cargo test --workspace`

### 4. `core/src/error.rs` cleanup — ~18 lines [dead-code]

(a) Delete `add_sub_errors` (error.rs:54–59) — grep re-run: sole hit is the definition; the singular
`add_sub_error` (49–52) is used and stays. (b) Delete `pub type Result<T>` (line 80) — grep for
`error::Result`: zero hits; macro-generated code emits `::core::result::Result<_,
::syan::error::ParseError>` (macro/recurse/emit.rs), and every `Result` in macro sources is `syn::Result`.
(c) Replace the hand-written `impl Clone for ParseError` (62–70) with `#[derive(Clone)]` on the struct
(line 33) and delete the three commented-out `// span: ...` historical lines (35, 43, 65) — verified the
struct's only fields are `message: String` and `sub_errors: Vec<Self>`, both `Clone`, and the manual impl
is the exact field-wise clone, so the derive is behavior-identical. The manual impl existed only for the
long-removed `Box<dyn Span>` field those comments memorialize (`ParseError::new`'s `_span: impl Span`
parameter still documents the intent and stays).

- Evidence (condensed): `add_sub_errors`/`error::Result` definition-only; fields all-Clone so derive ≡
  manual impl.
- Risk: **low** (pub items in `pub mod error`, but zero users; compile-verified by the plan's verifier).
- Verify: `cargo build -p syan && cargo test --workspace`

### 5. Delete `GroupAngle` + angle punct aliases — ~13 lines [dead-code]

Delete `pub type GroupAngle<T, S>` (`core/src/nested/group.rs:79`) and the four backing items in
`core/src/symbol.rs` 115–124: `pub type OpenAngle = Lt;` (116), `pub const OpenAngle` (119),
`pub type CloseAngle = Gt;` (121), `pub const CloseAngle` (124), together with their `///` doc lines and
`#[allow(non_upper_case_globals)]` attrs (the whole 115–124 block). **Companion edit:** trim the now-stale
parenthetical in the group.rs:13–16 block comment — the sentence "(`GroupAngle` has no proc-macro
delimiter, hence no impl.)" at lines 15–16.

Verified (grep re-run, `--exclude rust_old`): the only hits are the five definitions plus the group.rs:15
comment. The users live exclusively in `rust_old/` (not a workspace member, never compiled). The
underlying `Lt`/`Gt` symbols come from the table macro ending at symbol.rs:113 and are untouched.
Keep-alternative (flagged by the plan): `rust/` is an incremental rebuild of `rust_old` and its generics
port will likely re-need angle groups — but the 13 lines are trivially recreatable from git history.

- Evidence (condensed): all references outside definitions are in non-member `rust_old/`; `Lt`/`Gt` and
  `impl_group_unparse_tt!` (Paren/Brace/Bracket only) unaffected.
- Risk: **medium** (likely re-needed by a future `rust/` generics port; deletion is cheap to reverse).
- Verify: `cargo build -p syan && cargo test --workspace`
- Ordering note: do this **before** step 13 — deleting symbol.rs:115–124 shifts the doc-trim line numbers
  in step 13 down by ~10; step 13 should anchor on heading text, not line numbers.

### 6. Delete the `Joint!` type macro from `core/src/nested/joint.rs` — ~12 lines [dead-code]

Delete lines 17–28: the `mod _joint_impl { ... }` block (containing the `#[macro_export]
macro_rules! _joint_impl` and `pub use _joint_impl as Joint;`) plus the trailing `#[doc(inline)]
pub use _joint_impl::*;`. The `Joint` **struct** (defined above, re-exported by nested.rs
`pub use joint::Joint;`) is untouched — only the macro-namespace `Joint` goes.

Verified (grep re-run): zero invocations of `Joint!` anywhere in the worktree; every other `Joint`
reference is the struct (the `symbol!` proc-macro emits the type path directly:
`#syan_path::nested::Joint<(...)>` at macro/symbol.rs:129,144). The macro can never have worked: its
transcriber (joint.rs:22) references `$xrate`, which is not declared in the matcher at line 21 — a `$`
followed by a non-metavariable ident transcribes literally, so every invocation fails with "expected type,
found `$`" (empirically reproduced by the plan's verifier). It is also `#[doc(hidden)]`.

- Evidence (condensed): zero `Joint!` invocations; unbound `$xrate` makes any invocation a guaranteed
  expansion-site error.
- Risk: **low**.
- Verify: `cargo build -p syan && cargo test --workspace`

### 7. Delete `Attempt::into_inner` — ~6 lines [dead-code]

Delete the whole `impl<T> Attempt<T> { ... }` block at `core/src/nested/attempt.rs:18–23` (doc comment +
`pub fn into_inner(self) -> T { self.0 }`).

Verified (grep re-run): `into_inner` has exactly five hits — this definition, `Unordered::into_inner`
(unordered.rs:31, a different type, used by `nested_unordered.rs:43`), and two `Cell::into_inner` calls in
macro/visitor.rs:383–384. No caller of `Attempt::into_inner` exists; `nested_attempt.rs` and the OptView
impl use `.0`/Deref/`.attempt()` only. Since `.0` is `pub`, the method is pure sugar — no capability is
lost.

- Evidence (condensed): zero call sites; `.0` is pub so the method is redundant sugar.
- Risk: **low**.
- Verify: `cargo build -p syan && cargo test --workspace`

### 8. Drop `type Error` from `IntoParseStream` — ~4 lines [dead-code]

In `core/src/parse/into_parse_stream.rs`: delete `type Error;` (line 5), loosen the bound on line 6 from
`type Output: ParseStream<Atom = Self::Atom, Error = Self::Error>;` to
`type Output: ParseStream<Atom = Self::Atom>;`, and delete `type Error = T::Error;` from the blanket impl
(line 17). In the two concrete impls delete the corresponding lines: `core/src/source/string.rs:103`
(`type Error = Infallible;` — the `Infallible` import stays, it is still used elsewhere in string.rs, per
the plan's compile check) and `core/src/source/proc_macro2.rs:109`
(`type Error = core::convert::Infallible;`).

Verified (grep re-run): every `IntoParseStream` use site in core, macro-emitted code, `rust/`, and tests
is the APIT form `impl IntoParseStream<Atom = ...>` — such a type cannot be named, so `::Error` can never
be projected; `Self::Error` in parse signatures is `Parse::Error`, a different trait. The plan's verifier
ran the full workspace test suite (incl. 27 trybuild UI tests and doc-tests) clean on the exact change.

- Evidence (condensed): no `IntoParseStream::Error` projection anywhere; all bounds are `<Atom = ...>`
  only.
- Risk: **low** (compile+test verified; pub trait shape change in an unpublished workspace).
- Verify: `cargo build -p syan && cargo test --workspace`
- Ordering note: do **before** steps 10 and 14 (removing string.rs:103 / proc_macro2.rs:109 shifts their
  later-step line references by 1).

### 9. THE BIG ONE — fold the `Parse` impls in `literal/parse_impl.rs` — revised ~225 lines [dup-code]

`core/src/source/proc_macro2/literal/parse_impl.rs` (453 lines, 11 impls). All 11 impls were read and
classified. The `Some(token) => { stream.push(token); Err(...) } / None => Err(...)` tail is
**byte-identical in all 11** (hand-diffed: lines 22-26/66-70/111-115/171-175/222-226/256-260/297-301/
330-334/372-376/404-408/445-449).

**Classification — the honest answer is 8/11 fold, not 11:**

| Type | Lines | Matches on | Verdict |
|---|---|---|---|
| `Bool` | 3–30 | `TokenTree::Ident` | **stays hand-written** — wrong token kind for the scaffold |
| `ByteChar` | 32–74 | `TokenTree::Literal` | **stays hand-written** (uses shared `unescape`) — see caveat |
| `Char` | 76–119 | `TokenTree::Literal` | **stays hand-written** (uses shared `unescape`) — see caveat |
| `Integer` | 121–179 | `TokenTree::Literal` | folds |
| `Float` | 181–230 | `TokenTree::Literal` | folds |
| `Str` | 232–264 | `TokenTree::Literal` | folds |
| `StrRaw` | 266–305 | `TokenTree::Literal` | folds (via `parse_raw`) |
| `ByteStr` | 307–338 | `TokenTree::Literal` | folds |
| `ByteStrRaw` | 340–380 | `TokenTree::Literal` | folds (via `parse_raw`) |
| `CStr` | 382–412 | `TokenTree::Literal` | folds |
| `CStrRaw` | 414–453 | `TokenTree::Literal` | folds (via `parse_raw`) |

**Design: a plain generic fn owns the scaffold; a thin `macro_rules!` owns only the impl boilerplate.**
A fn is strictly better than a monolithic macro here because nothing *token-shaped* varies per type — only
a closure value — so the fn gets real type-checking, and the macro is reduced to the impl header +
`type Error` + fn signature, which is where `macro_rules!` actually earns its keep.

```rust
/// Shared scaffold for all `Literal`-based `Parse` impls: read one literal token,
/// hand its string form to `f`; on `None` (or a non-literal token / EOF), restore
/// the stream and fail — mirrors the existing push-back-on-failure behavior.
fn parse_lit<T>(
    stream: impl IntoParseStream<Atom = proc_macro2::TokenTree>,
    f: impl FnOnce(&str) -> Option<T>,
) -> Result<T, ParseError> {
    let mut stream = stream.into_parse_stream();
    match stream.next() {
        Some(proc_macro2::TokenTree::Literal(lit)) => {
            let lit_str = lit.to_string();
            match f(&lit_str) {
                Some(value) => Ok(value),
                None => {
                    stream.push(proc_macro2::TokenTree::Literal(lit));
                    Err(ParseError::new(Span::default(), "parse failed"))
                }
            }
        }
        Some(token) => {
            stream.push(token);
            Err(ParseError::new(Span::default(), "parse failed"))
        }
        None => Err(ParseError::new(Span::default(), "parse failed")),
    }
}

macro_rules! impl_parse_lit {
    ($Ty:ident, $body:expr) => {
        impl Parse<proc_macro2::TokenTree> for $Ty {
            type Error = ParseError;

            fn parse(
                stream: impl IntoParseStream<Atom = proc_macro2::TokenTree>,
            ) -> Result<Self, Self::Error> {
                parse_lit(stream, $body)
            }
        }
    };
}
```

**Shared helper (a): escape table for `ByteChar`/`Char`** — the two 6-arm escape matches (ByteChar 49–57
vs Char 94–102) are equivalent (all values ASCII, so ByteChar's `as u8` cast is lossless):

```rust
/// Shared escape table for ByteChar/Char (all-ASCII values; `c as u8` is lossless).
fn unescape(rest: &str) -> Option<char> {
    match rest {
        "n" => Some('\n'),
        "t" => Some('\t'),
        "r" => Some('\r'),
        "\\" => Some('\\'),
        "'" => Some('\''),
        "0" => Some('\0'),
        _ => None,
    }
}
```

**Shared helper (b): raw-string hash/quote logic** — triplicated verbatim in StrRaw/ByteStrRaw/CStrRaw
modulo the prefix, which the caller strips (`strip_prefix("r"/"br"/"cr")`) before calling:

```rust
/// Shared by StrRaw/ByteStrRaw/CStrRaw after the caller strips the "r"/"br"/"cr" prefix.
/// NOTE: intentionally reproduces the current hash-counting loop bit-for-bit — the
/// `while let Some('#') = chars.next()` loop consumes the first non-'#' char (the
/// opening quote) via the failed match arm, so `remaining` never starts with '"' and
/// every raw-string literal currently FAILS to parse, at any hash count including zero.
/// Pre-existing latent bug, identical in all three impls today (tests tolerate it via
/// `if let Ok`); this fold preserves it — do NOT "fix" it while applying this design.
fn parse_raw(rest: &str) -> Option<(String, usize)> {
    let mut hash_count = 0;
    let mut chars = rest.chars();
    while let Some('#') = chars.next() {
        hash_count += 1;
    }
    let remaining: String = chars.collect();
    if remaining.starts_with('"') && remaining.ends_with('"') {
        Some((remaining[1..remaining.len() - 1].to_string(), hash_count))
    } else {
        None
    }
}
```

**Per-type invocation lines (the 8 that fold):**

```rust
impl_parse_lit!(Str, |s: &str| {
    (s.starts_with('"') && s.ends_with('"')
        && !s.starts_with('r') && !s.starts_with('b') && !s.starts_with('c'))
        .then(|| Str { value: s[1..s.len() - 1].to_string() })
});

impl_parse_lit!(ByteStr, |s: &str| {
    (s.starts_with("b\"") && s.ends_with('"') && !s.starts_with("br"))
        .then(|| ByteStr { value: s[2..s.len() - 1].bytes().collect() })
});

impl_parse_lit!(CStr, |s: &str| {
    (s.starts_with("c\"") && s.ends_with('"') && !s.starts_with("cr"))
        .then(|| CStr { value: s[2..s.len() - 1].to_string() })
});

impl_parse_lit!(StrRaw, |s: &str| {
    let (value, hash_count) = parse_raw(s.strip_prefix('r')?)?;
    Some(StrRaw { value, hash_count })
});

impl_parse_lit!(ByteStrRaw, |s: &str| {
    let (value, hash_count) = parse_raw(s.strip_prefix("br")?)?;
    Some(ByteStrRaw { value: value.bytes().collect(), hash_count })
});

impl_parse_lit!(CStrRaw, |s: &str| {
    let (value, hash_count) = parse_raw(s.strip_prefix("cr")?)?;
    Some(CStrRaw { value, hash_count })
});

impl_parse_lit!(Integer, |s: &str| {
    const SUFFIXES: &[&str] = &[
        "u8", "u16", "u32", "u64", "u128", "usize",
        "i8", "i16", "i32", "i64", "i128", "isize",
    ];
    if s.contains('.') {
        return None;
    }
    for suffix in SUFFIXES {
        if let Some(value) = s.strip_suffix(suffix) {
            if value.chars().all(|c| c.is_ascii_digit() || c == '_'
                || (c == '-' && value.starts_with('-'))) {
                return Some(Integer {
                    value: value.to_string(),
                    suffix: Some((*suffix).to_string()),
                });
            }
            // no early return on a failed suffix match — the original loop keeps trying
        }
    }
    (s.chars().all(|c| c.is_ascii_digit() || c == '_' || (c == '-' && s.starts_with('-'))))
        .then(|| Integer { value: s.to_string(), suffix: None })
});

impl_parse_lit!(Float, |s: &str| {
    const SUFFIXES: &[&str] = &["f32", "f64"];
    if !s.contains('.') {
        return None;
    }
    for suffix in SUFFIXES {
        if let Some(value) = s.strip_suffix(suffix) {
            if value.parse::<f64>().is_ok() {
                return Some(Float {
                    value: value.to_string(),
                    suffix: Some((*suffix).to_string()),
                });
            }
        }
    }
    s.parse::<f64>().ok().map(|_| Float { value: s.to_string(), suffix: None })
});
```

**Impls that stay hand-written, and why:**
- **`Bool`** — matches `TokenTree::Ident`, not `Literal`; the scaffold's `Some(Literal(lit))` arm is the
  wrong shape. A one-off "parse_ident" helper for a single caller would cost more lines than the ~15-line
  hand-written body it replaces. Leave unchanged.
- **`ByteChar` and `Char`** — same outer scaffold, but their *inner* escape-match has two failure branches
  (bad escape char, ByteChar:56/Char:101; multi-char non-escape, ByteChar:58–60/Char:103–105) that do
  **not** push the literal back, unlike every other failure path in the file. Folding them through
  `parse_lit`'s `Option<T>` closure would push back on *every* `None` uniformly — a real (if
  arguably-fixing) behavior change on public parsing code. **Decision: preserve the inconsistency** — keep
  both hand-written but route their escape tables through the shared `unescape()` (behavior-identical,
  ~8 lines saved per impl). If the owner later wants the uniform push-back-on-all-failures behavior, both
  drop into `impl_parse_lit!` as a separate, explicitly flagged commit.

**Revised savings estimate (honest numbers):** before = 441 lines of impls; after ≈ 218 lines
(`parse_lit` 20 + `parse_raw` 11 + `unescape` 9 + `impl_parse_lit!` 12 + `Bool` unchanged 28 +
`ByteChar`/`Char` ~35 each + 8 invocation bodies ~51 + separators ~16). **≈ 225 lines saved** — slightly
*above* the plan's ~200 even though only 8/11 impls take the full fold, because the folded closure bodies
also tighten (`strip_suffix`/`bool::then` vs the original nested if/else) and `parse_raw`/`unescape`
remove duplication the original estimate didn't itemize. Also drops the stray blank line before each
closing brace (11×), already counted.

- Evidence (condensed): identical scaffold/tail in all Literal-matching impls; Bool matches Ident (as the
  plan's verifier already flagged); escape tables and raw-hash logic duplicated verbatim.
- Risk: **medium** — restructures 8 public `Parse` impls' internals (signatures unchanged); the
  no-push-back escape quirk and the raw-string latent bug are both deliberately preserved. Drops toward
  low because behavior is preserved bit-for-bit.
- Verify: `cargo build -p syan && cargo test --workspace` (must include `core/tests/proc_macro2_literal.rs`
  and the inline `mod tests` in `literal.rs`; also `rust/tests/rustsub_roundtrip.rs` exercises literal
  parsing end-to-end).

### 10. Dedup `literal/display_impl.rs` — ~18 lines [dup-code]

(a) Collapse the token-identical `Integer` (41–48) and `Float` (50–57) Display bodies (fields verified
`value: String`, `suffix: Option<String>` at literal.rs:22–25) to:

```rust
write!(f, "{}{}", self.value, self.suffix.as_deref().unwrap_or(""))
```

(b) Share the raw-string family (StrRaw 69–74, ByteStrRaw 94–100, CStrRaw 112–117 — differ only in prefix
and value rendering) via:

```rust
fn raw(
    f: &mut std::fmt::Formatter<'_>,
    prefix: &str,
    value: impl std::fmt::Display,
    hash_count: usize,
) -> std::fmt::Result {
    let hashes = "#".repeat(hash_count);
    write!(f, "{}{}\"{}\"{}", prefix, hashes, value, hashes)
}
```

Call sites: `raw(f, "r", &self.value, self.hash_count)` (StrRaw);
`raw(f, "br", String::from_utf8_lossy(&self.value), self.hash_count)` (ByteStrRaw — its field is
`Vec<u8>`, hence the lossy conversion, matching current behavior); `raw(f, "cr", &self.value,
self.hash_count)` (CStrRaw).

(c) Delete three narration comments (verbatim): line 3 `// Display implementations`; line 12
`// Handle common escape sequences` (ByteChar); line 28 `// Handle common escape sequences` (Char).

- Evidence (condensed): Integer/Float bodies identical modulo type name; raw trio shares the
  hashes/format shape; comments restate the match arms below them.
- Risk: **low** (output-identical refactor + comment-only). **Do this step before step 11** — step 11's
  `self.to_string()` calls inherit these Display impls, so land + verify Display parity first.
- Verify: `cargo build -p syan && cargo test --workspace` (Display coverage:
  `core/tests/proc_macro2_literal.rs` display tests + inline `test_display_implementations`).

### 11. Dedup `literal/unparse_impl.rs` via `emit_parsed` — ~40 lines [dup-code]

Six impls in 27–115 share the byte-identical 4-step scaffold (build `lit_str`;
`.parse::<proc_macro2::Literal>()` with fallback; `set_span(call_site)`; `write_one(TokenTree::Literal)`):
Integer 27–39, Float 41–53, StrRaw 62–72, ByteStrRaw 81–92, CStr 94–103, CStrRaw 105–115. **Correction to
the finding's cited range:** `Str` (55–60) and `ByteStr` (74–79) also sit inside 27–115 but use direct
`Literal::string`/`byte_string` constructors — they are not part of the fold and stay untouched. Extract:

```rust
fn emit_parsed<S: Emitter<proc_macro2::TokenTree>>(
    sink: &mut S,
    s: String,
    fallback: impl FnOnce() -> proc_macro2::Literal,
) -> Result<(), S::Error> {
    let mut literal = s.parse::<proc_macro2::Literal>().unwrap_or_else(|_| fallback());
    literal.set_span(proc_macro2::Span::call_site());
    sink.write_one(proc_macro2::TokenTree::Literal(literal))
}
```

Display parity was cross-checked per type: Integer/Float/StrRaw/ByteStrRaw/CStrRaw's `lit_str`
construction is literally the same `format!` as their Display body, so the five call sites become:

```rust
// Integer
emit_parsed(sink, self.to_string(), || proc_macro2::Literal::i64_unsuffixed(0))
// Float
emit_parsed(sink, self.to_string(), || proc_macro2::Literal::f64_unsuffixed(0.0))
// StrRaw
emit_parsed(sink, self.to_string(), || proc_macro2::Literal::string(&self.value))
// ByteStrRaw
emit_parsed(sink, self.to_string(), || proc_macro2::Literal::byte_string(&self.value))
// CStrRaw
emit_parsed(sink, self.to_string(), || proc_macro2::Literal::string(&self.value))
```

**`CStr` stays hand-written verbatim** — its divergence is real: Unparse builds
`format!("c\"{}\"", self.value)` (no escaping) while its Display escapes backslash/quote
(`replace('\\', "\\\\").replace('"', "\\\"")`). Folding it through `self.to_string()` would silently
change emitted tokens. Flag the escape inconsistency as a possible bug separately; do not fix here.

- Evidence (condensed): 6 impls share the identical parse/fallback/set_span/write_one block; Display ≡
  lit_str for 5 of them; CStr's Display/Unparse genuinely diverge.
- Risk: **low** (mechanical, behavior-preserving for the 5; CStr excluded). Depends on step 10 landing
  first (Display parity).
- Verify: `cargo build -p syan && cargo test --workspace` (round-trip coverage:
  `core/tests/proc_macro2_literal.rs` unparse/roundtrip tests, `rust/tests`).

### 12. Reformat the `impl_parse_for_char!` invocation in `source/string.rs` — ~86 lines [dup-code/format]

Lines 139–236 (98 lines) are exactly `impl_parse_for_char!(` + **96** comma-separated bare idents
(finding said ~100 — minor correction), one per line, + `);`. Change the invocation to brace-delimited
(`impl_parse_for_char! { _a, _b, ... }`) packed ~8–10 idents per line: rustfmt treats a brace-delimited
macro invocation as an opaque block and leaves it as typed, whereas the paren form parses as a
comma-separated expression list and gets exploded vertically. Zero semantic change (delimiter choice is
irrelevant to `macro_rules!` matching). Nuance vs the plan's cited precedent: `impl_char!` in symbol.rs
(70–113) is *also* paren-delimited but survives densely only because its body (`_a(a)@'a' ...`) doesn't
parse as an expression list — the brace form is still the standard, documented workaround and the right
fix here. Expected result: 98 lines → ~12 (≈86 saved).

- Evidence (condensed): pure rustfmt-behavior artifact; body is a plain ident list.
- Risk: **low** (formatting only). Apply **after** step 8 (which deletes string.rs:103, shifting this
  range by −1). Confirm with `cargo fmt --check` that rustfmt leaves the brace form alone.
- Verify: `cargo build -p syan && cargo test --workspace && cargo fmt --check`

### 13. Trim `core/src/symbol.rs` doc bloat — ~65 lines [comment]

All four doctests (252–260, 310–322, 329–339, 349–353) and the character-mapping table (343–347) are kept
— they are executed coverage / real contract. Line numbers below are pre-step-5 (after step 5 they shift
−10; anchor on the quoted headings).

(a) `imp::_Symbol` variant docs: DELETE 150–153 ("The symbol instance variant. … This is the only variant
that can be constructed…"); KEEP 156–161 ("Unreachable phantom variant… cannot be constructed due to the
[`Infallible`] field…" — real invariant); DELETE 165–168 ("Convenience re-export of the `Symbol`
variant…").

(b) The 52-line doc on `pub use imp::_Symbol as Symbol` (223–274): KEEP 223–227 (two-sentence summary)
**and line 229, the link definition `/// [\`Joint\`]: struct@crate::nested::Joint`** — line 226 references
`[Joint]`, so deleting the link def would leave a dangling rustdoc link. DELETE 231–234 (`# Variants`),
236–241 (`# Type Parameter` — its own `[\`chars\`]` link def at 241 is referenced nowhere else, safe to
delete together); KEEP 243–248 (`# Usage`, the "use the `Symbol!` macro" pointer) and 250–260 (doctest);
DELETE 262–266 (`# Traits`), 268–273 (`# Implementation Details`).

(c) `Symbol!` macro doc (294–374): KEEP 294–298 (summary); DELETE 300–304 (`# Syntax`, a non-executed
```` ```text ```` fence); KEEP 306–353 in full (the `# Examples` section: 3 doctests + the a-z/A-Z/0-9/_
mapping table); DELETE 355–359 (`# Type Structure`), 361–366 (`# Traits`), 368–373
(`# Implementation Details`).

- Evidence (condensed): deleted sections restate the item/derives/`_Phantom` doc or the doctests; all
  executed coverage and both real invariants (Infallible-uninhabited; chunking) survive.
- Risk: **low** (prose-only; the one trap is the `[Joint]` link def — keep line 229).
- Verify: `cargo build -p syan && cargo test --workspace` (doc-tests must stay green; optionally
  `cargo doc -p syan 2>&1 | grep -i warn` for the link check).

### 14. Trim narration comments in `core/src/source/proc_macro2.rs` — ~9 lines [comment]

Delete (verbatim first words quoted): line 70 `// Update is_joint based on the token type…` (restates the
`if let` at 71–75); lines 133–135 `// emit_sep should modify the spacing of the last token…` /
`// We need to convert the TokenStream to a vector, modify…` / `// and rebuild the stream`; line 140
`// Rebuild the stream with all tokens except the last` (**addendum** — same restatement species, the
plan's range list omitted it, hence ~9 not ~8 lines); line 143 `// Modify the last token's spacing if
it's a punctuation token`; line 146 `// Create a new punct with Alone spacing to indicate separation`;
line 153 `// For non-punct tokens, just add them back as-is`. Keep ONE folded contract line above
`fn write_sep`:

```rust
// write_sep re-spaces the trailing punct as Alone to signal separation.
```

- Evidence (condensed): each comment maps 1:1 to the adjacent statement; only the write_sep contract is
  worth one kept line.
- Risk: **low** (comment-only). Apply after step 8 (line 109 deletion shifts these by −1).
- Verify: `cargo build -p syan && cargo test --workspace`

### 15. Fix the stale wrapped-element doc in `core/src/visit.rs` — ~7 lines [comment]

Confirmed stale on close reading: the `//` block comment at **lines 60–63** (plan said 60–67; the stale
paragraph is exactly 60–63) justifies `SeqView<T>`'s type parameter by the coexistence of
`SeqView<Box<U>>` and `SeqView<U>` impls on `Vec<Box<U>>` — first words: `// The element type is a
**type parameter** (\`SeqView<T>\`, not an associated type) so the \`Box\`-wrapped // element forms
(\`Vec<Box<T>>\`, \`Option<Box<T>>\`) can implement the *unboxed* view…`. No such wrapped-element impl
exists: the actual impl block (217–324) is exactly `SeqView<T> for Vec<T>/VecDeque<T>/Punctuated<T,P>` and
`OptView<T> for Option<T>/Box<T>/Attempt<T>` — bare-element only, as the correct comment at 211–215
already states.

Replacement for 60–63 (the corrected rationale, one statement in the same style — consistent with
CLAUDE.md's "bare-element: the element *is* the viewed node… wrapped shapes descend by per-layer
recursion"):

```rust
// The element type is a **type parameter** (`SeqView<T>`, not an associated type); the traits are
// bare-element only — a wrapper like `Box<T>`/`Attempt<T>` implements `OptView<T>` directly (single-slot,
// always-full) and the visitor descends *through* wrapped shapes by recursing per layer, not via any
// wrapped-element `SeqView`/`OptView` impl.
```

Also fix the two stale doc phrases: in the `SeqView` trait doc at **65–66**, delete the clause `, and
their /// \`Box\`-wrapped element forms — box-transparent, so the element type is \`T\`, not \`Box<T>\`` —
corrected line:

```rust
/// A mutable, **sequence-like** view of an AST collection field (`Vec`/`VecDeque`/`Punctuated`),
/// bare-element — the element type is `T` itself, never a wrapped `Box<T>`. A generated
```

and in the `OptView` doc at **183–184**, delete `(and \`Option<Box<T>>\`, /// box-transparent)` —
corrected line:

```rust
/// A mutable, **Option-like** view (≤1 element) of an AST `Option` field, bare-element (nested
/// `Box`/`Attempt` layers descend separately). A generated
```

- Evidence (condensed): grep `SeqView<Box` finds no impl, only the stale comment; CLAUDE.md documents the
  wrapped-element model as removed.
- Risk: **low** (doc-only; the replacement wording matches the shipped bare-element design).
- Verify: `cargo build -p syan && cargo test --workspace`

## Refuted (do not apply)

### core/src/source/proc_macro2/literal.rs : 69-405  [test-dup] (claimed ~110 lines)
  Merge the inline `mod tests` with core/tests/proc_macro2_literal.rs into ONE file (suggest:
  core/tests/proc_macro2_literal.rs). Delete from the integration file the ~14 tests whose
  assertions are strict subsets of the inline ones: test_bool_parse_true/false/invalid (⊂
  test_bool_parsing_true/false/invalid, which checks 3 invalid inputs vs 1), test_char_parse (⊂
  test_char_parsing_simple), test_char_parse_escape (⊂ test_char_parsing_escape_sequences, 1
  escape vs 6), test_byte_char_parse (⊂ test_bytechar_parsing_simple class),
  test_char_rejects_byte_char (= test_char_parsing_invalid), test_byte_char_rejects_regular_char
  (= test_bytechar_parsing_invalid), test_integer_parse_plain/with_suffix (⊂
  test_integer_parsing_simple/with_suffixes), test_integer_rejects_float (⊂
  test_integer_parsing_invalid), test_float_rejects_integer (⊂ test_float_parsing_invalid),
  test_str_parse (⊂ test_str_parsing_simple),
  test_bool_display/test_integer_display/test_float_display (⊂ inline
  test_display_implementations). Conversely delete the inline test_display_implementations
  (374-404): every assert except `StrRaw{hash_count:0}` is covered by the integration display
  tests (test_char_display, test_byte_char_display, test_str_raw_display, test_byte_str_display,
  test_cstr_display, test_cstr_raw_display — which also add escape-value coverage the inline test
  lacks); move the one hash_count=0 assert into test_str_raw_display. Keep the integration-only
  unparse/roundtrip tests and the inline-only raw/underscore/escape parse tests. Also delete the
  14-line narration doc block at 71-84 and the 'Note:'/'TODO:'/'Test should not panic' narration
  comments at 144, 180, 292-295, 300, 326-327, 332, 358-359, 364.
  - REFUTED: Read both files side-by-side and grepped the whole worktree. The 11 parse-side subset claims verify (inline tests are strictly broader, same input classes; inline mod uses only public API so the merge compiles). REFUTED on the display tests: the proposal deletes integration test_bool_display/test_integer_display/test_float_display (claimed ⊂ inline test_display_implementations) AND deletes inline test_display_implementations (claimed covered by integration display tests) — circular. For Bool/Integer/Float those are each other's ONLY Display coverage (worktree grep: literal.rs:375-389 and proc_macro2_literal.rs:177-178,199-215 are the only Display asserts; unparse_impl.rs builds tokens directly, never via Display), so applied verbatim the three Display impls in display_impl.rs become untested. Salvageable by keeping the three integration display tests (~20 fewer lines saved). Secondary: the TODO comments at literal.rs:292-295/326-327/358-359 are non-derivable — they alone explain the intentional `if let Ok` (non-unwrap) pattern in the raw-literal tests and should be kept (one line each); the 71-84 doc block and 144/180/300/332/364 notes are safely deletable narration.

### core/src/symbol.rs : 207-220  [dead-code] (claimed ~14 lines)
  The generic `impl<Atom: From<String> + AtomParsedToAllChars> Unparse<Atom> for _Symbol<T>` is
  uninstantiable inside the workspace: no Atom type anywhere implements From<String>
  (proc_macro2::TokenTree gets its own Symbol Unparse in source/proc_macro2.rs:190-205; the string
  source's atom is WithSpan<char, Span>, which has no From<String>). Delete it — UNLESS it is
  intended as a downstream extension point for user-defined atoms, in which case keep (hence high
  risk).
  - REFUTED: Checked instantiability, downstream reachability, and documented intent. The impl IS uninstantiable in-workspace (only atoms are TokenTree and WithSpan<char,Span>; neither has From<String>, and no in-workspace atom satisfies the AtomParsedToAllChars blanket since bare char types have no Parse impls for any shipped atom). REFUTED as a deletion, though: docs/recurse-deferred-fixes-plan.md (lines 45-46, 65-66) explicitly documents this exact From<String>+AtomParsedToAllChars unparse path and tracks "a shipped From<String> atom" as a separate future feature — the impl is a deliberate generic-atom extension point, not accidental dead code. It is also the symmetric pair of the equally in-workspace-unused generic Parse impl (symbol.rs:192-205), and it is public API (pub mod symbol/chars, public blanket trait): any downstream atom with From<String> (String itself qualifies reflexively) plus bare-char Parse impls gets symbol unparse from exactly this impl, so removal is a semver-visible break of a documented planned-feature hook.

### docs : recurse-unbounded-plan.md (278) + recurse-natural-types-plan.md (403) + recurse-deferred-fixes-plan.md (184) + visitor-drive-plan.md (244) + visitor-edit-plan.md (346)  [dead-code] (claimed ~1455 lines)
  Delete the five completed plan documents (or, if the user wants them as design-history, commit
  them — they are currently untracked and unreferenced). All describe work CLAUDE.md's 'Shipped &
  tested' section now documents as the live contract.
  - REFUTED: Refuted the "unreferenced" premise: grep of the whole worktree (excluding docs/ itself) finds live source references to 3 of the 5 docs — core/src/parse/vtable.rs:9,28 cites docs/recurse-unbounded-plan.md (incl. §8.4 for a documented future optimization), core/src/visit.rs:58 cites docs/visitor-edit-plan.md, core/tests/spike_real_parsestream.rs:1 cites recurse-unbounded-plan.md §9.1, and macro/recurse/convert.rs:5 cites recurse-natural-types-plan.md §4. All but convert.rs are git-tracked. The finding's evidence only grepped CLAUDE.md and the memory file. Deletion would orphan these section-anchored pointers, and since docs/ is untracked (?? in git status, confirmed) the loss is unrecoverable. The proposal's alternative branch (commit the docs) is the right move; the deletion branch does not hold. Only visitor-drive-plan.md and recurse-deferred-fixes-plan.md are truly unreferenced, but the finding bundles all five.


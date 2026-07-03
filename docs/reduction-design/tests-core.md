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

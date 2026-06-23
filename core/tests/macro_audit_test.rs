//! Macro-crate audit — compile-fail findings (panics, cryptic/unsound generated code, missing
//! diagnostics) across `#[derive(Ast)]`, `visitor!()`, `#[derive(Parse/Unparse/Spanned)]`,
//! `symbol!`, and `#[recurse]`. Each `tests/ui/audit_*.rs` carries a header explaining the root
//! cause. Silent-wrong findings are demonstrated as runtime tests in `macro_audit_runtime_test.rs`.
//!
//! These are *known limitations*, captured so a fix has a regression target and the failure modes
//! are documented rather than surprising. Nothing here is fixed.
//!
//! ── Also found, but NOT encoded as a trybuild test (and why) ───────────────────────────────────
//! * visitor!(): a visited type's where-clause is dropped, producing ~24 E0277s (macro/visitor.rs
//!   never reads `Generics.where_clause`). Real, but the 24-error .stderr is too version-brittle to
//!   pin; same root cause as the Parse/Unparse where-clause findings below.
//! * visitor!(): listing the same type twice (`visitor!(T, T)`) emits ~18 duplicate-definition
//!   errors (E0428/E0201/E0119/E0592) — the visited list is never deduped. Real; large brittle
//!   .stderr, not pinned.
//! * #[group(self.x)]: a content field naming a non-existent / non-adjacent container is greedily
//!   pulled into the wrong substruct and the abort leaks the internal `__SyanSubstructOf_*_<nonce>`
//!   name (+ a secondary E0412). Real; the random nonce makes the .stderr non-deterministic.
//! * #[derive(Ast)] on a raw-ident type name (`struct r#Type`): panics building the metadata-macro
//!   name (`"__r#_type_ast_<nonce>" is not a valid identifier`). Real; nonce in the message.
//! * #[recurse]: cross-edge generic args are threaded positionally, so two cycle types that share
//!   the root's params but declare them in a different order (`Expr<S,T>` ↔ `Stmt<T,S>`) miscompile
//!   (E0308 in generated code). Real but exotic.
//! * #[recurse]: the "first type parameter is used as the span type" warning is stale after the
//!   heterogeneous-generics refactor (all params are threaded uniformly); nightly-only, so it can't
//!   be asserted by trybuild.
//! * #[joint]/#[alone] abort message has a typo ("alonw") and prints an empty field name for tuple
//!   fields — cosmetic.

#[test]
fn macro_audit_compile_fail() {
    let t = trybuild::TestCases::new();

    // ── attribute derives (Parse / Unparse / Spanned) ──────────────────────────────────────────
    // Parse panics outright on a where-clause (assert!).
    t.compile_fail("tests/ui/audit_parse_where_clause_panic.rs");
    // Unparse/Spanned silently drop a where-clause → cryptic E0277.
    t.compile_fail("tests/ui/audit_unparse_where_clause.rs");
    // Spanned mis-generates for the natural span-parameterized node shape (E0207/E0308).
    t.compile_fail("tests/ui/audit_spanned_composite_fields.rs");
    // Unparse on a zero-variant enum → E0004 (non-exhaustive empty match).
    t.compile_fail("tests/ui/audit_unparse_empty_enum.rs");
    // #[ignore_bounds] is a silent no-op — the field bound is still emitted.
    t.compile_fail("tests/ui/audit_ignore_bounds_noop.rs");
    // Generated parse-stream local `__syan_stream` is not hygienic (collides with a like-named field).
    t.compile_fail("tests/ui/audit_attribute_hygiene_local.rs");

    // ── symbol! ─────────────────────────────────────────────────────────────────────────────────
    // Panics on any unmapped character (unicode XID idents, control chars, …).
    t.compile_fail("tests/ui/audit_symbol_unsupported_char.rs");

    // ── #[derive(Ast)] / visitor!() ─────────────────────────────────────────────────────────────
    // #[subast(path<GenericArgs>)] accepted silently → cryptic error when the intermediate is drilled.
    t.compile_fail("tests/ui/audit_subast_generic_args.rs");
    // A union listed in visitor!() is silently dropped (misleading "no AST definitions resolved").
    t.compile_fail("tests/ui/audit_visitor_union.rs");

    // ── #[recurse] / #[recurse(visit)] ──────────────────────────────────────────────────────────
    // (#6 limit=1-generic and #7 foreign-dispatch are now FIXED — positive regression tests live in
    //  recurse_fixes.rs.)
    // Helper params __V / __R / __Rec are not hygienic (collide with a user param) → E0403.
    t.compile_fail("tests/ui/audit_recurse_helper_param_collision.rs");
    // Generated terminator `XxxTerm` collides with a user type of that name → E0428.
    t.compile_fail("tests/ui/audit_recurse_terminator_collision.rs");
    // A where-clause on a cycle type is not threaded into the regenerated items → cryptic E0277.
    t.compile_fail("tests/ui/audit_recurse_where_clause.rs");
}

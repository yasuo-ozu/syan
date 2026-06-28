//! Macro-crate audit — compile-fail findings (panics, cryptic/unsound generated code, missing
//! diagnostics) across `#[derive(Ast)]`, `visitor!()`, `#[derive(Parse/Unparse/Spanned)]`,
//! `symbol!`, and `#[recurse]`. Each `tests/ui/audit_*.rs` carries a header explaining the root
//! cause. Silent-wrong findings are demonstrated as runtime tests in `macro_audit_runtime_test.rs`.
//!
//! These are *known limitations*, captured so a fix has a regression target and the failure modes
//! are documented rather than surprising. (Audit findings #1–#8 have since been FIXED — see
//! where_clause_attribute.rs, visitor_tuple_field.rs, visitor_where_clause.rs, recurse_fixes.rs, and
//! the symbol! abort below; the entries still registered here remain open.)
//!
//! ── Also found, but NOT encoded as a trybuild test (and why) ───────────────────────────────────
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
    // (#1 Parse where-clause panic, #4 Unparse/Spanned where-clause drop, and #5 Spanned
    //  composite-field span inference are now FIXED — positive regression tests in
    //  where_clause_attribute.rs.)
    // Unparse on a zero-variant enum → E0004 (non-exhaustive empty match).
    t.compile_fail("tests/ui/audit_unparse_empty_enum.rs");
    // (#[ignore_bounds] is now HONORED — it suppresses the field's `: Parse` bound; a positive
    //  regression test lives in `ignore_bounds.rs`.)
    // Generated parse-stream local `__syan_stream` is not hygienic (collides with a like-named field).
    t.compile_fail("tests/ui/audit_attribute_hygiene_local.rs");

    // ── symbol! ─────────────────────────────────────────────────────────────────────────────────
    // #2 (now FIXED to a clean error): an unmapped character (a unicode XID ident, a control char, …)
    // is rejected with a clean spanned abort instead of panicking the proc-macro — still a (clean)
    // compile error, so it stays a compile-fail test.
    t.compile_fail("tests/ui/audit_symbol_unsupported_char.rs");

    // ── #[derive(Ast)] / visitor!() ─────────────────────────────────────────────────────────────
    // #[subast(path<GenericArgs>)] accepted silently → cryptic error when the intermediate is drilled.
    t.compile_fail("tests/ui/audit_subast_generic_args.rs");
    // A non-fully-qualified `#[subast(..)]` path (bare ident / `self::` / `super::`) is rejected with a
    // clear message (it would otherwise resolve in the consumer's scope, not the definition's).
    t.compile_fail("tests/ui/subast_non_full_path.rs");
    // A union listed in visitor!() is silently dropped (misleading "no AST definitions resolved").
    t.compile_fail("tests/ui/audit_visitor_union.rs");

    // ── #[recurse] ──────────────────────────────────────────────────────────────────────────────
    // (#6 limit=1-generic and #7 foreign-dispatch are now FIXED — positive regression tests live in
    //  recurse_fixes.rs.)
    // (A where-clause on a Parse-deriving cycle type is now THREADED through the generated engine /
    //  conversion / delegated impls — positive regression test in `recurse_where_clause.rs`.)
    // (Generated internal names — engine `__XxxRec`, terminator `XxxTerm`, depth default `__XxxDefault`,
    //  conversion traits `__ToNat`/`__FromNat` — now carry a per-expansion nonce, so a user type named
    //  `ExprTerm` no longer collides; positive regression test in `recurse_no_engine.rs`.)
    // KNOWN LIMITATION (#1, deferred): `Unparse`/`Spanned` on the natural type of a GROUP-FUL cycle is
    // engine-only (the group `Fill<Substruct>: Unparse` chain isn't delegable) → `.unparse()` on the
    // natural type doesn't resolve. Group-free cycles do get it (recurse_unparse_spanned.rs).
    t.compile_fail("tests/ui/recurse_group_ful_unparse.rs");
}

//! Macro-crate audit — compile-fail findings (panics, cryptic/unsound generated code, missing
//! diagnostics) across `#[derive(Ast)]`, `visitor!()`, `#[derive(Parse/Unparse/Spanned)]`,
//! `symbol!`, and `#[recurse]`. Each `tests/ui/audit_*.rs` carries a header explaining the root
//! cause. Silent-wrong findings are demonstrated as runtime tests in `macro_audit_runtime_test.rs`.
//!
//! These are *known limitations*, captured so a fix has a regression target and the failure modes
//! are documented rather than surprising. The entries registered below remain open.
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
//! * #[derive(Ast)]: its remaining lint, "`#[subast]` entry `X` matches no field", is invisible to
//!   every stable user. It goes through `proc_macro_error::emit_warning!`, and proc-macro-error's
//!   own docs say "Warnings are emitted only on nightly, they are ignored on stable". Verified with
//!   a deliberately bogus `#[subast(crate::Nonexistent)]` control (silent). Not trybuild-assertable
//!   for the same reason it is invisible, so it is untested.
//! * #[derive(Ast)]: the "follows nothing" lint and its `peel_head` helper have been REMOVED. The
//!   suspicion recorded here — that `peel_head` called `peel(ty, &HashSet::new())` with an EMPTY
//!   `user_types` while `peel` builds a head only under `user_types.contains(..)` — was confirmed:
//!   `Head::Path` has exactly one construction site, inside that gate, and the fallthrough recurses
//!   with the same empty set, so `peel_head` returned `None` for every type. The lint was dead on
//!   nightly too, independently of the emit layer. It could not be repaired in this codebase's
//!   idiom either: it ran only when `#[subast]` was absent, so it had no known-types set, and any
//!   head-finder for it needs the container denylist `peel` deliberately rejects.
//! * #[derive(Ast)]: nothing checks that a DECLARED `#[subast]` entry is actually reached by the
//!   generated traversal. That, not the removed lint, is what would have caught
//!   `macro_audit_runtime_test.rs`'s `visitor_map_value`: `Node` was declared, was found by the
//!   coarse `collect_type_idents` check, and was still dropped by `peel`.

#[test]
fn macro_audit_compile_fail() {
    let t = trybuild::TestCases::new();

    // ── attribute derives (Parse / Unparse / Spanned) ──────────────────────────────────────────
    // Unparse on a zero-variant enum → E0004 (non-exhaustive empty match).
    t.compile_fail("tests/ui/audit_unparse_empty_enum.rs");
    // Generated parse-stream local `__syan_stream` is not hygienic (collides with a like-named field).
    // Its atom is `proc_macro2::TokenTree`, so without the optional dependency it fails on the import
    // (E0433) rather than on the hygiene collision the golden .stderr pins — a different finding.
    #[cfg(feature = "proc_macro2")]
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
}

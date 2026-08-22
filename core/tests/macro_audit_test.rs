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
//! * #[derive(Ast)]: BOTH of its lints — "`#[subast]` entry `X` matches no field" and the
//!   "follows nothing" check — are invisible to every stable user. They go through
//!   `proc_macro_error::emit_warning!`, and proc-macro-error's own docs say "Warnings are emitted
//!   only on nightly, they are ignored on stable". Verified by building a `Vec<Node>` field with no
//!   `#[subast]` (silent) AND a deliberately bogus `#[subast(crate::Nonexistent)]` control (also
//!   silent), so it is the emit layer rather than one lint. This matters more than it looks: the
//!   "follows nothing" lint was written for exactly the failure pinned in
//!   `macro_audit_runtime_test.rs`'s `visitor_map_value` — the diagnostic already exists and simply
//!   cannot speak. A `compile_error!` path, a `cargo:warning=` emission, or nightly detection would
//!   each turn it back on. Not trybuild-assertable for the same reason it is invisible.
//! * #[derive(Ast)]: `peel_head` (ast.rs) calls `peel(ty, &HashSet::new())` with an EMPTY
//!   `user_types`, and `peel`'s head arm only fires on `user_types.contains(..)` — so it looks
//!   incapable of ever returning `Some`, which would make the "follows nothing" lint dead even on
//!   nightly. UNCONFIRMED: the dead emit layer above masks it, so it needs a nightly run to
//!   separate the two.

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

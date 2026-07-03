//! Macro audit — *silent-wrong* findings, demonstrated as runtime tests.
//!
//! Each test below COMPILES CLEANLY (no error, no warning) yet produces a wrong result — the most
//! insidious class of macro bug. The asserts pin the current (buggy) behavior with a `BUG:` comment
//! stating the correct expectation, so the day a fix lands the assertion flips and points here.
//! Compile-error / panic findings live in `macro_audit_test.rs` (+ `ui/audit_*.rs`).
//!
//! The two symbol! encoding bugs below remain open.
#![allow(dead_code)]

// ── BUG: symbol! re-encodes non-decimal / underscored int literals to canonical decimal ──────────
// The `LitInt` branch builds the symbol slot from `litint.base10_digits()`, discarding the base
// prefix and digit separators. A `Symbol!` is a type-level *name*, so the written spelling should be
// preserved (or non-decimal literals rejected).
#[test]
fn symbol_reencodes_int_literals() {
    use syan::symbol::Symbol;
    // BUG: each of these should preserve the written spelling (or be rejected); instead the literal
    // is silently normalized to decimal.
    assert_eq!(<Symbol![0xff]>::default().to_string(), "255", "0xff should stay \"0xff\"");
    assert_eq!(<Symbol![0b101]>::default().to_string(), "5", "0b101 should stay \"0b101\"");
    assert_eq!(<Symbol![0o17]>::default().to_string(), "15", "0o17 should stay \"0o17\"");
    assert_eq!(<Symbol![1_000]>::default().to_string(), "1000", "1_000 should stay \"1_000\"");
}

// ── BUG: symbol! leaks a raw identifier's `r#` prefix into the symbol string ─────────────────────
// The `Ident` branch uses `ident.to_string()`, which for a raw ident yields "r#type" (the `#` is
// encoded via `chars::Pound`). A raw ident is exactly how one names a symbol after a keyword, so the
// common `Symbol![r#type]` case is mis-encoded; the `r#` should be stripped.
#[test]
fn symbol_leaks_raw_ident_prefix() {
    use syan::symbol::Symbol;
    // BUG: should be "type"; the `r#` prefix leaks through.
    assert_eq!(<Symbol![r#type]>::default().to_string(), "r#type", "raw-ident prefix should be stripped");
}

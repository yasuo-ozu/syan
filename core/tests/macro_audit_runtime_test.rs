//! Macro audit — *silent-wrong* findings, demonstrated as runtime tests.
//!
//! Each test below COMPILES CLEANLY (no error, no warning) yet produces a wrong result — the most
//! insidious class of macro bug. The asserts pin the current (buggy) behavior with a `BUG:` comment
//! stating the correct expectation, so the day a fix lands the assertion flips and points here.
//! Compile-error / panic findings live in `macro_audit_test.rs` (+ `ui/audit_*.rs`).
#![allow(dead_code)]

// ── BUG 1: visitor!() silently skips a tuple-typed field ─────────────────────────────────────────
// `util::peel` has no `Type::Tuple` arm, so a field whose type is a tuple peels to `None` and
// visitor.rs `lower_field` treats it as a leaf (bound `_`). A visited type referenced ONLY inside a
// tuple field is never traversed — and no diagnostic fires (the "follows nothing" lint is off when a
// `#[subast]` is present, and the "unused entry" check DOES recurse into tuples so the entry looks
// used). Contrast: nested containers get a clean `abort!`, and `#[recurse(visit)]` DOES traverse
// tuples. The visitor!() path should either traverse tuple elements or reject them.
mod tuple_skip {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Debug, Ast)]
    pub enum Ty<S> {
        Unit(PhantomData<S>),
    }

    #[derive(Debug, Ast)]
    #[subast(crate::tuple_skip::Ty)]
    pub enum Expr<S> {
        Pair((Ty<S>, Ty<S>)),
        Lit(PhantomData<S>),
    }

    pub mod visit {
        syan::visit::visitor!(super::Expr, super::Ty);
    }
}

#[test]
fn visitor_tuple_field_is_silently_skipped() {
    use tuple_skip::{Expr, Ty};
    let e: Expr<()> = Expr::Pair((Ty::Unit(core::marker::PhantomData), Ty::Unit(core::marker::PhantomData)));
    let mut n = 0usize;
    e.visit(|_t: &Ty<()>| n += 1);
    // BUG: both `Ty` values inside the tuple should be visited (n == 2). The tuple field is
    // silently treated as a leaf, so n == 0, with no compile error and no warning.
    assert_eq!(n, 0, "tuple field silently skipped (correct would be 2)");
}

// ── BUG 2: symbol! re-encodes non-decimal / underscored int literals to canonical decimal ────────
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

// ── BUG 3: symbol! leaks a raw identifier's `r#` prefix into the symbol string ───────────────────
// The `Ident` branch uses `ident.to_string()`, which for a raw ident yields "r#type" (the `#` is
// encoded via `chars::Pound`). A raw ident is exactly how one names a symbol after a keyword, so the
// common `Symbol![r#type]` case is mis-encoded; the `r#` should be stripped.
#[test]
fn symbol_leaks_raw_ident_prefix() {
    use syan::symbol::Symbol;
    // BUG: should be "type"; the `r#` prefix leaks through.
    assert_eq!(<Symbol![r#type]>::default().to_string(), "r#type", "raw-ident prefix should be stripped");
}

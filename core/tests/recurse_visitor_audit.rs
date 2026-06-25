//! Audit suite for `visitor!()`-over-`#[recurse]` (and one acyclic `visitor!()` footgun). Each case is
//! a CONFIRMED open bug found by a parallel audit of the macro after retiring `#[recurse(visit)]`.
//!
//! These tests assert the DESIRED (fixed) behavior, so they are **RED until each bug is fixed** — a
//! failing case here is an open audit item, not a regression. (Contrast `recurse_audit_test.rs`, whose
//! cases already abort cleanly and stay green.)
//!
//! The "should compile once fixed" cases (A, C, E) are plain BULK test files — they fail to *build*
//! today, which is the finding — not trybuild fixtures:
//!   - A: `audit_visitor_recurse_helper_hygiene.rs`   (helper idents `__V`/`__R0`/`__W` not fresh-named → E0403)
//!   - C: `audit_visitor_recurse_nonroot_lifetime.rs` (non-root extra lifetime emitted after a type param)
//!   - E: `audit_visitor_followed_ref_breaks_mut.rs`  (followed `&T` field breaks the auto `visit_mut` → E0596)
//!
//! Only the "should be REJECTED once fixed" case (D) is a trybuild `compile_fail` (below): it *compiles*
//! today — the bug — and turns green once the macro rejects it with a clean `abort!` (then bless the
//! `.stderr`).
//!
//! ── Also found, but NOT encoded as a test (brittle/duplicate output) ────────────────────────────
//!  * B. `visitor!()` over two independent cycles with DISJOINT root params (`Expr<S>` + `Foo<T>`):
//!    `generate_module_mixed` keys the trait on the global union `{S, T}` and applies it to each
//!    per-cycle terminator (`impl<S, T, ..> VisitRec<S, T, __V> for ExprTerm<S>` — but `ExprTerm` takes
//!    only `<S>` → E0107, then an E0277 cascade). Real miscompile; ~180-line brittle output. Fix: key
//!    per cycle, or `abort!` when independent cycles don't share identical root params.
//!  * C'. The union-param ordering bug (case C) also surfaces on the acyclic side: when the visited
//!    types' union orders a lifetime after a type param (`visitor!(Leaf, Holder<'a, S>)`, union
//!    `<S, 'a>`), the generated generics are lifetime-after-type. Same root cause as C.

#[test]
fn recurse_visitor_audit() {
    let t = trybuild::TestCases::new();

    // D — RED until fixed (currently COMPILES — it must not). A rootless `C⇄D` sub-cycle with ≤1
    //   self-referential root is silently accepted and un-depth-limited (the `subgraph_is_cyclic` guard
    //   runs only on the multi-root path). SHOULD be REJECTED with a clean `abort!` once the guard runs
    //   on the single-root path too; then bless the `.stderr`.
    t.compile_fail("tests/ui/recurse_rootless_subcycle_single_root.rs");
}

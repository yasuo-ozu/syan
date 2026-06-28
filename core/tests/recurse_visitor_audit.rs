//! Regression tests for `visitor!()`-over-`#[recurse]` (and one acyclic `visitor!()`) audit findings
//! — all now FIXED. Each test guards a specific fix from this branch; the per-file headers explain the
//! original bug and the fix.
//!
//! The "must compile" findings (A, C, E) are plain bulk test files (their compiling is the check):
//!   - A: `audit_visitor_recurse_helper_hygiene.rs`   (helper idents `__V`/`__R0`/`__W` now fresh-named)
//!   - C: `audit_visitor_recurse_nonroot_lifetime.rs` (a non-root extra lifetime now emitted lifetime-first)
//!   - E: `audit_visitor_followed_ref_breaks_mut.rs`  (a followed `&T` is now a leaf on the mut side)
//! Plus C' (the acyclic union-param ordering) is fixed by sorting the trait's params lifetime-first;
//! exercised indirectly by the above.
//!
//! The "must be REJECTED" findings (B, D) abort cleanly and are pinned below:

#[test]
fn recurse_visitor_audit() {
    let t = trybuild::TestCases::new();

    // (B is now SUPPORTED: with natural types, a `visitor!()` over independent cycles with disjoint
    //  params is an ordinary union-param acyclic visitor — see `visitor_multicycle_disjoint_params.rs`.)

    // D (fixed): a rootless `C⇄D` sub-cycle with ≤1 self-referential root is now rejected (the
    //   `subgraph_is_cyclic` guard runs on the single-root path too) instead of silently compiling
    //   un-depth-limited.
    t.compile_fail("tests/ui/recurse_rootless_subcycle_single_root.rs");
}

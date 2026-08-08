//! `#[recurse]` (and `visitor!()`-over-`#[recurse]`) edge cases that are *cleanly rejected* —
//! compile-fail tests confirming each unsupported shape aborts with an intentional diagnostic rather
//! than silently misbehaving or emitting cryptic generated-code errors.
//!
//! Companion to `recurse_problems_test.rs` (the original `#[derive(Parse, Unparse)]` recurse
//! limitations). The *supported* container/tuple traversals live in `visitor_recurse_containers.rs`;
//! support for lifetime / type / const generic params on cycle types lives in `recurse_generics.rs`.
//!
//! Each `tests/ui/recurse_*.rs` file carries a header explaining the case. With one exception
//! (`limit = 0`, still a raw panic) these are deliberate `abort!`s with actionable messages.

#[test]
fn recurse_audit_compile_fail() {
    let t = trybuild::TestCases::new();

    // 1. `#[recurse(limit = 0)]` underflows `recursion_depth - 1` → macro panic.
    //    (Left as a panic on purpose; the smallest sound limit is 1.)
    t.compile_fail("tests/ui/recurse_limit_zero.rs");

    // (2. nested containers in a `#[recurse]` cycle are now traversed — see
    //  `visitor_nested_containers.rs`.)

    // 4. A cycle type may carry *extra* generic params, but must declare all of the ROOT's params
    //    (so the depth default is spellable). One that's missing a root param is rejected, naming it.
    //    (Lifetime / type / const params and per-type extras ARE supported — see recurse_generics.rs.)
    t.compile_fail("tests/ui/recurse_missing_root_param.rs");

    // 6. A multi-root cycle whose self-referential roots are NOT a feedback vertex set — i.e. a
    //    sub-cycle runs entirely through non-self-referential types, so the depth (which only
    //    decrements at a root) would never terminate. Rejected with a clear message. (Multi-root
    //    cycles where every cycle passes through a root ARE supported — see recurse_multiroot.rs.)
    t.compile_fail("tests/ui/recurse_multiroot_rootless_subcycle.rs");

    // 7. A non-identity generic argument on a back-edge to the root (`Expr<Vec<S>>`) makes the
    //    recursion non-regular; the single-`__Rec` depth machinery can't thread it, so it's
    //    rejected (was: the argument was silently dropped → miscompile).
    t.compile_fail("tests/ui/recurse_complex_root_param.rs");
}

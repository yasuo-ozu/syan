//! Pins the documented gap: a `visitor!(..)` directly over a `#[recurse]` cyclic type does not
//! compile (its metadata macro is only reachable under the internal renamed type, not the public
//! alias). See `tests/ui/recurse_cycle_visitor.rs` for the explanation.

#[test]
fn visitor_over_recurse_cycle_is_unsupported() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/recurse_cycle_visitor.rs");
}

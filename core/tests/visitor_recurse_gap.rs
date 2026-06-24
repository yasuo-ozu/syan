//! Pins the documented gap: a `visitor!(..)` directly over a `#[recurse]` cyclic type does not
//! compile. As of Phase 0, `#[recurse]` *does* emit a metadata macro under the cycle type's original
//! name (carrying a `@recurse { .. }` section), so the fetch now resolves — but the `visitor!()`
//! consumer does not yet understand `@recurse` and rejects it (`unknown section @recurse`). Consuming
//! that metadata is future work. See `tests/ui/recurse_cycle_visitor.rs` for the explanation.

#[test]
fn visitor_over_recurse_cycle_is_unsupported() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/recurse_cycle_visitor.rs");
}

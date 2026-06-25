//! Build-time diagnostics for visitor footguns (clear errors instead of opaque cascades). See the
//! `tests/ui/*.rs` companions.

#[test]
fn visitor_diagnostics() {
    let t = trybuild::TestCases::new();
    // Two visited types sharing a last segment collide on generated names.
    t.compile_fail("tests/ui/visited_collision.rs");
    // A field with nested containers (Vec<Option<T>>) is unsupported.
    t.compile_fail("tests/ui/nested_container.rs");
    // `visitor!()` over a `#[recurse]` cycle mixed with an acyclic type carrying a param no cycle
    // root has (would make the depth-generic `VisitRec` impls' param unconstrained — E0207).
    t.compile_fail("tests/ui/visitor_recurse_mixed_acyclic_extra_param.rs");
}

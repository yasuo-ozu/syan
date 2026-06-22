//! Build-time diagnostics for visitor footguns (clear errors instead of opaque cascades). See the
//! `tests/ui/*.rs` companions.

#[test]
fn visitor_diagnostics() {
    let t = trybuild::TestCases::new();
    // Two visited types sharing a last segment collide on generated names.
    t.compile_fail("tests/ui/visited_collision.rs");
    // A field with nested containers (Vec<Option<T>>) is unsupported.
    t.compile_fail("tests/ui/nested_container.rs");
}

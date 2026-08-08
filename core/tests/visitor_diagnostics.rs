//! Build-time diagnostics for visitor footguns (clear errors instead of opaque cascades). See the
//! `tests/ui/*.rs` companions.

#[test]
fn visitor_diagnostics() {
    let t = trybuild::TestCases::new();
    // Two visited types sharing a last segment collide on generated names.
    t.compile_fail("tests/ui/visited_collision.rs");
    // (Nested containers `Vec<Option<T>>` are now supported — see `visitor_nested_containers.rs`.)
    // `visitor!()` over a `#[recurse]` cycle mixed with an acyclic type carrying a param no cycle
    // root has (would make the depth-generic `VisitRec` impls' param unconstrained — E0207).
    t.compile_fail("tests/ui/visitor_recurse_mixed_acyclic_extra_param.rs");
    // `visitor!()` over a MULTI-ROOT `#[recurse]` cycle that omits a co-root (a root defines a depth
    // dimension and can't be drilled, so every root must be listed).
    t.compile_fail("tests/ui/visitor_recurse_unlisted_coroot.rs");
    // A `where`-bounded generic param not declared by every visited type (the bound would be
    // undischargeable on the param-less visited type).
    t.compile_fail("tests/ui/visitor_union_where_unshared_param.rs");
}

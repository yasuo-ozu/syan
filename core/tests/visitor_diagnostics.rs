//! Build-time diagnostics for visitor footguns (clear errors instead of opaque cascades). See the
//! `tests/ui/*.rs` companions.

#[test]
fn visitor_diagnostics() {
    let t = trybuild::TestCases::new();
    // Two visited types sharing a last segment collide on generated names.
    t.compile_fail("tests/ui/visited_collision.rs");
    // (Nested containers `Vec<Option<T>>` are now supported — see `visitor_nested_containers.rs`.)
    // (A former-`#[recurse]` cycle mixed with an acyclic type carrying an extra param is now SUPPORTED
    //  — natural types make it an ordinary union-param acyclic visitor; see
    //  `visitor_mixed_recurse_extra_param.rs`.)
    // `visitor!()` over a cycle that follows an unlisted intermediate forming a cycle of unlisted
    // intermediates (list one of them to break it).
    t.compile_fail("tests/ui/visitor_recurse_unlisted_coroot.rs");
    // (A `where`-bounded generic param not shared by all visited types is now SUPPORTED — the bounded
    //  param becomes a per-method generic, trait keyed on the shared subset; see
    //  `visitor_union_where_unshared_param.rs`.)
}

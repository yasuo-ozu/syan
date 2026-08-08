//! Build-time diagnostics for visitor footguns (clear errors instead of opaque cascades). See the
//! `tests/ui/*.rs` companions.

#[test]
fn visitor_diagnostics() {
    let t = trybuild::TestCases::new();
    // Two visited types sharing a last segment collide on generated names.
    t.compile_fail("tests/ui/visited_collision.rs");
    // `visitor!()` over a cycle that follows an unlisted intermediate forming a cycle of unlisted
    // intermediates (list one of them to break it).
    t.compile_fail("tests/ui/visitor_recurse_unlisted_coroot.rs");
    // Drilling that would loop through a cycle of unlisted intermediates is a compile error ("list one").
    t.compile_fail("tests/ui/drill_cycle.rs");
    // The container-edit views (`visit_*_seq`/`visit_*_opt`) are generated ONLY for a field marked
    // `#[seq]`/`#[opt]` (no auto-detection); overriding them for an unmarked field is a "not a member of
    // trait" error.
    t.compile_fail("tests/ui/visitor_edit_unmarked_no_view.rs");
    // A `#[seq]`/`#[opt]` field can't view an inherited (non-targeted) type — clean error, not E0599.
    t.compile_fail("tests/ui/visitor_edit_seq_inherited.rs");
    // Marker on a non-viewable / container-less / non-visited field → clean abort, not a cryptic trait error.
    t.compile_fail("tests/ui/visitor_edit_marker_array.rs");
    t.compile_fail("tests/ui/visitor_edit_marker_noncontainer.rs");
    t.compile_fail("tests/ui/visitor_edit_marker_unvisited.rs");
    // A `#[seq]`/`#[opt]` field whose top-level type wraps a container (`Box<Vec<T>>`) is not an edit
    // target — edit views need a bare single container (the field still descends).
    t.compile_fail("tests/ui/visitor_edit_marker_boxed.rs");
}

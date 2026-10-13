//! Build-time diagnostics for visitor footguns (clear errors instead of opaque cascades). See the
//! `tests/ui/*.rs` companions.

mod ui_toolchain;

#[test]
fn visitor_diagnostics() {
    if !ui_toolchain::should_run("visitor_diagnostics") {
        return;
    }
    let t = trybuild::TestCases::new();
    // `define_visit!()` over a cycle that follows an unlisted intermediate forming a cycle of unlisted
    // intermediates (list one of them to break it).
    t.compile_fail("tests/ui/visitor_recurse_unlisted_coroot.rs");
}

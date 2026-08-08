//! The cycle guard: a `visitor!(..)` whose drilling would loop through unlisted intermediates is a
//! compile error (see `tests/ui/drill_cycle.rs`).

#[test]
fn drill_cycle_is_rejected() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/drill_cycle.rs");
}

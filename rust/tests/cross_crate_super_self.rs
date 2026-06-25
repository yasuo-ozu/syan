//! Residual hole (3-crate): a downstream visitor inheriting an upstream intermediate that recorded a
//! `super::`/`self::`-relative ancestor can't resolve it — fundamental (a proc-macro can't requalify a
//! relative module path; only a leading `crate::` is rewritable). The companion *working* case (the
//! upstream intermediate using a `crate::`-rooted path) is `cross_crate_inherit_multilevel.rs`.
//!
//! See `tests/ui/cross_crate_super_self.rs` for the case + the explanation.

#[test]
fn cross_crate_super_self_ancestor_is_unresolvable() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/cross_crate_super_self.rs");
}

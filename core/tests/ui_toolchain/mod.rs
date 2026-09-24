//! Guard for the trybuild suites.
//!
//! The `tests/ui/*.stderr` fixtures compare rustc diagnostics verbatim, and rustc rewords them,
//! shortens type paths and adds follow-on errors between releases — so a fixture is only meaningful
//! on the toolchain it was blessed against. CI runs them in their own job pinned to [`BLESSED`]
//! while the ordinary test job skips them by name.
//!
//! A plain `cargo test` has no such arrangement, so without this guard it fails on any other
//! toolchain, for a reason that has nothing to do with the code under test. Each suite asks here
//! first and skips with a note saying how to run it.

/// The toolchain `tests/ui/*.stderr` was blessed against. Bumping this means re-blessing in the
/// same commit:
///
/// ```text
/// TRYBUILD=overwrite cargo +<version> test -p syan --all-features \
///     --test macro_audit_test --test recurse_core --test visitor_diagnostics
/// ```
///
/// Keep it in step with the `ui` job in `.github/workflows/ci.yml`.
pub const BLESSED: &str = "1.90.0";

/// Whether the UI fixtures should run: only on [`BLESSED`], or whenever `SYAN_UI=1` forces them
/// (which is how you see the mismatch when re-blessing for a new pin).
///
/// Returns `false` after printing why, so a caller can `return` and leave the test passing.
pub fn should_run(suite: &str) -> bool {
    if std::env::var_os("SYAN_UI").is_some_and(|v| v != "0") {
        return true;
    }
    match active_version() {
        Some(v) if v == BLESSED => true,
        other => {
            let found = other.unwrap_or_else(|| "unknown".into());
            println!(
                "skipping the `{suite}` UI fixtures: they are blessed against rustc {BLESSED}, and \
                 this is rustc {found}. Run them with `cargo +{BLESSED} test -p syan \
                 --all-features --test {suite}`, or set SYAN_UI=1 to run them here anyway."
            );
            false
        }
    }
}

/// The version of the rustc that will compile the fixtures — the `rustc` on `PATH`, which under
/// rustup is the toolchain `cargo` was invoked with (`cargo +1.90.0 test` sets `RUSTUP_TOOLCHAIN`
/// for this process, and the shim honours it).
fn active_version() -> Option<String> {
    let out =
        std::process::Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .arg("--version")
            .output()
            .ok()?;
    // `rustc 1.90.0 (1159e78c4 2025-09-14)` -> `1.90.0`
    String::from_utf8(out.stdout)
        .ok()?
        .split_whitespace()
        .nth(1)
        .map(str::to_owned)
}

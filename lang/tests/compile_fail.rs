#[test]
fn compile_fail_tests() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/*.rs");
}

/// Fixtures whose rustc output is feature-sensitive (the impl-suggestion list
/// renders differently between feature sets). Blessed under `--all-features`,
/// which is what `make test` and CI run; skipped in default-feature ad-hoc
/// runs so the goldens have exactly one canonical rendering.
#[cfg(feature = "idl-build")]
#[test]
fn compile_fail_tests_all_features() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail_all_features/*.rs");
}

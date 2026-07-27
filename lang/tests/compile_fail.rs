#[test]
fn compile_fail_tests() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/*.rs");
}

/// TEMPORARY, until zeropod >=0.3.4 publishes: these goldens bake the
/// machine-local git-checkout path of the zeropod `[patch]` (trybuild
/// normalizes registry paths, never git checkouts), so they cannot match on
/// another machine. Local runs keep the guard; when the published zeropod
/// replaces the patch, re-bless these under `tests/compile_fail/` and delete
/// this gate.
#[test]
fn compile_fail_tests_machine_local() {
    if std::env::var_os("CI").is_some() {
        return;
    }
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail_local/*.rs");
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

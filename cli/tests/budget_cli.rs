//! End-to-end coverage for `quasar profile --write-budget/--check-budget`.
//!
//! These assert the CI contract the unit tests cannot: the process exit code
//! and what lands on stdout. They need a real sBPF binary, which `make test`
//! provides by running `make build-sbf` first.

use {
    std::{
        error::Error,
        fs,
        path::{Path, PathBuf},
        process::{Command, Output},
    },
    tempfile::{tempdir, TempDir},
};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

/// A program built by `make build-sbf`, or `None` when the SBF artifacts have
/// not been built in this checkout.
///
/// Only `quasar_test_*` qualifies: `make build-sbf` builds those with
/// `--features debug`, so they carry the symbols the profiler needs. The
/// examples are built without it and may be stripped.
fn fixture_elf() -> Option<PathBuf> {
    let deploy = workspace_root().join("target").join("deploy");
    let mut candidates: Vec<PathBuf> = fs::read_dir(deploy)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().is_some_and(|ext| ext == "so")
                && path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .is_some_and(|name| name.starts_with("quasar_test_"))
        })
        .collect();
    candidates.sort();
    candidates.into_iter().next()
}

/// Returns `None` when there is nothing to profile, so a checkout without SBF
/// artifacts reports a skip instead of a spurious failure. CI always builds
/// them first, so there a missing fixture is a failure, not a skip — otherwise
/// these tests would quietly stop testing anything.
fn setup() -> Option<(TempDir, PathBuf, PathBuf)> {
    let Some(elf) = fixture_elf() else {
        assert!(
            std::env::var_os("CI").is_none(),
            "no target/deploy/quasar_test_*.so fixture in CI; `make build-sbf` must run first"
        );
        eprintln!("skipping: no target/deploy/quasar_test_*.so; run `make build-sbf` first");
        return None;
    };
    let dir = tempdir().expect("tempdir");
    let budget = dir.path().join("quasar-budget.toml");
    Some((dir, elf, budget))
}

fn profile(cwd: &Path, elf: &Path, budget: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_quasar"))
        .arg("profile")
        .arg(elf)
        .arg("--budget")
        .arg(budget)
        .args(extra)
        .current_dir(cwd)
        .output()
        .expect("run quasar profile")
}

fn combined(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn program_name(elf: &Path) -> String {
    elf.file_stem()
        .and_then(|s| s.to_str())
        .expect("program name")
        .to_string()
}

#[test]
fn write_budget_bootstraps_a_file_that_passes_its_own_check() -> Result<(), Box<dyn Error>> {
    let Some((dir, elf, budget)) = setup() else {
        return Ok(());
    };

    let write = profile(dir.path(), &elf, &budget, &["--write-budget"]);
    assert!(write.status.success(), "{}", combined(&write));
    assert!(budget.exists(), "--write-budget must create the file");

    let contents = fs::read_to_string(&budget)?;
    assert!(
        contents.contains(&format!("[programs.{}]", program_name(&elf))),
        "{contents}"
    );
    assert!(contents.contains("total_cu = "), "{contents}");
    assert!(contents.contains("binary_size = "), "{contents}");

    let check = profile(dir.path(), &elf, &budget, &["--check-budget"]);
    assert!(
        check.status.success(),
        "a freshly written budget must pass:\n{}",
        combined(&check)
    );

    Ok(())
}

#[test]
fn check_budget_exits_nonzero_and_names_every_violation() -> Result<(), Box<dyn Error>> {
    let Some((dir, elf, budget)) = setup() else {
        return Ok(());
    };

    // Ceilings of zero put every metric over budget at once.
    fs::write(
        &budget,
        format!(
            "[programs.{name}]\ntotal_cu = 0\nbinary_size = 0\n",
            name = program_name(&elf)
        ),
    )?;

    let check = profile(dir.path(), &elf, &budget, &["--check-budget"]);
    assert_eq!(
        check.status.code(),
        Some(1),
        "a violated budget must fail CI:\n{}",
        combined(&check)
    );

    let output = combined(&check);
    assert!(output.contains("total CU"), "{output}");
    assert!(output.contains("binary size"), "{output}");
    assert!(output.contains("budget check failed"), "{output}");

    Ok(())
}

#[test]
fn json_report_is_the_only_thing_on_stdout() -> Result<(), Box<dyn Error>> {
    let Some((dir, elf, budget)) = setup() else {
        return Ok(());
    };

    fs::write(
        &budget,
        format!(
            "[programs.{name}]\ntotal_cu = 0\n",
            name = program_name(&elf)
        ),
    )?;

    let check = profile(dir.path(), &elf, &budget, &["--check-budget", "--json"]);
    assert_eq!(check.status.code(), Some(1), "{}", combined(&check));

    // Parsing the whole of stdout proves no summary or ANSI codes leaked in.
    let stdout = String::from_utf8(check.stdout)?;
    let report: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| format!("stdout was not pure JSON: {e}\n{stdout}"))?;

    assert_eq!(report["program"], program_name(&elf));
    assert_eq!(report["ok"], false);
    assert_eq!(report["violations"][0]["metric"], "totalCu");
    assert!(report["measured"]["totalCu"].is_u64());

    Ok(())
}

#[test]
fn missing_budget_fails_with_a_bootstrap_hint() -> Result<(), Box<dyn Error>> {
    let Some((dir, elf, budget)) = setup() else {
        return Ok(());
    };

    let check = profile(dir.path(), &elf, &budget, &["--check-budget"]);
    assert_eq!(check.status.code(), Some(1), "{}", combined(&check));

    let output = combined(&check);
    assert!(output.contains("no budget file"), "{output}");
    assert!(output.contains("--write-budget"), "{output}");

    Ok(())
}

#[test]
fn malformed_budget_fails_naming_the_file() -> Result<(), Box<dyn Error>> {
    let Some((dir, elf, budget)) = setup() else {
        return Ok(());
    };

    fs::write(&budget, "[programs.demo]\ntotal_cu = \"lots\"\n")?;

    let check = profile(dir.path(), &elf, &budget, &["--check-budget"]);
    assert_eq!(check.status.code(), Some(1), "{}", combined(&check));

    let output = combined(&check);
    assert!(output.contains("failed to parse"), "{output}");
    assert!(output.contains("quasar-budget.toml"), "{output}");

    Ok(())
}

#[test]
fn budget_without_a_section_for_this_program_fails() -> Result<(), Box<dyn Error>> {
    let Some((dir, elf, budget)) = setup() else {
        return Ok(());
    };

    fs::write(&budget, "[programs.some_other_program]\ntotal_cu = 10\n")?;

    let check = profile(dir.path(), &elf, &budget, &["--check-budget"]);
    assert_eq!(check.status.code(), Some(1), "{}", combined(&check));

    let output = combined(&check);
    assert!(
        output.contains(&format!("[programs.{}]", program_name(&elf))),
        "{output}"
    );

    Ok(())
}

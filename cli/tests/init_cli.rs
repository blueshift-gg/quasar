use {
    std::{
        error::Error,
        fs,
        path::{Path, PathBuf},
        process::{Command, Output},
    },
    tempfile::tempdir,
};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn assert_success(label: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{label} should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

fn use_workspace_lang(project_dir: &Path) -> Result<(), Box<dyn Error>> {
    let manifest_path = project_dir.join("Cargo.toml");
    let manifest = fs::read_to_string(&manifest_path)?;
    let dependency = manifest
        .lines()
        .find(|line| line.starts_with("quasar-lang = "))
        .ok_or("generated manifest is missing quasar-lang")?;
    let local_dependency = format!(
        "quasar-lang = {{ path = {:?} }}",
        workspace_root().join("lang")
    );
    fs::write(
        manifest_path,
        manifest.replacen(dependency, &local_dependency, 1),
    )?;
    Ok(())
}

#[test]
fn generated_starters_pass_strict_lint() -> Result<(), Box<dyn Error>> {
    let temp = tempdir()?;
    let home = temp.path().join("home");
    fs::create_dir(&home)?;

    for template in ["minimal", "full"] {
        let name = format!("strict-{template}");
        let init = Command::new(env!("CARGO_BIN_EXE_quasar"))
            .arg("init")
            .arg(&name)
            .arg("--yes")
            .arg("--no-git")
            .arg("--test-language")
            .arg("none")
            .arg("--template")
            .arg(template)
            .arg("--toolchain")
            .arg("solana")
            .env("HOME", &home)
            .current_dir(temp.path())
            .output()?;
        assert_success(&format!("quasar init --template {template}"), &init);

        let project_dir = temp.path().join(name);
        use_workspace_lang(&project_dir)?;

        let lint = Command::new(env!("CARGO_BIN_EXE_quasar"))
            .arg("lint")
            .arg("--strict")
            .arg("--no-diff")
            .current_dir(&project_dir)
            .output()?;
        assert_success(
            &format!("quasar lint --strict for {template} starter"),
            &lint,
        );
    }

    Ok(())
}

use {
    std::{error::Error, fs, path::PathBuf, process::Command},
    tempfile::tempdir,
};

fn assert_success(label: &str, output: &std::process::Output) {
    assert!(
        output.status.success(),
        "{label} should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn canonical_init_is_non_interactive_and_has_no_global_side_effects() -> Result<(), Box<dyn Error>>
{
    let temp = tempdir()?;
    let home = temp.path().join("home");
    fs::create_dir(&home)?;
    let config_path = home.join(".quasar/config.toml");
    fs::create_dir_all(config_path.parent().unwrap())?;
    fs::write(&config_path, "[ui]\ncolor = false\n")?;

    let output = Command::new(env!("CARGO_BIN_EXE_quasar"))
        .args(["init", "canonical", "--no-git", "--verbose"])
        .env("HOME", &home)
        .env("QUASAR_DEV_ROOT", workspace_root())
        .current_dir(temp.path())
        .output()?;
    assert_success("quasar init", &output);

    let root = temp.path().join("canonical");
    assert!(!root.join(".git").exists());
    assert!(root.join("Cargo.lock").is_file());
    assert!(root.join("src/tests.rs").is_file());
    assert!(!root.join("package.json").exists());
    assert!(!root.join("src/state.rs").exists());
    assert_eq!(
        fs::read_to_string(config_path)?,
        "[ui]\ncolor = false\n",
        "init must not mutate global configuration"
    );
    Ok(())
}

#[test]
fn removed_scaffold_flags_are_unknown() -> Result<(), Box<dyn Error>> {
    for flag in [
        "--yes",
        "--test-language",
        "--rust-framework",
        "--ts-sdk",
        "--template",
        "--toolchain",
    ] {
        let temp = tempdir()?;
        let home = temp.path().join("home");
        fs::create_dir(&home)?;
        let output = Command::new(env!("CARGO_BIN_EXE_quasar"))
            .args(["init", "demo", flag])
            .env("HOME", home)
            .output()?;
        assert!(!output.status.success(), "{flag} unexpectedly succeeded");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("unexpected argument"), "{flag}: {stderr}");
    }
    Ok(())
}

#[test]
fn project_name_is_required() -> Result<(), Box<dyn Error>> {
    let temp = tempdir()?;
    let home = temp.path().join("home");
    fs::create_dir(&home)?;
    let output = Command::new(env!("CARGO_BIN_EXE_quasar"))
        .arg("init")
        .env("HOME", home)
        .output()?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Usage: quasar init <NAME>"));
    Ok(())
}

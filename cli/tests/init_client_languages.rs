use {
    std::{error::Error, fs, path::PathBuf, process::Command},
    tempfile::tempdir,
};

#[test]
fn generated_config_uses_the_canonical_client_targets() -> Result<(), Box<dyn Error>> {
    let temp = tempdir()?;
    let home = temp.path().join("home");
    fs::create_dir(&home)?;
    let output = Command::new(env!("CARGO_BIN_EXE_quasar"))
        .args(["init", "canonical", "--no-git"])
        .env("HOME", home)
        .env(
            "QUASAR_DEV_ROOT",
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap(),
        )
        .current_dir(temp.path())
        .output()?;
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let config = fs::read_to_string(temp.path().join("canonical/Quasar.toml"))?;
    assert_eq!(
        config,
        "[project]\nname = \"canonical\"\n\n[testing]\ncommand = { program = \"cargo\", args = \
         [\"test\"] }\n\n[clients]\npath = \"target/client\"\ntargets = [\"rust\", \
         \"kit\", \"web3\"]\n"
    );
    Ok(())
}

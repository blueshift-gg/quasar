use {
    std::{error::Error, fs, path::PathBuf, process::Command},
    tempfile::tempdir,
};

fn quasar(home: &std::path::Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_quasar"));
    command.env("HOME", home);
    command
}

#[test]
fn help_separates_core_commands_from_preview_tools() -> Result<(), Box<dyn Error>> {
    let temp = tempdir()?;
    let output = quasar(temp.path()).arg("--help").output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("Core commands:"));
    assert!(stdout.contains("Preview tools:"));
    assert!(stdout.contains("inspect validation"));
    assert!(stdout.contains("inspect asm"));
    assert!(stdout.contains("client  <idl> [--target target]"));
    assert!(!stdout.contains("--lang"));
    assert!(!stdout.contains("\n    audit "));
    assert!(!stdout.contains("\n    dump "));
    assert!(!stdout.contains("\n    add "));
    Ok(())
}

#[test]
fn removed_commands_have_no_aliases() -> Result<(), Box<dyn Error>> {
    let temp = tempdir()?;
    for command in ["add", "audit", "dump"] {
        let output = quasar(temp.path()).arg(command).output()?;
        assert!(!output.status.success(), "{command} unexpectedly succeeded");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("unrecognized subcommand"),
            "{command}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

#[test]
fn validation_json_is_deterministic_and_preview_labeled() -> Result<(), Box<dyn Error>> {
    let temp = tempdir()?;
    let idl = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../idl/tests/fixtures/programs/multisig.idl.json");
    let run = || {
        quasar(temp.path())
            .args(["inspect", "validation"])
            .arg(&idl)
            .arg("--json")
            .output()
    };
    let first = run()?;
    let second = run()?;
    assert!(first.status.success());
    assert_eq!(first.stdout, second.stdout);
    assert!(serde_json::from_slice::<serde_json::Value>(&first.stdout).is_ok());
    assert!(String::from_utf8_lossy(&first.stderr).contains("Preview tool"));
    Ok(())
}

#[test]
fn assembly_inspection_reports_a_missing_toolchain_cleanly() -> Result<(), Box<dyn Error>> {
    let temp = tempdir()?;
    let elf = temp.path().join("program.so");
    fs::write(&elf, b"not-an-elf")?;
    let output = quasar(temp.path())
        .args(["inspect", "asm"])
        .arg(elf)
        .output()?;
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Preview tool"), "{stderr}");
    assert!(stderr.contains("llvm-objdump not found"), "{stderr}");
    Ok(())
}

fn write_minimal_idl(path: &std::path::Path) -> Result<(), Box<dyn Error>> {
    fs::write(
        path,
        r#"{
  "spec": "quasar-idl/1.0.0",
  "name": "vault",
  "version": "0.1.0",
  "address": "11111111111111111111111111111111",
  "instructions": [],
  "accounts": [],
  "types": [],
  "events": [],
  "errors": []
}"#,
    )?;
    Ok(())
}

#[test]
fn client_targets_are_independent_and_default_to_stable_only() -> Result<(), Box<dyn Error>> {
    let temp = tempdir()?;
    let idl = temp.path().join("vault.json");
    write_minimal_idl(&idl)?;

    let kit = quasar(temp.path())
        .current_dir(temp.path())
        .args(["client"])
        .arg(&idl)
        .args(["--target", "kit"])
        .output()?;
    assert!(
        kit.status.success(),
        "{}",
        String::from_utf8_lossy(&kit.stderr)
    );
    assert!(temp
        .path()
        .join("target/client/kit/vault/client.ts")
        .is_file());
    assert!(!temp.path().join("target/client/web3").exists());
    assert!(!temp.path().join("target/client/python").exists());

    fs::remove_dir_all(temp.path().join("target"))?;
    let defaults = quasar(temp.path())
        .current_dir(temp.path())
        .arg("client")
        .arg(&idl)
        .output()?;
    assert!(
        defaults.status.success(),
        "{}",
        String::from_utf8_lossy(&defaults.stderr)
    );
    assert!(temp
        .path()
        .join("target/client/kit/vault/client.ts")
        .is_file());
    assert!(temp
        .path()
        .join("target/client/web3/vault/client.ts")
        .is_file());
    for preview in ["python", "go", "c"] {
        assert!(!temp.path().join("target/client").join(preview).exists());
    }
    Ok(())
}

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
    let idl = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/multisig.idl.json");
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

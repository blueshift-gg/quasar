use {
    quasar_cli::idl,
    std::{
        error::Error,
        fs,
        path::{Path, PathBuf},
        process::Command,
    },
    tempfile::tempdir,
};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn write_file(path: &Path, contents: impl AsRef<str>) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents.as_ref())?;
    Ok(())
}

fn generate_lockfile(manifest: &Path) -> Result<(), Box<dyn Error>> {
    let output = Command::new("cargo")
        .arg("generate-lockfile")
        .arg("--manifest-path")
        .arg(manifest)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "failed to generate fixture lockfile:\n{}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(())
}

#[test]
fn missing_idl_build_feature_reports_actionable_message() -> Result<(), Box<dyn Error>> {
    let temp = tempdir()?;
    let program_dir = temp.path().join("programs/missing-idl-build");

    write_file(
        &temp.path().join("Cargo.toml"),
        r#"[workspace]
members = ["programs/missing-idl-build"]
resolver = "3"
"#,
    )?;
    write_file(
        &program_dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "missing-idl-build"
version = "0.1.0"
edition = "2021"

[dependencies]
quasar-lang = {{ path = "{}" }}
"#,
            workspace_root().join("lang").display()
        ),
    )?;
    write_file(
        &program_dir.join("src/lib.rs"),
        r#"#![cfg_attr(not(test), no_std)]

use quasar_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
mod missing_idl_build {
    use super::*;

    pub fn noop(_ctx: Ctx<Noop>) -> Result<(), ProgramError> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Noop {}
"#,
    )?;

    let err = idl::generate(&program_dir, &[], &temp.path().join("clients"))
        .expect_err("IDL generation should fail without the idl-build feature");
    let message = err.to_string();

    assert!(
        !message.contains("Anyhow error"),
        "missing idl-build feature should not be hidden behind generic Anyhow output: {message}"
    );
    assert!(
        message.contains("idl-build = [\"quasar-lang/idl-build\"]"),
        "missing idl-build feature should include the Cargo.toml fix: {message}"
    );

    Ok(())
}

#[test]
fn idl_build_requires_an_up_to_date_lockfile() -> Result<(), Box<dyn Error>> {
    let temp = tempdir()?;
    let program_dir = temp.path().join("program");
    write_file(
        &program_dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "unlocked-idl-program"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["lib"]

[features]
idl-build = ["quasar-lang/idl-build"]

[dependencies]
quasar-lang = {{ path = "{}" }}
"#,
            workspace_root().join("lang").display()
        ),
    )?;
    write_file(
        &program_dir.join("src/lib.rs"),
        r#"#![no_std]
use quasar_lang::prelude::*;
declare_id!("11111111111111111111111111111111");

#[program]
mod unlocked_idl_program {
    use super::*;
    pub fn noop(_ctx: Ctx<Noop>) -> Result<(), ProgramError> { Ok(()) }
}

#[derive(Accounts)]
pub struct Noop {}
"#,
    )?;

    let error = idl::build(&program_dir).expect_err("an unlocked IDL build must fail");
    let message = error.to_string();
    assert!(message.contains("up-to-date Cargo.lock"), "{message}");
    assert!(message.contains("cargo generate-lockfile"), "{message}");
    Ok(())
}

#[test]
fn idl_command_accepts_dot_path_from_crate_directory() -> Result<(), Box<dyn Error>> {
    let temp = tempdir()?;
    let program_dir = temp.path().join("dot-path-program");

    write_file(
        &program_dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "dot-path-program"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "lib"]

[features]
idl-build = ["quasar-lang/idl-build"]

[dependencies]
quasar-lang = {{ path = "{}" }}
"#,
            workspace_root().join("lang").display()
        ),
    )?;
    write_file(
        &program_dir.join("src/lib.rs"),
        r#"#![no_std]

use quasar_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
mod dot_path_program {
    use super::*;

    pub fn noop(_ctx: Ctx<Noop>) -> Result<(), ProgramError> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Noop {}
"#,
    )?;
    generate_lockfile(&program_dir.join("Cargo.toml"))?;

    let output = Command::new(env!("CARGO_BIN_EXE_quasar"))
        .arg("idl")
        .arg(".")
        .current_dir(&program_dir)
        .output()?;

    assert!(
        output.status.success(),
        "quasar idl . should work from the crate directory\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        program_dir
            .join("target/idl/dot_path_program.json")
            .exists(),
        "IDL JSON should be written under the crate-local target directory"
    );

    Ok(())
}

#[test]
fn idl_build_does_not_compile_program_unit_tests() -> Result<(), Box<dyn Error>> {
    let temp = tempdir()?;
    let program_dir = temp.path().join("programs/idl-with-broken-unit-test");

    write_file(
        &temp.path().join("Cargo.toml"),
        r#"[workspace]
members = ["programs/idl-with-broken-unit-test"]
resolver = "3"
"#,
    )?;
    write_file(
        &program_dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "idl-with-broken-unit-test"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "lib"]

[features]
idl-build = ["quasar-lang/idl-build"]

[dependencies]
quasar-lang = {{ path = "{}" }}
"#,
            workspace_root().join("lang").display()
        ),
    )?;
    write_file(
        &program_dir.join("src/lib.rs"),
        r#"#![no_std]

use quasar_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
mod idl_with_broken_unit_test {
    use super::*;

    pub fn noop(_ctx: Ctx<Noop>) -> Result<(), ProgramError> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Noop {}

#[cfg(test)]
compile_error!("IDL generation compiled an unrelated unit test");
"#,
    )?;
    generate_lockfile(&temp.path().join("Cargo.toml"))?;

    let generated = idl::build(&program_dir)?;
    assert_eq!(generated.name, "idl_with_broken_unit_test");

    Ok(())
}

#[test]
fn idl_command_rejects_nonexistent_path() -> Result<(), Box<dyn Error>> {
    let temp = tempdir()?;
    let output = Command::new(env!("CARGO_BIN_EXE_quasar"))
        .arg("idl")
        .arg("does-not-exist")
        .current_dir(temp.path())
        .output()?;

    assert!(
        !output.status.success(),
        "quasar idl on a nonexistent path must fail\nstdout:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("does-not-exist"),
        "error must name the missing path so the user can act on it:\n{stderr}"
    );
    Ok(())
}

#[test]
fn idl_verify_rejects_invalid_json() -> Result<(), Box<dyn Error>> {
    let temp = tempdir()?;
    let idl_path = temp.path().join("broken.json");
    write_file(&idl_path, "{ this is not json")?;

    let output = Command::new(env!("CARGO_BIN_EXE_quasar"))
        .arg("idl")
        .arg("verify")
        .arg(&idl_path)
        .output()?;

    assert!(
        !output.status.success(),
        "quasar idl verify on invalid JSON must fail\nstdout:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    Ok(())
}

#[test]
fn idl_verify_rejects_idl_without_hashes() -> Result<(), Box<dyn Error>> {
    let temp = tempdir()?;
    let idl_path = temp.path().join("unhashed.json");
    write_file(
        &idl_path,
        serde_json::json!({
            "spec": "quasar-idl/1.0.0",
            "name": "unhashed",
            "version": "0.1.0",
            "address": "11111111111111111111111111111111",
            "metadata": {},
            "instructions": [],
            "accounts": [],
            "types": [],
            "events": [],
            "errors": []
        })
        .to_string(),
    )?;

    let output = Command::new(env!("CARGO_BIN_EXE_quasar"))
        .arg("idl")
        .arg("verify")
        .arg(&idl_path)
        .output()?;

    assert!(
        !output.status.success(),
        "quasar idl verify must fail when the IDL carries no hashes block\nstdout:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    Ok(())
}

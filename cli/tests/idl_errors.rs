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
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().expect("workspace root").to_path_buf()
}

fn write_file(path: &Path, contents: impl AsRef<str>) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents.as_ref())?;
    Ok(())
}

#[test]
fn idl_emission_requires_no_opt_in_feature() -> Result<(), Box<dyn Error>> {
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

    #[instruction(0)]
    pub fn noop(_ctx: Ctx<Noop>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Noop { pub payer: Signer }
"#,
    )?;

    let idl = idl::generate(&program_dir, &[], &temp.path().join("clients"))?;
    assert_eq!(idl.instructions.len(), 1);
    assert_eq!(idl.instructions[0].discriminator, [0]);

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

    #[instruction(0)]
    pub fn noop(_ctx: Ctx<Noop>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Noop { pub payer: Signer }
"#,
    )?;

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
        program_dir.join("target/idl/dot_path_program.json").exists(),
        "IDL JSON should be written under the crate-local target directory"
    );

    Ok(())
}

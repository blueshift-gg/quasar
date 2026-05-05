use {
    quasar_cli::idl,
    std::{
        error::Error,
        fs,
        path::{Path, PathBuf},
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

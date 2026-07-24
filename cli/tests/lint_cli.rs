use {
    quasar_idl::lint::LOCK_FILE_NAME,
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

fn write_project(program_dir: &Path, amount_ty: &str) -> Result<(), Box<dyn Error>> {
    write_file(
        &program_dir.join("Quasar.toml"),
        r#"[project]
name = "lint-demo"

[testing]
command = { program = "cargo", args = ["test", "tests::"] }

[clients]
path = "target/client"
targets = ["rust", "kit", "web3"]
"#,
    )?;
    write_file(
        &program_dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "lint-demo"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "lib"]

[features]
idl-build = ["quasar-lang/idl-build"]

[dependencies]
quasar-lang = {{ path = "{lang}" }}

# TEMPORARY: mirrors the workspace zeropod patch until zeropod >=0.3.4
# (solana-address <3, wincode 0.5) is published.
[patch.crates-io]
zeropod = {{ path = "{zeropod}/zeropod" }}
zeropod-derive = {{ path = "{zeropod}/zeropod-derive" }}
"#,
            lang = workspace_root().join("lang").display(),
            zeropod = workspace_root().join("vendor/zeropod").display()
        ),
    )?;
    write_file(
        &program_dir.join("src/lib.rs"),
        format!(
            r#"#![no_std]

use quasar_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
mod lint_demo {{
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn make(_ctx: Ctx<Make>, amount: {amount_ty}) -> Result<(), ProgramError> {{
        let _ = amount;
        Ok(())
    }}
}}

#[account(discriminator = 1)]
pub struct Vault {{
    pub version: u8,
    pub amount: {amount_ty},
    pub _reserved: [u8; 64],
}}

#[derive(Accounts)]
pub struct Make {{
    pub authority: Signer,
    #[account(mut)]
    pub vault: Account<Vault>,
}}
"#
        ),
    )?;
    Ok(())
}

#[test]
fn lint_update_lock_then_default_diff_catches_breaking_change() -> Result<(), Box<dyn Error>> {
    let temp = tempdir()?;
    let program_dir = temp.path().join("lint-demo");
    write_project(&program_dir, "u64")?;
    let lock = Command::new("cargo")
        .arg("generate-lockfile")
        .arg("--offline")
        .current_dir(&program_dir)
        .output()?;
    assert!(
        lock.status.success(),
        "failed to generate fixture lockfile: {}",
        String::from_utf8_lossy(&lock.stderr)
    );

    let update = Command::new(env!("CARGO_BIN_EXE_quasar"))
        .arg("lint")
        .arg("--update-lock")
        .current_dir(&program_dir)
        .output()?;
    assert!(
        update.status.success(),
        "quasar lint --update-lock should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&update.stdout),
        String::from_utf8_lossy(&update.stderr)
    );
    assert!(program_dir.join(LOCK_FILE_NAME).exists());

    write_project(&program_dir, "u32")?;
    let lint = Command::new(env!("CARGO_BIN_EXE_quasar"))
        .arg("lint")
        .current_dir(&program_dir)
        .output()?;

    assert!(
        !lint.status.success(),
        "quasar lint should fail once a lock exists and the surface \
         breaks\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&lint.stdout),
        String::from_utf8_lossy(&lint.stderr)
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&lint.stdout),
        String::from_utf8_lossy(&lint.stderr)
    );
    assert!(combined.contains("R002"), "{combined}");
    assert!(combined.contains("R008"), "{combined}");

    Ok(())
}

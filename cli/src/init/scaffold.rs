use {
    super::templates::GITIGNORE,
    crate::{
        config::QuasarConfig,
        error::{CliError, CliResult},
        program_keypair::{self, ProgramKeypair},
    },
    std::{
        fs,
        path::{Path, PathBuf},
        process::Command,
    },
};

pub(super) fn validate_target_dir(dir: &str) -> Result<(), CliError> {
    let root = Path::new(dir);
    if dir == "." {
        if root.join("Quasar.toml").exists() {
            return Err(CliError::message(
                "current directory is already a Quasar project",
            ));
        }
        if root.join("Cargo.toml").exists() {
            return Err(CliError::message(
                "current directory already contains a Rust project",
            ));
        }
        if fs::read_dir(root).is_ok_and(|mut entries| entries.next().is_some()) {
            return Err(CliError::message("current directory is not empty"));
        }
    } else if root.exists() {
        if !root.is_dir() {
            return Err(CliError::message(format!(
                "path '{dir}' exists and is not a directory"
            )));
        }
        if root.join("Quasar.toml").exists() {
            return Err(CliError::message(format!(
                "directory '{dir}' is already a Quasar project"
            )));
        }
        if fs::read_dir(root).is_ok_and(|mut entries| entries.next().is_some()) {
            return Err(CliError::message(format!(
                "directory '{dir}' already exists and is not empty"
            )));
        }
    }
    Ok(())
}

pub(super) fn scaffold(dir: &str, name: &str) -> CliResult {
    let current_dir = std::env::current_dir()?;
    let destination = if dir == "." {
        current_dir
    } else {
        current_dir.join(dir)
    };
    scaffold_at(&destination, name, Path::new("cargo"))
}

fn scaffold_at(destination: &Path, name: &str, cargo: &Path) -> CliResult {
    let destination_exists = destination.exists();
    let staging_parent = if destination_exists {
        destination
    } else {
        destination
            .parent()
            .ok_or_else(|| CliError::message("project destination has no parent directory"))?
    };
    let staging = tempfile::Builder::new()
        .prefix(".quasar-init-")
        .tempdir_in(staging_parent)
        .map_err(|error| {
            CliError::io_path("create staging directory under", staging_parent, error)
        })?;

    write_scaffold(staging.path(), name, cargo)?;
    install_scaffold(staging.path(), destination, destination_exists)
}

fn install_scaffold(staging: &Path, destination: &Path, destination_exists: bool) -> CliResult {
    if !destination_exists {
        return fs::rename(staging, destination)
            .map_err(|error| CliError::io_path("install project at", destination, error));
    }

    let mut entries = fs::read_dir(staging)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    let mut installed = Vec::<(PathBuf, PathBuf)>::new();
    for entry in entries {
        let source = entry.path();
        let target = destination.join(entry.file_name());
        if let Err(error) = fs::rename(&source, &target) {
            for (source, target) in installed.into_iter().rev() {
                let _ = fs::rename(target, source);
            }
            return Err(CliError::io_path("install project entry at", target, error));
        }
        installed.push((source, target));
    }
    Ok(())
}

fn write_scaffold(root: &Path, name: &str, cargo: &Path) -> CliResult {
    let src = root.join("src");
    fs::create_dir_all(&src)?;

    let config = QuasarConfig::canonical(name);
    fs::write(root.join("Quasar.toml"), config.to_toml())?;
    let manifest_path = root.join("Cargo.toml");
    let manifest = generate_cargo_toml(name, None);
    fs::write(&manifest_path, &manifest)?;
    fs::write(root.join(".gitignore"), GITIGNORE)?;

    let deploy_dir = root.join("target/deploy");
    fs::create_dir_all(&deploy_dir)?;
    let keypair = ProgramKeypair::generate();
    let program_id = keypair.program_id();
    program_keypair::write(
        &deploy_dir.join(format!("{name}-keypair.json")),
        &keypair,
        false,
        None,
    )?;

    let module_name = name.replace('-', "_");
    fs::write(
        src.join("lib.rs"),
        generate_lib_rs(&module_name, &program_id),
    )?;
    fs::write(src.join("tests.rs"), generate_tests_rs())?;

    let development_root = std::env::var_os("QUASAR_DEV_ROOT").map(std::path::PathBuf::from);
    if let Some(development_root) = &development_root {
        fs::write(
            &manifest_path,
            generate_cargo_toml(name, Some(development_root)),
        )?;
    }
    let output = Command::new(cargo)
        .arg("generate-lockfile")
        .arg("--quiet")
        .current_dir(root)
        .output();
    if development_root.is_some() {
        fs::write(&manifest_path, manifest)?;
    }
    let output = output.map_err(|error| {
        CliError::message(format!("failed to generate starter Cargo.lock: {error}"))
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CliError::message(format!(
            "failed to generate starter Cargo.lock\n{}",
            stderr.trim()
        )));
    }
    Ok(())
}

fn generate_cargo_toml(name: &str, development_root: Option<&Path>) -> String {
    let version = env!("CARGO_PKG_VERSION");
    let quasar_lang = if let Some(root) = development_root {
        format!(r#"{{ path = "{}" }}"#, root.join("lang").display())
    } else if version == "0.0.0" {
        r#"{ git = "https://github.com/blueshift-gg/quasar", branch = "master" }"#.to_string()
    } else {
        format!(r#""={version}""#)
    };
    let quasar_test = if let Some(root) = development_root {
        format!(r#"{{ path = "{}" }}"#, root.join("testing").display())
    } else if version == "0.0.0" {
        r#"{ git = "https://github.com/blueshift-gg/quasar", branch = "master" }"#.to_string()
    } else {
        format!(r#""={version}""#)
    };
    format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[lints.rust.unexpected_cfgs]
level = "warn"
check-cfg = [
    'cfg(target_os, values("solana"))',
]

[lib]
crate-type = ["cdylib", "lib"]

[features]
alloc = []
client = []
debug = []
idl-build = ["quasar-lang/idl-build"]

[dependencies]
quasar-lang = {quasar_lang}
solana-instruction = {{ version = "3.2.0" }}

[dev-dependencies]
quasar-test = {quasar_test}
"#
    )
}

fn generate_lib_rs(module_name: &str, program_id: &str) -> String {
    format!(
        r#"#![cfg_attr(not(test), no_std)]

use quasar_lang::prelude::*;

declare_id!("{program_id}");

#[derive(Accounts)]
pub struct Initialize {{
    pub payer: Signer,
}}

impl Initialize {{
    #[inline(always)]
    pub fn initialize(&self) -> Result<(), ProgramError> {{
        Ok(())
    }}
}}

#[program]
mod {module_name} {{
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn initialize(ctx: Ctx<Initialize>) -> Result<(), ProgramError> {{
        ctx.accounts.initialize()
    }}
}}

#[cfg(test)]
mod tests;
"#
    )
}

fn generate_tests_rs() -> &'static str {
    r#"use {crate::cpi::InitializeInstruction, quasar_test::prelude::*};

#[quasar_test]
fn initialize(q: &mut QuasarTest) {
    let payer = q.add_wallet();
    q.send(InitializeInstruction { payer }).succeeds();
}
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_config_is_the_canonical_schema() {
        let config = QuasarConfig::canonical("demo").to_toml();
        assert_eq!(
            config,
            "[project]\nname = \"demo\"\n\n[testing]\ncommand = { program = \"cargo\", args = \
             [\"test\", \"tests::\"] }\n\n[clients]\npath = \"target/client\"\ntargets = \
             [\"rust\", \"kit\", \"web3\"]\n"
        );
    }

    #[test]
    fn starter_has_only_the_minimal_program_and_rust_test() {
        let manifest = generate_cargo_toml("demo", None);
        let source = generate_lib_rs("demo", "11111111111111111111111111111111");
        let tests = generate_tests_rs();
        assert!(manifest.contains("quasar-test ="));
        assert!(!manifest.contains("mollusk"));
        assert!(!manifest.contains("typescript"));
        assert!(!source.contains("mod state"));
        assert!(tests.contains("#[quasar_test]"));
    }

    #[test]
    fn lockfile_failure_does_not_install_a_partial_project() {
        let sandbox = tempfile::tempdir().unwrap();
        let destination = sandbox.path().join("demo");
        let missing_cargo = sandbox.path().join("missing-cargo");

        let error = scaffold_at(&destination, "demo", &missing_cargo).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("failed to generate starter Cargo.lock"),
            "{error}"
        );
        assert!(!destination.exists());
    }

    #[test]
    fn lockfile_failure_preserves_an_existing_empty_destination() {
        let sandbox = tempfile::tempdir().unwrap();
        let destination = sandbox.path().join("demo");
        fs::create_dir(&destination).unwrap();
        let missing_cargo = sandbox.path().join("missing-cargo");

        scaffold_at(&destination, "demo", &missing_cargo).unwrap_err();

        assert!(destination.is_dir());
        assert!(fs::read_dir(&destination).unwrap().next().is_none());
    }
}

use {
    super::{
        schema::{
            QuasarClients, QuasarProject, QuasarRustTesting, QuasarTesting, QuasarToml,
            QuasarToolchain, QuasarTypeScriptTesting,
        },
        templates::*,
        types::{PackageManager, RustFramework, Template, TestLanguage, Toolchain, TypeScriptSdk},
    },
    crate::error::{CliError, CliResult},
    quasar_idl::codegen::typescript::{TsTarget, client_dependency_version},
    quasar_schema::snake_to_pascal,
    std::{fs, path::Path},
};

/// Check that the target directory is usable before prompting the user for
/// scaffolding parameters.
pub(super) fn validate_target_dir(dir: &str) -> Result<(), CliError> {
    let root = Path::new(dir);

    if dir == "." {
        if root.join("Quasar.toml").exists() {
            return Err(CliError::message("current directory is already a Quasar project"));
        }
        if root.join("Cargo.toml").exists() {
            return Err(CliError::message("current directory already contains a Rust project"));
        }
        if fs::read_dir(root).is_ok_and(|mut d| d.next().is_some()) {
            return Err(CliError::message("current directory is not empty"));
        }
    } else if root.exists() {
        if !root.is_dir() {
            return Err(CliError::message(format!("path '{dir}' exists and is not a directory")));
        }
        if root.join("Quasar.toml").exists() {
            return Err(CliError::message(format!(
                "directory '{dir}' is already a Quasar project"
            )));
        }
        if fs::read_dir(root).is_ok_and(|mut d| d.next().is_some()) {
            return Err(CliError::message(format!(
                "directory '{dir}' already exists and is not empty"
            )));
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn scaffold(
    dir: &str,
    name: &str,
    toolchain: Toolchain,
    test_language: TestLanguage,
    rust_framework: Option<RustFramework>,
    ts_sdk: Option<TypeScriptSdk>,
    template: Template,
    package_manager: Option<&PackageManager>,
    client_languages: &[String],
) -> CliResult {
    let root = Path::new(dir);
    let clients_path = "target/client";

    let src = root.join("src");
    fs::create_dir_all(&src)?;

    // Quasar.toml
    let config = QuasarToml {
        project: QuasarProject { name: name.to_string() },
        toolchain: QuasarToolchain { toolchain_type: toolchain.to_string() },
        testing: QuasarTesting {
            language: test_language.to_string(),
            rust: match (test_language, rust_framework) {
                (TestLanguage::Rust, Some(fw)) => Some(QuasarRustTesting {
                    framework: fw.to_string(),
                    test: crate::config::CommandSpec::new("cargo", ["test", "tests::"]),
                }),
                _ => None,
            },
            typescript: match (test_language, ts_sdk) {
                (TestLanguage::TypeScript, Some(sdk)) => {
                    let pm = package_manager.expect("package_manager required for TS");
                    Some(QuasarTypeScriptTesting {
                        framework: "litesvm".to_string(),
                        sdk: sdk.to_string(),
                        install: crate::config::CommandSpec::parse(pm.install_cmd())?,
                        test: crate::config::CommandSpec::parse(pm.test_cmd())?,
                    })
                }
                _ => None,
            },
        },
        clients: QuasarClients {
            path: clients_path.to_string(),
            languages: client_languages.to_vec(),
        },
    };
    let toml_str = toml::to_string_pretty(&config)?;
    fs::write(root.join("Quasar.toml"), toml_str)?;

    // Cargo.toml
    fs::write(root.join("Cargo.toml"), generate_cargo_toml(name, toolchain, test_language))?;

    // .cargo/config.toml (upstream only)
    if matches!(toolchain, Toolchain::Upstream) {
        let cargo_dir = root.join(".cargo");
        fs::create_dir_all(&cargo_dir)?;
        fs::write(cargo_dir.join("config.toml"), CARGO_CONFIG)?;
    }

    // .gitignore
    fs::write(root.join(".gitignore"), GITIGNORE)?;

    // Generate program keypair
    let deploy_dir = root.join("target").join("deploy");
    fs::create_dir_all(&deploy_dir)?;

    let signing_key = ed25519_dalek::SigningKey::generate(&mut rand::thread_rng());
    let program_id = bs58::encode(signing_key.verifying_key().as_bytes()).into_string();

    // Write keypair as Solana CLI-compatible JSON (64-byte array: secret + public)
    let mut keypair_bytes = Vec::with_capacity(64);
    keypair_bytes.extend_from_slice(signing_key.as_bytes());
    keypair_bytes.extend_from_slice(signing_key.verifying_key().as_bytes());
    let keypair_json = serde_json::to_string(&keypair_bytes)
        .map_err(|e| CliError::json_serialize("program keypair JSON", e))?;
    fs::write(deploy_dir.join(format!("{name}-keypair.json")), &keypair_json)?;

    // src/lib.rs
    let module_name = name.replace('-', "_");
    let has_rust_tests = matches!(test_language, TestLanguage::Rust);
    fs::write(
        src.join("lib.rs"),
        generate_lib_rs(&module_name, &program_id, template, has_rust_tests),
    )?;

    // Template-specific files
    match template {
        Template::Minimal => {
            // Everything lives in lib.rs; no instructions/ directory is needed.
        }
        Template::Full => {
            let instructions_dir = src.join("instructions");
            fs::create_dir_all(&instructions_dir)?;
            fs::write(instructions_dir.join("mod.rs"), INSTRUCTIONS_MOD)?;
            fs::write(instructions_dir.join("initialize.rs"), INSTRUCTION_INITIALIZE)?;
            fs::write(src.join("state.rs"), STATE_RS)?;
            fs::write(src.join("errors.rs"), ERRORS_RS)?;
        }
    }

    // Rust test scaffold
    if rust_framework.is_some() {
        fs::write(src.join("tests.rs"), generate_tests_rs(&module_name, toolchain))?;
    }

    // TypeScript test scaffold
    if let Some(sdk) = ts_sdk {
        let tests_dir = root.join("tests");
        fs::create_dir_all(&tests_dir)?;

        fs::write(root.join("package.json"), generate_package_json(name, sdk))?;
        fs::write(root.join("tsconfig.json"), TS_TEST_TSCONFIG)?;

        fs::write(
            tests_dir.join(format!("{name}.test.ts")),
            generate_test_ts(name, sdk, toolchain),
        )?;
    }

    // Generate Cargo.lock with the system cargo.  The Solana toolchain
    // bundles an older cargo that may fail to resolve crates using newer
    // Rust editions.  Creating the lockfile now means `cargo build-sbf`
    // will never have to perform dependency resolution itself.
    let lock_ok = std::process::Command::new("cargo")
        .arg("generate-lockfile")
        .current_dir(root)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success());

    if !lock_ok {
        eprintln!(
            "  {}",
            crate::style::dim(
                "note: could not generate Cargo.lock; run `cargo generate-lockfile` before \
                 building"
            )
        );
    }

    Ok(())
}

fn generate_cargo_toml(name: &str, toolchain: Toolchain, test_language: TestLanguage) -> String {
    let quasar_lang_dep = quasar_lang_dependency();
    let mut out = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2024"

[lints.rust.unexpected_cfgs]
level = "warn"
check-cfg = [
    'cfg(target_os, values("solana"))',
]

[lib]
crate-type = ["cdylib", "lib"]

[features]
no-entrypoint = []
debug = ["quasar-lang/debug"]

[dependencies]
quasar-lang = {quasar_lang_dep}
"#,
    );

    if matches!(toolchain, Toolchain::Solana) {
        out.push_str("solana-instruction = { version = \"=3.5.0\" }\n");
    }

    match test_language {
        TestLanguage::None => {}
        TestLanguage::Rust => {
            out.push_str(
                r#"
[dev-dependencies]
mollusk-svm = "0.16.2"
solana-account = { version = "4.5.0" }
solana-address = { version = "=2.7.0", features = ["decode"] }
solana-instruction = { version = "=3.5.0", features = ["bincode"] }
"#,
            );
        }
        TestLanguage::TypeScript => {}
    }

    out
}

fn generate_lib_rs(
    module_name: &str,
    program_id: &str,
    template: Template,
    has_tests: bool,
) -> String {
    let test_mod = if has_tests { "\n#[cfg(test)]\nmod tests;\n" } else { "" };

    match template {
        Template::Minimal => {
            format!(
                r#"#![cfg_attr(not(test), no_std)]

use quasar_lang::prelude::*;

declare_id!("{program_id}");

#[derive(Accounts)]
pub struct Initialize {{
    pub payer: Signer,
    pub system_program: Program<SystemProgram>,
}}

impl Initialize {{
    #[inline(always)]
    pub fn initialize(&self) -> Result<()> {{
        Ok(())
    }}
}}

#[program]
mod {module_name} {{
    use super::*;

    #[instruction(0)]
    pub fn initialize(ctx: Ctx<Initialize>) -> Result<()> {{
        ctx.accounts.initialize()
    }}
}}
{test_mod}"#
            )
        }
        Template::Full => {
            format!(
                r#"#![cfg_attr(not(test), no_std)]

use quasar_lang::prelude::*;

mod errors;
mod instructions;
mod state;
use instructions::*;

declare_id!("{program_id}");

#[program]
mod {module_name} {{
    use super::*;

    #[instruction(0)]
    pub fn initialize(ctx: Ctx<Initialize>) -> Result<()> {{
        ctx.accounts.initialize()
    }}
}}
{test_mod}"#
            )
        }
    }
}

fn quasar_lang_dependency() -> String {
    let version = env!("CARGO_PKG_VERSION");

    if version == "0.0.0" {
        r#"{ git = "https://github.com/blueshift-gg/quasar", branch = "master" }"#.to_string()
    } else {
        format!(r#""={version}""#)
    }
}

fn generate_package_json(name: &str, ts_sdk: TypeScriptSdk) -> String {
    let solana_dep = if matches!(ts_sdk, TypeScriptSdk::Kit) {
        format!("\"@solana/kit\": \"{}\"", client_dependency_version(TsTarget::Kit))
    } else {
        format!(
            "\"@solana/kit\": \"{}\",\n    \"@solana/web3.js\": \"{}\"",
            client_dependency_version(TsTarget::Kit),
            client_dependency_version(TsTarget::Web3js)
        )
    };
    format!(
        r#"{{
  "name": "{name}",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {{
    "check-types": "tsc --noEmit",
    "test": "vitest run"
  }},
  "dependencies": {{
    "litesvm": "1.3.0",
    "@solana/codecs": "^6.0.0",
    {solana_dep}
  }},
  "devDependencies": {{
    "@types/node": "^22.13.0",
    "typescript": "^5.9.3",
    "vitest": "^4.1.1"
  }}
}}
"#
    )
}

fn generate_test_ts(name: &str, ts_sdk: TypeScriptSdk, toolchain: Toolchain) -> String {
    let module_name = name.replace('-', "_");
    let so_name = match toolchain {
        Toolchain::Upstream => format!("lib{module_name}"),
        Toolchain::Solana => module_name.clone(),
    };
    let (imports, transaction) = match ts_sdk {
        TypeScriptSdk::Kit => (
            r#"import {
  AccountRole, appendTransactionMessageInstruction, createTransactionMessage,
  generateKeyPairSigner, pipe, setTransactionMessageFeePayerSigner,
  signTransactionMessageWithSigners,
} from "@solana/kit";"#,
            r#"    const payer = await generateKeyPairSigner();
    svm.airdrop(payer.address, lamports(2_000_000_000n));
    const instruction = {
      programAddress,
      accounts: [
        { address: payer.address, role: AccountRole.WRITABLE_SIGNER },
        { address: address("11111111111111111111111111111111"), role: AccountRole.READONLY },
      ],
      data: Uint8Array.from([0]),
    };
    const transaction = await pipe(
      createTransactionMessage({ version: 0 }),
      (tx) => setTransactionMessageFeePayerSigner(payer, tx),
      (tx) => svm.setTransactionMessageLifetimeUsingLatestBlockhash(tx),
      (tx) => appendTransactionMessageInstruction(instruction, tx),
      (tx) => signTransactionMessageWithSigners(tx),
    );"#,
        ),
        TypeScriptSdk::Web3js => (
            r#"import { Keypair, PublicKey, Transaction, TransactionInstruction } from "@solana/web3.js";
import { getTransactionDecoder } from "@solana/kit";"#,
            r#"    const payer = await Keypair.generate();
    svm.airdrop(address(payer.publicKey.toBase58()), lamports(2_000_000_000n));
    const instruction = new TransactionInstruction({
      programId: new PublicKey(programAddress),
      keys: [
        { pubkey: payer.publicKey, isSigner: true, isWritable: true },
        { pubkey: new PublicKey("11111111111111111111111111111111"), isSigner: false, isWritable: false },
      ],
      data: Buffer.from([0]),
    });
    const signed = new Transaction({
      feePayer: payer.publicKey,
      recentBlockhash: svm.latestBlockhash(),
    }).add(instruction);
    await signed.sign(payer);
    const transaction = getTransactionDecoder().decode(await signed.serialize());"#,
        ),
    };
    format!(
        r#"{imports}
import {{ address, lamports }} from "@solana/kit";
import {{ describe, it, expect }} from "vitest";
import {{ FailedTransactionMetadata, LiteSVM }} from "litesvm";
import {{ readFile }} from "node:fs/promises";

describe("{class_name} Program", () => {{
  it("initializes", async () => {{
    const idl = JSON.parse(await readFile("target/idl/{module_name}.json", "utf8")) as {{ address: string }};
    const programAddress = address(idl.address);
    const svm = new LiteSVM();
    svm.addProgram(programAddress, await readFile("target/deploy/{so_name}.so"));

{transaction}
    const result = svm.sendTransaction(transaction);
    expect(result, result.toString()).not.toBeInstanceOf(FailedTransactionMetadata);
  }});
}});
"#,
        class_name = snake_to_pascal(&module_name),
    )
}

fn generate_tests_rs(module_name: &str, toolchain: Toolchain) -> String {
    let libname = if matches!(toolchain, Toolchain::Upstream) {
        format!("lib{module_name}")
    } else {
        module_name.to_string()
    };
    format!(
        r#"use mollusk_svm::{{program::keyed_account_for_system_program, Mollusk}};
use solana_account::Account;
use solana_address::Address;
use solana_instruction::{{AccountMeta, Instruction}};

fn setup() -> Mollusk {{
    Mollusk::new(&crate::ID, "target/deploy/{libname}")
}}

fn initialize_instruction(payer: Address, system_program: Address) -> Instruction {{
    Instruction {{
        program_id: Address::from(crate::ID.to_bytes()),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(system_program, false),
        ],
        data: vec![0],
    }}
}}

#[test]
fn test_initialize() {{
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let instruction = initialize_instruction(payer, system_program);

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "initialize failed: {{:?}}",
        result.program_result,
    );
}}
"#
    )
}

#[cfg(test)]
mod tests {
    use {
        super::scaffold,
        crate::init::types::{PackageManager, Template, TestLanguage, Toolchain, TypeScriptSdk},
        quasar_idl::codegen::typescript::{TsTarget, client_dependency_version},
        serde_json::Value,
        std::fs,
        tempfile::tempdir,
    };

    #[test]
    fn scaffold_typescript_projects_use_selected_solana_dependency() {
        let temp = tempdir().expect("tempdir");
        let package_manager = PackageManager::Npm;

        for (dir_name, project_name, sdk, dep_name, target) in [
            ("kit", "demo-kit", TypeScriptSdk::Kit, "@solana/kit", TsTarget::Kit),
            ("web3", "demo-web3", TypeScriptSdk::Web3js, "@solana/web3.js", TsTarget::Web3js),
        ] {
            let project_dir = temp.path().join(dir_name);
            scaffold(
                project_dir.to_str().expect("utf8 path"),
                project_name,
                Toolchain::Upstream,
                TestLanguage::TypeScript,
                None,
                Some(sdk),
                Template::Minimal,
                Some(&package_manager),
                &[],
            )
            .expect("scaffold project");

            let package_json: Value = serde_json::from_str(
                &fs::read_to_string(project_dir.join("package.json")).expect("read package.json"),
            )
            .expect("valid package.json");

            let dependencies = &package_json["dependencies"];
            assert_eq!(dependencies[dep_name], client_dependency_version(target));
            let config: toml::Value =
                toml::from_str(&fs::read_to_string(project_dir.join("Quasar.toml")).unwrap())
                    .unwrap();
            assert!(config["toolchain"].get("arch").is_none());
            assert!(
                fs::read_to_string(project_dir.join(".cargo/config.toml"))
                    .unwrap()
                    .contains("link-arg=--arch=v3")
            );
        }
    }
    #[test]
    fn scaffolded_templates_build_add_and_test() {
        use crate::init::types::RustFramework;
        use std::process::Command;
        let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let output = Command::new("cargo")
            .args(["build", "-p", "quasar-cli", "--bin", "quasar"])
            .current_dir(workspace)
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        let cli = workspace.join("target/debug/quasar");
        let temp = tempdir().unwrap();
        let mut cases = vec![
            ("minimal-app", Template::Minimal, Toolchain::Solana, RustFramework::Mollusk),
            ("full-app", Template::Full, Toolchain::Solana, RustFramework::Mollusk),
        ];
        if let Ok(mode) = std::env::var("QUASAR_TEST_UPSTREAM") {
            if mode == "only" {
                cases.clear();
            }
            cases.push((
                "upstream-app",
                Template::Minimal,
                Toolchain::Upstream,
                RustFramework::Mollusk,
            ));
        }
        for (name, template, toolchain, framework) in cases {
            let project = temp.path().join(name);
            scaffold(
                project.to_str().unwrap(),
                name,
                toolchain,
                TestLanguage::Rust,
                Some(framework),
                None,
                template,
                None,
                &[],
            )
            .unwrap();
            let config: toml::Value =
                toml::from_str(&fs::read_to_string(project.join("Quasar.toml")).unwrap()).unwrap();
            assert_eq!(config["toolchain"].get("arch").and_then(toml::Value::as_str), None,);
            let manifest = project.join("Cargo.toml");
            let source = fs::read_to_string(&manifest).unwrap().replace(
                &format!("quasar-lang = \"={}\"", env!("CARGO_PKG_VERSION")),
                &format!("quasar-lang = {{ path = {:?} }}", workspace.join("lang")),
            );
            fs::write(manifest, source).unwrap();
            for args in [
                vec!["build"],
                vec!["add", "--instruction", "settle"],
                vec!["add", "--state", "extra_state"],
                vec!["add", "--error", "extra_error"],
                vec!["test"],
            ] {
                let output = Command::new(&cli).args(&args).current_dir(&project).output().unwrap();
                assert!(
                    output.status.success(),
                    "{name}: {args:?}\n{}\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            let elf_name = match toolchain {
                Toolchain::Solana => name.replace('-', "_"),
                Toolchain::Upstream => format!("lib{}", name.replace('-', "_")),
            };
            let elf = fs::read(project.join(format!("target/deploy/{elf_name}.so"))).unwrap();
            assert_eq!(&elf[48..52], &3u32.to_le_bytes(), "Rust templates must build SBPFv3");
            let interface: Value = serde_json::from_str(
                &fs::read_to_string(
                    project.join(format!("target/idl/{}.json", name.replace('-', "_"))),
                )
                .unwrap(),
            )
            .unwrap();
            assert_eq!(interface["instructions"][0]["discriminator"], serde_json::json!([0]));
            assert_eq!(interface["instructions"][1]["discriminator"], serde_json::json!([1]));
            let keypair = project.join(format!("target/deploy/{name}-keypair.json"));
            let identity = fs::read(&keypair).unwrap();
            let output =
                Command::new(&cli).args(["clean", "--all"]).current_dir(&project).output().unwrap();
            assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
            assert_eq!(fs::read(keypair).unwrap(), identity);
        }
    }
    #[test]
    fn scaffolded_typescript_tests_execute() {
        use std::process::Command;
        let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let build = Command::new("cargo")
            .args(["build", "-p", "quasar-cli", "--bin", "quasar"])
            .current_dir(workspace)
            .output()
            .unwrap();
        assert!(build.status.success());
        let cli = workspace.join("target/debug/quasar");
        let temp = tempdir().unwrap();
        for (name, sdk) in [("kit-app", TypeScriptSdk::Kit), ("web3-app", TypeScriptSdk::Web3js)] {
            let project = temp.path().join(name);
            scaffold(
                project.to_str().unwrap(),
                name,
                Toolchain::Solana,
                TestLanguage::TypeScript,
                None,
                Some(sdk),
                Template::Minimal,
                Some(&PackageManager::Npm),
                &["typescript".into()],
            )
            .unwrap();
            let manifest = project.join("Cargo.toml");
            let source = fs::read_to_string(&manifest).unwrap().replace(
                &format!("quasar-lang = \"={}\"", env!("CARGO_PKG_VERSION")),
                &format!("quasar-lang = {{ path = {:?} }}", workspace.join("lang")),
            );
            fs::write(manifest, source).unwrap();
            for args in [vec!["test"], vec!["build", "--debug"]] {
                let output = Command::new(&cli).args(&args).current_dir(&project).output().unwrap();
                assert!(
                    output.status.success(),
                    "{name}: {args:?}\n{}\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            let elf =
                fs::read(project.join(format!("target/deploy/{}.so", name.replace('-', "_"))))
                    .unwrap();
            assert_eq!(&elf[48..52], &3u32.to_le_bytes(), "TypeScript templates must build SBPFv3");
            let output = Command::new("npm")
                .args(["run", "check-types"])
                .current_dir(&project)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{name}: {}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}

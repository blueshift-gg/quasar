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
    std::{fs, path::Path},
};

/// Check that the target directory is usable before prompting the user for
/// scaffolding parameters.
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
        if fs::read_dir(root).is_ok_and(|mut d| d.next().is_some()) {
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
    fs::create_dir_all(&src).map_err(anyhow::Error::from)?;

    // Quasar.toml
    let config = QuasarToml {
        project: QuasarProject {
            name: name.to_string(),
        },
        toolchain: QuasarToolchain {
            toolchain_type: toolchain.to_string(),
        },
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
                        framework: "quasar-svm".to_string(),
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
    let toml_str = toml::to_string_pretty(&config).map_err(anyhow::Error::from)?;
    fs::write(root.join("Quasar.toml"), toml_str).map_err(anyhow::Error::from)?;

    // Cargo.toml
    fs::write(
        root.join("Cargo.toml"),
        generate_cargo_toml(name, toolchain, test_language, rust_framework),
    )
    .map_err(anyhow::Error::from)?;

    // .cargo/config.toml (upstream only)
    if matches!(toolchain, Toolchain::Upstream) {
        let cargo_dir = root.join(".cargo");
        fs::create_dir_all(&cargo_dir).map_err(anyhow::Error::from)?;
        fs::write(cargo_dir.join("config.toml"), CARGO_CONFIG).map_err(anyhow::Error::from)?;
    }

    // .gitignore
    fs::write(root.join(".gitignore"), GITIGNORE).map_err(anyhow::Error::from)?;

    // Generate program keypair
    let deploy_dir = root.join("target").join("deploy");
    fs::create_dir_all(&deploy_dir).map_err(anyhow::Error::from)?;

    let signing_key = ed25519_dalek::SigningKey::generate(&mut rand::thread_rng());
    let program_id = bs58::encode(signing_key.verifying_key().as_bytes()).into_string();

    // Write keypair as Solana CLI-compatible JSON (64-byte array: secret + public)
    let mut keypair_bytes = Vec::with_capacity(64);
    keypair_bytes.extend_from_slice(signing_key.as_bytes());
    keypair_bytes.extend_from_slice(signing_key.verifying_key().as_bytes());
    let keypair_json = serde_json::to_string(&keypair_bytes).map_err(anyhow::Error::from)?;
    fs::write(
        deploy_dir.join(format!("{name}-keypair.json")),
        &keypair_json,
    )
    .map_err(anyhow::Error::from)?;

    // src/lib.rs
    let module_name = name.replace('-', "_");
    let has_rust_tests = matches!(test_language, TestLanguage::Rust);
    fs::write(
        src.join("lib.rs"),
        generate_lib_rs(&module_name, &program_id, template, has_rust_tests),
    )
    .map_err(anyhow::Error::from)?;

    // Template-specific files
    match template {
        Template::Minimal => {
            // Everything lives in lib.rs — no instructions/ directory needed
        }
        Template::Full => {
            let instructions_dir = src.join("instructions");
            fs::create_dir_all(&instructions_dir).map_err(anyhow::Error::from)?;
            fs::write(instructions_dir.join("mod.rs"), INSTRUCTIONS_MOD)
                .map_err(anyhow::Error::from)?;
            fs::write(
                instructions_dir.join("initialize.rs"),
                INSTRUCTION_INITIALIZE,
            )
            .map_err(anyhow::Error::from)?;
            fs::write(src.join("state.rs"), STATE_RS).map_err(anyhow::Error::from)?;
            fs::write(src.join("errors.rs"), ERRORS_RS).map_err(anyhow::Error::from)?;
        }
    }

    // Rust test scaffold
    if let Some(fw) = rust_framework {
        fs::write(
            src.join("tests.rs"),
            generate_tests_rs(&module_name, fw, template, toolchain),
        )
        .map_err(anyhow::Error::from)?;
    }

    // TypeScript test scaffold
    if let Some(sdk) = ts_sdk {
        let tests_dir = root.join("tests");
        fs::create_dir_all(&tests_dir).map_err(anyhow::Error::from)?;

        fs::write(root.join("package.json"), generate_package_json(name, sdk))
            .map_err(anyhow::Error::from)?;
        fs::write(root.join("tsconfig.json"), TS_TEST_TSCONFIG).map_err(anyhow::Error::from)?;

        fs::write(
            tests_dir.join(format!("{}.test.ts", name)),
            generate_test_ts(name, sdk, toolchain),
        )
        .map_err(anyhow::Error::from)?;
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
                "note: could not generate Cargo.lock — run `cargo generate-lockfile` before \
                 building"
            )
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Generators
// ---------------------------------------------------------------------------

fn generate_cargo_toml(
    name: &str,
    toolchain: Toolchain,
    test_language: TestLanguage,
    rust_framework: Option<RustFramework>,
) -> String {
    let quasar_lang_dep = quasar_lang_dependency();
    let mut out = format!(
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
crate-type = ["cdylib"]

[features]
alloc = []
client = []
debug = []

[dependencies]
quasar-lang = {quasar_lang_dep}
"#,
    );

    if matches!(toolchain, Toolchain::Solana) {
        out.push_str("solana-instruction = { version = \"3.2.0\" }\n");
    }

    match (test_language, rust_framework) {
        (TestLanguage::None, _) => {}
        (TestLanguage::Rust, Some(RustFramework::Mollusk)) => {
            out.push_str(
                r#"
[dev-dependencies]
mollusk-svm = "0.10.3"
solana-account = {{ version = "3.4.0" }}
solana-address = {{ version = "2.2.0", features = ["decode"] }}
solana-instruction = {{ version = "3.2.0", features = ["bincode"] }}
"#,
            );
        }
        (TestLanguage::Rust, _) => {
            out.push_str(
                r#"
[dev-dependencies]
quasar-svm = {{ version = "0.1" }}
solana-account = {{ version = "3.4.0" }}
solana-address = {{ version = "2.2.0", features = ["decode"] }}
solana-instruction = {{ version = "3.2.0", features = ["bincode"] }}
solana-pubkey = {{ version = "4.1.0" }}
"#,
            );
        }
        (TestLanguage::TypeScript, _) => {}
    }

    out
}

fn generate_lib_rs(
    module_name: &str,
    program_id: &str,
    template: Template,
    has_tests: bool,
) -> String {
    let test_mod = if has_tests {
        "\n#[cfg(test)]\nmod tests;\n"
    } else {
        ""
    };

    match template {
        Template::Minimal => {
            format!(
                r#"#![cfg_attr(not(test), no_std)]

use quasar_lang::prelude::*;

declare_id!("{program_id}");

#[derive(Accounts)]
pub struct Initialize {{
    pub payer: Signer,
    pub system_program: Program<System>,
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

    #[instruction(discriminator = 0)]
    pub fn initialize(ctx: Ctx<Initialize>) -> Result<(), ProgramError> {{
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
        "\"@solana/kit\": \"^6.0.0\""
    } else {
        "\"@solana/web3.js\": \"github:blueshift-gg/solana-web3.js#v2\""
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
    "@blueshift-gg/quasar-svm": "^0.1.12",
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

    if matches!(ts_sdk, TypeScriptSdk::Kit) {
        format!(
            r#"import {{ generateKeyPairSigner }} from "@solana/kit";
import {{ AccountRole, address }} from "@solana/kit";
import {{ describe, it, expect }} from "vitest";
import {{ QuasarSvm, createKeyedSystemAccount }} from "@blueshift-gg/quasar-svm/kit";
import {{ readFile }} from "node:fs/promises";

describe.concurrent("{class_name} Program", async () => {{
  it("initializes", async () => {{
    const idl = JSON.parse(await readFile("target/idl/{module_name}.json", "utf8")) as {{ address: string }};
    const programAddress = address(idl.address);
    const vm = new QuasarSvm();
    vm.addProgram(programAddress, await readFile("target/deploy/{so_name}.so"));

    const payer = await generateKeyPairSigner();

    const initializeInstruction = {{
      programAddress,
      accounts: [
        {{ address: payer.address, role: AccountRole.WRITABLE_SIGNER }},
        {{ address: address("11111111111111111111111111111111"), role: AccountRole.READONLY }},
      ],
      data: Uint8Array.from([0]),
    }};

    const result = vm.processInstruction(initializeInstruction, [
      createKeyedSystemAccount(payer.address),
    ]);

    expect(result.status.ok, `initialize failed:\n${{result.logs.join("\n")}}`).toBe(true);
    }});
}});
"#,
            class_name = crate::utils::snake_to_pascal(&module_name)
        )
    } else {
        format!(
            r#"import {{ Buffer }} from "buffer";
import {{ Address, Keypair, TransactionInstruction }} from "@solana/web3.js";
import {{ readFile }} from "node:fs/promises";
import {{ describe, it, expect }} from "vitest";
import {{ QuasarSvm, createKeyedSystemAccount }} from "@blueshift-gg/quasar-svm/web3.js";

describe.concurrent("{class_name} Program", async () => {{
  it("initializes", async () => {{
    const idl = JSON.parse(await readFile("target/idl/{module_name}.json", "utf8")) as {{ address: string }};
    const programAddress = new Address(idl.address);
    const vm = new QuasarSvm();
    vm.addProgram(programAddress, await readFile("target/deploy/{so_name}.so"));

    const {{ publicKey: payer }} = await Keypair.generate();

    const initializeInstruction = new TransactionInstruction({{
      programId: programAddress,
      keys: [
        {{ pubkey: payer, isSigner: true, isWritable: true }},
        {{ pubkey: new Address("11111111111111111111111111111111"), isSigner: false, isWritable: false }},
      ],
      data: Buffer.from([0]),
    }});

    const result = vm.processInstruction(initializeInstruction, [
      createKeyedSystemAccount(payer),
    ]);

    expect(result.status.ok, `initialize failed:\n${{result.logs.join("\n")}}`).toBe(true);
    }});
}});
"#,
            class_name = crate::utils::snake_to_pascal(&module_name)
        )
    }
}

fn generate_tests_rs(
    module_name: &str,
    rust_framework: RustFramework,
    template: Template,
    toolchain: Toolchain,
) -> String {
    let mut libname = module_name.to_string();
    if matches!(toolchain, Toolchain::Upstream) {
        libname = format!("lib{libname}");
    };
    match (rust_framework, template) {
        (RustFramework::Mollusk, Template::Minimal | Template::Full) => {
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
        (RustFramework::QuasarSVM, Template::Minimal | Template::Full) => {
            format!(
                r#"use quasar_svm::{{Account, Pubkey, QuasarSvm}};
use solana_address::Address;
use solana_instruction::{{AccountMeta, Instruction}};

fn setup() -> QuasarSvm {{
    let elf = std::fs::read("../target/deploy/{libname}.so").unwrap();
    QuasarSvm::new()
        .with_program(&Pubkey::from(crate::ID), &elf)
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
    let mut svm = setup();

    let payer = Pubkey::new_unique();

    let instruction = initialize_instruction(
        Address::from(payer.to_bytes()),
        Address::from(quasar_svm::system_program::ID.to_bytes()),
    );

    let result = svm.process_instruction(
        &instruction,
        &[Account {{
            address: payer,
            lamports: 10_000_000_000,
            data: vec![],
            owner: quasar_svm::system_program::ID,
            executable: false,
        }}],
    );

    result.assert_success();
}}
"#
            )
        }
    }
}

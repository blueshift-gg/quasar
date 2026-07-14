use {
    super::{
        schema::{
            QuasarClients, QuasarProject, QuasarRustTesting, QuasarTesting, QuasarToml,
            QuasarToolchain, QuasarTypeScriptTesting,
        },
        templates::*,
        types::{PackageManager, RustFramework, Template, TestLanguage, Toolchain, TypeScriptSdk},
    },
    crate::{
        error::{CliError, CliResult},
        program_keypair::{self, ProgramKeypair},
    },
    quasar_idl::codegen::typescript::{client_dependency_version, TsTarget},
    quasar_schema::snake_to_pascal,
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
    fs::create_dir_all(&src)?;

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
    let toml_str = toml::to_string_pretty(&config)?;
    fs::write(root.join("Quasar.toml"), toml_str)?;

    // Cargo.toml
    fs::write(
        root.join("Cargo.toml"),
        generate_cargo_toml(name, toolchain, test_language, rust_framework),
    )?;

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

    let keypair = ProgramKeypair::generate();
    let program_id = keypair.program_id();
    program_keypair::write(
        &deploy_dir.join(format!("{name}-keypair.json")),
        &keypair,
        false,
        None,
    )?;

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
            fs::write(
                instructions_dir.join("initialize.rs"),
                INSTRUCTION_INITIALIZE,
            )?;
            fs::write(src.join("state.rs"), STATE_RS)?;
            fs::write(src.join("errors.rs"), ERRORS_RS)?;
        }
    }

    // Rust test scaffold
    if let Some(fw) = rust_framework {
        fs::write(
            src.join("tests.rs"),
            generate_tests_rs(&module_name, fw, template, toolchain),
        )?;
    }

    // TypeScript test scaffold
    if let Some(sdk) = ts_sdk {
        let tests_dir = root.join("tests");
        fs::create_dir_all(&tests_dir)?;

        fs::write(root.join("package.json"), generate_package_json(name, sdk))?;
        fs::write(root.join("tsconfig.json"), TS_TEST_TSCONFIG)?;

        fs::write(
            tests_dir.join(format!("{}.test.ts", name)),
            generate_test_ts(name, sdk, template, toolchain),
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
idl-build = ["quasar-lang/idl-build"]

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
solana-account = { version = "3.4.0" }
solana-address = { version = "2.2.0", features = ["decode"] }
solana-instruction = { version = "3.2.0", features = ["bincode"] }
"#,
            );
        }
        (TestLanguage::Rust, _) => {
            out.push_str(
                r#"
[dev-dependencies]
quasar-svm = { version = "0.1" }
solana-account = { version = "3.4.0" }
solana-address = { version = "2.2.0", features = ["decode"] }
solana-instruction = { version = "3.2.0", features = ["bincode"] }
solana-pubkey = { version = "4.1.0" }
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
pub mod instructions;
pub mod state;
use instructions::*;

declare_id!("{program_id}");

#[program]
mod {module_name} {{
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn initialize(ctx: Ctx<Initialize>, value: u64) -> Result<(), ProgramError> {{
        ctx.accounts.initialize(value, &ctx.bumps)
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
        format!(
            "\"@solana/kit\": \"{}\"",
            client_dependency_version(TsTarget::Kit)
        )
    } else {
        format!(
            "\"@solana/web3.js\": \"{}\"",
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

fn generate_test_ts(
    name: &str,
    ts_sdk: TypeScriptSdk,
    template: Template,
    toolchain: Toolchain,
) -> String {
    match template {
        Template::Minimal => generate_minimal_test_ts(name, ts_sdk, toolchain),
        Template::Full => generate_full_test_ts(name, ts_sdk, toolchain),
    }
}

fn generate_minimal_test_ts(name: &str, ts_sdk: TypeScriptSdk, toolchain: Toolchain) -> String {
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
        {{ address: payer.address, role: AccountRole.READONLY_SIGNER }},
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
            class_name = snake_to_pascal(&module_name)
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
        {{ pubkey: payer, isSigner: true, isWritable: false }},
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
            class_name = snake_to_pascal(&module_name)
        )
    }
}

fn generate_full_test_ts(name: &str, ts_sdk: TypeScriptSdk, toolchain: Toolchain) -> String {
    let module_name = name.replace('-', "_");
    let so_name = match toolchain {
        Toolchain::Upstream => format!("lib{module_name}"),
        Toolchain::Solana => module_name.clone(),
    };

    if matches!(ts_sdk, TypeScriptSdk::Kit) {
        format!(
            r#"import {{
  AccountRole,
  address,
  generateKeyPairSigner,
  getAddressEncoder,
  getProgramDerivedAddress,
}} from "@solana/kit";
import {{ readFile }} from "node:fs/promises";
import {{ describe, it, expect }} from "vitest";
import {{ QuasarSvm, createKeyedSystemAccount }} from "@blueshift-gg/quasar-svm/kit";

describe.concurrent("{class_name} Program", async () => {{
  it("initializes state", async () => {{
    const idl = JSON.parse(await readFile("target/idl/{module_name}.json", "utf8")) as {{ address: string }};
    const programAddress = address(idl.address);
    const vm = new QuasarSvm();
    vm.addProgram(programAddress, await readFile("target/deploy/{so_name}.so"));

    const payer = await generateKeyPairSigner();
    const [myAccount, bump] = await getProgramDerivedAddress({{
      programAddress,
      seeds: [
        new TextEncoder().encode("my-account"),
        getAddressEncoder().encode(payer.address),
      ],
    }});
    const value = 42n;
    const instructionData = new Uint8Array(9);
    instructionData[0] = 0;
    new DataView(instructionData.buffer).setBigUint64(1, value, true);

    const initializeInstruction = {{
      programAddress,
      accounts: [
        {{ address: payer.address, role: AccountRole.WRITABLE_SIGNER }},
        {{ address: myAccount, role: AccountRole.WRITABLE }},
        {{ address: address("11111111111111111111111111111111"), role: AccountRole.READONLY }},
      ],
      data: instructionData,
    }};

    const result = vm.processInstruction(initializeInstruction, [
      createKeyedSystemAccount(payer.address),
      createKeyedSystemAccount(myAccount, 0n),
    ]);

    expect(result.status.ok, `initialize failed:\n${{result.logs.join("\n")}}`).toBe(true);
    const stored = result.account(myAccount);
    if (!stored) throw new Error("initialized state account is missing");
    expect(stored.programAddress).toBe(programAddress);
    expect(stored.data).toHaveLength(107);
    expect(stored.data[0]).toBe(1);
    expect(stored.data[1]).toBe(1);
    expect(stored.data.slice(2, 34)).toEqual(getAddressEncoder().encode(payer.address));
    expect(new DataView(stored.data.buffer, stored.data.byteOffset).getBigUint64(34, true)).toBe(value);
    expect(stored.data[42]).toBe(bump);
    expect(stored.data.slice(43).every((byte) => byte === 0)).toBe(true);
  }});
}});
"#,
            class_name = snake_to_pascal(&module_name)
        )
    } else {
        format!(
            r#"import {{ Buffer }} from "node:buffer";
import {{ Address, Keypair, TransactionInstruction }} from "@solana/web3.js";
import {{ readFile }} from "node:fs/promises";
import {{ describe, it, expect }} from "vitest";
import {{ QuasarSvm, createKeyedSystemAccount }} from "@blueshift-gg/quasar-svm/web3.js";

describe.concurrent("{class_name} Program", async () => {{
  it("initializes state", async () => {{
    const idl = JSON.parse(await readFile("target/idl/{module_name}.json", "utf8")) as {{ address: string }};
    const programAddress = new Address(idl.address);
    const vm = new QuasarSvm();
    vm.addProgram(programAddress, await readFile("target/deploy/{so_name}.so"));

    const {{ publicKey: payer }} = await Keypair.generate();
    const [myAccount, bump] = await Address.findProgramAddress(
      [Buffer.from("my-account"), payer.toBytes()],
      programAddress,
    );
    const value = 42n;
    const instructionData = Buffer.alloc(9);
    instructionData[0] = 0;
    instructionData.writeBigUInt64LE(value, 1);

    const initializeInstruction = new TransactionInstruction({{
      programId: programAddress,
      keys: [
        {{ pubkey: payer, isSigner: true, isWritable: true }},
        {{ pubkey: myAccount, isSigner: false, isWritable: true }},
        {{ pubkey: new Address("11111111111111111111111111111111"), isSigner: false, isWritable: false }},
      ],
      data: instructionData,
    }});

    const result = vm.processInstruction(initializeInstruction, [
      createKeyedSystemAccount(payer),
      createKeyedSystemAccount(myAccount, 0n),
    ]);

    expect(result.status.ok, `initialize failed:\n${{result.logs.join("\n")}}`).toBe(true);
    const stored = result.accounts.find((account) => account.accountId.equals(myAccount));
    if (!stored) throw new Error("initialized state account is missing");
    expect(stored.accountInfo.owner.equals(programAddress)).toBe(true);
    expect(stored.accountInfo.data).toHaveLength(107);
    expect(stored.accountInfo.data[0]).toBe(1);
    expect(stored.accountInfo.data[1]).toBe(1);
    expect(stored.accountInfo.data.subarray(2, 34)).toEqual(Buffer.from(payer.toBytes()));
    expect(new DataView(
      stored.accountInfo.data.buffer,
      stored.accountInfo.data.byteOffset,
    ).getBigUint64(34, true)).toBe(value);
    expect(stored.accountInfo.data[42]).toBe(bump);
    expect(stored.accountInfo.data.subarray(43).every((byte) => byte === 0)).toBe(true);
  }});
}});
"#,
            class_name = snake_to_pascal(&module_name)
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
        (RustFramework::Mollusk, Template::Minimal) => {
            format!(
                r#"use mollusk_svm::Mollusk;
use solana_account::Account;
use solana_address::Address;
use solana_instruction::{{AccountMeta, Instruction}};

fn setup() -> Mollusk {{
    Mollusk::new(&crate::ID, "target/deploy/{libname}")
}}

fn initialize_instruction(payer: Address) -> Instruction {{
    Instruction {{
        program_id: Address::from(crate::ID.to_bytes()),
        accounts: vec![
            AccountMeta::new_readonly(payer, true),
        ],
        data: vec![0],
    }}
}}

#[test]
fn test_initialize() {{
    let mollusk = setup();
    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &Address::default());

    let instruction = initialize_instruction(payer);

    let result = mollusk.process_instruction(
        &instruction,
        &[(payer, payer_account)],
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
        (RustFramework::Mollusk, Template::Full) => {
            format!(
                r#"use mollusk_svm::{{program::keyed_account_for_system_program, Mollusk}};
use solana_account::Account;
use solana_address::Address;
use solana_instruction::{{AccountMeta, Instruction}};

const VALUE: u64 = 42;
const MY_ACCOUNT_SIZE: usize = 107;

fn setup() -> Mollusk {{
    Mollusk::new(&crate::ID, "target/deploy/{libname}")
}}

fn initialize_instruction(
    payer: Address,
    my_account: Address,
    system_program: Address,
) -> Instruction {{
    let mut data = vec![0];
    data.extend_from_slice(&VALUE.to_le_bytes());
    Instruction {{
        program_id: Address::from(crate::ID.to_bytes()),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(my_account, false),
            AccountMeta::new_readonly(system_program, false),
        ],
        data,
    }}
}}

#[test]
fn test_initialize() {{
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let payer = Address::new_unique();
    let (my_account, bump) =
        Address::find_program_address(&[b"my-account", payer.as_ref()], &crate::ID);
    let instruction = initialize_instruction(payer, my_account, system_program);

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, Account::new(10_000_000_000, 0, &system_program)),
            (my_account, Account::default()),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "initialize failed: {{:?}}",
        result.program_result,
    );
    let stored = &result.resulting_accounts[1].1;
    assert_eq!(stored.owner, crate::ID);
    assert_eq!(stored.data.len(), MY_ACCOUNT_SIZE);
    assert_eq!(stored.data[0], 1, "discriminator");
    assert_eq!(stored.data[1], 1, "version");
    assert_eq!(&stored.data[2..34], payer.as_ref(), "authority");
    assert_eq!(&stored.data[34..42], &VALUE.to_le_bytes(), "value");
    assert_eq!(stored.data[42], bump, "bump");
    assert!(stored.data[43..].iter().all(|byte| *byte == 0), "reserved");
}}
"#
            )
        }
        (RustFramework::QuasarSVM, Template::Minimal) => {
            format!(
                r#"use quasar_svm::{{Account, Pubkey, QuasarSvm}};
use solana_address::Address;
use solana_instruction::{{AccountMeta, Instruction}};

fn setup() -> QuasarSvm {{
    let elf = std::fs::read("target/deploy/{libname}.so").unwrap();
    QuasarSvm::new()
        .with_program(&Pubkey::from(crate::ID), &elf)
}}

fn initialize_instruction(payer: Address) -> Instruction {{
    Instruction {{
        program_id: Address::from(crate::ID.to_bytes()),
        accounts: vec![
            AccountMeta::new_readonly(payer, true),
        ],
        data: vec![0],
    }}
}}

#[test]
fn test_initialize() {{
    let mut svm = setup();

    let payer = Pubkey::new_unique();

    let instruction = initialize_instruction(Address::from(payer.to_bytes()));

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
        (RustFramework::QuasarSVM, Template::Full) => {
            format!(
                r#"use quasar_svm::{{Account, Pubkey, QuasarSvm}};
use solana_address::Address;
use solana_instruction::{{AccountMeta, Instruction}};

const VALUE: u64 = 42;
const MY_ACCOUNT_SIZE: usize = 107;

fn setup() -> QuasarSvm {{
    let elf = std::fs::read("target/deploy/{libname}.so").unwrap();
    QuasarSvm::new()
        .with_program(&Pubkey::from(crate::ID), &elf)
}}

fn initialize_instruction(payer: Pubkey, my_account: Pubkey) -> Instruction {{
    let mut data = vec![0];
    data.extend_from_slice(&VALUE.to_le_bytes());
    Instruction {{
        program_id: Address::from(crate::ID.to_bytes()),
        accounts: vec![
            AccountMeta::new(Address::from(payer.to_bytes()), true),
            AccountMeta::new(Address::from(my_account.to_bytes()), false),
            AccountMeta::new_readonly(
                Address::from(quasar_svm::system_program::ID.to_bytes()),
                false,
            ),
        ],
        data,
    }}
}}

fn system_account(address: Pubkey, lamports: u64) -> Account {{
    Account {{
        address,
        lamports,
        data: vec![],
        owner: quasar_svm::system_program::ID,
        executable: false,
    }}
}}

#[test]
fn test_initialize() {{
    let mut svm = setup();
    let payer = Pubkey::new_unique();
    let (my_account, bump) =
        Pubkey::find_program_address(&[b"my-account", payer.as_ref()], &crate::ID);
    let instruction = initialize_instruction(payer, my_account);

    let result = svm.process_instruction(
        &instruction,
        &[system_account(payer, 10_000_000_000), system_account(my_account, 0)],
    );

    result.assert_success();
    let stored = result.account(&my_account).expect("initialized state account");
    assert_eq!(stored.owner, crate::ID);
    assert_eq!(stored.data.len(), MY_ACCOUNT_SIZE);
    assert_eq!(stored.data[0], 1, "discriminator");
    assert_eq!(stored.data[1], 1, "version");
    assert_eq!(&stored.data[2..34], payer.as_ref(), "authority");
    assert_eq!(&stored.data[34..42], &VALUE.to_le_bytes(), "value");
    assert_eq!(stored.data[42], bump, "bump");
    assert!(stored.data[43..].iter().all(|byte| *byte == 0), "reserved");
}}
"#
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::{generate_test_ts, generate_tests_rs, scaffold},
        crate::{
            config::QuasarConfig,
            init::types::{
                PackageManager, RustFramework, Template, TestLanguage, Toolchain, TypeScriptSdk,
            },
        },
        quasar_idl::codegen::typescript::{client_dependency_version, TsTarget},
        serde_json::Value,
        std::{collections::BTreeSet, fs},
        tempfile::tempdir,
    };

    #[derive(Clone, Copy)]
    enum TestStack {
        NoTests,
        QuasarSvm,
        Mollusk,
        Kit,
        Web3,
    }

    impl TestStack {
        fn label(self) -> &'static str {
            match self {
                Self::NoTests => "none",
                Self::QuasarSvm => "rust/quasar-svm",
                Self::Mollusk => "rust/mollusk",
                Self::Kit => "typescript/kit",
                Self::Web3 => "typescript/web3.js",
            }
        }

        fn choices(self) -> (TestLanguage, Option<RustFramework>, Option<TypeScriptSdk>) {
            match self {
                Self::NoTests => (TestLanguage::None, None, None),
                Self::QuasarSvm => (TestLanguage::Rust, Some(RustFramework::QuasarSVM), None),
                Self::Mollusk => (TestLanguage::Rust, Some(RustFramework::Mollusk), None),
                Self::Kit => (TestLanguage::TypeScript, None, Some(TypeScriptSdk::Kit)),
                Self::Web3 => (TestLanguage::TypeScript, None, Some(TypeScriptSdk::Web3js)),
            }
        }
    }

    struct ScaffoldCase {
        label: &'static str,
        toolchain: Toolchain,
        stack: TestStack,
        template: Template,
        package_manager: Option<PackageManager>,
    }

    fn pairwise_cases() -> Vec<ScaffoldCase> {
        use {
            PackageManager::{Bun, Npm, Other, Pnpm, Yarn},
            Template::{Full, Minimal},
            TestStack::{Kit, Mollusk, NoTests, QuasarSvm, Web3},
            Toolchain::{Solana, Upstream},
        };

        let case = |label, toolchain, stack, template, package_manager| ScaffoldCase {
            label,
            toolchain,
            stack,
            template,
            package_manager,
        };
        let other = || {
            Some(Other {
                install: "custom-install --locked".into(),
                test: "custom-test --ci".into(),
            })
        };

        vec![
            case("none-solana-minimal", Solana, NoTests, Minimal, None),
            case("none-upstream-full", Upstream, NoTests, Full, None),
            case("quasar-svm-solana-full", Solana, QuasarSvm, Full, None),
            case(
                "quasar-svm-upstream-minimal",
                Upstream,
                QuasarSvm,
                Minimal,
                None,
            ),
            case("mollusk-solana-minimal", Solana, Mollusk, Minimal, None),
            case("mollusk-upstream-full", Upstream, Mollusk, Full, None),
            case("kit-pnpm-solana-minimal", Solana, Kit, Minimal, Some(Pnpm)),
            case("web3-pnpm-upstream-full", Upstream, Web3, Full, Some(Pnpm)),
            case(
                "kit-bun-upstream-minimal",
                Upstream,
                Kit,
                Minimal,
                Some(Bun),
            ),
            case("web3-bun-solana-full", Solana, Web3, Full, Some(Bun)),
            case("kit-npm-solana-full", Solana, Kit, Full, Some(Npm)),
            case(
                "web3-npm-upstream-minimal",
                Upstream,
                Web3,
                Minimal,
                Some(Npm),
            ),
            case("kit-yarn-upstream-full", Upstream, Kit, Full, Some(Yarn)),
            case(
                "web3-yarn-solana-minimal",
                Solana,
                Web3,
                Minimal,
                Some(Yarn),
            ),
            case("kit-other-solana-minimal", Solana, Kit, Minimal, other()),
            case("web3-other-upstream-full", Upstream, Web3, Full, other()),
        ]
    }
    #[test]
    fn every_scaffold_choice_is_covered_by_a_pairwise_matrix() {
        let temp = tempdir().expect("tempdir");
        let mut toolchain_template = BTreeSet::new();
        let mut toolchain_stack = BTreeSet::new();
        let mut template_stack = BTreeSet::new();
        let mut package_managers = BTreeSet::new();
        let mut toolchain_package_manager = BTreeSet::new();
        let mut template_package_manager = BTreeSet::new();
        let mut sdk_package_manager = BTreeSet::new();

        for case in pairwise_cases() {
            let (language, rust_framework, ts_sdk) = case.stack.choices();
            let toolchain = case.toolchain.to_string();
            let template = case.template.to_string();
            let stack = case.stack.label().to_string();
            toolchain_template.insert((toolchain.clone(), template.clone()));
            toolchain_stack.insert((toolchain.clone(), stack.clone()));
            template_stack.insert((template.clone(), stack));

            let install_command = case
                .package_manager
                .as_ref()
                .map(|manager| manager.install_cmd().to_string());
            let test_command = case
                .package_manager
                .as_ref()
                .map(|manager| manager.test_cmd().to_string());
            if let (Some(manager), Some(sdk)) = (&case.package_manager, ts_sdk) {
                let manager = manager.to_string();
                let sdk = sdk.to_string();
                package_managers.insert(manager.clone());
                toolchain_package_manager.insert((toolchain, manager.clone()));
                template_package_manager.insert((template, manager.clone()));
                sdk_package_manager.insert((sdk, manager));
            }

            let project_dir = temp.path().join(case.label);
            let clients = if matches!(language, TestLanguage::TypeScript) {
                vec!["typescript".to_string()]
            } else {
                Vec::new()
            };
            scaffold(
                project_dir.to_str().expect("utf8 path"),
                case.label,
                case.toolchain,
                language,
                rust_framework,
                ts_sdk,
                case.template,
                case.package_manager.as_ref(),
                &clients,
            )
            .unwrap_or_else(|error| panic!("{}: scaffold failed: {error}", case.label));

            let config: QuasarConfig = toml::from_str(
                &fs::read_to_string(project_dir.join("Quasar.toml")).expect("read Quasar.toml"),
            )
            .unwrap_or_else(|error| panic!("{}: invalid Quasar.toml: {error}", case.label));
            assert_eq!(
                config.toolchain.toolchain_type,
                case.toolchain.to_string(),
                "{}",
                case.label
            );
            assert_eq!(
                config.testing.language,
                language.to_string(),
                "{}",
                case.label
            );
            assert_eq!(config.clients.languages, clients, "{}", case.label);

            let cargo: toml::Value = toml::from_str(
                &fs::read_to_string(project_dir.join("Cargo.toml")).expect("read Cargo.toml"),
            )
            .unwrap_or_else(|error| panic!("{}: invalid Cargo.toml: {error}", case.label));
            assert_eq!(
                cargo["dependencies"].get("solana-instruction").is_some(),
                matches!(case.toolchain, Toolchain::Solana),
                "{}",
                case.label,
            );
            assert_eq!(
                project_dir.join(".cargo/config.toml").exists(),
                matches!(case.toolchain, Toolchain::Upstream),
                "{}",
                case.label,
            );
            assert_eq!(
                project_dir.join("src/state.rs").exists(),
                matches!(case.template, Template::Full),
                "{}",
                case.label,
            );

            if let Some(framework) = rust_framework {
                let rust = config
                    .testing
                    .rust
                    .as_ref()
                    .unwrap_or_else(|| panic!("{}: missing Rust config", case.label));
                assert_eq!(rust.framework, framework.to_string(), "{}", case.label);
                let manifest = fs::read_to_string(project_dir.join("Cargo.toml"))
                    .expect("read generated Cargo.toml");
                assert!(
                    manifest.contains(match framework {
                        RustFramework::QuasarSVM => "quasar-svm =",
                        RustFramework::Mollusk => "mollusk-svm =",
                    }),
                    "{}: missing selected Rust framework dependency",
                    case.label,
                );
                assert!(project_dir.join("src/tests.rs").exists(), "{}", case.label);
            } else {
                assert!(config.testing.rust.is_none(), "{}", case.label);
            }

            if let Some(sdk) = ts_sdk {
                let typescript = config
                    .testing
                    .typescript
                    .as_ref()
                    .unwrap_or_else(|| panic!("{}: missing TypeScript config", case.label));
                assert_eq!(typescript.sdk, sdk.to_string(), "{}", case.label);
                assert_eq!(
                    typescript.install.display(),
                    install_command.expect("TypeScript install command"),
                    "{}",
                    case.label,
                );
                assert_eq!(
                    typescript.test.display(),
                    test_command.expect("TypeScript test command"),
                    "{}",
                    case.label,
                );

                let package_json: Value = serde_json::from_str(
                    &fs::read_to_string(project_dir.join("package.json"))
                        .expect("read package.json"),
                )
                .unwrap_or_else(|error| panic!("{}: invalid package.json: {error}", case.label));
                let (dependency, target) = match sdk {
                    TypeScriptSdk::Kit => ("@solana/kit", TsTarget::Kit),
                    TypeScriptSdk::Web3js => ("@solana/web3.js", TsTarget::Web3js),
                };
                assert_eq!(
                    package_json["dependencies"][dependency],
                    client_dependency_version(target),
                    "{}",
                    case.label,
                );
                assert!(
                    project_dir
                        .join("tests")
                        .join(format!("{}.test.ts", case.label))
                        .exists(),
                    "{}",
                    case.label,
                );
            } else {
                assert!(config.testing.typescript.is_none(), "{}", case.label);
                assert!(!project_dir.join("package.json").exists(), "{}", case.label);
            }
        }

        assert_eq!(toolchain_template.len(), 4, "toolchain x template coverage");
        assert_eq!(toolchain_stack.len(), 10, "toolchain x test stack coverage");
        assert_eq!(template_stack.len(), 10, "template x test stack coverage");
        assert_eq!(package_managers.len(), 5, "package-manager choice coverage");
        assert_eq!(
            toolchain_package_manager.len(),
            10,
            "toolchain x package-manager coverage",
        );
        assert_eq!(
            template_package_manager.len(),
            10,
            "template x package-manager coverage",
        );
        assert_eq!(
            sdk_package_manager.len(),
            10,
            "TypeScript SDK x package-manager coverage",
        );
    }

    #[test]
    fn full_templates_generate_stateful_tests_without_changing_minimal_tests() {
        for framework in [RustFramework::Mollusk, RustFramework::QuasarSVM] {
            let full = generate_tests_rs("demo", framework, Template::Full, Toolchain::Solana);
            assert!(full.contains("my-account"));
            assert!(full.contains("MY_ACCOUNT_SIZE"));
            assert!(full.contains("data[42]"));

            let minimal =
                generate_tests_rs("demo", framework, Template::Minimal, Toolchain::Solana);
            assert!(!minimal.contains("my-account"));
            assert!(!minimal.contains("MY_ACCOUNT_SIZE"));
        }

        for sdk in [TypeScriptSdk::Kit, TypeScriptSdk::Web3js] {
            let full = generate_test_ts("demo", sdk, Template::Full, Toolchain::Solana);
            assert!(full.contains("my-account"));
            assert!(full.contains("initializes state"));
            assert!(full.contains("data[42]"));

            let minimal = generate_test_ts("demo", sdk, Template::Minimal, Toolchain::Solana);
            assert!(!minimal.contains("my-account"));
            assert!(minimal.contains("it(\"initializes\""));
        }

        let web3 = generate_test_ts(
            "demo",
            TypeScriptSdk::Web3js,
            Template::Full,
            Toolchain::Solana,
        );
        assert!(web3.contains("await Address.findProgramAddress"));
        assert!(web3.contains("Buffer.from(payer.toBytes())"));
        assert!(!web3.contains("findProgramAddressSync"));
        assert!(!web3.contains("toBuffer()"));
    }
}

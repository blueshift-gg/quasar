use {
    crate::error::CliResult,
    dialoguer::{theme::ColorfulTheme, Input, Select},
    serde::Serialize,
    std::{fmt, fs, path::Path},
};

#[derive(Debug, Clone, Copy)]
enum Toolchain {
    Solana,
    Upstream,
}

impl fmt::Display for Toolchain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Toolchain::Solana => write!(f, "solana"),
            Toolchain::Upstream => write!(f, "upstream"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Framework {
    None,
    Mollusk,
    QuasarSVMWeb3js,
    QuasarSVMKit,
}

impl Framework {
    fn has_typescript(&self) -> bool {
        matches!(self, Framework::QuasarSVMWeb3js | Framework::QuasarSVMKit)
    }

    fn is_kit(&self) -> bool {
        matches!(self, Framework::QuasarSVMKit)
    }

    fn has_rust_tests(&self) -> bool {
        matches!(self, Framework::Mollusk)
    }
}

impl fmt::Display for Framework {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Framework::None => write!(f, "none"),
            Framework::Mollusk => write!(f, "mollusk"),
            Framework::QuasarSVMWeb3js => write!(f, "quasarsvm-web3js"),
            Framework::QuasarSVMKit => write!(f, "quasarsvm-kit"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Template {
    Minimal,
    Full,
}

#[derive(Serialize)]
struct QuasarToml {
    project: QuasarProject,
    toolchain: QuasarToolchain,
    testing: QuasarTesting,
}

#[derive(Serialize)]
struct QuasarProject {
    name: String,
}

#[derive(Serialize)]
struct QuasarToolchain {
    #[serde(rename = "type")]
    toolchain_type: String,
}

#[derive(Serialize)]
struct QuasarTesting {
    framework: String,
}

pub fn run(name: Option<String>) -> CliResult {
    let theme = ColorfulTheme::default();

    // Project name
    let name: String = {
        let mut prompt = Input::with_theme(&theme).with_prompt("Project name");
        if let Some(default) = name {
            prompt = prompt.default(default);
        }
        prompt.interact_text().map_err(anyhow::Error::from)?
    };

    // Toolchain
    let toolchain_items = &[
        "solana    (cargo build-sbf)",
        "upstream  (cargo +nightly build-bpf)",
    ];
    let toolchain_idx = Select::with_theme(&theme)
        .with_prompt("Toolchain")
        .items(toolchain_items)
        .default(0)
        .interact()
        .map_err(anyhow::Error::from)?;
    let toolchain = match toolchain_idx {
        0 => Toolchain::Solana,
        _ => Toolchain::Upstream,
    };

    // Testing framework
    let framework_items = &["None", "Mollusk", "QuasarSVM/Web3.js", "QuasarSVM/Kit"];
    let framework_idx = Select::with_theme(&theme)
        .with_prompt("Testing framework")
        .items(framework_items)
        .default(1)
        .interact()
        .map_err(anyhow::Error::from)?;
    let framework = match framework_idx {
        0 => Framework::None,
        1 => Framework::Mollusk,
        2 => Framework::QuasarSVMWeb3js,
        _ => Framework::QuasarSVMKit,
    };

    // Template
    let template_items = &[
        "Minimal (instruction file only)",
        "Full(state, events, and instruction files)",
    ];
    let template_idx = Select::with_theme(&theme)
        .with_prompt("Template")
        .items(template_items)
        .default(0)
        .interact()
        .map_err(anyhow::Error::from)?;
    let template = match template_idx {
        0 => Template::Minimal,
        _ => Template::Full,
    };

    scaffold(&name, toolchain, framework, template)?;

    println!("\nCreated project: {name}/");
    Ok(())
}

fn scaffold(
    name: &str,
    toolchain: Toolchain,
    framework: Framework,
    template: Template,
) -> CliResult {
    let root = Path::new(name);

    if root.exists() {
        eprintln!("Error: directory '{}' already exists", name);
        std::process::exit(1);
    }

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
            framework: framework.to_string(),
        },
    };
    let toml_str = toml::to_string_pretty(&config).map_err(anyhow::Error::from)?;
    fs::write(root.join("Quasar.toml"), toml_str).map_err(anyhow::Error::from)?;

    // Cargo.toml
    fs::write(
        root.join("Cargo.toml"),
        generate_cargo_toml(name, toolchain, framework),
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
    let has_rust_tests = framework.has_rust_tests();
    fs::write(
        src.join("lib.rs"),
        generate_lib_rs(&module_name, &program_id, template, has_rust_tests),
    )
    .map_err(anyhow::Error::from)?;

    // Template-specific files
    match template {
        Template::Minimal => {
            let instructions_dir = src.join("instructions");
            fs::create_dir_all(&instructions_dir).map_err(anyhow::Error::from)?;
            fs::write(instructions_dir.join("mod.rs"), INSTRUCTIONS_MOD)
                .map_err(anyhow::Error::from)?;
            fs::write(
                instructions_dir.join("initialize.rs"),
                INSTRUCTION_INITIALIZE,
            )
            .map_err(anyhow::Error::from)?;
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
            fs::write(src.join("events.rs"), EVENTS_RS).map_err(anyhow::Error::from)?;
        }
    }

    // Rust test scaffold
    if framework.has_rust_tests() {
        fs::write(
            src.join("tests.rs"),
            generate_tests_rs(&module_name, framework, template, toolchain),
        )
        .map_err(anyhow::Error::from)?;
    }

    // TypeScript test scaffold
    if framework.has_typescript() {
        let tests_dir = root.join("tests");
        fs::create_dir_all(&tests_dir).map_err(anyhow::Error::from)?;

        // package.json and tsconfig.json go in the project root
        fs::write(
            root.join("package.json"),
            generate_package_json(name, framework),
        )
        .map_err(anyhow::Error::from)?;
        fs::write(root.join("tsconfig.json"), TS_TEST_TSCONFIG).map_err(anyhow::Error::from)?;

        // Test files go in tests/
        fs::write(
            tests_dir.join(format!("{}.test.ts", name)),
            generate_test_ts(name, framework),
        )
        .map_err(anyhow::Error::from)?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Generators
// ---------------------------------------------------------------------------

fn generate_cargo_toml(name: &str, toolchain: Toolchain, framework: Framework) -> String {
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
quasar-core = {{ git = "https://github.com/blueshift-gg/quasar" }}
"#,
    );

    if matches!(toolchain, Toolchain::Solana) {
        out.push_str("solana-instruction = { version = \"3.2.0\" }\n");
    }

    // Dev dependencies based on testing framework
    let client_dep = format!("{name}-client = {{ path = \"target/client/rust/{name}-client\" }}\n");

    match framework {
        Framework::None => {}
        Framework::Mollusk => {
            out.push_str(&format!(
                r#"
[dev-dependencies]
{client_dep}mollusk-svm = "0.10.3"
solana-account = {{ version = "3.4.0" }}
solana-address = {{ version = "2.2.0", features = ["decode"] }}
solana-instruction = {{ version = "3.2.0", features = ["bincode"] }}
"#,
            ));
        }
        Framework::QuasarSVMWeb3js | Framework::QuasarSVMKit => {
            out.push_str(&format!(
                r#"
[dev-dependencies]
{client_dep}solana-account = {{ version = "3.4.0" }}
solana-address = {{ version = "2.2.0", features = ["decode"] }}
solana-instruction = {{ version = "3.2.0", features = ["bincode"] }}
"#,
            ));
        }
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

use quasar_core::prelude::*;

mod instructions;
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
        Template::Full => {
            format!(
                r#"#![cfg_attr(not(test), no_std)]

use quasar_core::prelude::*;

mod events;
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

fn generate_package_json(name: &str, framework: Framework) -> String {
    let solana_dep = if framework.is_kit() {
        "\"@solana/kit\": \"^6.0.0\""
    } else {
        "\"@solana/web3.js\": \"github:blueshift-gg/web3.js#v2\""
    };
    format!(
        r#"{{
  "name": "{name}",
  "version": "0.1.0",
  "private": true,
  "type": "commonjs",
  "scripts": {{
    "test": "mocha --require tsx --delay tests/*.test.ts"
  }},
  "dependencies": {{
    "@blueshift-gg/quasar-svm": "^0.1",
    {solana_dep}
  }},
  "devDependencies": {{
    "@types/chai": "^5.2.0",
    "@types/mocha": "^10.0.0",
    "@types/node": "^22.0.0",
    "chai": "^6.2.2",
    "mocha": "^11.7.5",
    "tsx": "^4.21.0",
    "typescript": "^5.9.3"
  }}
}}
"#
    )
}

fn generate_test_ts(name: &str, framework: Framework) -> String {
    let module_name = name.replace('-', "_");
    let class_name = snake_to_pascal(&module_name);

    if framework.is_kit() {
        format!(
            r#"import {{ generateKeyPairSigner, address, lamports, Account }} from "@solana/kit";
import {{ {class_name}Client, PROGRAM_ADDRESS }} from "../target/client/typescript/{module_name}/kit";
import {{ describe, it, run }} from "mocha";
import {{ QuasarSvm }} from "@blueshift-gg/quasar-svm/kit";
import {{ readFile }} from "node:fs/promises";
import {{ assert }} from "chai";

const {class_name}Program = new {class_name}Client();

describe("{class_name} Program", async () => {{

  const vm = new QuasarSvm()
    .addSystemProgram()
    .addProgram(PROGRAM_ADDRESS, await readFile("target/deploy/{name}.so"))

  const payer = await generateKeyPairSigner();

  it("initializes", async () => {{
    const initializeInstruction = {class_name}Program.createInitializeInstruction({{
      payer: payer.address,
    }});

    const accounts: Account<Uint8Array>[] = [
      {{
        address: payer.address,
        data: new Uint8Array(),
        executable: false,
        lamports: lamports(1_000_000_000n),
        programAddress: address("11111111111111111111111111111111"),
        space: 0n,
      }}
    ];

    const result = vm.processInstruction(initializeInstruction, accounts);

    assert.equal(result.status, 0);
  }});

  run()
}});
"#
        )
    } else {
        format!(
            r#"import {{ Keypair, SystemProgram, KeyedAccountInfo }} from "@solana/web3.js";
import {{ {class_name}Client }} from "../target/client/typescript/{module_name}/web3.js";
import {{ readFile }} from "node:fs/promises";
import {{ describe, it, run }} from "mocha";
import {{ assert }} from "chai";
import {{ QuasarSvm }} from "@blueshift-gg/quasar-svm/web3.js";

const {class_name}Program = new {class_name}Client();

describe("{class_name} Program", async () => {{
  const vm = new QuasarSvm()
    .addSystemProgram()
    .addProgram({class_name}Client.programId, await readFile("target/deploy/{name}.so"));

  const {{ publicKey: payer }} = await Keypair.generate();

  it("initializes", async () => {{
    const initializeInstruction = {class_name}Program.createInitializeInstruction({{
      payer,
    }});

    const accounts = [
      {{
        accountId: payer,
        accountInfo: {{
          executable: false,
          owner: SystemProgram.programId,
          lamports: 1_000_000_000n,
          data: new Uint8Array(),
          rentEpoch: 0n,
        }},
      }} as KeyedAccountInfo,
    ];

    const result = vm.processInstruction(initializeInstruction, accounts);

    assert.equal(result.status, 0);
  }});

  run();
}});
"#
        )
    }
}

fn snake_to_pascal(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().to_string() + &chars.collect::<String>(),
            }
        })
        .collect()
}

fn generate_tests_rs(
    module_name: &str,
    framework: Framework,
    template: Template,
    toolchain: Toolchain,
) -> String {
    let mut libname = module_name.to_string();
    if matches!(toolchain, Toolchain::Upstream) {
        libname = format!("lib{libname}");
    };
    let client_crate = format!("{module_name}_client");

    match (framework, template) {
        (Framework::Mollusk, Template::Minimal | Template::Full) => {
            format!(
                r#"extern crate std;

use mollusk_svm::{{program::keyed_account_for_system_program, Mollusk}};
use solana_account::Account;
use solana_address::Address;
use solana_instruction::Instruction;

use {client_crate}::InitializeInstruction;

fn setup() -> Mollusk {{
    Mollusk::new(&crate::ID, "target/deploy/{libname}")
}}

#[test]
fn test_initialize() {{
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let instruction: Instruction = InitializeInstruction {{
        payer,
        system_program,
    }}
    .into();

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
        _ => r#"extern crate std;

#[test]
fn test_initialize() {
    // TODO: implement test
}
"#
        .to_string(),
    }
}

// ---------------------------------------------------------------------------
// Static templates
// ---------------------------------------------------------------------------

const GITIGNORE: &str = "\
# Build artifacts
/target

# Lock files
Cargo.lock
package-lock.json
yarn.lock

# Dependencies
node_modules

# Environment
.env
.env.*

# OS
.DS_Store
";

const CARGO_CONFIG: &str = r#"[unstable]
build-std = ["core", "alloc"]

[target.bpfel-unknown-none]
rustflags = [
"--cfg", "feature=\"mem_unaligned\"",
"-C", "linker=sbpf-linker",
"-C", "panic=abort",
"-C", "relocation-model=static",
"-C", "link-arg=--disable-memory-builtins",
"-C", "link-arg=--llvm-args=--bpf-stack-size=4096",
"-C", "link-arg=--disable-expand-memcpy-in-order",
"-C", "link-arg=--export=entrypoint",
"-C", "target-cpu=v2",
]
[alias]
build-bpf = "build --release --target bpfel-unknown-none"
"#;

const INSTRUCTIONS_MOD: &str = r#"mod initialize;
pub use initialize::*;
"#;

const INSTRUCTION_INITIALIZE: &str = r#"use quasar_core::prelude::*;

#[derive(Accounts)]
pub struct Initialize<'info> {
    pub payer: &'info mut Signer,
    pub system_program: &'info Program<System>,
}

impl<'info> Initialize<'info> {
    #[inline(always)]
    pub fn initialize(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
"#;

const STATE_RS: &str = r#"use quasar_core::prelude::*;

#[account(discriminator = 1)]
pub struct MyAccount {
    pub authority: Address,
    pub value: u64,
}
"#;

const EVENTS_RS: &str = r#"use quasar_core::prelude::*;

#[event(discriminator = 0)]
pub struct InitializeEvent {
    pub authority: Address,
}
"#;

const TS_TEST_TSCONFIG: &str = r#"{
  "compilerOptions": {
    "target": "es2020",
    "module": "commonjs",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "resolveJsonModule": true,
    "types": ["node", "mocha"]
  },
  "include": ["tests/*.test.ts"]
}
"#;

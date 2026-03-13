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
    Mollusk,
    LiteSVM,
    QuasarVM,
    LiteSVMWeb3js,
    LiteSVMKit,
    QuasarVMWeb3js,
    QuasarVMKit,
}

impl Framework {
    fn has_typescript(&self) -> bool {
        matches!(
            self,
            Framework::LiteSVMWeb3js
                | Framework::LiteSVMKit
                | Framework::QuasarVMWeb3js
                | Framework::QuasarVMKit
        )
    }
}

impl fmt::Display for Framework {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Framework::Mollusk => write!(f, "mollusk"),
            Framework::LiteSVM => write!(f, "litesvm"),
            Framework::QuasarVM => write!(f, "quasarvm"),
            Framework::LiteSVMWeb3js => write!(f, "litesvm-web3js"),
            Framework::LiteSVMKit => write!(f, "litesvm-kit"),
            Framework::QuasarVMWeb3js => write!(f, "quasarvm-web3js"),
            Framework::QuasarVMKit => write!(f, "quasarvm-kit"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Template {
    Bare,
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

pub fn run() -> CliResult {
    let theme = ColorfulTheme::default();

    // Project name
    let name: String = Input::with_theme(&theme)
        .with_prompt("Project name")
        .interact_text()
        .map_err(anyhow::Error::from)?;

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
    let framework_items = &[
        "Mollusk",
        "LiteSVM",
        "QuasarVM",
        "LiteSVM/Web3.js",
        "LiteSVM/Kit",
        "QuasarVM/Web3.js",
        "QuasarVM/Kit",
    ];
    let framework_idx = Select::with_theme(&theme)
        .with_prompt("Testing framework")
        .items(framework_items)
        .default(0)
        .interact()
        .map_err(anyhow::Error::from)?;
    let framework = match framework_idx {
        0 => Framework::Mollusk,
        1 => Framework::LiteSVM,
        2 => Framework::QuasarVM,
        3 => Framework::LiteSVMWeb3js,
        4 => Framework::LiteSVMKit,
        5 => Framework::QuasarVMWeb3js,
        _ => Framework::QuasarVMKit,
    };

    // Template
    let template_items = &["Bare", "Minimal", "Full"];
    let template_idx = Select::with_theme(&theme)
        .with_prompt("Template")
        .items(template_items)
        .default(0)
        .interact()
        .map_err(anyhow::Error::from)?;
    let template = match template_idx {
        0 => Template::Bare,
        1 => Template::Minimal,
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

    // src/lib.rs
    let module_name = name.replace('-', "_");
    fs::write(src.join("lib.rs"), generate_lib_rs(&module_name, template))
        .map_err(anyhow::Error::from)?;

    // Template-specific files
    match template {
        Template::Bare => {}
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

    // TypeScript test scaffold
    if framework.has_typescript() {
        let tests_dir = root.join("tests");
        fs::create_dir_all(&tests_dir).map_err(anyhow::Error::from)?;
        fs::write(
            tests_dir.join("package.json"),
            generate_package_json(name, framework),
        )
        .map_err(anyhow::Error::from)?;
        fs::write(tests_dir.join("tsconfig.json"), TSCONFIG).map_err(anyhow::Error::from)?;
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

[lib]
crate-type = ["cdylib"]

[features]
alloc = []
client = []
debug = []

[dependencies]
quasar = {{ version = "0.1.0" }}
"#,
    );

    if matches!(toolchain, Toolchain::Solana) {
        out.push_str("solana-instruction = { version = \"3.2.0\" }\n");
    }

    // Dev dependencies based on testing framework
    match framework {
        Framework::Mollusk => {
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
        Framework::LiteSVM | Framework::LiteSVMWeb3js | Framework::LiteSVMKit => {
            out.push_str(
                r#"
[dev-dependencies]
litesvm = "0.6"
solana-account = { version = "3.4.0" }
solana-address = { version = "2.2.0", features = ["decode"] }
solana-instruction = { version = "3.2.0", features = ["bincode"] }
"#,
            );
        }
        Framework::QuasarVM | Framework::QuasarVMWeb3js | Framework::QuasarVMKit => {
            out.push_str(
                r#"
[dev-dependencies]
solana-account = { version = "3.4.0" }
solana-address = { version = "2.2.0", features = ["decode"] }
solana-instruction = { version = "3.2.0", features = ["bincode"] }
"#,
            );
        }
    }

    out
}

fn generate_lib_rs(module_name: &str, template: Template) -> String {
    match template {
        Template::Bare => {
            format!(
                r#"#![no_std]

use quasar::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
mod {module_name} {{
    use super::*;
}}
"#
            )
        }
        Template::Minimal => {
            format!(
                r#"#![no_std]

use quasar::prelude::*;

mod instructions;
use instructions::*;

declare_id!("11111111111111111111111111111111");

#[program]
mod {module_name} {{
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn initialize(ctx: Ctx<Initialize>) -> Result<(), ProgramError> {{
        ctx.accounts.initialize()
    }}
}}
"#
            )
        }
        Template::Full => {
            format!(
                r#"#![no_std]

use quasar::prelude::*;

mod events;
mod instructions;
mod state;
use instructions::*;

declare_id!("11111111111111111111111111111111");

#[program]
mod {module_name} {{
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn initialize(ctx: Ctx<Initialize>) -> Result<(), ProgramError> {{
        ctx.accounts.initialize()
    }}
}}
"#
            )
        }
    }
}

fn generate_package_json(name: &str, framework: Framework) -> String {
    let (test_dep, test_dep_version) = match framework {
        Framework::LiteSVMWeb3js | Framework::QuasarVMWeb3js => ("@solana/web3.js", "^1.95.0"),
        Framework::LiteSVMKit | Framework::QuasarVMKit => ("@solana/kit", "^2.0.0"),
        _ => unreachable!(),
    };

    format!(
        r#"{{
  "name": "{name}-tests",
  "version": "0.1.0",
  "private": true,
  "scripts": {{
    "test": "npx ts-mocha -p ./tsconfig.json {name}.test.ts"
  }},
  "dependencies": {{
    "{test_dep}": "{test_dep_version}"
  }},
  "devDependencies": {{
    "@types/mocha": "^10.0.0",
    "chai": "^4.3.0",
    "mocha": "^10.0.0",
    "ts-mocha": "^10.0.0",
    "typescript": "^5.0.0"
  }}
}}
"#
    )
}

fn generate_test_ts(name: &str, framework: Framework) -> String {
    let import_line = match framework {
        Framework::LiteSVMWeb3js | Framework::QuasarVMWeb3js => {
            "import { Connection, PublicKey } from \"@solana/web3.js\";"
        }
        Framework::LiteSVMKit | Framework::QuasarVMKit => {
            "import { address } from \"@solana/kit\";"
        }
        _ => unreachable!(),
    };

    let module_name = name.replace('-', "_");

    format!(
        r#"{import_line}

describe("{module_name}", () => {{
  it("initializes", async () => {{
    // TODO: implement test
  }});
}});
"#
    )
}

// ---------------------------------------------------------------------------
// Static templates
// ---------------------------------------------------------------------------

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

const INSTRUCTION_INITIALIZE: &str = r#"use quasar::prelude::*;

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

const STATE_RS: &str = r#"use quasar::prelude::*;

#[account(discriminator = 1)]
pub struct MyAccount {
    pub authority: Address,
    pub value: u64,
}
"#;

const EVENTS_RS: &str = r#"use quasar::prelude::*;

#[event(discriminator = 0)]
pub struct InitializeEvent {
    pub authority: Address,
}
"#;

const TSCONFIG: &str = r#"{
  "compilerOptions": {
    "target": "es2020",
    "module": "commonjs",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "resolveJsonModule": true,
    "outDir": "./dist"
  },
  "include": ["*.test.ts"]
}
"#;

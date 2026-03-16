use {
    crate::{
        config::{GlobalConfig, GlobalDefaults},
        error::CliResult,
        toolchain,
    },
    dialoguer::{theme::ColorfulTheme, Input, Select},
    serde::Serialize,
    std::{fmt, fs, path::Path},
};

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

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
    QuasarSVMRust,
}

impl Framework {
    fn has_typescript(&self) -> bool {
        matches!(self, Framework::QuasarSVMWeb3js | Framework::QuasarSVMKit)
    }

    fn is_kit(&self) -> bool {
        matches!(self, Framework::QuasarSVMKit)
    }

    fn has_rust_tests(&self) -> bool {
        matches!(self, Framework::Mollusk | Framework::QuasarSVMRust)
    }
}

impl fmt::Display for Framework {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Framework::None => write!(f, "none"),
            Framework::Mollusk => write!(f, "mollusk"),
            Framework::QuasarSVMWeb3js => write!(f, "quasarsvm-web3js"),
            Framework::QuasarSVMKit => write!(f, "quasarsvm-kit"),
            Framework::QuasarSVMRust => write!(f, "quasarsvm-rust"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Template {
    Minimal,
    Full,
}

impl fmt::Display for Template {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Template::Minimal => write!(f, "minimal"),
            Template::Full => write!(f, "full"),
        }
    }
}

// ---------------------------------------------------------------------------
// Quasar.toml schema
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Banner — sparse blue aurora + FIGlet "Quasar" text reveal
// ---------------------------------------------------------------------------

fn print_banner() {
    use std::io::{self, IsTerminal, Write};

    let stdout = io::stdout();
    if !stdout.is_terminal() {
        println!("\n  Quasar\n  Build programs that execute at the speed of light\n");
        return;
    }

    use std::{thread, time::Duration};
    let mut out = stdout.lock();
    write!(out, "\x1b[?25l").ok();

    let w: usize = 70;
    let h: usize = 11; // 1 blank + 7 figlet + 1 blank + 1 tagline + 1 byline
    let n_total: usize = 8; // 6 aurora + 1 transition + 1 final

    // FIGlet "Quasar" — block style, 7 lines tall
    #[rustfmt::skip]
    let figlet: [&str; 7] = [
        " ██████╗ ██╗   ██╗ █████╗ ███████╗ █████╗ ██████╗ ",
        "██╔═══██╗██║   ██║██╔══██╗██╔════╝██╔══██╗██╔══██╗",
        "██║   ██║██║   ██║███████║███████╗███████║██████╔╝",
        "██║▄▄ ██║██║   ██║██╔══██║╚════██║██╔══██║██╔══██╗",
        "╚██████╔╝╚██████╔╝██║  ██║███████║██║  ██║██║  ██║",
        " ╚══▀▀═╝  ╚═════╝ ╚═╝  ╚═╝╚══════╝╚═╝  ╚═╝╚═╝  ╚═╝",
        "",
    ];
    let fig: Vec<Vec<char>> = figlet.iter().map(|l| l.chars().collect()).collect();
    let fig_w = fig.iter().map(|l| l.len()).max().unwrap_or(0);
    let fig_off = w.saturating_sub(fig_w) / 2;

    // Reserve space
    writeln!(out).ok();
    for _ in 0..h {
        writeln!(out).ok();
    }
    out.flush().ok();

    for frame in 0..n_total {
        write!(out, "\x1b[{h}A").ok();
        let is_final = frame == n_total - 1;
        let is_trans = frame == n_total - 2;
        let fade: f32 = if is_trans { 0.15 } else { 1.0 };

        #[allow(clippy::needless_range_loop)]
        for li in 0..h {
            write!(out, "\x1b[2K  ").ok();

            if is_final {
                // ── Final: gradient FIGlet text + tagline ──
                match li {
                    1..=7 => {
                        let row = &fig[li - 1];
                        for _ in 0..fig_off {
                            write!(out, " ").ok();
                        }
                        for &ch in row.iter() {
                            if ch != ' ' {
                                write!(out, "\x1b[36m{ch}\x1b[0m").ok();
                            } else {
                                write!(out, " ").ok();
                            }
                        }
                    }
                    9 => {
                        let tagline = "Build programs that execute at the speed of light";
                        let tag_off = w.saturating_sub(tagline.len()) / 2;
                        for _ in 0..tag_off {
                            write!(out, " ").ok();
                        }
                        write!(out, "\x1b[1m{tagline}\x1b[0m").ok();
                    }
                    10 => {
                        // "by " in grey + "blueshift.gg" in blue, centered
                        let byline_len = 15; // "by blueshift.gg"
                        let by_off = w.saturating_sub(byline_len) / 2;
                        for _ in 0..by_off {
                            write!(out, " ").ok();
                        }
                        write!(out, "\x1b[90mby \x1b[36mblueshift.gg\x1b[0m").ok();
                    }
                    _ => {}
                }
            } else {
                // ── Aurora frame (+ text overlay on transition) ──
                for ci in 0..w {
                    // FIGlet text overlay during transition
                    if is_trans && (1..=7).contains(&li) {
                        let tc = ci.wrapping_sub(fig_off);
                        if tc < fig_w {
                            let ch = fig[li - 1].get(tc).copied().unwrap_or(' ');
                            if ch != ' ' {
                                write!(out, "\x1b[36m{ch}\x1b[0m").ok();
                                continue;
                            }
                        }
                    }

                    // Aurora cell
                    let d = aurora_density(ci, li, frame) * fade;

                    if d < 0.10 {
                        write!(out, " ").ok();
                    } else if d < 0.25 {
                        write!(out, "\x1b[38;2;15;25;85m░\x1b[0m").ok();
                    } else if d < 0.42 {
                        write!(out, "\x1b[38;2;30;55;145m░\x1b[0m").ok();
                    } else if d < 0.60 {
                        write!(out, "\x1b[38;2;50;95;200m▒\x1b[0m").ok();
                    } else if d < 0.78 {
                        write!(out, "\x1b[38;2;75;140;235m▓\x1b[0m").ok();
                    } else {
                        write!(out, "\x1b[38;2;100;170;255m█\x1b[0m").ok();
                    }
                }
            }
            writeln!(out).ok();
        }
        out.flush().ok();

        if frame < n_total - 1 {
            thread::sleep(Duration::from_millis(if frame == 0 { 60 } else { 80 }));
        }
    }

    write!(out, "\x1b[?25h").ok();
    writeln!(out).ok();
    out.flush().ok();
}

/// Aurora density — sine waves flowing rightward, tuned for sparse output.
fn aurora_density(col: usize, line: usize, frame: usize) -> f32 {
    let c = col as f32;
    let l = line as f32;
    let f = frame as f32;

    let w1 = ((c - f * 5.0) / 8.0 + l * 0.35).sin();
    let w2 = ((c - f * 3.5) / 5.5 - l * 0.25).sin() * 0.45;
    let w3 = ((c - f * 7.0) / 12.0 + l * 0.15).sin() * 0.3;

    ((w1 + w2 + w3 + 1.5) / 3.5).clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------
// ANSI helpers (delegate to shared style module)
// ---------------------------------------------------------------------------

fn color(code: u8, s: &str) -> String {
    crate::style::color(code, s)
}

fn bold(s: &str) -> String {
    crate::style::bold(s)
}

fn dim(s: &str) -> String {
    crate::style::dim(s)
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn run(
    name: Option<String>,
    yes: bool,
    no_git: bool,
    framework_override: Option<String>,
    template_override: Option<String>,
    toolchain_override: Option<String>,
) -> CliResult {
    let globals = GlobalConfig::load();

    // Skip prompts when a name is provided (or --yes is set), or when explicit
    // flags given
    let skip_prompts = yes
        || name.is_some()
        || framework_override.is_some()
        || template_override.is_some()
        || toolchain_override.is_some();

    if globals.ui.animation {
        print_banner();
    }

    let theme = ColorfulTheme::default();

    // Project name
    let name: String = if skip_prompts {
        name.unwrap_or_else(|| {
            eprintln!("  {}", crate::style::fail("--yes requires a project name"));
            std::process::exit(1);
        })
    } else {
        let mut prompt = Input::with_theme(&theme).with_prompt("Project name");
        if let Some(default) = name {
            prompt = prompt.default(default);
        }
        prompt.interact_text().map_err(anyhow::Error::from)?
    };

    // When scaffolding into ".", derive the crate name from the current directory
    let crate_name = if name == "." {
        std::env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "my-program".to_string())
    } else {
        name.clone()
    };

    // Toolchain
    let toolchain_default = match toolchain_override
        .as_deref()
        .or(globals.defaults.toolchain.as_deref())
    {
        Some("upstream") => 1,
        _ => 0,
    };
    let toolchain_idx = if skip_prompts {
        toolchain_default
    } else {
        let toolchain_items = &[
            "solana    (cargo build-sbf)",
            "upstream  (cargo +nightly build-bpf)",
        ];
        Select::with_theme(&theme)
            .with_prompt("Toolchain")
            .items(toolchain_items)
            .default(toolchain_default)
            .interact()
            .map_err(anyhow::Error::from)?
    };
    let toolchain = match toolchain_idx {
        0 => Toolchain::Solana,
        _ => Toolchain::Upstream,
    };

    // For upstream: sbpf-linker must be installed
    if matches!(toolchain, Toolchain::Upstream) && !toolchain::has_sbpf_linker() {
        eprintln!();
        eprintln!("  {} sbpf-linker not found.", color(196, "\u{2718}"));
        eprintln!();
        eprintln!("  Install platform-tools first:");
        eprintln!(
            "    {}",
            bold("git clone https://github.com/anza-xyz/platform-tools")
        );
        eprintln!("    {}", bold("cd platform-tools"));
        eprintln!("    {}", bold("cargo install-with-gallery"));
        eprintln!();
        std::process::exit(1);
    }

    // Testing framework
    let framework_default = match framework_override
        .as_deref()
        .or(globals.defaults.framework.as_deref())
    {
        Some("mollusk") => 1,
        Some("quasarsvm-rust") => 2,
        Some("quasarsvm-web3js") => 3,
        Some("quasarsvm-kit") => 4,
        Some("none") => 0,
        _ => 2,
    };
    let framework_idx = if skip_prompts {
        framework_default
    } else {
        let framework_items = &[
            "None",
            "Mollusk",
            "QuasarSVM/Rust",
            "QuasarSVM/Web3.js",
            "QuasarSVM/Kit",
        ];
        Select::with_theme(&theme)
            .with_prompt("Testing framework")
            .items(framework_items)
            .default(framework_default)
            .interact()
            .map_err(anyhow::Error::from)?
    };
    let framework = match framework_idx {
        0 => Framework::None,
        1 => Framework::Mollusk,
        2 => Framework::QuasarSVMRust,
        3 => Framework::QuasarSVMWeb3js,
        _ => Framework::QuasarSVMKit,
    };

    // Template
    let template_default = match template_override
        .as_deref()
        .or(globals.defaults.template.as_deref())
    {
        Some("full") => 1,
        _ => 0,
    };
    let template_idx = if skip_prompts {
        template_default
    } else {
        let template_items = &[
            "Minimal (instruction file only)",
            "Full (state, events, and instruction files)",
        ];
        Select::with_theme(&theme)
            .with_prompt("Template")
            .items(template_items)
            .default(template_default)
            .interact()
            .map_err(anyhow::Error::from)?
    };
    let template = match template_idx {
        0 => Template::Minimal,
        _ => Template::Full,
    };

    if skip_prompts {
        println!();
        println!(
            "  {} {} {} {} {}",
            dim("Using:"),
            bold(&toolchain.to_string()),
            dim("+"),
            bold(&framework.to_string()),
            bold(&template.to_string()),
        );
    }

    scaffold(&name, &crate_name, toolchain, framework, template)?;

    // git init (unless --no-git or already in a git repo)
    if !no_git {
        let root = Path::new(&name);
        let already_git = if name == "." {
            Path::new(".git").exists()
        } else {
            root.join(".git").exists()
        };
        if !already_git {
            let _ = std::process::Command::new("git")
                .args(["init", "--quiet"])
                .current_dir(root)
                .status();
        }
    }

    // Save preferences for next time
    let new_globals = GlobalConfig {
        defaults: GlobalDefaults {
            toolchain: Some(toolchain.to_string()),
            framework: Some(framework.to_string()),
            template: Some(template.to_string()),
        },
        ui: globals.ui,
    };
    let _ = new_globals.save(); // best-effort

    // Success message
    println!();
    println!(
        "  {}  Created {} {}",
        color(83, "\u{2714}"),
        bold(&crate_name),
        dim("project")
    );
    println!();
    println!("  {}", dim("Next steps:"));
    if name != "." {
        println!(
            "    {}  {}",
            color(45, "\u{276f}"),
            bold(&format!("cd {name}"))
        );
    }
    println!("    {}  {}", color(45, "\u{276f}"), bold("quasar build"));
    if framework.has_rust_tests() || framework.has_typescript() {
        println!("    {}  {}", color(45, "\u{276f}"), bold("quasar test"));
    }
    println!();
    println!(
        "  {} saved to {}",
        dim("Preferences"),
        dim(&GlobalConfig::path().display().to_string()),
    );
    println!();

    Ok(())
}

fn scaffold(
    dir: &str,
    name: &str,
    toolchain: Toolchain,
    framework: Framework,
    template: Template,
) -> CliResult {
    let root = Path::new(dir);

    if dir == "." {
        // Scaffold into current directory — check it doesn't already have a project
        if root.join("Cargo.toml").exists() || root.join("Quasar.toml").exists() {
            eprintln!(
                "  {}",
                crate::style::fail("current directory already contains a project")
            );
            std::process::exit(1);
        }
    } else if root.exists() {
        eprintln!(
            "  {}",
            crate::style::fail(&format!("directory '{dir}' already exists"))
        );
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
            generate_test_ts(name, framework, toolchain),
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
        Framework::QuasarSVMRust => {
            out.push_str(&format!(
                r#"
[dev-dependencies]
{client_dep}quasar-svm = {{ git = "https://github.com/blueshift-gg/quasar-svm" }}
solana-account = {{ version = "3.4.0" }}
solana-address = {{ version = "2.2.0", features = ["decode"] }}
solana-instruction = {{ version = "3.2.0", features = ["bincode"] }}
solana-pubkey = {{ version = "4.1.0" }}
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

fn generate_test_ts(name: &str, framework: Framework, toolchain: Toolchain) -> String {
    let module_name = name.replace('-', "_");
    let class_name = snake_to_pascal(&module_name);
    let so_name = match toolchain {
        Toolchain::Upstream => format!("lib{module_name}"),
        Toolchain::Solana => module_name.clone(),
    };

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
    .addProgram(PROGRAM_ADDRESS, await readFile("target/deploy/{so_name}.so"))

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

    assert.isTrue(result.status.ok, `initialize failed:\n${{result.logs.join("\n")}}`);
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
    .addProgram({class_name}Client.programId, await readFile("target/deploy/{so_name}.so"));

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

    assert.isTrue(result.status.ok, `initialize failed:\n${{result.logs.join("\n")}}`);
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
        (Framework::QuasarSVMRust, Template::Minimal | Template::Full) => {
            format!(
                r#"extern crate std;

use quasar_svm::{{Account, ExecutionStatus, Instruction, Pubkey, QuasarSvm}};
use solana_address::Address;

use {client_crate}::InitializeInstruction;

fn setup() -> QuasarSvm {{
    let elf = include_bytes!("../target/deploy/{libname}.so");
    QuasarSvm::new()
        .with_program(&Pubkey::from(crate::ID), elf)
}}

#[test]
fn test_initialize() {{
    let mut svm = setup();

    let payer = Pubkey::new_unique();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitializeInstruction {{
        payer: Address::from(payer.to_bytes()),
        system_program: Address::from(system_program.to_bytes()),
    }}
    .into();

    let result = svm.process_transaction(
        &[instruction],
        &[(payer, Account::new(10_000_000_000, 0, &system_program))],
    );

    match result.status() {{
        ExecutionStatus::Success => {{}},
        ExecutionStatus::Err(e) => panic!("initialize failed: {{e}}\n{{:?}}", result.logs),
    }}
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
"--cfg", "target_os=\"solana\"",
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

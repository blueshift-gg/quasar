use {
    clap::{ArgAction, Args, CommandFactory, Parser, Subcommand},
    std::path::PathBuf,
};

pub mod build;
pub mod cfg;
pub mod clean;
pub mod client;
pub mod config;
pub mod deploy;
pub mod dump;
pub mod error;
pub mod idl;
pub mod init;
pub mod keys;
pub mod lint;
pub mod new;
pub mod style;
pub mod test;
pub mod toolchain;
pub mod utils;
pub use error::CliResult;

#[derive(Parser, Debug)]
#[command(
    name = "quasar",
    version,
    about = "Build programs that execute at the speed of light",
    disable_help_subcommand = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Scaffold a new Quasar project
    Init(InitCommand),
    /// Add instructions, state, and errors to the project
    Add(AddCommand),
    /// Compile the on-chain program
    Build(BuildCommand),
    /// Run the test suite
    Test(TestCommand),
    /// Deploy the program to a cluster
    Deploy(DeployCommand),
    /// Remove build artifacts
    Clean(CleanCommand),
    /// Manage global settings
    Config(ConfigCommand),
    /// Generate the IDL for a program crate
    Idl(IdlCommand),
    /// Generate client code from the program IDL
    Client(ClientCommand),
    /// Measure compute-unit usage
    Profile(ProfileCommand),
    /// Run the account relationship linter
    Lint(LintCommand),
    /// Dump sBPF assembly
    Dump(DumpCommand),
    /// Manage program keypair
    Keys(KeysCommand),
    /// Generate shell completions
    Completions(CompletionsCommand),
}

// ---------------------------------------------------------------------------
// Command args
// ---------------------------------------------------------------------------

#[derive(Args, Debug, Default)]
pub struct InitCommand {
    /// Project name — skips the interactive name prompt
    #[arg(value_name = "NAME")]
    pub name: Option<String>,

    /// Skip prompts and use saved defaults
    #[arg(long, short, action = ArgAction::SetTrue)]
    pub yes: bool,

    /// Skip git init and the initial commit
    #[arg(long, action = ArgAction::SetTrue)]
    pub no_git: bool,

    /// Test language (none, rust, typescript)
    #[arg(long)]
    pub test_language: Option<String>,

    /// Rust test framework (quasar-svm, mollusk)
    #[arg(long)]
    pub rust_framework: Option<String>,

    /// TypeScript SDK (kit, web3.js)
    #[arg(long)]
    pub ts_sdk: Option<String>,

    /// Project template (minimal, full)
    #[arg(long)]
    pub template: Option<String>,

    /// Toolchain (solana, upstream)
    #[arg(long)]
    pub toolchain: Option<String>,
}

#[derive(Args, Debug)]
pub struct AddCommand {
    /// Add a new instruction handler
    #[arg(short, long, value_name = "NAME")]
    pub instruction: Option<String>,

    /// Add a new state account
    #[arg(short, long, value_name = "NAME")]
    pub state: Option<String>,

    /// Add a new error enum
    #[arg(short, long, value_name = "NAME")]
    pub error: Option<String>,
}

#[derive(Args, Debug, Default)]
pub struct BuildCommand {
    /// Emit debug symbols (required for profiling)
    #[arg(long, action = ArgAction::SetTrue)]
    pub debug: bool,

    /// Stream the underlying build command output directly
    #[arg(long, action = ArgAction::SetTrue)]
    pub verbose: bool,

    /// Watch src/ for changes and rebuild automatically
    #[arg(long, short, action = ArgAction::SetTrue)]
    pub watch: bool,

    /// Cargo features to enable (comma-separated or repeated)
    #[arg(long, value_name = "FEATURES")]
    pub features: Option<String>,

    /// Run the account relationship linter
    #[arg(long, action = ArgAction::SetTrue)]
    pub lint: bool,
}

#[derive(Args, Debug, Default)]
pub struct LintCommand {
    /// Apply auto-fixes for missing constraints
    #[arg(long, action = ArgAction::SetTrue)]
    pub fix: bool,

    /// Output the account graph (ascii, mermaid, dot, json)
    #[arg(long, value_name = "FORMAT")]
    pub graph: Option<String>,
}

#[derive(Args, Debug, Default)]
pub struct TestCommand {
    /// Build with debug symbols before testing
    #[arg(long, action = ArgAction::SetTrue)]
    pub debug: bool,

    /// Forward `--show-output` to `cargo test`
    #[arg(long, action = ArgAction::SetTrue)]
    pub show_output: bool,

    /// Only run tests whose name matches PATTERN
    #[arg(long, short, value_name = "PATTERN")]
    pub filter: Option<String>,

    /// Watch src/ for changes and re-run tests automatically
    #[arg(long, short, action = ArgAction::SetTrue)]
    pub watch: bool,

    /// Skip the build step (use existing binary)
    #[arg(long, action = ArgAction::SetTrue)]
    pub no_build: bool,

    /// Cargo features to enable (comma-separated or repeated)
    #[arg(long, value_name = "FEATURES")]
    pub features: Option<String>,
}

#[derive(Args, Debug, Default)]
pub struct DeployCommand {
    /// Path to a program keypair (default: target/deploy/<name>-keypair.json)
    #[arg(long, value_name = "KEYPAIR")]
    pub program_keypair: Option<PathBuf>,

    /// Upgrade authority keypair (default: Solana CLI default keypair)
    #[arg(long, value_name = "KEYPAIR")]
    pub upgrade_authority: Option<PathBuf>,

    /// Payer keypair (default: Solana CLI default keypair)
    #[arg(long, short, value_name = "KEYPAIR")]
    pub keypair: Option<PathBuf>,

    /// Cluster URL (default: Solana CLI configured cluster)
    #[arg(long, short, value_name = "URL")]
    pub url: Option<String>,

    /// Skip the build step
    #[arg(long, action = ArgAction::SetTrue)]
    pub skip_build: bool,
}

#[derive(Args, Debug, Default)]
pub struct CleanCommand {
    /// Also run cargo clean (removes all build artifacts)
    #[arg(long, short, action = ArgAction::SetTrue)]
    pub all: bool,
}

#[derive(Args, Debug)]
pub struct ConfigCommand {
    #[command(subcommand)]
    pub action: Option<ConfigAction>,
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Read a single config value
    Get {
        /// Config key (e.g. ui.animation, defaults.toolchain, defaults.git)
        #[arg(value_name = "KEY")]
        key: String,
    },
    /// Write a config value
    Set {
        /// Config key
        #[arg(value_name = "KEY")]
        key: String,
        /// New value
        #[arg(value_name = "VALUE")]
        value: String,
    },
    /// Print every config value
    List,
    /// Restore factory defaults
    Reset,
}

#[derive(Args, Debug)]
pub struct IdlCommand {
    /// Path to the program crate directory
    #[arg(value_name = "PATH")]
    pub crate_path: PathBuf,
}

#[derive(Args, Debug)]
pub struct ClientCommand {
    /// Path to an IDL JSON file (e.g. target/idl/my_program.json)
    #[arg(value_name = "IDL")]
    pub idl_path: PathBuf,

    /// Languages to generate (default: all). Comma-separated.
    /// Options: typescript, python, golang
    #[arg(long, value_delimiter = ',', value_name = "LANG")]
    pub lang: Vec<String>,
}

#[derive(Args, Debug, Clone)]
pub struct DumpCommand {
    /// Path to a compiled .so (auto-detected from target/deploy/ if omitted)
    #[arg(value_name = "ELF")]
    pub elf_path: Option<PathBuf>,

    /// Disassemble only this symbol (demangled name)
    #[arg(long, short, value_name = "SYMBOL")]
    pub function: Option<String>,

    /// Interleave source code (requires debug build)
    #[arg(long, short = 'S', action = ArgAction::SetTrue)]
    pub source: bool,
}

#[derive(Args, Debug, Clone)]
pub struct ProfileCommand {
    /// Path to a compiled .so (auto-detected from target/deploy/ if omitted)
    #[arg(value_name = "ELF")]
    pub elf_path: Option<PathBuf>,

    /// Compare CU cost against an on-chain program by name
    #[arg(long = "diff", value_name = "PROGRAM", conflicts_with = "elf_path")]
    pub diff_program: Option<String>,

    /// Upload the profile result and get a shareable link
    #[arg(long, action = ArgAction::SetTrue, conflicts_with = "diff_program")]
    pub share: bool,

    /// Show full terminal output with all functions
    #[arg(long, action = ArgAction::SetTrue)]
    pub expand: bool,

    /// Watch src/ for changes and re-profile automatically
    #[arg(long, short, action = ArgAction::SetTrue)]
    pub watch: bool,
}

#[derive(Args, Debug)]
pub struct KeysCommand {
    #[command(subcommand)]
    pub action: KeysAction,
}

#[derive(Subcommand, Debug)]
pub enum KeysAction {
    /// Print the program ID from the keypair file
    List,
    /// Update declare_id!() to match the keypair
    Sync,
    /// Generate a new program keypair
    New {
        /// Overwrite existing keypair
        #[arg(long, action = ArgAction::SetTrue)]
        force: bool,
    },
}

#[derive(Args, Debug)]
pub struct CompletionsCommand {
    /// Shell to generate completions for
    #[arg(value_enum)]
    pub shell: clap_complete::Shell,
}

// ---------------------------------------------------------------------------
// Run
// ---------------------------------------------------------------------------

pub fn run(cli: Cli) -> CliResult {
    match cli.command {
        Command::Init(cmd) => init::run(cmd),
        Command::Add(cmd) => {
            if cmd.instruction.is_none() && cmd.state.is_none() && cmd.error.is_none() {
                return Err(error::CliError::message(
                    "specify at least one of -i/--instruction, -s/--state, or -e/--error",
                ));
            }
            if let Some(name) = cmd.instruction {
                new::run_instruction(&name)?;
            }
            if let Some(name) = cmd.state {
                new::run_state(&name)?;
            }
            if let Some(name) = cmd.error {
                new::run_error(&name)?;
            }
            Ok(())
        }
        Command::Build(cmd) => {
            build::run(cmd.debug, cmd.verbose, cmd.watch, cmd.features, cmd.lint)
        }
        Command::Test(cmd) => test::run(
            cmd.debug,
            cmd.show_output,
            cmd.filter,
            cmd.watch,
            cmd.no_build,
            cmd.features,
        ),
        Command::Deploy(cmd) => deploy::run(
            cmd.program_keypair,
            cmd.upgrade_authority,
            cmd.keypair,
            cmd.url,
            cmd.skip_build,
        ),
        Command::Clean(cmd) => clean::run(cmd.all),
        Command::Config(cmd) => cfg::run(cmd.action),
        Command::Idl(cmd) => idl::run(cmd),
        Command::Client(cmd) => client::run(cmd),
        Command::Lint(cmd) => lint::run(cmd),
        Command::Dump(cmd) => dump::run(cmd.elf_path, cmd.function, cmd.source),
        Command::Completions(cmd) => {
            clap_complete::generate(
                cmd.shell,
                &mut Cli::command(),
                "quasar",
                &mut std::io::stdout(),
            );
            Ok(())
        }
        Command::Keys(cmd) => match cmd.action {
            KeysAction::List => keys::list(),
            KeysAction::Sync => keys::sync(),
            KeysAction::New { force } => keys::new(force),
        },
        Command::Profile(cmd) => {
            if cmd.watch {
                return profile_watch(cmd.expand);
            }

            let elf_path = if let Some(path) = cmd.elf_path {
                path
            } else if cmd.diff_program.is_none() {
                // Auto-build with debug symbols for profiling
                build::profile_build()?
            } else {
                // --diff mode doesn't need an ELF
                std::path::PathBuf::new()
            };

            quasar_profile::run(quasar_profile::ProfileCommand {
                elf_path: if elf_path.as_os_str().is_empty() {
                    None
                } else {
                    Some(elf_path)
                },
                diff_program: cmd.diff_program,
                share: cmd.share,
                expand: cmd.expand,
            });
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// Custom help — shown for `quasar`, `quasar -h`, `quasar --help`, `quasar help`
// ---------------------------------------------------------------------------

pub fn print_help() {
    let v = env!("CARGO_PKG_VERSION");

    println!();
    println!(
        "  {} {}",
        style::bold("quasar"),
        style::dim(&format!("v{v}"))
    );
    println!(
        "  {}",
        style::dim("Build programs that execute at the speed of light")
    );
    println!();
    println!("  {}", style::bold("Commands:"));
    print_cmd(
        "init    [name] [-y] [--no-git] [--template]",
        "Scaffold a new project",
    );
    print_cmd(
        "add     [-i name] [-s name] [-e name]",
        "Add instructions, state, errors",
    );
    print_cmd(
        "build   [--debug] [--verbose] [-w] [--features]",
        "Compile the on-chain program",
    );
    print_cmd(
        "test    [--debug] [--show-output] [-f] [-w] [--features]",
        "Run the test suite",
    );
    print_cmd(
        "deploy  [-u url] [-k keypair] [--skip-build]",
        "Deploy to a cluster",
    );
    print_cmd("clean   [-a]", "Remove build artifacts");
    print_cmd("config  [get|set|list|reset]", "Manage global settings");
    print_cmd("idl     <path>", "Generate the program IDL");
    print_cmd(
        "client  <idl> [--lang ts,py,go]",
        "Generate client code from IDL",
    );
    print_cmd(
        "lint    [--fix] [--graph FORMAT]",
        "Check account relationships",
    );
    print_cmd(
        "profile [elf] [--expand] [--diff] [-w]",
        "Measure compute-unit usage",
    );
    print_cmd("keys    [list|sync|new]", "Manage program keypair");
    print_cmd("dump    [elf] [-f] [-S]", "Dump sBPF assembly");
    println!();
    println!("  {}", style::bold("Options:"));
    print_cmd("-h, --help", "Print help");
    print_cmd("-V, --version", "Print version");
    println!();
    println!(
        "  Run {} for details on any command.",
        style::bold("quasar <command> --help")
    );
    println!();
}

fn print_cmd(cmd: &str, desc: &str) {
    println!("    {}  {}", style::color(45, &format!("{cmd:<34}")), desc);
}

fn profile_watch(expand: bool) -> CliResult {
    build::watch_loop(|| {
        let elf = build::profile_build()?;
        quasar_profile::run(quasar_profile::ProfileCommand {
            elf_path: Some(elf),
            diff_program: None,
            share: false,
            expand,
        });
        Ok(())
    })
}

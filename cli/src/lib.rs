use {
    clap::{ArgAction, Args, CommandFactory, Parser, Subcommand},
    std::path::PathBuf,
};

mod build;
mod cfg;
mod clean;
mod client;
mod config;
mod deploy;
mod error;
pub mod idl;
mod init;
mod inspect_asm;
mod inspect_validation;
mod keys;
mod lint;
mod output;
mod profile;
mod program_keypair;
mod style;
mod test;
mod toolchain;
mod utils;
mod verify;
use error::CliResult;

#[derive(Parser, Debug)]
#[command(
    name = "quasar",
    version,
    about = "Build programs that execute at the speed of light",
    disable_help_subcommand = true
)]
struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Scaffold a new Quasar project
    Init(InitCommand),
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
    /// Audit the program surface for pre-deploy and upgrade-safety issues
    Lint(LintCommand),
    /// Verify local artifacts against a deployed program
    Verify(VerifyCommand),
    /// Measure compute-unit usage
    Profile(ProfileCommand),
    /// Preview inspection tools
    Inspect(InspectCommand),
    /// Manage program keypair
    Keys(KeysCommand),
    /// Generate shell completions
    Completions(CompletionsCommand),
}

#[derive(Args, Debug, Default)]
struct InitCommand {
    /// Project name
    #[arg(value_name = "NAME")]
    pub name: String,

    /// Skip git init and the initial commit
    #[arg(long, action = ArgAction::SetTrue)]
    pub no_git: bool,

    /// Show each scaffold step as it runs
    #[arg(long, action = ArgAction::SetTrue)]
    pub verbose: bool,
}

#[derive(Args, Debug, Default)]
struct BuildCommand {
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
}

#[derive(Args, Debug, Default)]
struct TestCommand {
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

    /// Show build/test commands as they run
    #[arg(long, action = ArgAction::SetTrue)]
    pub verbose: bool,
}

#[derive(Args, Debug, Default)]
struct DeployCommand {
    /// Path to a program keypair (default: `target/deploy/<name>-keypair.json`)
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

    /// Skip the post-deploy byte and authority verification
    #[arg(long, action = ArgAction::SetTrue)]
    pub skip_verify: bool,
}

#[derive(Args, Debug, Default)]
struct VerifyCommand {
    /// Program address (defaults to the program keypair address)
    #[arg(long, value_name = "ADDRESS", conflicts_with = "program_keypair")]
    pub program_id: Option<String>,

    /// Path to the program keypair (default:
    /// `target/deploy/<name>-keypair.json`)
    #[arg(long, value_name = "KEYPAIR")]
    pub program_keypair: Option<PathBuf>,

    /// Expected upgrade authority keypair
    #[arg(long, value_name = "KEYPAIR")]
    pub upgrade_authority: Option<PathBuf>,

    /// Cluster URL (default: Solana CLI configured cluster)
    #[arg(long, short, value_name = "URL")]
    pub url: Option<String>,

    /// Local ELF to compare (default: `target/deploy/<name>.so`)
    #[arg(long, value_name = "ELF")]
    pub elf_path: Option<PathBuf>,

    /// Deployment manifest to validate (auto-detected when omitted)
    #[arg(long, value_name = "MANIFEST")]
    pub manifest: Option<PathBuf>,
}

#[derive(Args, Debug, Default)]
struct CleanCommand {
    /// Also run cargo clean (removes all build artifacts)
    #[arg(long, short, action = ArgAction::SetTrue)]
    pub all: bool,
}

#[derive(Args, Debug)]
struct ConfigCommand {
    #[command(subcommand)]
    pub action: Option<ConfigAction>,
}

#[derive(Subcommand, Debug)]
enum ConfigAction {
    /// Read a single config value
    Get {
        /// Config key (currently ui.color)
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
#[command(args_conflicts_with_subcommands = true)]
struct IdlCommand {
    /// Path to the program crate directory (generate IDL + Rust client)
    #[arg(value_name = "PATH")]
    pub crate_path: Option<PathBuf>,

    #[command(subcommand)]
    pub action: Option<IdlAction>,
}

#[derive(Subcommand, Debug)]
enum IdlAction {
    /// Verify Quasar-producer integrity and ABI hashes against `hashes`.
    Verify {
        /// Path to an IDL JSON file (e.g. target/idl/my_program.json)
        #[arg(value_name = "IDL")]
        idl_path: PathBuf,
    },
}

#[derive(Args, Debug)]
struct ClientCommand {
    /// Path to an IDL JSON file (e.g. target/idl/my_program.json)
    #[arg(value_name = "IDL")]
    pub idl_path: PathBuf,

    /// Languages to generate (default: all). Comma-separated.
    /// Options: typescript, python, golang
    #[arg(long, value_delimiter = ',', value_name = "LANG")]
    pub lang: Vec<String>,
}

#[derive(Args, Debug, Default)]
struct LintCommand {
    /// Write the current program surface to quasar.lock.json
    #[arg(long, action = ArgAction::SetTrue)]
    pub update_lock: bool,

    /// Do not compare against quasar.lock.json even when it exists
    #[arg(long, action = ArgAction::SetTrue)]
    pub no_diff: bool,

    /// Treat warnings and info findings as failures
    #[arg(long, action = ArgAction::SetTrue)]
    pub strict: bool,
}

#[derive(Args, Debug)]
struct InspectCommand {
    #[command(subcommand)]
    pub action: InspectAction,
}

#[derive(Subcommand, Debug)]
enum InspectAction {
    /// Preview: print the compiler's resolved validation plan
    Validation(ValidationInspectCommand),
    /// Preview: dump sBPF assembly
    Asm(AsmInspectCommand),
}

#[derive(Args, Debug, Default)]
struct ValidationInspectCommand {
    /// Generated IDL JSON (defaults to the current project's target/idl output)
    #[arg(value_name = "IDL")]
    pub idl_path: Option<PathBuf>,

    /// Print the validation plan as JSON
    #[arg(long, action = ArgAction::SetTrue)]
    pub json: bool,
}

#[derive(Args, Debug, Clone)]
struct AsmInspectCommand {
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
struct ProfileCommand {
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

    /// Budget file used by --write-budget or --assert-budget
    #[arg(long, value_name = "FILE", default_value = "quasar-budget.toml")]
    pub budget: PathBuf,

    /// Write current ceilings to the budget file
    #[arg(
        long,
        action = ArgAction::SetTrue,
        conflicts_with_all = ["assert_budget", "diff_program", "share", "watch"]
    )]
    pub write_budget: bool,

    /// Fail with exit code 2 when a budget ceiling is exceeded
    #[arg(
        long,
        action = ArgAction::SetTrue,
        conflicts_with_all = ["write_budget", "diff_program", "share", "watch"]
    )]
    pub assert_budget: bool,

    /// Percentage added to ceilings written by --write-budget
    #[arg(long, default_value_t = 5, requires = "write_budget")]
    pub headroom: u32,

    /// Print deterministic machine-readable output and skip the flamegraph
    /// server
    #[arg(
        long,
        action = ArgAction::SetTrue,
        conflicts_with_all = ["diff_program", "share", "watch"]
    )]
    pub json: bool,
}

#[derive(Args, Debug)]
struct KeysCommand {
    #[command(subcommand)]
    pub action: KeysAction,
}

#[derive(Subcommand, Debug)]
enum KeysAction {
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
struct CompletionsCommand {
    /// Shell to generate completions for
    #[arg(value_enum)]
    pub shell: clap_complete::Shell,
}

/// Run the Quasar executable.
///
/// This is the only supported Rust entrypoint for `quasar-cli`; command
/// modules and parser types are implementation details.
pub fn entrypoint() {
    style::init(true);
    let globals = config::GlobalConfig::load().unwrap_or_default();
    style::init(globals.ui.color);

    let args: Vec<String> = std::env::args().collect();
    if args.len() == 1 || (args.len() == 2 && matches!(args[1].as_str(), "--help" | "-h" | "help"))
    {
        print_help();
        return;
    }

    let cli = Cli::parse();
    if let Err(error) = run(cli) {
        eprintln!("\n  {} {error}", style::fail(""));
        std::process::exit(error.exit_code());
    }
}

fn run(cli: Cli) -> CliResult {
    match cli.command {
        Command::Init(cmd) => init::run(cmd),
        Command::Build(cmd) => build::run(cmd.debug, cmd.verbose, cmd.watch, cmd.features),
        Command::Test(cmd) => test::run(
            cmd.debug,
            cmd.show_output,
            cmd.filter,
            cmd.watch,
            cmd.no_build,
            cmd.features,
            cmd.verbose,
        ),
        Command::Deploy(cmd) => deploy::run_with_verification(
            cmd.program_keypair,
            cmd.upgrade_authority,
            cmd.keypair,
            cmd.url,
            cmd.skip_build,
            cmd.skip_verify,
        ),
        Command::Clean(cmd) => clean::run(cmd.all),
        Command::Config(cmd) => cfg::run(cmd.action),
        Command::Idl(cmd) => idl::run(cmd),
        Command::Client(cmd) => client::run(cmd),
        Command::Lint(cmd) => lint::run(cmd),
        Command::Verify(cmd) => verify::run(cmd),
        Command::Inspect(cmd) => match cmd.action {
            InspectAction::Validation(command) => inspect_validation::run(command),
            InspectAction::Asm(command) => {
                inspect_asm::run(command.elf_path, command.function, command.source)
            }
        },
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

            profile::run(profile::ProfileCommand {
                elf_path: if elf_path.as_os_str().is_empty() {
                    None
                } else {
                    Some(elf_path)
                },
                diff_program: cmd.diff_program,
                share: cmd.share,
                expand: cmd.expand,
                budget_path: cmd.budget,
                write_budget: cmd.write_budget,
                assert_budget: cmd.assert_budget,
                headroom_percent: cmd.headroom,
                json: cmd.json,
            });
            Ok(())
        }
    }
}

/// Print the custom top-level help shown for `quasar`, `quasar -h`,
/// `quasar --help`, and `quasar help`.
fn print_help() {
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
    println!("  {}", style::bold("Core commands:"));
    print_cmd(
        "init    <name> [--no-git] [--verbose]",
        "Scaffold the canonical starter",
    );
    print_cmd(
        "build   [--debug] [--verbose] [-w] [--features]",
        "Compile the on-chain program",
    );
    print_cmd(
        "test    [--debug] [--show-output] [-f] [-w] [--features] [--verbose]",
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
    print_cmd("lint    [--update-lock] [--strict]", "Check release safety");
    print_cmd(
        "verify  [--program-id] [--manifest]",
        "Verify a deployed program",
    );
    print_cmd(
        "profile [elf] [--write-budget|--assert-budget] [--json]",
        "Measure compute-unit usage",
    );
    print_cmd("keys    [list|sync|new]", "Manage program keypair");
    println!();
    println!("  {}", style::bold("Preview tools:"));
    print_cmd(
        "inspect validation [idl] [--json]",
        "Show compiler validation plans",
    );
    print_cmd("inspect asm [elf] [-f] [-S]", "Dump sBPF assembly");
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
        profile::run(profile::ProfileCommand {
            elf_path: Some(elf),
            diff_program: None,
            share: false,
            expand,
            budget_path: PathBuf::from("quasar-budget.toml"),
            write_budget: false,
            assert_budget: false,
            headroom_percent: 5,
            json: false,
        });
        Ok(())
    })
}

#[cfg(test)]
mod profile_cli_tests {
    use {
        super::{Cli, Command},
        clap::Parser,
        std::path::PathBuf,
    };

    #[test]
    fn profile_budget_flags_have_safe_defaults() {
        let cli = Cli::try_parse_from(["quasar", "profile", "program.so"])
            .expect("plain profile command");
        let Command::Profile(profile) = cli.command else {
            panic!("expected profile command");
        };
        assert_eq!(profile.budget, PathBuf::from("quasar-budget.toml"));
        assert_eq!(profile.headroom, 5);
        assert!(!profile.write_budget);
        assert!(!profile.assert_budget);
    }

    #[test]
    fn profile_rejects_conflicting_budget_modes() {
        let error = Cli::try_parse_from([
            "quasar",
            "profile",
            "program.so",
            "--write-budget",
            "--assert-budget",
        ])
        .expect_err("budget modes must conflict");
        assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
    }
}

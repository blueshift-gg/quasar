use {
    clap::{ArgAction, Args, Parser, Subcommand},
    std::path::PathBuf,
};

pub mod build;
pub mod cfg;
pub mod clean;
pub mod config;
pub mod dump;
pub mod error;
pub mod idl;
pub mod init;
pub mod style;
pub mod test;
pub mod toolchain;
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
    /// Measure compute-unit usage
    Profile(ProfileCommand),
    /// Dump sBPF assembly
    Dump(DumpCommand),
}

// ---------------------------------------------------------------------------
// Command args
// ---------------------------------------------------------------------------

#[derive(Args, Debug, Default)]
pub struct InitCommand {
    /// Project name — skips the interactive name prompt
    #[arg(value_name = "NAME")]
    pub name: Option<String>,
}

#[derive(Args, Debug, Default)]
pub struct BuildCommand {
    /// Emit debug symbols (required for profiling)
    #[arg(long, action = ArgAction::SetTrue)]
    pub debug: bool,

    /// Watch src/ for changes and rebuild automatically
    #[arg(long, short, action = ArgAction::SetTrue)]
    pub watch: bool,
}

#[derive(Args, Debug, Default)]
pub struct TestCommand {
    /// Build with debug symbols before testing
    #[arg(long, action = ArgAction::SetTrue)]
    pub debug: bool,

    /// Only run tests whose name matches PATTERN
    #[arg(long, short, value_name = "PATTERN")]
    pub filter: Option<String>,
}

#[derive(Args, Debug, Default)]
pub struct DeployCommand {}

#[derive(Args, Debug, Default)]
pub struct CleanCommand {}

#[derive(Args, Debug)]
pub struct ConfigCommand {
    #[command(subcommand)]
    pub action: Option<ConfigAction>,
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Read a single config value
    Get {
        /// Config key (e.g. ui.animation, defaults.toolchain)
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
}

// ---------------------------------------------------------------------------
// Run
// ---------------------------------------------------------------------------

pub fn run(cli: Cli) -> CliResult {
    match cli.command {
        Command::Init(cmd) => init::run(cmd.name),
        Command::Build(cmd) => build::run(cmd.debug, cmd.watch),
        Command::Test(cmd) => test::run(cmd.debug, cmd.filter),
        Command::Deploy(_) => todo!(),
        Command::Clean(_) => clean::run(),
        Command::Config(cmd) => cfg::run(cmd.action),
        Command::Idl(cmd) => idl::run(cmd),
        Command::Dump(cmd) => dump::run(cmd.elf_path, cmd.function, cmd.source),
        Command::Profile(cmd) => {
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
    print_cmd("init   [name]", "Scaffold a new project");
    print_cmd("build  [--debug] [--watch]", "Compile the on-chain program");
    print_cmd("test   [--debug] [--filter]", "Run the test suite");
    print_cmd("deploy", "Deploy to a cluster");
    print_cmd("clean", "Remove build artifacts");
    print_cmd("config [get|set|list|reset]", "Manage global settings");
    print_cmd("idl    <path>", "Generate the program IDL");
    print_cmd(
        "profile [elf] [--expand] [--diff]",
        "Measure compute-unit usage",
    );
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

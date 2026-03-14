use {
    clap::{ArgAction, Args, Parser, Subcommand},
    std::path::PathBuf,
};

pub mod build;
pub mod config;
pub mod error;
pub mod idl;
pub mod init;
pub mod test;
pub use error::CliResult;

#[derive(Parser, Debug)]
#[command(
    name = "quasar",
    version,
    about = "A tool for building, testing, and profiling SBF programs"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    Init(InitCommand),
    Build(BuildCommand),
    Test(TestCommand),
    Deploy(DeployCommand),
    Idl(IdlCommand),
    Profile(ProfileCommand),
}

#[derive(Args, Debug, Clone)]
pub struct ProfileCommand {
    #[arg(value_name = "PATH_TO_ELF_SO")]
    pub elf_path: Option<PathBuf>,
    #[arg(long = "diff", value_name = "PROGRAM", conflicts_with = "elf_path")]
    pub diff_program: Option<String>,
    #[arg(long, action = ArgAction::SetTrue, conflicts_with = "diff_program")]
    pub share: bool,
}

#[derive(Args, Debug, Default)]
pub struct InitCommand {
    /// Project name (pre-fills the name prompt)
    #[arg(value_name = "NAME")]
    pub name: Option<String>,
}

#[derive(Args, Debug, Default)]
pub struct BuildCommand {
    /// Build in debug mode (unoptimized, with debug symbols needed for
    /// profiling)
    #[arg(long, action = ArgAction::SetTrue)]
    pub debug: bool,
}

#[derive(Args, Debug, Default)]
pub struct TestCommand {
    /// Build and test in debug mode (unoptimized, with debug symbols needed for
    /// profiling)
    #[arg(long, action = ArgAction::SetTrue)]
    pub debug: bool,
}

#[derive(Args, Debug, Default)]
pub struct DeployCommand {}

#[derive(Args, Debug)]
pub struct IdlCommand {
    /// Path to the Quasar program crate
    #[arg(value_name = "PATH")]
    pub crate_path: PathBuf,
}

pub fn run(cli: Cli) -> CliResult {
    match cli.command {
        Command::Profile(command) => {
            let elf_path = command.elf_path.or_else(resolve_default_profile_elf_path);

            quasar_profile::run(quasar_profile::ProfileCommand {
                elf_path,
                diff_program: command.diff_program,
                share: command.share,
            });

            Ok(())
        }
        Command::Idl(command) => idl::run(command),
        Command::Init(command) => init::run(command.name),
        Command::Build(command) => build::run(command.debug),
        Command::Test(command) => test::run(command.debug),
        Command::Deploy(_) => todo!(),
    }
}

fn resolve_default_profile_elf_path() -> Option<PathBuf> {
    let config = config::QuasarConfig::load().ok()?;
    let module_name = config.module_name();
    let project_name = config.project.name;

    [
        PathBuf::from("target")
            .join("deploy")
            .join(format!("{project_name}.so")),
        PathBuf::from("target")
            .join("deploy")
            .join(format!("{module_name}.so")),
        PathBuf::from("target")
            .join("deploy")
            .join(format!("lib{module_name}.so")),
    ]
    .into_iter()
    .find(|path| path.exists())
}

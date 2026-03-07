use std::path::PathBuf;

use clap::{ArgAction, Args, Parser, Subcommand};

pub mod error;
pub mod idl;
pub mod init;
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
    pub elf_path: PathBuf,
    #[arg(long, action = ArgAction::SetTrue)]
    pub share: bool,
}

#[derive(Args, Debug, Default)]
pub struct InitCommand {}

#[derive(Args, Debug, Default)]
pub struct BuildCommand {}

#[derive(Args, Debug, Default)]
pub struct TestCommand {}

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
            quasar_profile::run(quasar_profile::ProfileCommand {
                elf_path: command.elf_path,
                share: command.share,
            });

            Ok(())
        }
        Command::Idl(command) => idl::run(command),
        Command::Init(_) => init::run(),
        Command::Build(_) => todo!(),
        Command::Test(_) => todo!(),
        Command::Deploy(_) => todo!(),
    }
}

use std::path::PathBuf;

use clap::{ArgAction, Args, Parser, Subcommand};

pub mod error;
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
pub struct InitCommand {}

#[derive(Args, Debug, Default)]
pub struct BuildCommand {}

#[derive(Args, Debug, Default)]
pub struct TestCommand {}

#[derive(Args, Debug, Default)]
pub struct DeployCommand {}

pub fn run(cli: Cli) -> CliResult {
    match cli.command {
        Command::Profile(command) => {
            quasar_profile::run(quasar_profile::ProfileCommand {
                elf_path: command.elf_path,
                diff_program: command.diff_program,
                share: command.share,
            });

            Ok(())
        }
        Command::Init(_) => todo!(),
        Command::Build(_) => todo!(),
        Command::Test(_) => todo!(),
        Command::Deploy(_) => todo!(),
    }
}

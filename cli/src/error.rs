use thiserror::Error;

pub type CliResult = Result<(), CliError>;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("Error: path does not exist")]
    PathDoesNotExist,
    #[error("Io error")]
    IoError(#[from] std::io::Error),
    #[error("Toml parse error")]
    TomlParseError(#[from] toml::de::Error),
    #[error("Anyhow error")]
    Anyhow(#[from] anyhow::Error),
}

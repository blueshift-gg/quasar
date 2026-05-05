use thiserror::Error;

pub type CliResult = Result<(), CliError>;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("Io error")]
    IoError(#[from] std::io::Error),
    #[error("Toml parse error")]
    TomlParseError(#[from] toml::de::Error),
    #[error("Toml serialize error")]
    TomlSerError(#[from] toml::ser::Error),
    #[error("{0}")]
    Anyhow(#[from] anyhow::Error),
    #[error("{0}")]
    Message(String),
    #[error("{message}")]
    ProcessFailure { message: String, code: i32 },
}

impl CliError {
    pub fn message(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }

    pub fn process_failure(message: impl Into<String>, code: i32) -> Self {
        Self::ProcessFailure {
            message: message.into(),
            code,
        }
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            Self::ProcessFailure { code, .. } => *code,
            _ => 1,
        }
    }
}

use thiserror::Error;

pub type CliResult = Result<(), CliError>;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("TOML parse error: {0}")]
    TomlParseError(#[from] toml::de::Error),
    #[error("TOML serialize error: {0}")]
    TomlSerError(#[from] toml::ser::Error),
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("prompt error: {0}")]
    Dialoguer(#[from] dialoguer::Error),
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

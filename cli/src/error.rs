use {
    std::{
        io,
        path::{Path, PathBuf},
    },
    thiserror::Error,
};

pub type CliResult = Result<(), CliError>;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("failed to {action} {}: {source}", path.display())]
    IoPath {
        action: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),
    #[error("failed to parse TOML: {0}")]
    TomlParseError(#[from] toml::de::Error),
    #[error("failed to serialize TOML: {0}")]
    TomlSerError(#[from] toml::ser::Error),
    #[error("failed to parse {context}: {source}")]
    JsonParse {
        context: String,
        source: serde_json::Error,
    },
    #[error("failed to serialize {context}: {source}")]
    JsonSerialize {
        context: &'static str,
        source: serde_json::Error,
    },
    #[error("prompt failed: {0}")]
    Prompt(#[from] dialoguer::Error),
    #[error("{0}")]
    Message(String),
    #[error("{message}")]
    ProcessFailure { message: String, code: i32 },
}

impl CliError {
    pub fn message(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }

    pub fn io_path(action: &'static str, path: impl AsRef<Path>, source: io::Error) -> Self {
        Self::IoPath {
            action,
            path: path.as_ref().to_path_buf(),
            source,
        }
    }

    pub fn json_parse(context: impl Into<String>, source: serde_json::Error) -> Self {
        Self::JsonParse {
            context: context.into(),
            source,
        }
    }

    pub fn json_serialize(context: &'static str, source: serde_json::Error) -> Self {
        Self::JsonSerialize { context, source }
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

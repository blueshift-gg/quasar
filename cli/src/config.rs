use {
    crate::error::CliError,
    serde::{Deserialize, Serialize},
    std::path::{Path, PathBuf},
};

// ---------------------------------------------------------------------------
// Project config (Quasar.toml)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct QuasarConfig {
    pub project: ProjectConfig,
    pub toolchain: ToolchainConfig,
    pub testing: TestingConfig,
    #[serde(default)]
    pub clients: Option<ClientsConfig>,
    #[serde(default)]
    pub lint: Option<LintConfig>,
}

#[derive(Debug, Deserialize, Default)]
pub struct LintConfig {
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct ToolchainConfig {
    #[serde(rename = "type")]
    pub toolchain_type: String,
}

#[derive(Debug, Deserialize)]
pub struct TestingConfig {
    pub language: String,
    #[serde(default)]
    pub rust: Option<RustTestingConfig>,
    #[serde(default)]
    pub typescript: Option<TypeScriptTestingConfig>,
}

#[derive(Debug, Deserialize)]
pub struct RustTestingConfig {
    pub framework: String,
    pub test: CommandSpec,
}

#[derive(Debug, Deserialize)]
pub struct TypeScriptTestingConfig {
    pub framework: String,
    pub sdk: String,
    pub install: CommandSpec,
    pub test: CommandSpec,
}

#[derive(Debug, Deserialize)]
pub struct ClientsConfig {
    pub path: PathBuf,
    pub languages: Vec<String>,
}

impl QuasarConfig {
    pub fn load() -> Result<Self, CliError> {
        Self::load_from(Path::new("Quasar.toml"))
    }

    pub fn load_from(path: &Path) -> Result<Self, CliError> {
        if !path.exists() {
            return Err(CliError::message(format!(
                "{} not found.\n\n  Are you in a Quasar project directory?\n  Run quasar init to \
                 create a new project.",
                path.display()
            )));
        }
        let contents = std::fs::read_to_string(path)
            .map_err(|e| CliError::message(format!("failed to read {}: {e}", path.display())))?;
        let config: QuasarConfig = toml::from_str(&contents).map_err(|e| {
            // Check if this looks like an old-format config (pre-init-rework)
            if contents.contains("[testing]\nframework")
                || contents.contains("[testing]\r\nframework")
                || (contents.contains("[testing]") && !contents.contains("language"))
            {
                CliError::message(
                    "Quasar.toml uses an outdated format.\n\n  Run quasar config reset and \
                     re-init your project.",
                )
            } else {
                CliError::message(format!("invalid {}: {e}", path.display()))
            }
        })?;
        Ok(config)
    }

    pub fn is_solana_toolchain(&self) -> bool {
        self.toolchain.toolchain_type == "solana"
    }

    pub fn module_name(&self) -> String {
        self.project.name.replace('-', "_")
    }

    pub fn has_typescript_tests(&self) -> bool {
        self.testing.language == "typescript"
    }

    pub fn has_rust_tests(&self) -> bool {
        self.testing.language == "rust"
    }

    pub fn lint_enabled(&self) -> bool {
        self.lint.as_ref().is_some_and(|l| l.enabled)
    }

    pub fn client_path(&self) -> PathBuf {
        self.clients
            .as_ref()
            .map(|c| c.path.clone())
            .unwrap_or(PathBuf::from("target").join("client"))
    }

    pub fn client_languages(&self) -> Vec<&str> {
        match self.clients {
            Some(ref c) => c.languages.iter().map(|s| s.as_str()).collect(),
            None => {
                // Backward compat: infer from testing framework
                let mut langs = vec!["rust"];
                if self.has_typescript_tests() {
                    langs.push("typescript");
                }
                langs
            }
        }
    }
}

pub fn resolve_client_path() -> Result<PathBuf, CliError> {
    let config_path = Path::new("Quasar.toml");
    if !config_path.exists() {
        return Ok(PathBuf::from("target").join("client"));
    }

    QuasarConfig::load_from(config_path).map(|config| config.client_path())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawCommandSpec", into = "RawCommandSpec")]
pub struct CommandSpec {
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
}

impl CommandSpec {
    pub fn new(
        program: impl Into<String>,
        args: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
        }
    }

    pub fn display(&self) -> String {
        let mut parts = Vec::with_capacity(self.args.len() + 1);
        parts.push(self.program.clone());
        parts.extend(self.args.iter().cloned());
        shlex::try_join(parts.iter().map(String::as_str)).unwrap_or_else(|_| self.program.clone())
    }

    pub fn parse(command: &str) -> Result<Self, CliError> {
        let Some(parts) = shlex::split(command) else {
            return Err(CliError::message(format!(
                "invalid command syntax: {command}"
            )));
        };
        Self::from_parts(parts).map_err(CliError::message)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
enum RawCommandSpec {
    String(String),
    Args(Vec<String>),
    Structured {
        program: String,
        #[serde(default)]
        args: Vec<String>,
    },
}

impl TryFrom<RawCommandSpec> for CommandSpec {
    type Error = String;

    fn try_from(value: RawCommandSpec) -> Result<Self, Self::Error> {
        match value {
            RawCommandSpec::String(command) => Self::parse(&command).map_err(|e| e.to_string()),
            RawCommandSpec::Args(parts) => Self::from_parts(parts),
            RawCommandSpec::Structured { program, args } => {
                if program.trim().is_empty() {
                    return Err("command program cannot be empty".to_string());
                }
                Ok(Self { program, args })
            }
        }
    }
}

impl From<CommandSpec> for RawCommandSpec {
    fn from(value: CommandSpec) -> Self {
        Self::Structured {
            program: value.program,
            args: value.args,
        }
    }
}

impl CommandSpec {
    fn from_parts(parts: Vec<String>) -> Result<Self, String> {
        let mut parts = parts.into_iter();
        let Some(program) = parts.next() else {
            return Err("command cannot be empty".to_string());
        };
        Ok(Self {
            program,
            args: parts.collect(),
        })
    }
}

// ---------------------------------------------------------------------------
// Global config (~/.quasar/config.toml) — saved preferences across projects
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct GlobalConfig {
    #[serde(default)]
    pub defaults: GlobalDefaults,
    #[serde(default)]
    pub ui: UiConfig,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct GlobalDefaults {
    pub toolchain: Option<String>,
    pub test_language: Option<String>,
    pub rust_framework: Option<String>,
    pub ts_sdk: Option<String>,
    pub template: Option<String>,
    pub git: Option<String>,
    pub package_manager: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct UiConfig {
    /// Show the animated banner on `quasar init` (default: true)
    #[serde(default = "default_true")]
    pub animation: bool,
    /// Use colored output (default: true)
    #[serde(default = "default_true")]
    pub color: bool,
}

fn default_true() -> bool {
    true
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            animation: true,
            color: true,
        }
    }
}

impl GlobalConfig {
    pub fn path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".quasar")
            .join("config.toml")
    }

    pub fn load() -> Result<Self, CliError> {
        let path = Self::path();
        if path.exists() {
            let contents = std::fs::read_to_string(&path).map_err(|e| {
                CliError::message(format!("failed to read {}: {e}", path.display()))
            })?;
            toml::from_str(&contents)
                .map_err(|e| CliError::message(format!("invalid {}: {e}", path.display())))
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<(), CliError> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let toml_str = toml::to_string_pretty(self)?;
        std::fs::write(path, toml_str)?;
        Ok(())
    }

    pub fn load_from_str(s: &str) -> Result<Self, CliError> {
        toml::from_str(s).map_err(CliError::from)
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_animation_enabled() {
        let config = GlobalConfig::default();
        assert!(config.ui.animation);
    }

    #[test]
    fn animation_disabled_survives_roundtrip() {
        let config = GlobalConfig {
            ui: UiConfig {
                animation: false,
                ..UiConfig::default()
            },
            ..GlobalConfig::default()
        };
        let toml_str = config.to_toml();
        let loaded = GlobalConfig::load_from_str(&toml_str).unwrap();
        assert!(!loaded.ui.animation);
    }

    #[test]
    fn empty_config_defaults_animation_true() {
        let loaded = GlobalConfig::load_from_str("").unwrap();
        assert!(loaded.ui.animation);
    }

    #[test]
    fn saved_config_disables_animation() {
        // Simulates the init flow: default config → save with animation: false
        let globals = GlobalConfig::default();
        assert!(globals.ui.animation);

        let saved = GlobalConfig {
            defaults: GlobalDefaults {
                toolchain: Some("solana".into()),
                test_language: Some("rust".into()),
                rust_framework: Some("quasar-svm".into()),
                ts_sdk: None,
                template: Some("minimal".into()),
                git: Some("commit".into()),
                package_manager: None,
            },
            ui: UiConfig {
                animation: false,
                ..globals.ui
            },
        };
        let toml_str = saved.to_toml();
        let reloaded = GlobalConfig::load_from_str(&toml_str).unwrap();
        assert!(!reloaded.ui.animation);
        assert_eq!(reloaded.defaults.toolchain.as_deref(), Some("solana"));
        assert_eq!(reloaded.defaults.git.as_deref(), Some("commit"));
    }

    #[test]
    fn command_spec_deserializes_legacy_string() {
        let config: QuasarConfig = toml::from_str(
            r#"
            [project]
            name = "demo"

            [toolchain]
            type = "solana"

            [testing]
            language = "rust"

            [testing.rust]
            framework = "quasar-svm"
            test = "cargo test tests::"
            "#,
        )
        .unwrap();

        let test = &config.testing.rust.unwrap().test;
        assert_eq!(test.program, "cargo");
        assert_eq!(test.args, vec!["test", "tests::"]);
    }

    #[test]
    fn command_spec_deserializes_structured_command() {
        let config: QuasarConfig = toml::from_str(
            r#"
            [project]
            name = "demo"

            [toolchain]
            type = "solana"

            [testing]
            language = "typescript"

            [testing.typescript]
            framework = "quasar-svm"
            sdk = "kit"
            install = { program = "pnpm", args = ["install", "--frozen-lockfile"] }
            test = ["pnpm", "vitest", "run"]
            "#,
        )
        .unwrap();

        let ts = config.testing.typescript.unwrap();
        assert_eq!(ts.install.program, "pnpm");
        assert_eq!(ts.install.args, vec!["install", "--frozen-lockfile"]);
        assert_eq!(ts.test.program, "pnpm");
        assert_eq!(ts.test.args, vec!["vitest", "run"]);
    }

    #[test]
    fn invalid_global_config_is_not_silently_ignored() {
        assert!(GlobalConfig::load_from_str("ui = ").is_err());
    }
}

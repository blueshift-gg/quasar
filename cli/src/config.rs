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
    pub framework: String,
}

impl QuasarConfig {
    pub fn load() -> Result<Self, CliError> {
        Self::load_from(Path::new("Quasar.toml"))
    }

    pub fn load_from(path: &Path) -> Result<Self, CliError> {
        if !path.exists() {
            use crate::style;
            eprintln!(
                "\n  {}",
                style::fail(&format!("{} not found.", path.display()))
            );
            eprintln!();
            eprintln!("  Are you in a Quasar project directory?");
            eprintln!(
                "  Run {} to create a new project.",
                style::bold("quasar init")
            );
            eprintln!();
            std::process::exit(1);
        }
        let contents = std::fs::read_to_string(path).map_err(|e| {
            eprintln!(
                "\n  {}",
                crate::style::fail(&format!("Failed to read {}: {e}", path.display()))
            );
            e
        })?;
        let config: QuasarConfig = toml::from_str(&contents).map_err(|e| {
            eprintln!(
                "\n  {}",
                crate::style::fail(&format!("Invalid {}: {e}", path.display()))
            );
            e
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
        matches!(
            self.testing.framework.as_str(),
            "quasarsvm-web3js" | "quasarsvm-kit"
        )
    }

    pub fn has_rust_tests(&self) -> bool {
        matches!(
            self.testing.framework.as_str(),
            "mollusk" | "quasarsvm-rust"
        )
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
    pub framework: Option<String>,
    pub template: Option<String>,
    pub git: Option<String>,
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

    pub fn load() -> Self {
        let path = Self::path();
        if path.exists() {
            let contents = std::fs::read_to_string(&path).unwrap_or_default();
            toml::from_str(&contents).unwrap_or_default()
        } else {
            Self::default()
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

    pub fn load_from_str(s: &str) -> Self {
        toml::from_str(s).unwrap_or_default()
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
        let loaded = GlobalConfig::load_from_str(&toml_str);
        assert!(!loaded.ui.animation);
    }

    #[test]
    fn empty_config_defaults_animation_true() {
        let loaded = GlobalConfig::load_from_str("");
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
                framework: Some("quasarsvm-rust".into()),
                template: Some("minimal".into()),
                git: Some("commit".into()),
            },
            ui: UiConfig {
                animation: false,
                ..globals.ui
            },
        };
        let toml_str = saved.to_toml();
        let reloaded = GlobalConfig::load_from_str(&toml_str);
        assert!(!reloaded.ui.animation);
        assert_eq!(reloaded.defaults.toolchain.as_deref(), Some("solana"));
        assert_eq!(reloaded.defaults.git.as_deref(), Some("commit"));
    }
}

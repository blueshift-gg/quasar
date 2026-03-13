use {crate::error::CliError, serde::Deserialize, std::path::Path};

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
            eprintln!(
                "Error: {} not found. Are you in a Quasar project directory?",
                path.display()
            );
            std::process::exit(1);
        }
        let contents = std::fs::read_to_string(path)?;
        let config: QuasarConfig = toml::from_str(&contents)?;
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
        matches!(self.testing.framework.as_str(), "mollusk")
    }
}

use {
    crate::error::CliError,
    clap::ValueEnum,
    serde::{Deserialize, Serialize},
    std::path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct QuasarConfig {
    pub project: ProjectConfig,
    pub testing: TestingConfig,
    pub clients: ClientsConfig,
}

impl Default for QuasarConfig {
    fn default() -> Self {
        Self::canonical("quasar-program")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProjectConfig {
    pub name: String,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            name: "quasar-program".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TestingConfig {
    pub command: CommandSpec,
}

impl Default for TestingConfig {
    fn default() -> Self {
        Self {
            command: CommandSpec::new("cargo", ["test", "tests::"]),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ClientsConfig {
    pub path: PathBuf,
    pub targets: Vec<ClientTarget>,
}

impl Default for ClientsConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("target/client"),
            targets: vec![ClientTarget::Rust, ClientTarget::Kit, ClientTarget::Web3],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
#[value(rename_all = "lower")]
pub enum ClientTarget {
    Rust,
    Kit,
    Web3,
    Python,
    Go,
    C,
}

impl ClientTarget {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Kit => "kit",
            Self::Web3 => "web3",
            Self::Python => "python",
            Self::Go => "go",
            Self::C => "c",
        }
    }
}

impl QuasarConfig {
    pub fn canonical(name: impl Into<String>) -> Self {
        Self {
            project: ProjectConfig { name: name.into() },
            testing: TestingConfig::default(),
            clients: ClientsConfig::default(),
        }
    }

    pub fn load() -> Result<Self, CliError> {
        Self::load_from(Path::new("Quasar.toml"))
    }

    pub fn load_from(path: &Path) -> Result<Self, CliError> {
        if !path.exists() {
            return Err(CliError::message(format!(
                "{} not found.\n\n  Are you in a Quasar project directory?\n  Run `quasar init \
                 <NAME>` to create a new project.",
                path.display()
            )));
        }
        let contents = std::fs::read_to_string(path)
            .map_err(|error| CliError::io_path("read", path, error))?;
        Self::parse(&contents)
            .map_err(|error| CliError::message(format!("invalid {}: {error}", path.display())))
    }

    pub fn parse(contents: &str) -> Result<Self, String> {
        for (removed, replacement) in [
            (
                "toolchain",
                "remove [toolchain]; Quasar always uses `cargo build-sbf`",
            ),
            (
                "testing.language",
                "replace it with `testing.command = { program = \"cargo\", args = [...] }`",
            ),
            (
                "testing.rust",
                "replace it with the single typed `testing.command`",
            ),
            (
                "testing.typescript",
                "TypeScript testing was removed; use Rust `quasar-test`",
            ),
            (
                "clients.languages",
                "rename it to `clients.targets` and use rust, kit, web3, python, go, or c",
            ),
        ] {
            if contains_toml_path(contents, removed) {
                return Err(format!(
                    "unsupported `{removed}` configuration; {replacement}"
                ));
            }
        }

        toml::from_str(contents).map_err(|error| {
            format!(
                "{error}\n  supported sections: [project], [testing], [clients]\n  supported \
                 client targets: rust, kit, web3, python, go, c"
            )
        })
    }

    pub fn to_toml(&self) -> String {
        let string = |value: &str| toml::Value::String(value.to_string()).to_string();
        let args = self
            .testing
            .command
            .args
            .iter()
            .map(|argument| string(argument))
            .collect::<Vec<_>>()
            .join(", ");
        let targets = self
            .clients
            .targets
            .iter()
            .map(|target| string(target.as_str()))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "[project]\nname = {}\n\n[testing]\ncommand = {{ program = {}, args = [{}] \
             }}\n\n[clients]\npath = {}\ntargets = [{}]\n",
            string(&self.project.name),
            string(&self.testing.command.program),
            args,
            string(&self.clients.path.to_string_lossy()),
            targets,
        )
    }

    pub fn module_name(&self) -> String {
        self.project.name.replace('-', "_")
    }

    pub fn client_path(&self) -> PathBuf {
        self.clients.path.clone()
    }

    pub fn client_targets(&self) -> &[ClientTarget] {
        &self.clients.targets
    }

    pub fn codegen_targets(&self) -> Vec<&'static str> {
        let mut targets = Vec::new();
        for target in &self.clients.targets {
            let target = match target {
                ClientTarget::Rust => continue,
                ClientTarget::Kit | ClientTarget::Web3 => "typescript",
                ClientTarget::Python => "python",
                ClientTarget::Go => "golang",
                ClientTarget::C => "c",
            };
            if !targets.contains(&target) {
                targets.push(target);
            }
        }
        targets
    }
}

fn contains_toml_path(contents: &str, path: &str) -> bool {
    let (section, key) = path
        .rsplit_once('.')
        .map_or((None, path), |(section, key)| (Some(section), key));
    let mut current_section = "";
    for line in contents.lines() {
        let line = line.split('#').next().unwrap_or_default().trim();
        if line.starts_with('[') && line.ends_with(']') {
            current_section = line.trim_matches(&['[', ']'][..]).trim();
            if section.is_none() && current_section == key {
                return true;
            }
            continue;
        }
        if line
            .split_once('=')
            .is_some_and(|(candidate, _)| candidate.trim() == key)
            && section.is_none_or(|expected| expected == current_section)
        {
            return true;
        }
    }
    false
}

pub fn resolve_client_path() -> Result<PathBuf, CliError> {
    let config_path = Path::new("Quasar.toml");
    if !config_path.exists() {
        return Ok(PathBuf::from("target").join("client"));
    }
    QuasarConfig::load_from(config_path).map(|config| config.client_path())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
}

#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct GlobalConfig {
    pub ui: UiConfig,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct UiConfig {
    pub color: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self { color: true }
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
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(&path)
            .map_err(|error| CliError::io_path("read", &path, error))?;
        toml::from_str(&contents)
            .map_err(|error| CliError::message(format!("invalid {}: {error}", path.display())))
    }

    pub fn save(&self) -> Result<(), CliError> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, toml::to_string_pretty(self)?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_config_serializes_to_the_public_schema() {
        let config = QuasarConfig::canonical("demo").to_toml();
        assert_eq!(
            config,
            "[project]\nname = \"demo\"\n\n[testing]\ncommand = { program = \"cargo\", args = \
             [\"test\", \"tests::\"] }\n\n[clients]\npath = \"target/client\"\ntargets = [\"rust\", \
             \"kit\", \"web3\"]\n"
        );
    }

    #[test]
    fn missing_sections_receive_canonical_defaults() {
        let config = QuasarConfig::parse("[project]\nname = \"demo\"\n").unwrap();
        assert_eq!(config.testing, TestingConfig::default());
        assert_eq!(config.clients, ClientsConfig::default());
    }

    #[test]
    fn client_targets_are_closed() {
        let error = QuasarConfig::parse(
            "[project]\nname = \"demo\"\n[clients]\ntargets = [\"typescript\"]\n",
        )
        .unwrap_err();
        assert!(error.contains("rust, kit, web3, python, go, c"), "{error}");
    }

    #[test]
    fn removed_values_name_the_supported_replacement() {
        let error = QuasarConfig::parse("[toolchain]\ntype = \"upstream\"\n").unwrap_err();
        assert!(error.contains("cargo build-sbf"), "{error}");

        let error = QuasarConfig::parse("[clients]\nlanguages = [\"typescript\"]\n").unwrap_err();
        assert!(error.contains("clients.targets"), "{error}");
    }

    #[test]
    fn testing_command_must_be_structured() {
        let error = QuasarConfig::parse("[testing]\ncommand = \"cargo test\"\n").unwrap_err();
        assert!(error.contains("invalid type"), "{error}");
    }
}

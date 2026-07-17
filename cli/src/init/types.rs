use std::fmt;

#[derive(Debug, Clone, Copy)]
pub(super) enum Toolchain {
    Solana,
    Upstream,
}

impl fmt::Display for Toolchain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Toolchain::Solana => write!(f, "solana"),
            Toolchain::Upstream => write!(f, "upstream"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum TestLanguage {
    None,
    Rust,
    TypeScript,
}

impl fmt::Display for TestLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TestLanguage::None => write!(f, "none"),
            TestLanguage::Rust => write!(f, "rust"),
            TestLanguage::TypeScript => write!(f, "typescript"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum RustFramework {
    QuasarSVM,
    Mollusk,
}

impl fmt::Display for RustFramework {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RustFramework::QuasarSVM => write!(f, "quasar-svm"),
            RustFramework::Mollusk => write!(f, "mollusk"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum TypeScriptSdk {
    Kit,
    Web3js,
}

impl fmt::Display for TypeScriptSdk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeScriptSdk::Kit => write!(f, "kit"),
            TypeScriptSdk::Web3js => write!(f, "web3.js"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum Template {
    Minimal,
    Full,
}

impl fmt::Display for Template {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Template::Minimal => write!(f, "minimal"),
            Template::Full => write!(f, "full"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum GitSetup {
    InitializeAndCommit,
    Initialize,
    Skip,
}

impl GitSetup {
    pub(super) fn from_config(value: Option<&str>) -> Self {
        match value {
            Some("init") => GitSetup::Initialize,
            Some("skip") => GitSetup::Skip,
            _ => GitSetup::InitializeAndCommit,
        }
    }

    pub(super) fn from_index(idx: usize) -> Self {
        match idx {
            1 => GitSetup::Initialize,
            2 => GitSetup::Skip,
            _ => GitSetup::InitializeAndCommit,
        }
    }

    pub(super) fn index(self) -> usize {
        match self {
            GitSetup::InitializeAndCommit => 0,
            GitSetup::Initialize => 1,
            GitSetup::Skip => 2,
        }
    }

    pub(super) fn prompt_label(self) -> &'static str {
        match self {
            GitSetup::InitializeAndCommit => "Initialize + Commit",
            GitSetup::Initialize => "Initialize",
            GitSetup::Skip => "Skip",
        }
    }

    pub(super) fn summary_label(self) -> &'static str {
        match self {
            GitSetup::InitializeAndCommit => "git: init + commit",
            GitSetup::Initialize => "git: init",
            GitSetup::Skip => "git: skip",
        }
    }
}

impl fmt::Display for GitSetup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GitSetup::InitializeAndCommit => write!(f, "commit"),
            GitSetup::Initialize => write!(f, "init"),
            GitSetup::Skip => write!(f, "skip"),
        }
    }
}

#[derive(Debug, Clone)]
pub(super) enum PackageManager {
    Pnpm,
    Bun,
    Npm,
    Yarn,
    Other { install: String, test: String },
}

impl PackageManager {
    pub(super) fn install_cmd(&self) -> &str {
        match self {
            PackageManager::Pnpm => "pnpm install",
            PackageManager::Bun => "bun install",
            PackageManager::Npm => "npm install",
            PackageManager::Yarn => "yarn install",
            PackageManager::Other { install, .. } => install,
        }
    }

    pub(super) fn test_cmd(&self) -> &str {
        match self {
            PackageManager::Pnpm => "pnpm test",
            PackageManager::Bun => "bun run test",
            PackageManager::Npm => "npm test",
            PackageManager::Yarn => "yarn test",
            PackageManager::Other { test, .. } => test,
        }
    }

    pub(super) fn from_config(value: Option<&str>) -> usize {
        match value {
            Some("bun") => 1,
            Some("npm") => 2,
            Some("yarn") => 3,
            _ => 0,
        }
    }
}

impl fmt::Display for PackageManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackageManager::Pnpm => write!(f, "pnpm"),
            PackageManager::Bun => write!(f, "bun"),
            PackageManager::Npm => write!(f, "npm"),
            PackageManager::Yarn => write!(f, "yarn"),
            PackageManager::Other { .. } => write!(f, "other"),
        }
    }
}

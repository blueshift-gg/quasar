mod banner;
mod scaffold;
mod templates;

use {
    crate::{
        config::{GlobalConfig, GlobalDefaults, UiConfig},
        error::CliResult,
        toolchain,
    },
    dialoguer::{theme::ColorfulTheme, Input, MultiSelect, Select},
    serde::Serialize,
    std::{fmt, path::Path, process::Command},
};

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
enum Toolchain {
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
enum TestLanguage {
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
enum RustFramework {
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
enum TypeScriptSdk {
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
enum Template {
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
enum GitSetup {
    InitializeAndCommit,
    Initialize,
    Skip,
}

impl GitSetup {
    fn from_config(value: Option<&str>) -> Self {
        match value {
            Some("init") => GitSetup::Initialize,
            Some("skip") => GitSetup::Skip,
            _ => GitSetup::InitializeAndCommit,
        }
    }

    fn from_index(idx: usize) -> Self {
        match idx {
            1 => GitSetup::Initialize,
            2 => GitSetup::Skip,
            _ => GitSetup::InitializeAndCommit,
        }
    }

    fn index(self) -> usize {
        match self {
            GitSetup::InitializeAndCommit => 0,
            GitSetup::Initialize => 1,
            GitSetup::Skip => 2,
        }
    }

    fn prompt_label(self) -> &'static str {
        match self {
            GitSetup::InitializeAndCommit => "Initialize + Commit",
            GitSetup::Initialize => "Initialize",
            GitSetup::Skip => "Skip",
        }
    }

    fn summary_label(self) -> &'static str {
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
enum PackageManager {
    Pnpm,
    Bun,
    Npm,
    Yarn,
    Other { install: String, test: String },
}

impl PackageManager {
    fn install_cmd(&self) -> &str {
        match self {
            PackageManager::Pnpm => "pnpm install",
            PackageManager::Bun => "bun install",
            PackageManager::Npm => "npm install",
            PackageManager::Yarn => "yarn install",
            PackageManager::Other { install, .. } => install,
        }
    }

    fn test_cmd(&self) -> &str {
        match self {
            PackageManager::Pnpm => "pnpm test",
            PackageManager::Bun => "bun test",
            PackageManager::Npm => "npm test",
            PackageManager::Yarn => "yarn test",
            PackageManager::Other { test, .. } => test,
        }
    }

    fn from_config(value: Option<&str>) -> usize {
        match value {
            Some("bun") => 1,
            Some("npm") => 2,
            Some("yarn") => 3,
            _ => 0, // pnpm default
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

// ---------------------------------------------------------------------------
// Quasar.toml schema
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct QuasarToml {
    project: QuasarProject,
    toolchain: QuasarToolchain,
    testing: QuasarTesting,
    clients: QuasarClients,
}

#[derive(Serialize)]
struct QuasarProject {
    name: String,
}

#[derive(Serialize)]
struct QuasarToolchain {
    #[serde(rename = "type")]
    toolchain_type: String,
}

#[derive(Serialize)]
struct QuasarTesting {
    language: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    rust: Option<QuasarRustTesting>,
    #[serde(skip_serializing_if = "Option::is_none")]
    typescript: Option<QuasarTypeScriptTesting>,
}

#[derive(Serialize)]
struct QuasarRustTesting {
    framework: String,
    test: String,
}

#[derive(Serialize)]
struct QuasarTypeScriptTesting {
    framework: String,
    sdk: String,
    install: String,
    test: String,
}

#[derive(Serialize)]
struct QuasarClients {
    languages: Vec<String>,
}

// ---------------------------------------------------------------------------
// ANSI helpers (delegate to shared style module)
// ---------------------------------------------------------------------------

fn color(code: u8, s: &str) -> String {
    crate::style::color(code, s)
}

fn bold(s: &str) -> String {
    crate::style::bold(s)
}

fn dim(s: &str) -> String {
    crate::style::dim(s)
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn run(cmd: crate::InitCommand) -> CliResult {
    let globals = GlobalConfig::load();

    let name = cmd.name;
    let no_git = cmd.no_git;
    let test_language_override = cmd.test_language;
    let rust_framework_override = cmd.rust_framework;
    let ts_sdk_override = cmd.ts_sdk;
    let template_override = cmd.template;
    let toolchain_override = cmd.toolchain;

    // Only skip prompts when --yes is explicitly set
    let skip_prompts = cmd.yes;

    // Validate explicit flag values before proceeding
    if let Some(ref t) = test_language_override {
        if !matches!(t.as_str(), "none" | "rust" | "typescript") {
            eprintln!(
                "  {}",
                crate::style::fail(&format!("unknown test language: {t}"))
            );
            eprintln!("  {}", dim("valid: none, rust, typescript"));
            std::process::exit(1);
        }
    }
    if let Some(ref f) = rust_framework_override {
        if !matches!(f.as_str(), "quasar-svm" | "mollusk") {
            eprintln!(
                "  {}",
                crate::style::fail(&format!("unknown rust framework: {f}"))
            );
            eprintln!("  {}", dim("valid: quasar-svm, mollusk"));
            std::process::exit(1);
        }
    }
    if let Some(ref s) = ts_sdk_override {
        if !matches!(s.as_str(), "kit" | "web3.js") {
            eprintln!(
                "  {}",
                crate::style::fail(&format!("unknown TypeScript SDK: {s}"))
            );
            eprintln!("  {}", dim("valid: kit, web3.js"));
            std::process::exit(1);
        }
    }
    if let Some(ref t) = template_override {
        if !matches!(t.as_str(), "minimal" | "full") {
            eprintln!(
                "  {}",
                crate::style::fail(&format!("unknown template: {t}"))
            );
            eprintln!("  {}", dim("valid: minimal, full"));
            std::process::exit(1);
        }
    }
    if let Some(ref t) = toolchain_override {
        if !matches!(t.as_str(), "solana" | "upstream") {
            eprintln!(
                "  {}",
                crate::style::fail(&format!("unknown toolchain: {t}"))
            );
            eprintln!("  {}", dim("valid: solana, upstream"));
            std::process::exit(1);
        }
    }

    if globals.ui.animation && !skip_prompts {
        banner::print_banner();
    }

    let theme = ColorfulTheme::default();

    // Project name
    let name: String = if skip_prompts {
        name.unwrap_or_else(|| {
            eprintln!(
                "  {}",
                crate::style::fail("a project name is required when using flags")
            );
            eprintln!(
                "  {}",
                crate::style::dim(
                    "usage: quasar init <name> [--test-language ...] [--template ...]"
                )
            );
            std::process::exit(1);
        })
    } else {
        let mut prompt = Input::with_theme(&theme).with_prompt("Project name");
        if let Some(default) = name {
            prompt = prompt.default(default);
        }
        prompt.interact_text().map_err(anyhow::Error::from)?
    };

    // When scaffolding into ".", derive the crate name from the current directory
    let crate_name = if name == "." {
        std::env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "my-program".to_string())
    } else {
        name.clone()
    };

    // Toolchain
    let toolchain_default = match toolchain_override
        .as_deref()
        .or(globals.defaults.toolchain.as_deref())
    {
        Some("upstream") => 1,
        _ => 0,
    };
    let toolchain_idx = if skip_prompts {
        toolchain_default
    } else {
        let toolchain_items = &[
            "solana    (cargo build-sbf)",
            "upstream  (cargo +nightly build-bpf)",
        ];
        Select::with_theme(&theme)
            .with_prompt("Toolchain")
            .items(toolchain_items)
            .default(toolchain_default)
            .interact()
            .map_err(anyhow::Error::from)?
    };
    let toolchain = match toolchain_idx {
        0 => Toolchain::Solana,
        _ => Toolchain::Upstream,
    };

    // For upstream: sbpf-linker must be installed
    if matches!(toolchain, Toolchain::Upstream) && !toolchain::has_sbpf_linker() {
        eprintln!();
        eprintln!("  {} sbpf-linker not found.", color(196, "\u{2718}"));
        eprintln!();
        eprintln!("  Install platform-tools first:");
        eprintln!(
            "    {}",
            bold("git clone https://github.com/anza-xyz/platform-tools")
        );
        eprintln!("    {}", bold("cd platform-tools"));
        eprintln!("    {}", bold("cargo install-with-gallery"));
        eprintln!();
        std::process::exit(1);
    }

    let lang_default = match test_language_override
        .as_deref()
        .or(globals.defaults.test_language.as_deref())
    {
        Some("none") => 0,
        Some("typescript") => 2,
        _ => 1, // rust default
    };
    let rust_fw_default = match rust_framework_override
        .as_deref()
        .or(globals.defaults.rust_framework.as_deref())
    {
        Some("mollusk") => 1,
        _ => 0, // quasar-svm default
    };
    let ts_sdk_default = match ts_sdk_override
        .as_deref()
        .or(globals.defaults.ts_sdk.as_deref())
    {
        Some("web3.js") => 1,
        _ => 0, // kit default
    };

    // Test language
    let test_lang_idx = if skip_prompts {
        lang_default
    } else {
        let lang_items = &["None", "Rust", "TypeScript"];
        Select::with_theme(&theme)
            .with_prompt("Test language")
            .items(lang_items)
            .default(lang_default)
            .interact()
            .map_err(anyhow::Error::from)?
    };
    let test_language = match test_lang_idx {
        1 => TestLanguage::Rust,
        2 => TestLanguage::TypeScript,
        _ => TestLanguage::None,
    };

    // Rust test framework (only if Rust)
    let rust_framework = if matches!(test_language, TestLanguage::Rust) {
        let idx = if skip_prompts {
            rust_fw_default
        } else {
            let items = &["QuasarSVM", "Mollusk"];
            Select::with_theme(&theme)
                .with_prompt("Rust test framework")
                .items(items)
                .default(rust_fw_default)
                .interact()
                .map_err(anyhow::Error::from)?
        };
        Some(match idx {
            1 => RustFramework::Mollusk,
            _ => RustFramework::QuasarSVM,
        })
    } else {
        None
    };

    // TypeScript SDK (only if TypeScript)
    let ts_sdk = if matches!(test_language, TestLanguage::TypeScript) {
        let idx = if skip_prompts {
            ts_sdk_default
        } else {
            let items = &["Kit", "Web3.js"];
            Select::with_theme(&theme)
                .with_prompt("TypeScript SDK")
                .items(items)
                .default(ts_sdk_default)
                .interact()
                .map_err(anyhow::Error::from)?
        };
        Some(match idx {
            1 => TypeScriptSdk::Web3js,
            _ => TypeScriptSdk::Kit,
        })
    } else {
        None
    };

    // Package manager (only for TypeScript)
    let package_manager = if matches!(test_language, TestLanguage::TypeScript) {
        let pm_default = PackageManager::from_config(globals.defaults.package_manager.as_deref());
        let pm_idx = if skip_prompts {
            pm_default
        } else {
            let pm_items = &["pnpm", "bun", "npm", "yarn", "other"];
            Select::with_theme(&theme)
                .with_prompt("Package manager")
                .items(pm_items)
                .default(pm_default)
                .interact()
                .map_err(anyhow::Error::from)?
        };
        Some(match pm_idx {
            0 => PackageManager::Pnpm,
            1 => PackageManager::Bun,
            2 => PackageManager::Npm,
            3 => PackageManager::Yarn,
            _ => {
                let install: String = Input::with_theme(&theme)
                    .with_prompt("Install command")
                    .default("pnpm install".into())
                    .interact_text()
                    .map_err(anyhow::Error::from)?;
                let test: String = Input::with_theme(&theme)
                    .with_prompt("Test command")
                    .default("pnpm test".into())
                    .interact_text()
                    .map_err(anyhow::Error::from)?;
                PackageManager::Other { install, test }
            }
        })
    } else {
        None
    };

    // Client languages — Rust always included, test language forced on
    let ts_tests = matches!(test_language, TestLanguage::TypeScript);
    let client_languages: Vec<String> = if skip_prompts {
        let mut langs = vec!["rust".to_string()];
        if ts_tests {
            langs.push("typescript".to_string());
        }
        langs
    } else {
        // Forced languages shown in prompt text, not selectable
        let mut forced = vec!["Rust"];
        if ts_tests {
            forced.push("TypeScript");
        }

        let all_optional: &[(&str, &str)] = &[
            ("TypeScript", "typescript"),
            ("Golang (Experimental)", "golang"),
            ("Python (Experimental)", "python"),
        ];
        let optional: Vec<(&str, &str)> = all_optional
            .iter()
            .copied()
            .filter(|(display, _)| !forced.contains(display))
            .collect();

        let prompt = format!(
            "Additional client languages ({} always included)",
            forced.join(", ")
        );

        let display_items: Vec<&str> = optional.iter().map(|(d, _)| *d).collect();
        let selected = MultiSelect::with_theme(&theme)
            .with_prompt(&prompt)
            .items(&display_items)
            .interact()
            .map_err(anyhow::Error::from)?;

        let mut langs: Vec<String> = vec!["rust".to_string()];
        if ts_tests {
            langs.push("typescript".to_string());
        }
        for &i in &selected {
            langs.push(optional[i].1.to_string());
        }
        langs
    };

    // Template
    let template_default = match template_override
        .as_deref()
        .or(globals.defaults.template.as_deref())
    {
        Some("full") => 1,
        _ => 0,
    };
    let template_idx = if skip_prompts {
        template_default
    } else {
        let template_items = &[
            "Minimal (instruction file only)",
            "Full (state, errors, and instruction files)",
        ];
        Select::with_theme(&theme)
            .with_prompt("Template")
            .items(template_items)
            .default(template_default)
            .interact()
            .map_err(anyhow::Error::from)?
    };
    let template = match template_idx {
        0 => Template::Minimal,
        _ => Template::Full,
    };

    // Git setup
    let git_default = GitSetup::from_config(globals.defaults.git.as_deref());
    let git_setup = if no_git {
        GitSetup::Skip
    } else if skip_prompts {
        git_default
    } else {
        let git_items = &[
            GitSetup::InitializeAndCommit.prompt_label(),
            GitSetup::Initialize.prompt_label(),
            GitSetup::Skip.prompt_label(),
        ];
        let git_idx = Select::with_theme(&theme)
            .with_prompt("Initialize a new git repo?")
            .items(git_items)
            .default(git_default.index())
            .interact()
            .map_err(anyhow::Error::from)?;
        GitSetup::from_index(git_idx)
    };

    if skip_prompts {
        println!();
        let fw_label = match test_language {
            TestLanguage::None => "no tests".to_string(),
            TestLanguage::Rust => format!("rust/{}", rust_framework.unwrap()),
            TestLanguage::TypeScript => format!("typescript/{}", ts_sdk.unwrap()),
        };
        println!(
            "  {} {} {} {} {} {} {}",
            dim("Using:"),
            bold(&toolchain.to_string()),
            dim("+"),
            bold(&fw_label),
            bold(&template.to_string()),
            dim("+"),
            bold(git_setup.summary_label()),
        );
    }

    scaffold::scaffold(
        &name,
        &crate_name,
        toolchain,
        test_language,
        rust_framework,
        ts_sdk,
        template,
        package_manager.as_ref(),
        &client_languages,
        !skip_prompts,
    )?;

    // Optional git setup (unless already in a git repo)
    maybe_initialize_git_repo(&name, git_setup);

    // Save preferences for next time (disable animation after first run)
    let saved_git_default = if no_git {
        globals.defaults.git.clone()
    } else {
        Some(git_setup.to_string())
    };
    let saved_pm = package_manager
        .as_ref()
        .map(|pm| pm.to_string())
        .or_else(|| globals.defaults.package_manager.clone());

    let new_globals = GlobalConfig {
        defaults: GlobalDefaults {
            toolchain: Some(toolchain.to_string()),
            test_language: Some(test_language.to_string()),
            rust_framework: rust_framework.map(|f| f.to_string()),
            ts_sdk: ts_sdk.map(|s| s.to_string()),
            template: Some(template.to_string()),
            git: saved_git_default,
            package_manager: saved_pm,
        },
        ui: UiConfig {
            animation: false,
            ..globals.ui
        },
    };
    let _ = new_globals.save(); // best-effort

    // Success message
    println!();
    println!(
        "  {}  Created {} {}",
        color(83, "\u{2714}"),
        bold(&crate_name),
        dim("project")
    );
    println!();
    println!("  {}", dim("Next steps:"));
    if name != "." {
        println!(
            "    {}  {}",
            color(45, "\u{276f}"),
            bold(&format!("cd {name}"))
        );
    }
    println!("    {}  {}", color(45, "\u{276f}"), bold("quasar build"));
    if !matches!(test_language, TestLanguage::None) {
        println!("    {}  {}", color(45, "\u{276f}"), bold("quasar test"));
    }
    println!();
    println!(
        "  {} saved to {}",
        dim("Preferences"),
        dim(&GlobalConfig::path().display().to_string()),
    );
    println!();

    Ok(())
}

// ---------------------------------------------------------------------------
// Git helpers
// ---------------------------------------------------------------------------

fn maybe_initialize_git_repo(name: &str, git_setup: GitSetup) {
    if matches!(git_setup, GitSetup::Skip) {
        return;
    }

    let root = Path::new(name);
    let already_git = if name == "." {
        Path::new(".git").exists()
    } else {
        root.join(".git").exists()
    };

    if !already_git {
        let _ = initialize_git_repo(root, git_setup);
    }
}

fn initialize_git_repo(root: &Path, git_setup: GitSetup) -> bool {
    run_git(root, &["init", "--quiet"])
        && match git_setup {
            GitSetup::InitializeAndCommit => {
                run_git(root, &["add", "."])
                    && run_git(root, &["commit", "-am", "chore: initial commit", "--quiet"])
            }
            GitSetup::Initialize | GitSetup::Skip => true,
        }
}

fn run_git(root: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(args)
        .current_dir(root)
        .status()
        .ok()
        .is_some_and(|status| status.success())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use {
        super::*,
        std::{
            env, fs,
            path::PathBuf,
            sync::Mutex,
            time::{SystemTime, UNIX_EPOCH},
        },
    };

    static PATH_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn initialize_git_repo_runs_init_add_and_commit() {
        let _guard = PATH_LOCK.lock().unwrap();
        let sandbox = create_test_sandbox("success");
        let _env = TestGitEnv::new(&sandbox, None);
        let root = sandbox.join("repo");
        fs::create_dir_all(&root).unwrap();

        let ok = initialize_git_repo(&root, GitSetup::InitializeAndCommit);

        assert!(ok);
        assert_eq!(
            read_git_log(&sandbox),
            vec![
                "init --quiet",
                "add .",
                "commit -am chore: initial commit --quiet",
            ]
        );
    }

    #[test]
    fn initialize_git_repo_can_skip_initial_commit() {
        let _guard = PATH_LOCK.lock().unwrap();
        let sandbox = create_test_sandbox("init-only");
        let _env = TestGitEnv::new(&sandbox, None);
        let root = sandbox.join("repo");
        fs::create_dir_all(&root).unwrap();

        let ok = initialize_git_repo(&root, GitSetup::Initialize);

        assert!(ok);
        assert_eq!(read_git_log(&sandbox), vec!["init --quiet"]);
    }

    #[test]
    fn initialize_git_repo_stops_when_git_init_fails() {
        let _guard = PATH_LOCK.lock().unwrap();
        let sandbox = create_test_sandbox("fail-init");
        let _env = TestGitEnv::new(&sandbox, Some("init"));
        let root = sandbox.join("repo");
        fs::create_dir_all(&root).unwrap();

        let ok = initialize_git_repo(&root, GitSetup::InitializeAndCommit);

        assert!(!ok);
        assert_eq!(read_git_log(&sandbox), vec!["init --quiet"]);
    }

    fn create_test_sandbox(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = env::temp_dir().join(format!(
            "quasar-init-{label}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(dir.join("bin")).unwrap();
        dir
    }

    fn read_git_log(sandbox: &Path) -> Vec<String> {
        fs::read_to_string(sandbox.join("git.log"))
            .unwrap_or_default()
            .lines()
            .map(|line| line.to_string())
            .collect()
    }

    struct TestGitEnv {
        old_path: Option<std::ffi::OsString>,
        old_log: Option<std::ffi::OsString>,
        old_fail_on: Option<std::ffi::OsString>,
    }

    impl TestGitEnv {
        fn new(sandbox: &Path, fail_on: Option<&str>) -> Self {
            let bin_dir = sandbox.join("bin");
            let log_path = sandbox.join("git.log");
            write_fake_git(&bin_dir.join("git"));

            let old_path = env::var_os("PATH");
            let old_log = env::var_os("QUASAR_TEST_GIT_LOG");
            let old_fail_on = env::var_os("QUASAR_TEST_GIT_FAIL_ON");

            let mut path = std::ffi::OsString::new();
            path.push(bin_dir.as_os_str());
            path.push(":");
            if let Some(existing) = &old_path {
                path.push(existing);
            }

            // Safety: tests hold PATH_LOCK, so process-global env mutation stays
            // serialized.
            unsafe {
                env::set_var("PATH", path);
                env::set_var("QUASAR_TEST_GIT_LOG", &log_path);
            }
            if let Some(cmd) = fail_on {
                // Safety: tests hold PATH_LOCK, so process-global env mutation stays
                // serialized.
                unsafe {
                    env::set_var("QUASAR_TEST_GIT_FAIL_ON", cmd);
                }
            } else {
                // Safety: tests hold PATH_LOCK, so process-global env mutation stays
                // serialized.
                unsafe {
                    env::remove_var("QUASAR_TEST_GIT_FAIL_ON");
                }
            }

            Self {
                old_path,
                old_log,
                old_fail_on,
            }
        }
    }

    impl Drop for TestGitEnv {
        fn drop(&mut self) {
            // Safety: tests hold PATH_LOCK, so process-global env mutation stays
            // serialized.
            unsafe {
                restore_env_var("PATH", self.old_path.as_ref());
                restore_env_var("QUASAR_TEST_GIT_LOG", self.old_log.as_ref());
                restore_env_var("QUASAR_TEST_GIT_FAIL_ON", self.old_fail_on.as_ref());
            }
        }
    }

    unsafe fn restore_env_var(key: &str, value: Option<&std::ffi::OsString>) {
        if let Some(value) = value {
            env::set_var(key, value);
        } else {
            env::remove_var(key);
        }
    }

    fn write_fake_git(path: &Path) {
        fs::write(
            path,
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$QUASAR_TEST_GIT_LOG\"\nif [ \"$1\" = \
             \"$QUASAR_TEST_GIT_FAIL_ON\" ]; then\n  exit 1\nfi\nexit 0\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut perms = fs::metadata(path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(path, perms).unwrap();
        }
    }
}

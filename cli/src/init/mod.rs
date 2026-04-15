mod banner;
mod git;
mod scaffold;
mod schema;
mod templates;
mod types;

use {
    crate::{
        config::{GlobalConfig, GlobalDefaults, UiConfig},
        error::{CliError, CliResult},
        toolchain,
    },
    dialoguer::{theme::ColorfulTheme, Input, MultiSelect, Select},
    git::maybe_initialize_git_repo,
    types::{
        GitSetup, PackageManager, RustFramework, Template, TestLanguage, Toolchain, TypeScriptSdk,
    },
};

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
    let globals = GlobalConfig::load()?;

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
            return Err(CliError::message(format!(
                "unknown test language: {t}\n  valid: none, rust, typescript"
            )));
        }
    }
    if let Some(ref f) = rust_framework_override {
        if !matches!(f.as_str(), "quasar-svm" | "mollusk") {
            return Err(CliError::message(format!(
                "unknown rust framework: {f}\n  valid: quasar-svm, mollusk"
            )));
        }
    }
    if let Some(ref s) = ts_sdk_override {
        if !matches!(s.as_str(), "kit" | "web3.js") {
            return Err(CliError::message(format!(
                "unknown TypeScript SDK: {s}\n  valid: kit, web3.js"
            )));
        }
    }
    if let Some(ref t) = template_override {
        if !matches!(t.as_str(), "minimal" | "full") {
            return Err(CliError::message(format!(
                "unknown template: {t}\n  valid: minimal, full"
            )));
        }
    }
    if let Some(ref t) = toolchain_override {
        if !matches!(t.as_str(), "solana" | "upstream") {
            return Err(CliError::message(format!(
                "unknown toolchain: {t}\n  valid: solana, upstream"
            )));
        }
    }

    if globals.ui.animation && !skip_prompts {
        banner::print_banner();
    }

    let theme = ColorfulTheme::default();

    // Project name
    let name: String = if skip_prompts {
        name.ok_or_else(|| {
            CliError::message(
                "a project name is required when using flags\n  usage: quasar init <name> \
                 [--test-language ...] [--template ...]",
            )
        })?
    } else {
        let mut prompt = Input::with_theme(&theme).with_prompt("Project name");
        if let Some(default) = name {
            prompt = prompt.default(default);
        }
        prompt.interact_text()?
    };

    // Validate the target directory before prompting for remaining options
    scaffold::validate_target_dir(&name)?;

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
            .interact()?
    };
    let toolchain = match toolchain_idx {
        0 => Toolchain::Solana,
        _ => Toolchain::Upstream,
    };

    // For upstream: sbpf-linker must be installed
    if matches!(toolchain, Toolchain::Upstream) && !toolchain::has_sbpf_linker() {
        return Err(CliError::message(
            "sbpf-linker not found.\n\n  Install platform-tools first:\n    git clone https://github.com/anza-xyz/platform-tools\n    cd platform-tools\n    cargo install-with-gallery",
        ));
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
            .interact()?
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
                .interact()?
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
                .interact()?
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
                .interact()?
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
                    .interact_text()?;
                let test: String = Input::with_theme(&theme)
                    .with_prompt("Test command")
                    .default("pnpm test".into())
                    .interact_text()?;
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
            .interact()?;

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
            .interact()?
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
            .interact()?;
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

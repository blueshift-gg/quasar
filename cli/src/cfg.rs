use {
    crate::{config::GlobalConfig, error::CliResult, style, ConfigAction},
    dialoguer::{theme::ColorfulTheme, Select},
};

pub fn run(action: Option<ConfigAction>) -> CliResult {
    let mut config = GlobalConfig::load();

    match action {
        // No subcommand: interactive menu
        None => run_interactive(&mut config)?,
        Some(ConfigAction::Get { key }) => {
            let val = get_value(&config, &key);
            match val {
                Some(v) => println!("{v}"),
                None => unknown_key(&key),
            }
        }
        Some(ConfigAction::Set { key, value }) => {
            if let Err(valid) = validate_value(&key, &value) {
                eprintln!(
                    "  {}",
                    style::fail(&format!("invalid value for {key}: {value}"))
                );
                eprintln!("  {}", style::dim(&format!("valid: {valid}")));
                std::process::exit(1);
            }
            if set_value(&mut config, &key, &value) {
                config.save()?;
                println!("  {}", style::success(&format!("{key} = {value}")));
            } else {
                unknown_key(&key);
            }
        }
        Some(ConfigAction::List) => print_all(&config),
        Some(ConfigAction::Reset) => {
            let was_animated = config.ui.animation;
            config = GlobalConfig::default();
            // Preserve animation=false once it's been shown
            if !was_animated {
                config.ui.animation = false;
            }
            config.save()?;
            println!("  {}", style::success("config reset to defaults"));
            println!();
            print_all(&config);
        }
    }

    Ok(())
}

fn unknown_key(key: &str) -> ! {
    eprintln!("  {}", style::fail(&format!("unknown config key: {key}")));
    eprintln!();
    eprintln!("  Available keys:");
    eprintln!("    defaults.toolchain, defaults.framework, defaults.template, defaults.git");
    eprintln!("    ui.animation, ui.color");
    std::process::exit(1);
}

fn print_all(config: &GlobalConfig) {
    let path = GlobalConfig::path();
    println!("  {}", style::dim(&format!("config: {}", path.display())));
    println!();
    println!("  [defaults]");
    println!(
        "    toolchain  = {}",
        config.defaults.toolchain.as_deref().unwrap_or("(not set)")
    );
    println!(
        "    framework  = {}",
        config.defaults.framework.as_deref().unwrap_or("(not set)")
    );
    println!(
        "    template   = {}",
        config.defaults.template.as_deref().unwrap_or("(not set)")
    );
    println!(
        "    git        = {}",
        config.defaults.git.as_deref().unwrap_or("(not set)")
    );
    println!();
    println!("  [ui]");
    println!("    animation  = {}", config.ui.animation);
    println!("    color      = {}", config.ui.color);
}

// ---------------------------------------------------------------------------
// Interactive config menu
// ---------------------------------------------------------------------------

struct ConfigItem {
    key: &'static str,
    label: &'static str,
    kind: ConfigKind,
}

enum ConfigKind {
    Bool,
    Choice(&'static [&'static str]),
}

const ITEMS: &[ConfigItem] = &[
    ConfigItem {
        key: "defaults.toolchain",
        label: "Default toolchain",
        kind: ConfigKind::Choice(&["solana", "upstream"]),
    },
    ConfigItem {
        key: "defaults.framework",
        label: "Default test framework",
        kind: ConfigKind::Choice(&[
            "none",
            "mollusk",
            "quasarsvm-rust",
            "quasarsvm-web3js",
            "quasarsvm-kit",
        ]),
    },
    ConfigItem {
        key: "defaults.template",
        label: "Default template",
        kind: ConfigKind::Choice(&["minimal", "full"]),
    },
    ConfigItem {
        key: "defaults.git",
        label: "Default git setup",
        kind: ConfigKind::Choice(&["commit", "init", "skip"]),
    },
    ConfigItem {
        key: "ui.animation",
        label: "Show init animation",
        kind: ConfigKind::Bool,
    },
    ConfigItem {
        key: "ui.color",
        label: "Colored output",
        kind: ConfigKind::Bool,
    },
];

fn run_interactive(config: &mut GlobalConfig) -> CliResult {
    let theme = ColorfulTheme::default();
    let path = GlobalConfig::path();

    loop {
        let items: Vec<String> = ITEMS
            .iter()
            .map(|item| {
                let val = get_value(config, item.key).unwrap_or_default();
                format!("{:<24} {}", item.label, style::dim(&val))
            })
            .chain(std::iter::once(String::from("Exit")))
            .collect();

        println!();
        println!("  {}", style::dim(&format!("config: {}", path.display())));

        let selection = Select::with_theme(&theme)
            .with_prompt("  Settings")
            .items(&items)
            .default(0)
            .interact_opt()
            .unwrap_or(None);

        let Some(idx) = selection else {
            break;
        };

        if idx >= ITEMS.len() {
            break;
        }

        let item = &ITEMS[idx];
        let changed = match &item.kind {
            ConfigKind::Bool => toggle_bool(config, item, &theme),
            ConfigKind::Choice(options) => pick_choice(config, item, options, &theme),
        };

        if changed {
            config.save()?;
            println!("  {}", style::success(&format!("{} saved", item.key)));
        }
    }

    Ok(())
}

fn toggle_bool(config: &mut GlobalConfig, item: &ConfigItem, theme: &ColorfulTheme) -> bool {
    let current = get_value(config, item.key).unwrap_or_default();
    let current_bool = current == "true";
    let options = ["true", "false"];
    let default = if current_bool { 0 } else { 1 };

    let sel = Select::with_theme(theme)
        .with_prompt(format!("  {}", item.label))
        .items(&options)
        .default(default)
        .interact_opt()
        .unwrap_or(None);

    if let Some(idx) = sel {
        let new_val = options[idx];
        if new_val != current {
            set_value(config, item.key, new_val);
            return true;
        }
    }
    false
}

fn pick_choice(
    config: &mut GlobalConfig,
    item: &ConfigItem,
    options: &[&str],
    theme: &ColorfulTheme,
) -> bool {
    let current = get_value(config, item.key).unwrap_or_default();
    let default = options.iter().position(|&o| o == current).unwrap_or(0);

    let sel = Select::with_theme(theme)
        .with_prompt(format!("  {}", item.label))
        .items(options)
        .default(default)
        .interact_opt()
        .unwrap_or(None);

    if let Some(idx) = sel {
        let new_val = options[idx];
        if new_val != current {
            set_value(config, item.key, new_val);
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Get / Set helpers
// ---------------------------------------------------------------------------

fn get_value(config: &GlobalConfig, key: &str) -> Option<String> {
    match key {
        "defaults.toolchain" => Some(
            config
                .defaults
                .toolchain
                .as_deref()
                .unwrap_or("(not set)")
                .to_string(),
        ),
        "defaults.framework" => Some(
            config
                .defaults
                .framework
                .as_deref()
                .unwrap_or("(not set)")
                .to_string(),
        ),
        "defaults.template" => Some(
            config
                .defaults
                .template
                .as_deref()
                .unwrap_or("(not set)")
                .to_string(),
        ),
        "defaults.git" => Some(
            config
                .defaults
                .git
                .as_deref()
                .unwrap_or("(not set)")
                .to_string(),
        ),
        "ui.animation" => Some(config.ui.animation.to_string()),
        "ui.color" => Some(config.ui.color.to_string()),
        _ => None,
    }
}

fn set_value(config: &mut GlobalConfig, key: &str, value: &str) -> bool {
    match key {
        "defaults.toolchain" => config.defaults.toolchain = some_or_none(value),
        "defaults.framework" => config.defaults.framework = some_or_none(value),
        "defaults.template" => config.defaults.template = some_or_none(value),
        "defaults.git" => config.defaults.git = some_or_none(value),
        "ui.animation" => config.ui.animation = parse_bool(value),
        "ui.color" => config.ui.color = parse_bool(value),
        _ => return false,
    }
    true
}

/// Returns Ok(()) if valid, Err(valid_options_string) if not.
fn validate_value(key: &str, value: &str) -> Result<(), &'static str> {
    match key {
        "defaults.toolchain" => {
            if matches!(value, "solana" | "upstream" | "none" | "null" | "") {
                Ok(())
            } else {
                Err("solana, upstream")
            }
        }
        "defaults.framework" => {
            if matches!(
                value,
                "none"
                    | "mollusk"
                    | "quasarsvm-rust"
                    | "quasarsvm-web3js"
                    | "quasarsvm-kit"
                    | "null"
                    | ""
            ) {
                Ok(())
            } else {
                Err("none, mollusk, quasarsvm-rust, quasarsvm-web3js, quasarsvm-kit")
            }
        }
        "defaults.template" => {
            if matches!(value, "minimal" | "full" | "none" | "null" | "") {
                Ok(())
            } else {
                Err("minimal, full")
            }
        }
        "defaults.git" => {
            if matches!(value, "commit" | "init" | "skip" | "none" | "null" | "") {
                Ok(())
            } else {
                Err("commit, init, skip")
            }
        }
        "ui.animation" | "ui.color" => {
            if matches!(
                value,
                "true" | "false" | "1" | "0" | "yes" | "no" | "on" | "off"
            ) {
                Ok(())
            } else {
                Err("true, false")
            }
        }
        _ => Ok(()), // unknown keys are handled elsewhere
    }
}

fn some_or_none(s: &str) -> Option<String> {
    if s.is_empty() || s == "none" || s == "null" {
        None
    } else {
        Some(s.to_string())
    }
}

fn parse_bool(s: &str) -> bool {
    matches!(s, "true" | "1" | "yes" | "on")
}

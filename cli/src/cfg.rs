use crate::{
    config::GlobalConfig,
    error::{CliError, CliResult},
    style, ConfigAction,
};

pub fn run(action: Option<ConfigAction>) -> CliResult {
    let mut config = GlobalConfig::load()?;
    match action.unwrap_or(ConfigAction::List) {
        ConfigAction::Get { key } => println!("{}", get_value(&config, &key)?),
        ConfigAction::Set { key, value } => {
            set_value(&mut config, &key, &value)?;
            config.save()?;
            println!("  {}", style::success(&format!("{key} = {value}")));
        }
        ConfigAction::List => print_all(&config),
        ConfigAction::Reset => {
            config = GlobalConfig::default();
            config.save()?;
            println!("  {}", style::success("config reset to defaults"));
            print_all(&config);
        }
    }
    Ok(())
}

fn get_value(config: &GlobalConfig, key: &str) -> Result<String, CliError> {
    match key {
        "ui.color" => Ok(config.ui.color.to_string()),
        _ => Err(unknown_key(key)),
    }
}

fn set_value(config: &mut GlobalConfig, key: &str, value: &str) -> CliResult {
    match key {
        "ui.color" => {
            config.ui.color = match value {
                "true" | "1" | "yes" | "on" => true,
                "false" | "0" | "no" | "off" => false,
                _ => {
                    return Err(CliError::message(format!(
                        "invalid value for ui.color: {value}\n  valid: true, false"
                    )));
                }
            };
            Ok(())
        }
        _ => Err(unknown_key(key)),
    }
}

fn print_all(config: &GlobalConfig) {
    println!(
        "  {}",
        style::dim(&format!("config: {}", GlobalConfig::path().display()))
    );
    println!();
    println!("  [ui]");
    println!("    color = {}", config.ui.color);
}

fn unknown_key(key: &str) -> CliError {
    CliError::message(format!(
        "unknown config key: {key}\n\n  Available keys:\n    ui.color"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_color_is_a_global_default() {
        let config = GlobalConfig::default();
        assert_eq!(get_value(&config, "ui.color").unwrap(), "true");
        assert!(get_value(&config, "defaults.toolchain").is_err());
    }

    #[test]
    fn color_rejects_untyped_values() {
        let mut config = GlobalConfig::default();
        let error = set_value(&mut config, "ui.color", "sometimes").unwrap_err();
        assert!(error.to_string().contains("true, false"));
    }
}

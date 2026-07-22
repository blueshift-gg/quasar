use {
    crate::{
        config::{CommandSpec, QuasarConfig},
        error::{CliError, CliResult},
        style, utils,
    },
    std::{ffi::OsStr, process::Command},
};

pub fn run(
    debug: bool,
    show_output: bool,
    filter: Option<String>,
    watch: bool,
    no_build: bool,
    features: Option<String>,
    verbose: bool,
) -> CliResult {
    if watch {
        crate::build::watch_loop(|| {
            run_once(
                debug,
                show_output,
                filter.as_deref(),
                no_build,
                features.as_deref(),
                verbose,
            )
        });
    }
    run_once(
        debug,
        show_output,
        filter.as_deref(),
        no_build,
        features.as_deref(),
        verbose,
    )
}

fn run_once(
    debug: bool,
    show_output: bool,
    filter: Option<&str>,
    no_build: bool,
    features: Option<&str>,
    verbose: bool,
) -> CliResult {
    let config = QuasarConfig::load()?;
    if !no_build {
        crate::build::run(debug, verbose, false, features.map(String::from))?;
    }
    let program_path = compiled_program_path(&config)?;
    let command = effective_test_command(&config.testing.command, filter, show_output, features);
    run_command_with_env(
        &command,
        verbose,
        ("QUASAR_PROGRAM_PATH", program_path.as_os_str()),
    )
}

fn compiled_program_path(config: &QuasarConfig) -> Result<std::path::PathBuf, CliError> {
    let program_path = utils::find_so(config, false).ok_or_else(|| {
        CliError::message(
            "compiled program not found; run `quasar test` without `--no-build` or build the \
             program first",
        )
    })?;
    program_path.canonicalize().map_err(|error| {
        CliError::message(format!(
            "failed to resolve compiled program {}: {error}",
            program_path.display()
        ))
    })
}

fn run_command_with_env(
    command: &CommandSpec,
    verbose: bool,
    environment: (&str, &OsStr),
) -> CliResult {
    eprintln!(
        "  {}",
        style::step(&format!("Running {}...", command.display()))
    );
    if verbose {
        eprintln!("  {}", style::dim(&format!("$ {}", command.display())));
    }
    let status = Command::new(&command.program)
        .args(&command.args)
        .env(environment.0, environment.1)
        .status();
    match status {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(CliError::process_failure(
            format!("{} failed", command.display()),
            status.code().unwrap_or(1),
        )),
        Err(error) => Err(CliError::message(format!(
            "failed to run {}: {error}",
            command.display()
        ))),
    }
}

fn effective_test_command(
    command: &CommandSpec,
    filter: Option<&str>,
    show_output: bool,
    features: Option<&str>,
) -> CommandSpec {
    let mut args = command.args.clone();
    let mut separator = args.iter().position(|argument| argument == "--");
    if let Some(features) = features.filter(|_| is_cargo_test(command)) {
        let index = separator.unwrap_or(args.len());
        args.splice(
            index..index,
            ["--features".to_string(), features.to_string()],
        );
        separator = separator.map(|index| index + 2);
    }
    if let Some(pattern) = filter {
        match separator {
            Some(index) => {
                args.insert(index, pattern.to_string());
                separator = Some(index + 1);
            }
            None => args.push(pattern.to_string()),
        }
    }
    if show_output {
        match separator {
            Some(index) => args.insert(index + 1, "--show-output".to_string()),
            None => args.extend(["--".to_string(), "--show-output".to_string()]),
        }
    }
    CommandSpec::new(command.program.clone(), args)
}

fn is_cargo_test(command: &CommandSpec) -> bool {
    command.program == "cargo"
        && command
            .args
            .first()
            .is_some_and(|argument| argument == "test")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_preserves_the_complete_cargo_test_graph() {
        let command = CommandSpec::new("cargo", ["test", "--", "--nocapture"]);
        let effective = effective_test_command(&command, Some("my_test"), false, None);
        assert_eq!(effective.args, vec!["test", "my_test", "--", "--nocapture"]);
    }

    #[test]
    fn show_output_reuses_existing_separator() {
        let command = CommandSpec::new("cargo", ["test", "--", "--nocapture"]);
        let effective = effective_test_command(&command, None, true, None);
        assert_eq!(
            effective.args,
            vec!["test", "--", "--show-output", "--nocapture"]
        );
    }

    #[test]
    fn features_apply_to_the_program_and_host_test_builds() {
        let command = CommandSpec::new("cargo", ["test"]);
        let effective = effective_test_command(&command, None, false, Some("token-2022,debug"));
        assert_eq!(
            effective.args,
            vec!["test", "--features", "token-2022,debug"]
        );
    }

    #[test]
    fn feature_flags_do_not_mutate_custom_test_commands() {
        let command = CommandSpec::new("nextest", ["run"]);
        let effective = effective_test_command(&command, None, false, Some("debug"));
        assert_eq!(effective.args, vec!["run"]);
    }

    #[test]
    fn command_is_never_deserialized_from_a_shell_string() {
        let error = QuasarConfig::parse("[testing]\ncommand = \"cargo test\"\n").unwrap_err();
        assert!(error.contains("invalid type"));
    }
}

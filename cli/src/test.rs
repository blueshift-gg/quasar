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
    let command = effective_test_command(&config.testing.command, filter, show_output);
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
) -> CommandSpec {
    let mut args = command.args.clone();
    let mut separator = args.iter().position(|argument| argument == "--");
    if let Some(pattern) = filter {
        let command_arg_end = separator.unwrap_or(args.len());
        if let Some(index) = args[..command_arg_end]
            .iter()
            .position(|argument| argument == "tests::")
        {
            args[index].push_str(pattern.strip_prefix("tests::").unwrap_or(pattern));
        } else {
            match separator {
                Some(index) => {
                    args.insert(index, pattern.to_string());
                    separator = Some(index + 1);
                }
                None => args.push(pattern.to_string()),
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_is_combined_with_generated_scope() {
        let command = CommandSpec::new("cargo", ["test", "tests::", "--", "--nocapture"]);
        let effective = effective_test_command(&command, Some("my_test"), false);
        assert_eq!(
            effective.args,
            vec!["test", "tests::my_test", "--", "--nocapture"]
        );
    }

    #[test]
    fn show_output_reuses_existing_separator() {
        let command = CommandSpec::new("cargo", ["test", "tests::", "--", "--nocapture"]);
        let effective = effective_test_command(&command, None, true);
        assert_eq!(
            effective.args,
            vec!["test", "tests::", "--", "--show-output", "--nocapture"]
        );
    }

    #[test]
    fn command_is_never_deserialized_from_a_shell_string() {
        let error = QuasarConfig::parse("[testing]\ncommand = \"cargo test\"\n").unwrap_err();
        assert!(error.contains("invalid type"));
    }
}

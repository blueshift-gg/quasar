use {
    crate::{
        config::{CommandSpec, QuasarConfig},
        error::{CliError, CliResult},
        style, utils,
    },
    std::{ffi::OsStr, path::Path, process::Command},
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
        run_watch(debug, show_output, filter, no_build, features, verbose);
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

    if config.has_typescript_tests() {
        run_typescript_tests(&config, filter, show_output, verbose)
    } else if config.has_rust_tests() {
        run_rust_tests(&config, filter, show_output, verbose)
    } else {
        println!("  {}", style::warn("no test framework configured"));
        Ok(())
    }
}

fn run_watch(
    debug: bool,
    show_output: bool,
    filter: Option<String>,
    no_build: bool,
    features: Option<String>,
    verbose: bool,
) -> ! {
    crate::build::watch_loop(|| {
        run_once(
            debug,
            show_output,
            filter.as_deref(),
            no_build,
            features.as_deref(),
            verbose,
        )
    })
}

fn run_typescript_tests(
    config: &QuasarConfig,
    filter: Option<&str>,
    show_output: bool,
    verbose: bool,
) -> CliResult {
    let ts = config.testing.typescript.as_ref();
    let default_install = CommandSpec::new("npm", ["install"]);
    let default_test = CommandSpec::new("npx", ["vitest", "run"]);
    let install_cmd = ts.map(|t| &t.install).unwrap_or(&default_install);
    let test_cmd = ts.map(|t| &t.test).unwrap_or(&default_test);

    if !std::path::Path::new("node_modules").exists() {
        run_command(install_cmd, verbose)?;
    }

    let program_path = compiled_program_path(config)?;
    let command = effective_test_command(test_cmd, filter, show_output);
    run_command_with_env(
        &command,
        verbose,
        Some(("QUASAR_PROGRAM_PATH", program_path.as_os_str())),
    )
}

fn run_rust_tests(
    config: &QuasarConfig,
    filter: Option<&str>,
    show_output: bool,
    verbose: bool,
) -> CliResult {
    let default_test = CommandSpec::new("cargo", ["test", "tests::"]);
    let test_cmd = config
        .testing
        .rust
        .as_ref()
        .map(|r| &r.test)
        .unwrap_or(&default_test);

    let program_path = compiled_program_path(config)?;
    let command = effective_test_command(test_cmd, filter, show_output);
    run_command_with_env(
        &command,
        verbose,
        Some(("QUASAR_PROGRAM_PATH", program_path.as_os_str())),
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

fn run_command(command: &CommandSpec, verbose: bool) -> CliResult {
    run_command_with_env(command, verbose, None)
}

fn run_command_with_env(
    command: &CommandSpec,
    verbose: bool,
    environment: Option<(&str, &OsStr)>,
) -> CliResult {
    eprintln!(
        "  {}",
        style::step(&format!("Running {}...", command.display()))
    );
    if verbose {
        eprintln!("  {}", style::dim(&format!("$ {}", command.display())));
    }

    let mut process = Command::new(&command.program);
    process.args(&command.args);
    if let Some((key, value)) = environment {
        process.env(key, value);
    }
    let status = process.status();

    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(CliError::process_failure(
            format!("{} failed", command.display()),
            s.code().unwrap_or(1),
        )),
        Err(e) => Err(CliError::message(format!(
            "failed to run {}: {e}",
            command.display()
        ))),
    }
}

fn effective_test_command(
    test_cmd: &CommandSpec,
    filter: Option<&str>,
    show_output: bool,
) -> CommandSpec {
    CommandSpec::new(
        test_cmd.program.clone(),
        test_command_args(test_cmd, filter, show_output),
    )
}

fn test_command_args(
    test_cmd: &CommandSpec,
    filter: Option<&str>,
    show_output: bool,
) -> Vec<String> {
    let mut args = test_cmd.args.clone();

    if is_cargo_program(&test_cmd.program) {
        let mut separator = args.iter().position(|arg| arg == "--");

        if let Some(pattern) = filter {
            let command_arg_end = separator.unwrap_or(args.len());
            if let Some(index) = args[..command_arg_end]
                .iter()
                .position(|arg| arg == "tests::")
            {
                let pattern = pattern.strip_prefix("tests::").unwrap_or(pattern);
                args[index].push_str(pattern);
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
                None => {
                    args.push("--".to_string());
                    args.push("--show-output".to_string());
                }
            }
        }
    } else if let Some(pattern) = filter {
        if is_npm_script_command(test_cmd) && !args.iter().any(|arg| arg == "--") {
            args.push("--".to_string());
        }
        args.extend(["-t".to_string(), pattern.to_string()]);
    }

    args
}

fn is_cargo_program(program: &str) -> bool {
    Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "cargo" || name == "cargo.exe")
}

fn is_npm_script_command(command: &CommandSpec) -> bool {
    let Some(program) = Path::new(&command.program)
        .file_name()
        .and_then(|name| name.to_str())
    else {
        return false;
    };
    matches!(program, "npm" | "npm.cmd")
        && matches!(
            command.args.first().map(String::as_str),
            Some("test" | "run")
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_filter_is_combined_with_generated_scope() {
        let cmd = CommandSpec::new("cargo", ["test", "tests::", "--", "--nocapture"]);
        let args = test_command_args(&cmd, Some("my_test"), false);

        assert_eq!(args, vec!["test", "tests::my_test", "--", "--nocapture"]);
    }

    #[test]
    fn fully_scoped_cargo_filter_is_not_duplicated() {
        let cmd = CommandSpec::new("cargo", ["test", "tests::"]);
        let args = test_command_args(&cmd, Some("tests::my_test"), false);

        assert_eq!(args, vec!["test", "tests::my_test"]);
    }

    #[test]
    fn cargo_filter_without_generated_scope_is_appended() {
        let cmd = CommandSpec::new("cargo", ["test", "--lib"]);
        let args = test_command_args(&cmd, Some("my_test"), false);

        assert_eq!(args, vec!["test", "--lib", "my_test"]);
    }

    #[test]
    fn cargo_show_output_reuses_existing_separator() {
        let cmd = CommandSpec::new("cargo", ["test", "tests::", "--", "--nocapture"]);
        let args = test_command_args(&cmd, None, true);

        assert_eq!(
            args,
            vec!["test", "tests::", "--", "--show-output", "--nocapture"]
        );
    }

    #[test]
    fn cargo_filter_and_show_output_keep_test_binary_args_ordered() {
        let cmd = CommandSpec::new("cargo", ["test", "tests::", "--", "--nocapture"]);
        let args = test_command_args(&cmd, Some("my_test"), true);

        assert_eq!(
            args,
            vec![
                "test",
                "tests::my_test",
                "--",
                "--show-output",
                "--nocapture"
            ]
        );
    }

    #[test]
    fn effective_cargo_test_command_includes_runtime_arguments() {
        let cmd = CommandSpec::new("cargo", ["test", "tests::", "--", "--nocapture"]);
        let command = effective_test_command(&cmd, Some("my_test"), true);

        assert_eq!(
            command.display(),
            "cargo test tests::my_test -- --show-output --nocapture"
        );
    }

    #[test]
    fn non_cargo_filter_uses_t_flag() {
        let cmd = CommandSpec::new("npx", ["vitest", "run"]);
        let args = test_command_args(&cmd, Some("my_test"), true);

        assert_eq!(args, vec!["vitest", "run", "-t", "my_test"]);
    }

    #[test]
    fn effective_non_cargo_test_command_includes_runtime_filter() {
        let cmd = CommandSpec::new("npx", ["vitest", "run"]);
        let command = effective_test_command(&cmd, Some("my_test"), true);

        assert_eq!(command.display(), "npx vitest run -t my_test");
    }

    #[test]
    fn npm_script_filter_is_forwarded_to_the_test_runner() {
        let cmd = CommandSpec::new("npm", ["test"]);
        let command = effective_test_command(&cmd, Some("my_test"), false);

        assert_eq!(command.display(), "npm test -- -t my_test");
    }

    #[test]
    fn other_package_managers_forward_script_arguments_without_a_separator() {
        for cmd in [
            CommandSpec::new("pnpm", ["test"]),
            CommandSpec::new("yarn", ["test"]),
            CommandSpec::new("bun", ["run", "test"]),
        ] {
            let command = effective_test_command(&cmd, Some("my_test"), false);
            assert_eq!(command.args.last().map(String::as_str), Some("my_test"));
            assert!(!command.args.iter().any(|arg| arg == "--"));
        }
    }

    #[test]
    fn cargo_executable_path_is_treated_like_cargo() {
        let cmd = CommandSpec::new("/usr/bin/cargo", ["test"]);
        let args = test_command_args(&cmd, None, true);

        assert_eq!(args, vec!["test", "--", "--show-output"]);
    }
}

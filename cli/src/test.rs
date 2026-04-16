use {
    crate::{
        config::{CommandSpec, QuasarConfig},
        error::{CliError, CliResult},
        style,
    },
    std::{path::Path, process::Command},
};

pub fn run(
    debug: bool,
    show_output: bool,
    filter: Option<String>,
    watch: bool,
    no_build: bool,
    features: Option<String>,
) -> CliResult {
    if watch {
        run_watch(debug, show_output, filter, no_build, features);
    }
    run_once(
        debug,
        show_output,
        filter.as_deref(),
        no_build,
        features.as_deref(),
    )
}

fn run_once(
    debug: bool,
    show_output: bool,
    filter: Option<&str>,
    no_build: bool,
    features: Option<&str>,
) -> CliResult {
    let config = QuasarConfig::load()?;

    if !no_build {
        crate::build::run(debug, false, false, features.map(String::from), false)?;
    }

    if config.has_typescript_tests() {
        run_typescript_tests(&config, filter, show_output)
    } else if config.has_rust_tests() {
        run_rust_tests(&config, filter, show_output)
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
) -> ! {
    crate::build::watch_loop(|| {
        run_once(
            debug,
            show_output,
            filter.as_deref(),
            no_build,
            features.as_deref(),
        )
    })
}

// ---------------------------------------------------------------------------
// TypeScript (vitest)
// ---------------------------------------------------------------------------

fn run_typescript_tests(
    config: &QuasarConfig,
    filter: Option<&str>,
    show_output: bool,
) -> CliResult {
    let ts = config.testing.typescript.as_ref();
    let default_install = CommandSpec::new("npm", ["install"]);
    let default_test = CommandSpec::new("npx", ["vitest", "run"]);
    let install_cmd = ts.map(|t| &t.install).unwrap_or(&default_install);
    let test_cmd = ts.map(|t| &t.test).unwrap_or(&default_test);

    if !std::path::Path::new("node_modules").exists() {
        run_command(install_cmd)?;
    }

    run_test_cmd(test_cmd, filter, show_output)
}

// ---------------------------------------------------------------------------
// Rust (cargo test)
// ---------------------------------------------------------------------------

fn run_rust_tests(config: &QuasarConfig, filter: Option<&str>, show_output: bool) -> CliResult {
    let default_test = CommandSpec::new("cargo", ["test", "tests::"]);
    let test_cmd = config
        .testing
        .rust
        .as_ref()
        .map(|r| &r.test)
        .unwrap_or(&default_test);

    run_test_cmd(test_cmd, filter, show_output)
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn run_command(command: &CommandSpec) -> CliResult {
    let status = Command::new(&command.program).args(&command.args).status();

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

fn run_test_cmd(test_cmd: &CommandSpec, filter: Option<&str>, show_output: bool) -> CliResult {
    let mut cmd = Command::new(&test_cmd.program);
    cmd.args(test_command_args(test_cmd, filter, show_output));

    let status = cmd.status();

    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(CliError::process_failure(
            format!("{} failed", test_cmd.display()),
            s.code().unwrap_or(1),
        )),
        Err(e) => Err(CliError::message(format!(
            "failed to run {}: {e}",
            test_cmd.display()
        ))),
    }
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
                None => {
                    args.push("--".to_string());
                    args.push("--show-output".to_string());
                }
            }
        }
    } else if let Some(pattern) = filter {
        args.push("-t".to_string());
        args.push(pattern.to_string());
    }

    args
}

fn is_cargo_program(program: &str) -> bool {
    Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "cargo" || name == "cargo.exe")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_filter_is_inserted_before_existing_separator() {
        let cmd = CommandSpec::new("cargo", ["test", "tests::", "--", "--nocapture"]);
        let args = test_command_args(&cmd, Some("my_test"), false);

        assert_eq!(
            args,
            vec!["test", "tests::", "my_test", "--", "--nocapture"]
        );
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
                "tests::",
                "my_test",
                "--",
                "--show-output",
                "--nocapture"
            ]
        );
    }

    #[test]
    fn non_cargo_filter_uses_t_flag() {
        let cmd = CommandSpec::new("npx", ["vitest", "run"]);
        let args = test_command_args(&cmd, Some("my_test"), true);

        assert_eq!(args, vec!["vitest", "run", "-t", "my_test"]);
    }

    #[test]
    fn cargo_executable_path_is_treated_like_cargo() {
        let cmd = CommandSpec::new("/usr/bin/cargo", ["test"]);
        let args = test_command_args(&cmd, None, true);

        assert_eq!(args, vec!["test", "--", "--show-output"]);
    }
}

#[path = "build_diagnostics.rs"]
mod diagnostics;
#[path = "build_lockfile.rs"]
mod lockfile;
#[path = "build_watch.rs"]
mod watch;

pub(crate) use watch::watch_loop;
/// platform-tools v1.52 ships Cargo 1.89 which supports Cargo.lock v4
/// and handles edition-2024 crate manifests in the Solana dep tree.
const PLATFORM_TOOLS_VERSION: &str = "v1.52";
use {
    crate::{
        config::QuasarConfig,
        error::{CliError, CliResult},
        style, toolchain, utils,
    },
    diagnostics::{extract_warnings, format_build_errors},
    lockfile::ensure_lockfile,
    std::{
        fs,
        path::{Path, PathBuf},
        process::{Command, ExitStatus, Output, Stdio},
        time::Instant,
    },
};

enum BuildResult {
    Captured(Output),
    Streamed(ExitStatus),
}

pub fn run(debug: bool, verbose: bool, watch: bool, features: Option<String>) -> CliResult {
    if watch {
        run_watch(debug, verbose, features);
    }

    run_once(debug, verbose, features.as_deref())
}

fn run_once(debug: bool, verbose: bool, features: Option<&str>) -> CliResult {
    let config = QuasarConfig::load()?;
    let clients_path = config.client_path();
    let start = Instant::now();
    let mut progress = style::Progress::new(verbose);

    let languages = config.codegen_targets();
    let crate_root = utils::find_program_crate(&config);
    progress.step("Generating IDL and clients...");
    let idl = crate::idl::generate(&crate_root, &languages, &clients_path)?;
    progress.done("Generated IDL and clients");
    progress.step("Linting program surface...");
    crate::lint::run_for_build(&crate_root, &idl)?;
    progress.done("Linted program surface");

    if verbose {
        eprintln!("  {}", style::step("Building program..."));
    }
    let sp = if verbose {
        indicatif::ProgressBar::hidden()
    } else {
        style::spinner("Building...")
    };

    toolchain::check_build_sbf_supports(PLATFORM_TOOLS_VERSION).map_err(|error| {
        sp.finish_and_clear();
        CliError::message(error)
    })?;
    ensure_lockfile(&sp)?;

    // In a workspace, scope the build to the program crate so we don't try
    // to compile CLIs, test suites, or other members for the BPF target.
    let manifest = crate_root.join("Cargo.toml");
    let scoped = manifest.exists() && crate_root != Path::new(".");

    let mut command = Command::new("cargo");
    command.args(["build-sbf", "--tools-version", PLATFORM_TOOLS_VERSION]);
    if scoped {
        command.args(["--manifest-path", &manifest.to_string_lossy()]);
    }
    if debug {
        command.arg("--debug");
    }
    if let Some(features) = features {
        command.args(["--features", features]);
    }
    let output = run_build_command(&mut command, verbose);

    sp.finish_and_clear();
    progress.clear();

    match output {
        Ok(BuildResult::Captured(o)) if o.status.success() => {
            let elapsed = start.elapsed();

            // Show warnings even on success
            let stderr = String::from_utf8_lossy(&o.stderr);
            let warnings = extract_warnings(&stderr);
            if !warnings.is_empty() {
                eprintln!();
                for line in &warnings {
                    eprintln!("  {line}");
                }
            }

            let so_path = utils::find_so(&config, false);
            let size_info = so_path
                .and_then(|p| {
                    let meta = fs::metadata(&p).ok()?;
                    let new_size = meta.len();
                    let delta = size_delta(&p, new_size);
                    save_last_size(&p, new_size);
                    Some(format!(
                        " ({}{delta})",
                        style::dim(&style::human_size(new_size))
                    ))
                })
                .unwrap_or_default();

            println!(
                "  {}",
                style::success(&format!(
                    "Build complete in {}{size_info}",
                    style::bold(&style::human_duration(elapsed))
                ))
            );
            Ok(())
        }
        Ok(BuildResult::Captured(o)) => {
            let elapsed = start.elapsed();
            let stderr = String::from_utf8_lossy(&o.stderr);
            Err(CliError::process_failure(
                format_build_errors(&stderr, elapsed),
                o.status.code().unwrap_or(1),
            ))
        }
        Ok(BuildResult::Streamed(status)) if status.success() => {
            let elapsed = start.elapsed();

            let so_path = utils::find_so(&config, false);
            let size_info = so_path
                .and_then(|p| {
                    let meta = fs::metadata(&p).ok()?;
                    let new_size = meta.len();
                    let delta = size_delta(&p, new_size);
                    save_last_size(&p, new_size);
                    Some(format!(
                        " ({}{delta})",
                        style::dim(&style::human_size(new_size))
                    ))
                })
                .unwrap_or_default();

            println!(
                "  {}",
                style::success(&format!(
                    "Build complete in {}{size_info}",
                    style::bold(&style::human_duration(elapsed))
                ))
            );
            Ok(())
        }
        Ok(BuildResult::Streamed(status)) => Err(CliError::process_failure(
            format!(
                "build failed after {}",
                style::human_duration(start.elapsed())
            ),
            status.code().unwrap_or(1),
        )),
        Err(e) => Err(CliError::message(format!(
            "failed to run build command: {e}"
        ))),
    }
}

/// Build with debug symbols only (no feature flags) for profiling.
/// Copies the .so to target/profile/ and returns the path.
pub fn profile_build() -> Result<PathBuf, crate::error::CliError> {
    let config = QuasarConfig::load()?;
    let clients_path = config.client_path();
    let start = Instant::now();

    let languages = config.codegen_targets();
    let crate_root = utils::find_program_crate(&config);
    crate::idl::generate(&crate_root, &languages, &clients_path)?;

    let sp = style::spinner("Profile build...");

    toolchain::check_build_sbf_supports(PLATFORM_TOOLS_VERSION).map_err(|error| {
        sp.finish_and_clear();
        CliError::message(error)
    })?;
    ensure_lockfile(&sp)?;

    let manifest = crate_root.join("Cargo.toml");
    let scoped = manifest.exists() && crate_root != Path::new(".");

    let mut command = Command::new("cargo");
    command.args([
        "build-sbf",
        "--tools-version",
        PLATFORM_TOOLS_VERSION,
        "--debug",
    ]);
    if scoped {
        command.args(["--manifest-path", &manifest.to_string_lossy()]);
    }
    let output = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    sp.finish_and_clear();

    match output {
        Ok(o) if o.status.success() => {
            let elapsed = start.elapsed();
            let program = config.module_name();
            let profile_dir = PathBuf::from(crate::profile::PROFILE_DIR);
            fs::create_dir_all(&profile_dir)?;

            // Find the built .so and copy to target/profile/
            let src = utils::find_unstripped_sbf(&config).ok_or_else(|| {
                CliError::message(
                    "profile build succeeded but no unstripped SBF artifact was found under \
                     target/deploy/debug",
                )
            })?;

            let dest = profile_dir.join(format!("{}.so", program));
            fs::copy(&src, &dest).map_err(|e| {
                eprintln!(
                    "  {}",
                    style::fail(&format!("failed to copy {}: {e}", src.display()))
                );
                e
            })?;

            let size = fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
            println!(
                "  {}",
                style::success(&format!(
                    "Profile build in {} ({})",
                    style::bold(&style::human_duration(elapsed)),
                    style::dim(&style::human_size(size))
                ))
            );

            Ok(dest)
        }
        Ok(o) => {
            let elapsed = start.elapsed();
            let stderr = String::from_utf8_lossy(&o.stderr);
            Err(CliError::process_failure(
                format_build_errors(&stderr, elapsed),
                o.status.code().unwrap_or(1),
            ))
        }
        Err(e) => Err(CliError::message(format!(
            "failed to run build command: {e}"
        ))),
    }
}

fn run_watch(debug: bool, verbose: bool, features: Option<String>) -> ! {
    watch_loop(|| run_once(debug, verbose, features.as_deref()))
}

fn run_build_command(cmd: &mut Command, verbose: bool) -> std::io::Result<BuildResult> {
    if verbose {
        let status = cmd
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()?;
        Ok(BuildResult::Streamed(status))
    } else {
        let output = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output()?;
        Ok(BuildResult::Captured(output))
    }
}

const LAST_SIZE_FILE: &str = "target/.quasar-last-size";

fn size_delta(so_path: &Path, new_size: u64) -> String {
    let key = so_path.to_string_lossy();
    let last = fs::read_to_string(LAST_SIZE_FILE)
        .ok()
        .and_then(|contents| {
            contents
                .lines()
                .find_map(|line| parse_size_entry(line, &key))
        });

    let Some(prev) = last else {
        return String::new();
    };

    if new_size == prev {
        return String::new();
    }

    let diff = new_size as i64 - prev as i64;
    if diff > 0 {
        format!(
            ", {}",
            style::color(196, &format!("+{}", style::human_size(diff as u64)))
        )
    } else {
        format!(
            ", {}",
            style::color(83, &format!("-{}", style::human_size((-diff) as u64)))
        )
    }
}

fn save_last_size(so_path: &Path, size: u64) {
    let key = so_path.to_string_lossy();
    let entry = format!("{key} {size}");

    // Read existing entries, replace or append
    let mut lines: Vec<String> = fs::read_to_string(LAST_SIZE_FILE)
        .unwrap_or_default()
        .lines()
        .filter(|line| !size_entry_matches(line, &key))
        .map(String::from)
        .collect();
    lines.push(entry);
    let _ = fs::write(LAST_SIZE_FILE, lines.join("\n"));
}

fn parse_size_entry(line: &str, key: &str) -> Option<u64> {
    let (entry_key, size) = line.rsplit_once(' ')?;
    (entry_key == key).then(|| size.parse().ok()).flatten()
}

fn size_entry_matches(line: &str, key: &str) -> bool {
    line.rsplit_once(' ')
        .is_some_and(|(entry_key, _)| entry_key == key)
}

#[cfg(test)]
mod tests {
    use super::{parse_size_entry, size_entry_matches};

    #[test]
    fn size_entry_match_is_exact_even_for_prefix_paths() {
        let key = "target/deploy/app.so";
        let longer = "target/deploy/app.so.backup 200";

        assert_eq!(parse_size_entry(longer, key), None);
        assert!(!size_entry_matches(longer, key));
    }

    #[test]
    fn size_entry_allows_spaces_in_paths() {
        let key = "target/deploy/my app.so";
        let line = "target/deploy/my app.so 1234";

        assert_eq!(parse_size_entry(line, key), Some(1234));
        assert!(size_entry_matches(line, key));
    }
}

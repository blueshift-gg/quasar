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
    lockfile::{ensure_lockfile, missing_sbpf_linker, read_target_rustflags},
    std::{
        fs,
        path::{Path, PathBuf},
        process::{Command, Stdio},
        time::Instant,
    },
};

pub fn run(debug: bool, watch: bool, features: Option<String>, lint: bool) -> CliResult {
    if watch {
        run_watch(debug, features);
    }

    run_once(debug, features.as_deref(), lint)
}

fn run_once(debug: bool, features: Option<&str>, lint_flag: bool) -> CliResult {
    let config = QuasarConfig::load()?;
    let start = Instant::now();

    let languages = config.client_languages();
    let crate_root = utils::find_program_crate(&config);
    let parsed = crate::idl::generate(&crate_root, &languages)?;

    if lint_flag || config.lint_enabled() {
        crate::lint::run_lint_on_parsed(&parsed, &quasar_idl::lint::LintConfig::default())?;
    }

    let sp = style::spinner("Building...");

    if config.is_solana_toolchain() {
        toolchain::check_build_sbf_supports(PLATFORM_TOOLS_VERSION).map_err(|e| {
            sp.finish_and_clear();
            CliError::message(e)
        })?;
        ensure_lockfile(&sp)?;
    }

    // In a workspace, scope the build to the program crate so we don't try
    // to compile CLIs, test suites, or other members for the BPF target.
    let manifest = crate_root.join("Cargo.toml");
    let scoped = manifest.exists() && crate_root != Path::new(".");

    let output = if config.is_solana_toolchain() {
        let mut cmd = Command::new("cargo");
        cmd.args(["build-sbf", "--tools-version", PLATFORM_TOOLS_VERSION]);
        if scoped {
            cmd.args(["--manifest-path", &manifest.to_string_lossy()]);
        }
        if debug {
            cmd.arg("--debug");
        }
        if let Some(f) = features {
            cmd.args(["--features", f]);
        }
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output()
    } else {
        if !toolchain::has_sbpf_linker() {
            sp.finish_and_clear();
            return Err(missing_sbpf_linker());
        }

        let mut cmd = Command::new("cargo");
        if debug {
            cmd.env("RUSTFLAGS", "-C link-arg=--btf -C debuginfo=2");
        }
        cmd.arg("build-bpf");
        if scoped {
            cmd.args(["--manifest-path", &manifest.to_string_lossy()]);
        }
        if let Some(f) = features {
            cmd.args(["--features", f]);
        }
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output()
    };

    sp.finish_and_clear();

    match output {
        Ok(o) if o.status.success() => {
            let elapsed = start.elapsed();

            if !config.is_solana_toolchain() {
                let program = config.module_name();
                let src = PathBuf::from("target")
                    .join("bpfel-unknown-none")
                    .join("release")
                    .join(format!("lib{}.so", program));
                let dest_dir = PathBuf::from("target").join("deploy");
                fs::create_dir_all(&dest_dir)?;
                let dest = dest_dir.join(format!("lib{}.so", program));
                fs::copy(&src, &dest).map_err(|e| {
                    eprintln!(
                        "  {}",
                        style::fail(&format!("failed to copy {}: {e}", src.display()))
                    );
                    e
                })?;
            }

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

/// Build with debug symbols only (no feature flags) for profiling.
/// Copies the .so to target/profile/ and returns the path.
pub fn profile_build() -> Result<PathBuf, crate::error::CliError> {
    let config = QuasarConfig::load()?;
    let start = Instant::now();

    let languages = config.client_languages();
    let crate_root = utils::find_program_crate(&config);
    let _parsed = crate::idl::generate(&crate_root, &languages)?;

    let sp = style::spinner("Profile build...");

    if config.is_solana_toolchain() {
        toolchain::check_build_sbf_supports(PLATFORM_TOOLS_VERSION).map_err(|e| {
            sp.finish_and_clear();
            CliError::message(e)
        })?;
        ensure_lockfile(&sp)?;
    }

    let manifest = crate_root.join("Cargo.toml");
    let scoped = manifest.exists() && crate_root != Path::new(".");

    let output = if config.is_solana_toolchain() {
        let mut cmd = Command::new("cargo");
        cmd.args([
            "build-sbf",
            "--tools-version",
            PLATFORM_TOOLS_VERSION,
            "--debug",
        ]);
        if scoped {
            cmd.args(["--manifest-path", &manifest.to_string_lossy()]);
        }
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output()
    } else {
        if !toolchain::has_sbpf_linker() {
            sp.finish_and_clear();
            return Err(missing_sbpf_linker());
        }

        // Read existing rustflags from .cargo/config.toml and append debug flags
        let existing_flags = read_target_rustflags();
        let mut all_flags = existing_flags;
        all_flags.extend([
            "-C".to_string(),
            "link-arg=--btf".to_string(),
            "-C".to_string(),
            "debuginfo=2".to_string(),
        ]);

        // Use CARGO_ENCODED_RUSTFLAGS (0x1f-separated) which takes priority
        let encoded = all_flags.join("\x1f");
        let mut cmd = Command::new("cargo");
        cmd.env("CARGO_ENCODED_RUSTFLAGS", encoded);
        cmd.arg("build-bpf");
        if scoped {
            cmd.args(["--manifest-path", &manifest.to_string_lossy()]);
        }
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output()
    };

    sp.finish_and_clear();

    match output {
        Ok(o) if o.status.success() => {
            let elapsed = start.elapsed();
            let program = config.module_name();
            let profile_dir = PathBuf::from("target").join("profile");
            fs::create_dir_all(&profile_dir)?;

            // Find the built .so and copy to target/profile/
            let src = if config.is_solana_toolchain() {
                // build-sbf --debug puts it in target/deploy/ or
                // target/sbf-solana-solana/release/
                utils::find_so(&config, false).unwrap_or_else(|| {
                    PathBuf::from("target")
                        .join("sbf-solana-solana")
                        .join("release")
                        .join(format!("{}.so", program))
                })
            } else {
                PathBuf::from("target")
                    .join("bpfel-unknown-none")
                    .join("release")
                    .join(format!("lib{}.so", program))
            };

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

fn run_watch(debug: bool, features: Option<String>) -> ! {
    watch_loop(|| run_once(debug, features.as_deref(), false))
}

// ---------------------------------------------------------------------------
// Build size tracking
// ---------------------------------------------------------------------------

const LAST_SIZE_FILE: &str = "target/.quasar-last-size";

fn size_delta(so_path: &Path, new_size: u64) -> String {
    let key = so_path.to_string_lossy();
    let last = fs::read_to_string(LAST_SIZE_FILE)
        .ok()
        .and_then(|contents| {
            contents
                .lines()
                .find(|l| l.starts_with(&*key))
                .and_then(|l| l.rsplit_once(' '))
                .and_then(|(_, s)| s.parse::<u64>().ok())
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
        .filter(|l| !l.starts_with(&*key))
        .map(String::from)
        .collect();
    lines.push(entry);
    let _ = fs::write(LAST_SIZE_FILE, lines.join("\n"));
}

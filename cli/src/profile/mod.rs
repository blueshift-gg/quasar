mod aggregate;
mod budget;
mod dwarf;
mod elf;
mod output;
mod serve;
mod walk;

use {
    self::elf::DebugLevel,
    memmap2::Mmap,
    sha2::{Digest, Sha256},
    std::{
        collections::HashSet,
        fs::{self, File},
        io::{self, copy},
        path::{Path, PathBuf},
        process::Command,
    },
    toml::Value,
};

const SERVER_HOST: &str = "127.0.0.1";
const SERVER_PORT: u16 = 7777;
const FRONTEND_INDEX: &[u8] = include_bytes!("index.html");
/// Project-local ownership root for generated profiler artifacts.
pub(crate) const PROFILE_DIR: &str = "target/profile";

pub(crate) struct ProfileCommand {
    pub elf_path: Option<PathBuf>,
    pub diff_program: Option<String>,
    pub share: bool,
    pub expand: bool,
    pub budget_path: PathBuf,
    pub write_budget: bool,
    pub assert_budget: bool,
    pub headroom_percent: u32,
    pub json: bool,
}

pub(crate) fn run(command: ProfileCommand) {
    if command.write_budget && command.assert_budget {
        fail("--write-budget and --assert-budget cannot be used together");
    }
    if command.headroom_percent > 1_000 {
        fail("--headroom must be between 0 and 1000 percent");
    }
    if let Some(program) = command.diff_program {
        run_diff(program);
        return;
    }

    let elf_path = command.elf_path.unwrap_or_else(|| {
        eprintln!(
            "Error: missing ELF path. Put the program at target/deploy/<program>.so or pass \
             `quasar profile <PATH_TO_ELF_SO>`."
        );
        std::process::exit(1);
    });
    let public_gist = command.share;
    let expand = command.expand;

    if !elf_path.exists() {
        eprintln!("Error: file not found: {}", elf_path.display());
        std::process::exit(1);
    }

    let file = std::fs::File::open(&elf_path).unwrap_or_else(|e| {
        eprintln!("Error: failed to open {}: {}", elf_path.display(), e);
        std::process::exit(1);
    });

    // SAFETY: `file` remains open while the read-only mapping is parsed, and
    // this process does not mutate or truncate the ELF during analysis.
    let mmap = unsafe { Mmap::map(&file) }.unwrap_or_else(|e| {
        eprintln!("Error: failed to mmap {}: {}", elf_path.display(), e);
        std::process::exit(1);
    });

    let info = elf::load(&mmap, &elf_path);

    let resolver = match info.debug_level {
        DebugLevel::Dwarf => {
            let symbols = dwarf::SymbolResolver::new(&info.symbols);
            match dwarf::DwarfResolver::try_new(&mmap) {
                Some(dwarf) => dwarf::Resolver::Dwarf(dwarf, symbols),
                None => {
                    eprintln!("Warning: failed to load DWARF; falling back to symbols");
                    dwarf::Resolver::Symbol(symbols)
                }
            }
        }
        DebugLevel::SymbolsOnly => {
            dwarf::Resolver::Symbol(dwarf::SymbolResolver::new(&info.symbols))
        }
        DebugLevel::Stripped => {
            eprintln!(
                "Error: binary is fully stripped. Use the unstripped binary from \
                 target/deploy/debug/<program>.so.debug instead of target/deploy/"
            );
            std::process::exit(1);
        }
    };

    let program_name = profile_program_name(&elf_path);
    let version = resolve_program_version(&elf_path, &program_name);
    let profile_binary_size = fs::metadata(&elf_path)
        .map(|metadata| metadata.len())
        .unwrap_or_else(|error| {
            fail(&format!(
                "failed to read metadata for {}: {error}",
                elf_path.display()
            ))
        });
    let deploy_binary_size = deploy_binary_size(
        &elf_path,
        &program_name,
        profile_binary_size,
        command.write_budget || command.assert_budget || command.json,
    )
    .unwrap_or_else(|message| fail(&message));

    let result = aggregate::profile(&mmap, &info, &resolver);
    let measurement = budget::Measurement::from_profile(&result, &program_name, deploy_binary_size);
    let mut violations = Vec::new();

    if command.write_budget {
        budget::write(&command.budget_path, &measurement, command.headroom_percent)
            .unwrap_or_else(|message| fail(&message));
    } else if command.assert_budget {
        violations = budget::assert(&command.budget_path, &measurement)
            .unwrap_or_else(|message| fail(&message));
    }

    if command.json {
        let status = if command.write_budget {
            Some(budget::BudgetStatus {
                path: &command.budget_path,
                status: "written",
                violations: &[],
            })
        } else if command.assert_budget {
            Some(budget::BudgetStatus {
                path: &command.budget_path,
                status: if violations.is_empty() {
                    "passed"
                } else {
                    "failed"
                },
                violations: &violations,
            })
        } else {
            None
        };
        let report = budget::MachineReport {
            version: budget::BUDGET_VERSION,
            measurement: &measurement,
            budget: status,
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .unwrap_or_else(|error| fail(&format!("failed to serialize profile: {error}")))
        );
        if !violations.is_empty() {
            std::process::exit(2);
        }
        return;
    }

    output::print_summary(&result, &program_name, profile_binary_size, expand);

    if command.write_budget {
        println!(
            "  budget    wrote {} with {}% headroom",
            command.budget_path.display(),
            command.headroom_percent
        );
        return;
    }

    if command.assert_budget {
        if violations.is_empty() {
            println!("  budget    passed {}", command.budget_path.display());
            return;
        }
        eprintln!("\nBudget violations in {}:", command.budget_path.display());
        for violation in &violations {
            eprintln!(
                "  {}: actual {} exceeds maximum {}",
                violation.metric, violation.actual, violation.maximum
            );
        }
        std::process::exit(2);
    }

    let profile_root = profile_web_root();
    ensure_frontend_assets(&profile_root);
    let profiles_dir = profile_root.join("profiles");

    let binary_hash = sha256_file(&elf_path).unwrap_or_else(|e| {
        eprintln!("Error: failed to hash {}: {}", elf_path.display(), e);
        std::process::exit(1);
    });

    let now = chrono::Utc::now();
    let timestamp = now.format("%Y-%m-%d-%H-%M-%S-%3f");

    let file_name = format!("{}__{}.profile.json", program_name, timestamp);
    let local_output_path = profiles_dir.join(&file_name);

    output::write_json(
        &result,
        &local_output_path,
        &program_name,
        &version,
        profile_binary_size,
        &binary_hash,
    );
    if public_gist {
        ensure_gh_installed();
        let desc = format!("{} CU profile v{}", program_name, version);
        let gist_url = create_gist(&local_output_path, &desc);
        println!("  {gist_url}");
        return;
    }

    // Start the flamegraph server in the background.
    let url = format!(
        "http://{}:{}/?program={}",
        SERVER_HOST, SERVER_PORT, program_name
    );
    match serve::serve_background(&profile_root, SERVER_PORT, &program_name) {
        Ok(_) => output::print_flamegraph_link(&url),
        Err(_) => {
            // Port busy; server is already running, so show the link.
            if serve::is_alive(SERVER_PORT) {
                output::print_flamegraph_link(&url);
            }
        }
    }
}

fn fail(message: &str) -> ! {
    eprintln!("Error: {message}");
    std::process::exit(1);
}

fn profile_program_name(elf_path: &Path) -> String {
    let file_name = elf_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown");
    file_name
        .strip_suffix(".so.debug")
        .or_else(|| file_name.strip_suffix(".so"))
        .unwrap_or(file_name)
        .to_string()
}

/// Measure the deployable artifact when the profiler is reading its unstripped
/// companion. For arbitrary ELF paths, the input file remains the only honest
/// size source.
fn deploy_binary_size(
    elf_path: &Path,
    program_name: &str,
    fallback: u64,
    require_companion: bool,
) -> Result<u64, String> {
    let Some(parent) = elf_path.parent() else {
        return Ok(fallback);
    };
    let parent_name = parent.file_name().and_then(|name| name.to_str());
    let candidate = match parent_name {
        Some("profile") => parent
            .parent()
            .map(|target| target.join("deploy").join(format!("{program_name}.so"))),
        Some("debug")
            if parent
                .parent()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
                == Some("deploy") =>
        {
            parent
                .parent()
                .map(|deploy| deploy.join(format!("{program_name}.so")))
        }
        _ => None,
    };
    let Some(candidate) = candidate else {
        return Ok(fallback);
    };
    match fs::metadata(&candidate) {
        Ok(metadata) => Ok(metadata.len()),
        Err(error) if require_companion => Err(format!(
            "cannot measure deploy binary size: expected {} beside the unstripped profile \
             artifact: {error}",
            candidate.display()
        )),
        Err(_) => Ok(fallback),
    }
}

fn run_diff(program: String) {
    let profile_root = profile_web_root();
    ensure_frontend_assets(&profile_root);
    println!(
        "http://{}:{}/?program={}&view=diff",
        SERVER_HOST, SERVER_PORT, program
    );
    eprintln!("Press Ctrl-C to stop the profiler server.");
    serve::serve_blocking(&profile_root, SERVER_PORT).unwrap_or_else(|e| {
        eprintln!("Error: failed to start local profiler server: {}", e);
        std::process::exit(1);
    });
}

fn ensure_frontend_assets(profile_root: &Path) {
    materialize_frontend_assets(profile_root).unwrap_or_else(|e| {
        eprintln!(
            "Error: failed to materialize profiler assets under {}: {}",
            profile_root.display(),
            e
        );
        std::process::exit(1);
    });
}

fn materialize_frontend_assets(profile_root: &Path) -> io::Result<()> {
    fs::create_dir_all(profile_root.join("profiles"))?;
    let root_index = profile_root.join("index.html");
    let current = fs::read(&root_index).ok();
    if current.as_deref() != Some(FRONTEND_INDEX) {
        fs::write(root_index, FRONTEND_INDEX)?;
    }
    Ok(())
}

pub(crate) fn profile_web_root() -> PathBuf {
    PathBuf::from(PROFILE_DIR)
}

fn resolve_program_version(elf_path: &std::path::Path, program_name: &str) -> String {
    let workspace_root = find_workspace_root(elf_path).or_else(|| {
        std::env::current_dir()
            .ok()
            .and_then(|cwd| find_workspace_root(&cwd))
    });

    let Some(workspace_root) = workspace_root else {
        return "unknown".to_string();
    };

    let mut candidates = HashSet::new();
    let stem = program_name.trim();
    if !stem.is_empty() {
        candidates.insert(stem.to_string());
        candidates.insert(stem.replace('_', "-"));
    }
    if let Some(no_lib) = stem.strip_prefix("lib") {
        candidates.insert(no_lib.to_string());
        candidates.insert(no_lib.replace('_', "-"));
    }

    if let Some(version) = find_matching_package_version(&workspace_root, &candidates) {
        return version;
    }

    read_workspace_version(&workspace_root).unwrap_or_else(|| "unknown".to_string())
}

fn ensure_gh_installed() {
    let status = Command::new("gh")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    match status {
        Ok(s) if s.success() => {}
        _ => {
            eprintln!("Error: GitHub CLI (gh) is required to publish profile gists.");
            eprintln!("Install: https://cli.github.com/");
            std::process::exit(1);
        }
    }
}

fn create_gist(path: &Path, desc: &str) -> String {
    let mut cmd = Command::new("gh");
    cmd.arg("gist")
        .arg("create")
        .arg(path)
        .arg("--desc")
        .arg(desc)
        .arg("--public");

    let output = cmd.output().unwrap_or_else(|e| {
        eprintln!("Error: failed to run gh gist create: {}", e);
        std::process::exit(1);
    });

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("Error: gh gist create failed");
        if !stderr.trim().is_empty() {
            eprintln!("{}", stderr.trim());
        }
        std::process::exit(1);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let url = stdout.trim();
    if url.is_empty() {
        eprintln!("Error: gh gist create returned no URL");
        std::process::exit(1);
    }
    url.to_string()
}

fn find_workspace_root(start: &std::path::Path) -> Option<PathBuf> {
    let mut cur = if start.is_dir() {
        start.to_path_buf()
    } else {
        start.parent()?.to_path_buf()
    };

    loop {
        let cargo = cur.join("Cargo.toml");
        if cargo.exists() {
            if let Ok(content) = fs::read_to_string(&cargo) {
                if let Ok(value) = content.parse::<Value>() {
                    if value.get("workspace").is_some() {
                        return Some(cur);
                    }
                }
            }
        }
        if !cur.pop() {
            return None;
        }
    }
}

fn find_matching_package_version(
    workspace_root: &Path,
    candidates: &HashSet<String>,
) -> Option<String> {
    let mut stack = vec![workspace_root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };

        for entry in entries {
            let Ok(entry) = entry else {
                continue;
            };
            let path = entry.path();

            if path.is_dir() {
                let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                if name == "target" || name == ".git" {
                    continue;
                }
                stack.push(path);
                continue;
            }

            if path.file_name().and_then(|s| s.to_str()) != Some("Cargo.toml") {
                continue;
            }

            let Ok(content) = fs::read_to_string(&path) else {
                continue;
            };
            let Ok(value) = content.parse::<Value>() else {
                continue;
            };
            let Some(package) = value.get("package").and_then(|v| v.as_table()) else {
                continue;
            };
            let Some(name) = package.get("name").and_then(|v| v.as_str()) else {
                continue;
            };
            if !candidates.contains(name) {
                continue;
            }
            let Some(version) = package.get("version").and_then(|v| v.as_str()) else {
                continue;
            };
            return Some(version.to_string());
        }
    }

    None
}

fn read_workspace_version(workspace_root: &std::path::Path) -> Option<String> {
    let cargo = workspace_root.join("Cargo.toml");
    let content = fs::read_to_string(cargo).ok()?;
    let value: Value = content.parse().ok()?;
    value
        .get("workspace")?
        .get("package")?
        .get("version")?
        .as_str()
        .map(ToString::to_string)
}

fn sha256_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();

    copy(&mut file, &mut hasher)?;

    let result = hasher.finalize();
    let hex = hex::encode(result);
    Ok(hex)
}

#[cfg(test)]
mod tests {
    use {
        super::{
            deploy_binary_size, materialize_frontend_assets, output::last_profile_path,
            profile_program_name, profile_web_root, FRONTEND_INDEX, PROFILE_DIR,
        },
        std::{fs, path::PathBuf},
        tempfile::tempdir,
    };

    #[test]
    fn materializes_embedded_frontend_without_runtime_source_lookup() {
        let temp = tempdir().unwrap();
        let profile_root = temp.path().join("target/profile");

        materialize_frontend_assets(&profile_root).unwrap();
        assert_eq!(
            fs::read(profile_root.join("index.html")).unwrap(),
            FRONTEND_INDEX
        );
        assert!(profile_root.join("profiles").is_dir());

        fs::write(profile_root.join("index.html"), b"stale").unwrap();
        materialize_frontend_assets(&profile_root).unwrap();
        assert_eq!(
            fs::read(profile_root.join("index.html")).unwrap(),
            FRONTEND_INDEX
        );
    }

    #[test]
    fn keeps_profile_root_and_baseline_under_project_target() {
        let profile_root = PathBuf::from(PROFILE_DIR);

        assert_eq!(profile_web_root(), profile_root);
        assert_eq!(
            last_profile_path("demo"),
            profile_root.join(".last-profile.demo")
        );
    }

    #[test]
    fn derives_the_same_program_name_from_profile_and_debug_artifacts() {
        assert_eq!(
            profile_program_name(std::path::Path::new("target/profile/demo.so")),
            "demo"
        );
        assert_eq!(
            profile_program_name(std::path::Path::new("target/deploy/debug/demo.so.debug")),
            "demo"
        );
    }

    #[test]
    fn budget_size_prefers_the_deployable_companion() {
        let temp = tempdir().unwrap();
        let target = temp.path().join("target");
        let deploy = target.join("deploy/demo.so");
        let profile = target.join("profile/demo.so");
        let debug = target.join("deploy/debug/demo.so.debug");
        fs::create_dir_all(deploy.parent().unwrap()).unwrap();
        fs::create_dir_all(profile.parent().unwrap()).unwrap();
        fs::create_dir_all(debug.parent().unwrap()).unwrap();
        fs::write(&deploy, [0_u8; 7]).unwrap();
        fs::write(&profile, [0_u8; 70]).unwrap();
        fs::write(&debug, [0_u8; 700]).unwrap();

        assert_eq!(deploy_binary_size(&profile, "demo", 70, true), Ok(7));
        assert_eq!(deploy_binary_size(&debug, "demo", 700, true), Ok(7));
        assert_eq!(
            deploy_binary_size(&temp.path().join("custom.so"), "custom", 11, true),
            Ok(11)
        );

        fs::remove_file(&deploy).unwrap();
        assert!(deploy_binary_size(&debug, "demo", 700, true)
            .unwrap_err()
            .contains("cannot measure deploy binary size"));
        assert_eq!(deploy_binary_size(&debug, "demo", 700, false), Ok(700));
    }
}

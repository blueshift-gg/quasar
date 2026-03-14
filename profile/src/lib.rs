mod aggregate;
mod dwarf;
mod elf;
mod output;
mod serve;
mod walk;

use {
    crate::elf::DebugLevel,
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

pub struct ProfileCommand {
    pub elf_path: Option<PathBuf>,
    pub diff_program: Option<String>,
    pub share: bool,
    pub expand: bool,
}

pub fn run(command: ProfileCommand) {
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

    let mmap = unsafe { Mmap::map(&file) }.unwrap_or_else(|e| {
        eprintln!("Error: failed to mmap {}: {}", elf_path.display(), e);
        std::process::exit(1);
    });

    let info = elf::load(&mmap, &elf_path);

    let resolver = match info.debug_level {
        DebugLevel::Dwarf => dwarf::Resolver::Dwarf(
            dwarf::DwarfResolver::new(&mmap),
            dwarf::SymbolResolver::new(&info.symbols),
        ),
        DebugLevel::SymbolsOnly => {
            dwarf::Resolver::Symbol(dwarf::SymbolResolver::new(&info.symbols))
        }
        DebugLevel::Stripped => {
            eprintln!(
                "Error: binary is fully stripped. Use the unstripped binary from \
                 target/sbf-solana-solana/release/ instead of target/deploy/"
            );
            std::process::exit(1);
        }
    };

    let program_name = elf_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");
    let version = resolve_program_version(&elf_path, program_name);
    let binary_size = fs::metadata(&elf_path).map(|m| m.len()).unwrap_or(0);
    let profile_root = profile_web_root();
    let profiles_dir = profile_root.join("profiles");
    fs::create_dir_all(&profiles_dir).unwrap_or_else(|e| {
        eprintln!(
            "Error: failed to create profile directory {}: {}",
            profiles_dir.display(),
            e
        );
        std::process::exit(1);
    });

    let result = aggregate::profile(&mmap, &info, &resolver);

    output::print_summary(&result, program_name, binary_size, expand);

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
        program_name,
        &version,
        binary_size,
        &binary_hash,
    );
    if public_gist {
        ensure_frontend_assets(&profile_root);
        ensure_gh_installed();
        let desc = format!("{} CU profile v{}", program_name, version);
        let gist_url = create_gist(&local_output_path, &desc);
        println!("  {gist_url}");
        return;
    }

    // Start flamegraph server in the background (auto-shuts down when idle)
    if has_frontend_assets(&profile_root) {
        let url = format!(
            "http://{}:{}/?program={}",
            SERVER_HOST, SERVER_PORT, program_name
        );
        match serve::serve_background(&profile_root, SERVER_PORT, program_name) {
            Ok(_) => output::print_flamegraph_link(&url),
            Err(_) => {
                // Port busy — server already running, just show the link
                if serve::is_alive(SERVER_PORT) {
                    output::print_flamegraph_link(&url);
                }
            }
        }
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

fn has_frontend_assets(profile_root: &Path) -> bool {
    profile_root.join("index.html").exists()
}

fn ensure_frontend_assets(profile_root: &Path) {
    fs::create_dir_all(profile_root).unwrap_or_else(|e| {
        eprintln!(
            "Error: failed to create profiler web root {}: {}",
            profile_root.display(),
            e
        );
        std::process::exit(1);
    });

    let root_index = profile_root.join("index.html");
    if !root_index.exists() {
        eprintln!(
            "Error: missing frontend artifact at {}",
            root_index.display()
        );
        eprintln!("Add the compiled single-file frontend index.html to quasar/profile/.");
        std::process::exit(1);
    }

    let profiles_dir = profile_root.join("profiles");
    fs::create_dir_all(&profiles_dir).unwrap_or_else(|e| {
        eprintln!(
            "Error: failed to create profile directory {}: {}",
            profiles_dir.display(),
            e
        );
        std::process::exit(1);
    });
}

fn profile_web_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
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

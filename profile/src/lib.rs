mod aggregate;
mod dwarf;
mod elf;
mod output;
mod walk;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::{
    collections::HashSet,
    fs::{self, File},
    io::{self, copy},
};

use elf::DebugLevel;
use memmap2::Mmap;
use toml::Value;

use sha2::{Digest, Sha256};

const PROFILER_BASE_URL: &str = "https://quasar-profiler.blueshift.gg";

pub struct ProfileCommand {
    pub elf_path: PathBuf,
    pub output: Option<PathBuf>,
    pub no_gist: bool,
    pub share: bool,
}

pub fn run(command: ProfileCommand) {
    let elf_path = command.elf_path;
    let output_path = command.output;
    let no_gist = command.no_gist;
    let public_gist = command.share;

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

    eprintln!("quasar-profile: {}", elf_path.display());

    let resolver = match info.debug_level {
        DebugLevel::Dwarf => {
            eprintln!("DWARF debug info: yes");
            dwarf::Resolver::Dwarf(
                dwarf::DwarfResolver::new(&mmap),
                dwarf::SymbolResolver::new(&info.symbols),
            )
        }
        DebugLevel::SymbolsOnly => {
            eprintln!("DWARF debug info: no (symbol table only)");
            eprintln!(
                "Warning: inline functions will not be resolved. \
                 Rebuild with debug info for full resolution."
            );
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
    let output_path = output_path.unwrap_or_else(|| {
        let name = elf_path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|name| format!("{}.profile.json", name))
            .unwrap_or_else(|| "profile.json".to_string());
        PathBuf::from(name)
    });

    let result = aggregate::profile(&mmap, &info, &resolver);

    output::print_summary(&result);

    let binary_hash = sha256_file(&elf_path).unwrap_or_else(|e| {
        eprintln!("Error: failed to hash {}: {}", elf_path.display(), e);
        std::process::exit(1);
    });

    output::write_json(
        &result,
        &output_path,
        program_name,
        &version,
        binary_size,
        &binary_hash,
    );
    eprintln!("Profile JSON written to: {}", output_path.display());

    if !no_gist {
        ensure_gh_installed();
        let desc = format!("{} CU profile v{}", program_name, version);
        let gist_url = create_gist(&output_path, &desc, public_gist);
        let profiler_url = profiler_url_from_gist(&gist_url).unwrap_or_else(|| {
            eprintln!("Error: failed to parse gist URL: {}", gist_url);
            std::process::exit(1);
        });
        println!("{}", profiler_url);
    } else {
        eprintln!("--no-gist enabled; no profiler URL generated");
    }
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

fn create_gist(path: &Path, desc: &str, public: bool) -> String {
    let mut cmd = Command::new("gh");
    cmd.arg("gist")
        .arg("create")
        .arg(path)
        .arg("--desc")
        .arg(desc);
    if public {
        cmd.arg("--public");
    }

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

fn profiler_url_from_gist(gist_url: &str) -> Option<String> {
    let no_query = gist_url.split('?').next()?.trim_end_matches('/');
    let no_scheme = no_query
        .strip_prefix("https://")
        .or_else(|| no_query.strip_prefix("http://"))
        .unwrap_or(no_query);
    let mut parts = no_scheme.split('/');
    let host = parts.next()?;
    if host != "gist.github.com" {
        return None;
    }
    let owner = parts.next()?;
    let gist_id = parts.next()?;
    if owner.is_empty() || gist_id.is_empty() {
        return None;
    }
    Some(format!(
        "{}/?bench={}/{}",
        PROFILER_BASE_URL, owner, gist_id
    ))
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

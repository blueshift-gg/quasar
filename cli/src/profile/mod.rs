mod aggregate;
mod budget;
mod dwarf;
mod elf;
mod output;
mod walk;

use {
    self::elf::DebugLevel,
    crate::error::{CliError, CliResult},
    memmap2::Mmap,
    std::{
        fs,
        path::{Path, PathBuf},
    },
};

/// Project-local ownership root for generated profiler artifacts.
pub(crate) const PROFILE_DIR: &str = "target/profile";

pub(crate) struct ProfileCommand {
    pub elf_path: Option<PathBuf>,
    pub expand: bool,
    pub budget_path: PathBuf,
    pub write_budget: bool,
    pub assert_budget: bool,
    pub headroom_percent: u32,
    pub json: bool,
}

pub(crate) fn run(command: ProfileCommand) -> CliResult {
    if command.write_budget && command.assert_budget {
        return Err(CliError::message(
            "--write-budget and --assert-budget cannot be used together",
        ));
    }
    if command.headroom_percent > 1_000 {
        return Err(CliError::message(
            "--headroom must be between 0 and 1000 percent",
        ));
    }

    let elf_path = command.elf_path.ok_or_else(|| {
        CliError::message(
            "missing ELF path; put the program at target/deploy/<program>.so or pass `quasar \
             profile <PATH_TO_ELF_SO>`",
        )
    })?;
    if !elf_path.exists() {
        return Err(CliError::message(format!(
            "file not found: {}",
            elf_path.display()
        )));
    }

    let file = std::fs::File::open(&elf_path)
        .map_err(|error| CliError::io_path("open", &elf_path, error))?;
    // SAFETY: `file` remains open while the read-only mapping is parsed, and
    // this process does not mutate or truncate the ELF during analysis.
    let mmap = unsafe { Mmap::map(&file) }
        .map_err(|error| CliError::io_path("memory-map", &elf_path, error))?;
    let info = elf::load(&mmap, &elf_path)?;

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
            return Err(CliError::message(
                "binary is fully stripped; use the unstripped binary from \
                 target/deploy/debug/<program>.so.debug instead of target/deploy/",
            ));
        }
    };

    let program_name = profile_program_name(&elf_path);
    let profile_binary_size = fs::metadata(&elf_path)
        .map(|metadata| metadata.len())
        .map_err(|error| CliError::io_path("read metadata for", &elf_path, error))?;
    let deploy_binary_size = deploy_binary_size(
        &elf_path,
        &program_name,
        profile_binary_size,
        command.write_budget || command.assert_budget || command.json,
    )
    .map_err(CliError::message)?;

    let result = aggregate::profile(&mmap, &info, &resolver);
    let measurement = budget::Measurement::from_profile(&result, &program_name, deploy_binary_size);
    let mut violations = Vec::new();
    if command.write_budget {
        budget::write(&command.budget_path, &measurement, command.headroom_percent)
            .map_err(CliError::message)?;
    } else if command.assert_budget {
        violations =
            budget::assert(&command.budget_path, &measurement).map_err(CliError::message)?;
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
                .map_err(|error| CliError::json_serialize("profile report", error))?
        );
        return if violations.is_empty() {
            Ok(())
        } else {
            Err(CliError::process_failure("profile budget exceeded", 2))
        };
    }

    output::print_summary(&result, &program_name, profile_binary_size, command.expand);
    if command.write_budget {
        println!(
            "  budget    wrote {} with {}% headroom",
            command.budget_path.display(),
            command.headroom_percent
        );
    } else if command.assert_budget && violations.is_empty() {
        println!("  budget    passed {}", command.budget_path.display());
    } else if command.assert_budget {
        eprintln!("\nBudget violations in {}:", command.budget_path.display());
        for violation in &violations {
            eprintln!(
                "  {}: actual {} exceeds maximum {}",
                violation.metric, violation.actual, violation.maximum
            );
        }
        return Err(CliError::process_failure("profile budget exceeded", 2));
    }

    Ok(())
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

pub(crate) fn profile_root() -> PathBuf {
    PathBuf::from(PROFILE_DIR)
}

#[cfg(test)]
mod tests {
    use {
        super::{
            deploy_binary_size, output::last_profile_path, profile_program_name, profile_root,
            PROFILE_DIR,
        },
        std::{fs, path::PathBuf},
        tempfile::tempdir,
    };

    #[test]
    fn keeps_profile_root_and_baseline_under_project_target() {
        let expected_root = PathBuf::from(PROFILE_DIR);

        assert_eq!(profile_root(), expected_root);
        assert_eq!(
            last_profile_path("demo"),
            expected_root.join(".last-profile.demo")
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

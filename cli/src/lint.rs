use {
    crate::{
        config::QuasarConfig,
        error::{CliError, CliResult},
        style, utils, LintCommand,
    },
    quasar_idl::{
        lint::{self, Diagnostic, LintConfig, LintReport, ProgramSurface, Severity},
        types::Idl,
    },
    std::path::Path,
};

pub fn run(cmd: LintCommand) -> CliResult {
    let config = QuasarConfig::load()?;
    let crate_root = utils::find_program_crate(&config);
    let idl = crate::idl::build(&crate_root)?;
    let lockfile_exists = lint::lock_path(&crate_root).exists();
    let previous_lock = if cmd.no_diff {
        None
    } else {
        load_existing_lock(&crate_root)?
    };
    let lint_config = LintConfig {
        strict: cmd.strict,
        lockfile_present: cmd.update_lock || lockfile_exists,
    };
    let current = ProgramSurface::from_idl(&idl);

    let mut report = lint::run(&idl, &lint_config);
    if !cmd.update_lock && !cmd.no_diff {
        if let Some(previous) = previous_lock {
            report.extend(lint::diff(&previous, &current));
        }
    }

    print_report(&report);

    if report.should_fail(&lint_config) {
        return Err(CliError::process_failure("lint check failed", 1));
    }

    if cmd.update_lock {
        let path = lint::lock_path(&crate_root);
        lint::save_lockfile(&path, &current).map_err(|e| CliError::message(e.to_string()))?;
        println!("  {}", style::success(&format!("wrote {}", path.display())));
    } else if report.is_empty() {
        println!("  {}", style::success("lint clean"));
    }

    Ok(())
}

pub fn run_for_build(crate_root: &Path, idl: &Idl) -> CliResult {
    let previous_lock = load_existing_lock(crate_root)?;
    let lint_config = LintConfig {
        strict: false,
        lockfile_present: previous_lock.is_some(),
    };
    let mut report = lint::run(idl, &lint_config);
    if let Some(previous) = previous_lock {
        let current = ProgramSurface::from_idl(idl);
        report.extend(lint::diff(&previous, &current));
    }

    if report.is_empty() {
        return Ok(());
    }

    print_report(&report);
    if report.should_fail(&lint_config) {
        Err(CliError::process_failure("lint check failed", 1))
    } else {
        Ok(())
    }
}

/// Require a persisted compatibility surface before deploying instructions
/// whose discriminators were assigned automatically.
pub(crate) fn require_deploy_lock(crate_root: &Path, idl: &Idl) -> CliResult {
    let current = ProgramSurface::from_idl(idl);
    let has_auto_discriminator = current
        .instructions
        .iter()
        .any(|instruction| instruction.discriminator_source.as_deref() == Some("auto"));
    if !has_auto_discriminator {
        return Ok(());
    }

    let path = lint::lock_path(crate_root);
    if !path.is_file() {
        return Err(CliError::message(format!(
            "deployment requires {} because this program uses automatic instruction \
             discriminators\n\n  Run `quasar lint --update-lock`, review the generated \
             compatibility surface, and commit it before deployment.",
            path.display()
        )));
    }
    Ok(())
}

fn load_existing_lock(crate_root: &Path) -> Result<Option<ProgramSurface>, CliError> {
    let path = lint::lock_path(crate_root);
    if !path.exists() {
        return Ok(None);
    }
    lint::load_lockfile(&path)
        .map(Some)
        .map_err(|e| CliError::message(e.to_string()))
}

fn print_report(report: &LintReport) {
    if report.is_empty() {
        return;
    }

    eprintln!("  {}", style::warn("Quasar lint findings"));
    for diagnostic in &report.diagnostics {
        print_diagnostic(diagnostic);
    }
}

fn print_diagnostic(diagnostic: &Diagnostic) {
    let label = match diagnostic.severity {
        Severity::Error => style::fail(diagnostic.severity.as_str()),
        Severity::Warning => style::warn(diagnostic.severity.as_str()),
        Severity::Info => style::step(diagnostic.severity.as_str()),
    };

    eprintln!(
        "    {label} {} {}: {}",
        diagnostic.rule, diagnostic.target, diagnostic.message
    );
    eprintln!("      {}", diagnostic.rule.title());
    if let Some(suggestion) = &diagnostic.suggestion {
        eprintln!("      fix: {suggestion}");
    }
}

#[cfg(test)]
mod deploy_lock_tests {
    use {
        super::require_deploy_lock,
        quasar_idl::types::{Idl, IdlHashes, IdlInstruction, IdlMetadata, CURRENT_SPEC},
        serde_json::json,
        std::{collections::BTreeMap, fs},
        tempfile::tempdir,
    };

    fn idl(source: Option<&str>) -> Idl {
        let mut extra = BTreeMap::new();
        if let Some(source) = source {
            extra.insert(
                "quasar:instructionDiscriminatorSource".to_string(),
                json!({ "initialize": source }),
            );
        }
        Idl {
            spec: CURRENT_SPEC.to_string(),
            name: "demo".to_string(),
            version: "0.1.0".to_string(),
            address: "11111111111111111111111111111111".to_string(),
            metadata: IdlMetadata {
                extra,
                ..IdlMetadata::default()
            },
            docs: vec![],
            instructions: vec![IdlInstruction {
                name: "initialize".to_string(),
                discriminator: vec![0],
                docs: vec![],
                accounts: vec![],
                args: vec![],
                layout: None,
                remaining_accounts: None,
            }],
            accounts: vec![],
            types: vec![],
            events: vec![],
            errors: vec![],
            extensions: None,
            hashes: Some(IdlHashes {
                idl: "unused".to_string(),
                abi: "unused".to_string(),
            }),
        }
    }

    #[test]
    fn rejects_automatic_discriminator_without_lock() {
        let root = tempdir().unwrap();
        let error = require_deploy_lock(root.path(), &idl(Some("auto"))).unwrap_err();
        assert!(error.to_string().contains("lint --update-lock"));
    }

    #[test]
    fn accepts_automatic_discriminator_with_lock() {
        let root = tempdir().unwrap();
        fs::write(root.path().join("quasar.lock.json"), "{}").unwrap();
        require_deploy_lock(root.path(), &idl(Some("auto"))).unwrap();
    }

    #[test]
    fn accepts_explicit_discriminator_without_lock() {
        let root = tempdir().unwrap();
        require_deploy_lock(root.path(), &idl(Some("explicit"))).unwrap();
    }
}

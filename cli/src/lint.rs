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
    let lint_config = LintConfig { strict: cmd.strict };
    let current = ProgramSurface::from_idl(&idl);

    let mut report = lint::run(&idl, &lint_config);
    if !cmd.update_lock && !cmd.no_diff {
        if let Some(previous) = load_existing_lock(&crate_root)? {
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
    let lint_config = LintConfig::default();
    let mut report = lint::run(idl, &lint_config);
    if let Some(previous) = load_existing_lock(crate_root)? {
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

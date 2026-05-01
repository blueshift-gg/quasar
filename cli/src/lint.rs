use {
    crate::{config::QuasarConfig, error::CliResult, utils, LintCommand},
    quasar_idl::lint::{self, comparative, snapshot::ProgramSnapshot},
    std::path::Path,
};

pub fn run(cmd: LintCommand) -> CliResult {
    let config = QuasarConfig::load()?;
    let crate_root = utils::find_program_crate(&config);

    let lint_config = lint::LintConfig {
        fix: cmd.fix,
        graph: cmd.graph.as_deref().map(parse_graph_format),
    };

    let parsed = quasar_idl::parser::parse_program(&crate_root);

    if cmd.update_lock {
        let snap = ProgramSnapshot::from_parsed(&parsed);
        let path = lint::snapshot::lock_path(&crate_root);
        snap.save(&path).map_err(|e| {
            crate::error::CliError::process_failure(format!("writing lock file: {e}"), 1)
        })?;
        println!("Wrote {}", path.display());
    }

    let mut report = lint::run_lint(&parsed, &lint_config);

    if cmd.diff {
        let path = lint::snapshot::lock_path(&crate_root);
        match ProgramSnapshot::load(&path) {
            Ok(old) => {
                let new = ProgramSnapshot::from_parsed(&parsed);
                report.diagnostics.extend(comparative::run_all(&old, &new));
            }
            Err(e) => {
                eprintln!("Skipping diff rules: {e}");
            }
        }
    }

    lint::output::print_report(&report);

    if report.has_errors() {
        Err(crate::error::CliError::process_failure(
            "lint check failed".to_string(),
            1,
        ))
    } else {
        Ok(())
    }
}

/// Run lint from a crate path (standalone `quasar lint`).
///
/// Convenience wrapper used by `quasar build --lint` — no diff/lock
/// path, just the single-build rules. The diff family runs only via
/// the explicit `--diff` flag on the standalone subcommand.
pub fn run_lint_from_path(crate_path: &Path, lint_config: &lint::LintConfig) -> CliResult {
    let parsed = quasar_idl::parser::parse_program(crate_path);
    run_lint_on_parsed(&parsed, lint_config)
}

/// Run lint on an already-parsed program (called from build to avoid
/// double-parse).
pub fn run_lint_on_parsed(
    parsed: &quasar_idl::parser::ParsedProgram,
    lint_config: &lint::LintConfig,
) -> CliResult {
    let report = lint::run_lint(parsed, lint_config);
    lint::output::print_report(&report);

    if report.has_errors() {
        Err(crate::error::CliError::process_failure(
            "lint check failed".to_string(),
            1,
        ))
    } else {
        Ok(())
    }
}

fn parse_graph_format(s: &str) -> lint::GraphFormat {
    match s {
        "mermaid" => lint::GraphFormat::Mermaid,
        "dot" => lint::GraphFormat::Dot,
        "json" => lint::GraphFormat::Json,
        _ => lint::GraphFormat::Ascii,
    }
}

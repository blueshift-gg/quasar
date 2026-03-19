use crate::{error::{CliError, CliResult}, style, AuditCommand};

pub fn run(cmd: AuditCommand) -> CliResult {
    let crate_path = &cmd.crate_path;

    if !crate_path.exists() {
        return Err(CliError::PathDoesNotExist(crate_path.display().to_string()));
    }

    let report = quasar_audit::audit_program(crate_path);

    if report.findings.is_empty() {
        println!();
        println!("  {}", style::success("No issues found"));
        println!();
        return Ok(());
    }

    let mut findings = report.findings;
    findings.sort_by_key(|f| f.severity);

    println!();
    println!(
        "  {} {}",
        style::bold("Audit results:"),
        style::dim(&format!("{} finding(s)", findings.len())),
    );
    println!();

    let mut criticals = 0;
    let mut warnings = 0;
    let mut infos = 0;

    for finding in &findings {
        match finding.severity {
            quasar_audit::Severity::Critical => criticals += 1,
            quasar_audit::Severity::Warning => warnings += 1,
            quasar_audit::Severity::Info => infos += 1,
        }

        let severity_str = match finding.severity {
            quasar_audit::Severity::Critical => style::fail("CRITICAL"),
            quasar_audit::Severity::Warning => style::warn("WARNING"),
            quasar_audit::Severity::Info => style::dim("INFO"),
        };

        let location = match (&finding.instruction, &finding.field) {
            (Some(ix), Some(field)) => format!("{} → {}", ix, field),
            (Some(ix), None) => ix.clone(),
            _ => "global".to_string(),
        };

        println!(
            "  {} {} {}",
            severity_str,
            style::bold(&format!("[{}]", finding.rule)),
            style::dim(&location),
        );
        println!("    {}", finding.message);
        if let Some(url) = finding.learn_url {
            println!("    {} {}", style::dim("Learn more:"), style::dim(url));
        }
        println!();
    }

    let mut parts = Vec::new();
    if criticals > 0 {
        parts.push(style::fail(&format!("{} critical", criticals)));
    }
    if warnings > 0 {
        parts.push(style::warn(&format!("{} warning(s)", warnings)));
    }
    if infos > 0 {
        parts.push(style::dim(&format!("{} info", infos)));
    }
    println!("  {}", parts.join("  "));
    println!();

    Ok(())
}

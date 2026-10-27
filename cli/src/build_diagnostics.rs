use crate::style;

/// Extract warning lines from cargo output (for display on success).
pub(super) fn extract_warnings(stderr: &str) -> Vec<String> {
    scan_diagnostics(stderr, false)
}

fn scan_diagnostics(stderr: &str, include_errors: bool) -> Vec<String> {
    let mut lines = Vec::new();
    let mut capture = false;

    for line in stderr.lines() {
        if line.starts_with("warning") || (include_errors && line.starts_with("error")) {
            if line.contains("warnings emitted")
                || line.contains("warning emitted")
                || line.contains("user-defined alias")
                || line.contains("shadowing")
            {
                continue;
            }
            capture = true;
            lines.push(line.to_string());
        } else if capture {
            if line.starts_with("  ")
                || line.starts_with(" -->")
                || (include_errors && line.starts_with("Caused by:"))
                || line.is_empty()
            {
                lines.push(line.to_string());
            } else {
                capture = false;
            }
        }
    }

    lines
}

/// Extract and display only the meaningful error/warning lines from cargo
/// output.
pub(super) fn format_build_errors(stderr: &str, elapsed: std::time::Duration) -> String {
    let errors = scan_diagnostics(stderr, true);

    if errors.is_empty() {
        if !stderr.is_empty() {
            return format!("{stderr}\n\nbuild failed in {}", style::human_duration(elapsed));
        }
        return format!("build failed in {}", style::human_duration(elapsed));
    }

    let err_count = errors.iter().filter(|l| l.starts_with("error")).count();
    let warn_count = errors.iter().filter(|l| l.starts_with("warning")).count();

    let mut summary = Vec::new();
    if err_count > 0 {
        summary.push(format!("{err_count} error{}", if err_count == 1 { "" } else { "s" }));
    }
    if warn_count > 0 {
        summary.push(format!("{warn_count} warning{}", if warn_count == 1 { "" } else { "s" }));
    }

    format!(
        "{}\n\nbuild failed in {} ({})",
        errors.join("\n"),
        style::human_duration(elapsed),
        summary.join(", ")
    )
}

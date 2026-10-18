use crate::style;

/// Extract warning lines from cargo output (for display on success).
pub(super) fn extract_warnings(stderr: &str) -> Vec<String> {
    let mut warnings = Vec::new();
    let mut capture = false;

    for line in stderr.lines() {
        if line.starts_with("warning") {
            if line.contains("warnings emitted")
                || line.contains("warning emitted")
                || line.contains("user-defined alias")
                || line.contains("shadowing")
            {
                continue;
            }
            capture = true;
            warnings.push(line.to_string());
        } else if capture {
            if line.starts_with("  ") || line.starts_with(" -->") || line.is_empty() {
                warnings.push(line.to_string());
            } else {
                capture = false;
            }
        }
    }

    warnings
}

/// Extract and display only the meaningful error/warning lines from cargo
/// output.
pub(super) fn format_build_errors(stderr: &str, elapsed: std::time::Duration) -> String {
    let mut errors: Vec<String> = Vec::new();
    let mut capture = false;

    for line in stderr.lines() {
        if line.starts_with("error") || line.starts_with("warning") {
            if line.contains("warnings emitted")
                || line.contains("warning emitted")
                || line.contains("user-defined alias")
                || line.contains("shadowing")
            {
                continue;
            }
            capture = true;
            errors.push(line.to_string());
        } else if capture {
            if line.starts_with("  ")
                || line.starts_with(" -->")
                || line.starts_with("Caused by:")
                || line.is_empty()
            {
                errors.push(line.to_string());
            } else {
                capture = false;
            }
        }
    }

    if errors.is_empty() {
        if !stderr.is_empty() {
            return format!("{stderr}\n\nbuild failed in {}", style::human_duration(elapsed));
        }
        return format!("build failed in {}", style::human_duration(elapsed));
    }

    let err_count = errors.iter().filter(|l| l.starts_with("error")).count();
    let warn_count = errors.iter().filter(|l| l.starts_with("warning")).count();

    let mut summary = String::new();
    if err_count > 0 {
        summary.push_str(&format!("{err_count} error{}", if err_count == 1 { "" } else { "s" }));
    }
    if warn_count > 0 {
        if !summary.is_empty() {
            summary.push_str(", ");
        }
        summary
            .push_str(&format!("{warn_count} warning{}", if warn_count == 1 { "" } else { "s" }));
    }

    let mut message = String::new();
    for line in &errors {
        message.push_str(line);
        message.push('\n');
    }
    message.push('\n');
    message.push_str(&format!("build failed in {} ({summary})", style::human_duration(elapsed)));
    message
}

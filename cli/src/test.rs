use {
    crate::{config::QuasarConfig, error::CliResult, style},
    std::{
        process::{Command, Stdio},
        time::Instant,
    },
};

pub fn run(debug: bool, filter: Option<String>, watch: bool, no_build: bool) -> CliResult {
    if watch {
        return run_watch(debug, filter, no_build);
    }
    run_once(debug, filter.as_deref(), no_build)
}

fn run_once(debug: bool, filter: Option<&str>, no_build: bool) -> CliResult {
    let config = QuasarConfig::load()?;

    if !no_build {
        crate::build::run(debug, false, None)?;
    }

    let sp = style::spinner("Testing...");
    let start = Instant::now();

    let is_mollusk = config.testing.framework == "mollusk";
    let result = if config.has_typescript_tests() {
        run_typescript_tests(filter)
    } else if config.has_rust_tests() {
        run_rust_tests(filter)
    } else {
        sp.finish_and_clear();
        println!("  {}", style::warn("no test framework configured"));
        return Ok(());
    };

    sp.finish_and_clear();

    let elapsed = start.elapsed();

    match result {
        Ok(summary) => {
            println!();
            for line in &summary.lines {
                println!("    {line}");
            }
            println!();
            println!(
                "  {}",
                style::dim(&format!(
                    "{} passed ({})",
                    summary.passed,
                    style::human_duration(elapsed)
                ))
            );
            Ok(())
        }
        Err(summary) => {
            println!();
            for line in &summary.lines {
                println!("    {line}");
            }
            println!();
            eprintln!(
                "  {} passed, {} failed ({})",
                summary.passed,
                summary.failed,
                style::human_duration(elapsed)
            );
            if is_mollusk {
                eprintln!();
                eprintln!(
                    "  {}",
                    style::dim(
                        "Tip: enable the \"debug\" feature for more descriptive error messages."
                    )
                );
            }
            std::process::exit(1);
        }
    }
}

fn run_watch(debug: bool, filter: Option<String>, no_build: bool) -> CliResult {
    if let Err(e) = run_once(debug, filter.as_deref(), no_build) {
        eprintln!("  {}", style::fail(&format!("{e}")));
    }

    loop {
        let baseline = crate::build::collect_mtimes(std::path::Path::new("src"));
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let current = crate::build::collect_mtimes(std::path::Path::new("src"));
            if current != baseline {
                if let Err(e) = run_once(debug, filter.as_deref(), no_build) {
                    eprintln!("  {}", style::fail(&format!("{e}")));
                }
                break;
            }
        }
    }
}

struct TestSummary {
    passed: usize,
    failed: usize,
    lines: Vec<String>,
}

// ---------------------------------------------------------------------------
// TypeScript (mocha --reporter json)
// ---------------------------------------------------------------------------

fn run_typescript_tests(filter: Option<&str>) -> Result<TestSummary, TestSummary> {
    if !std::path::Path::new("node_modules").exists() {
        let o = Command::new("npm")
            .args(["install"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();

        match o {
            Ok(o) if o.status.success() => {}
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                if !stderr.is_empty() {
                    eprint!("{stderr}");
                }
                eprintln!("  {}", style::fail("npm install failed"));
                std::process::exit(o.status.code().unwrap_or(1));
            }
            Err(e) => {
                eprintln!(
                    "  {}",
                    style::fail(&format!("failed to run npm install: {e}"))
                );
                std::process::exit(1);
            }
        }
    }

    // Run mocha with JSON reporter to get structured results
    let mut cmd = Command::new("npx");
    cmd.args(["mocha", "--require", "tsx", "--delay", "--reporter", "json"]);

    // Find test files matching the glob pattern from package.json
    // Default to tests/*.test.ts
    cmd.arg("tests/*.test.ts");

    if let Some(pattern) = filter {
        cmd.args(["--grep", pattern]);
    }

    let output = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output();

    let o = match output {
        Ok(o) => o,
        Err(e) => {
            eprintln!("  {}", style::fail(&format!("failed to run mocha: {e}")));
            std::process::exit(1);
        }
    };

    let stdout = String::from_utf8_lossy(&o.stdout);

    // Try to parse JSON output
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
        return parse_mocha_json(&json);
    }

    // Fallback: couldn't parse JSON, show raw output
    let stderr = String::from_utf8_lossy(&o.stderr);
    if !stderr.is_empty() {
        eprint!("{stderr}");
    }
    if !stdout.is_empty() {
        print!("{stdout}");
    }

    if o.status.success() {
        Ok(TestSummary {
            passed: 0,
            failed: 0,
            lines: vec![],
        })
    } else {
        eprintln!("  {}", style::fail("tests failed"));
        std::process::exit(o.status.code().unwrap_or(1));
    }
}

fn parse_mocha_json(json: &serde_json::Value) -> Result<TestSummary, TestSummary> {
    let mut lines = Vec::new();
    let mut passed = 0usize;
    let mut failed = 0usize;

    if let Some(passes) = json.get("passes").and_then(|v| v.as_array()) {
        for test in passes {
            let title = test
                .get("fullTitle")
                .and_then(|t| t.as_str())
                .unwrap_or("?");
            lines.push(style::success(title));
            passed += 1;
        }
    }

    if let Some(failures) = json.get("failures").and_then(|v| v.as_array()) {
        for test in failures {
            let title = test
                .get("fullTitle")
                .and_then(|t| t.as_str())
                .unwrap_or("?");
            lines.push(style::fail(title));

            // Show error message indented
            if let Some(err) = test.get("err") {
                if let Some(msg) = err.get("message").and_then(|m| m.as_str()) {
                    for line in msg.lines().take(10) {
                        lines.push(format!("    {}", format_failure_line(line)));
                    }
                }
            }

            failed += 1;
        }
    }

    let summary = TestSummary {
        passed,
        failed,
        lines,
    };

    if failed > 0 {
        Err(summary)
    } else {
        Ok(summary)
    }
}

// ---------------------------------------------------------------------------
// Rust (cargo test)
// ---------------------------------------------------------------------------

fn run_rust_tests(filter: Option<&str>) -> Result<TestSummary, TestSummary> {
    let mut cmd = Command::new("cargo");
    cmd.args(["test", "tests::"]);
    if let Some(pattern) = filter {
        cmd.arg(pattern);
    }

    let output = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output();

    let o = match output {
        Ok(o) => o,
        Err(e) => {
            eprintln!(
                "  {}",
                style::fail(&format!("failed to run cargo test: {e}"))
            );
            std::process::exit(1);
        }
    };

    let stdout = String::from_utf8_lossy(&o.stdout);
    let stderr = String::from_utf8_lossy(&o.stderr);

    // Check for compilation errors (no test results at all)
    if !o.status.success() && !stdout.contains("test result:") {
        if !stderr.is_empty() {
            eprint!("{stderr}");
        }
        eprintln!("  {}", style::fail("build failed"));
        std::process::exit(o.status.code().unwrap_or(1));
    }

    parse_cargo_test_output(&stdout, &stderr)
}

/// Format a test failure detail line with special handling for program logs.
fn format_failure_line(line: &str) -> String {
    // Program invoke/success/failed traces
    if line.starts_with("Program ")
        && (line.contains("invoke [") || line.contains(" success") || line.contains(" failed"))
    {
        return style::dim(line);
    }
    // Program CU consumption
    if line.starts_with("Program ") && line.contains("consumed") && line.contains("compute units") {
        return style::dim(line);
    }
    // Program log lines - show them prominently
    if line.starts_with("Program log:") || line.starts_with("Program data:") {
        return line.to_string();
    }
    // Error type names - highlight in red
    if line.contains("ProgramError::") || line.contains("InstructionError::") {
        return style::fail(line);
    }
    // Common error patterns from our ProgramError Display
    if line.starts_with("invalid ")
        || line.starts_with("insufficient ")
        || line.starts_with("incorrect ")
        || line.starts_with("missing ")
        || line.starts_with("account ")
        || line.starts_with("arithmetic ")
        || line.starts_with("compute budget")
        || line.starts_with("custom program error")
        || line.starts_with("runtime error")
        || line.starts_with("borsh ")
    {
        return style::fail(line);
    }
    // Default - keep as-is
    line.to_string()
}

fn parse_cargo_test_output(stdout: &str, stderr: &str) -> Result<TestSummary, TestSummary> {
    let mut lines = Vec::new();
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut in_failure_block = false;
    let mut failure_lines: Vec<String> = Vec::new();

    for line in stdout.lines().chain(stderr.lines()) {
        let trimmed = line.trim();

        // test foo::bar ... ok
        if trimmed.starts_with("test ") && trimmed.ends_with("... ok") {
            let name = trimmed
                .strip_prefix("test ")
                .and_then(|s| s.strip_suffix(" ... ok"))
                .unwrap_or("?");
            lines.push(style::success(name));
            passed += 1;
        }
        // test foo::bar ... FAILED
        else if trimmed.starts_with("test ") && trimmed.ends_with("... FAILED") {
            let name = trimmed
                .strip_prefix("test ")
                .and_then(|s| s.strip_suffix(" ... FAILED"))
                .unwrap_or("?");
            lines.push(style::fail(name));
            failed += 1;
        }
        // Capture failure details
        else if trimmed == "failures:" {
            in_failure_block = true;
        } else if in_failure_block && trimmed == "failures:" {
            // Second "failures:" header (list of failed test names) — stop capturing
            in_failure_block = false;
        } else if in_failure_block && trimmed.starts_with("---- ") {
            // New failure detail block
            if !failure_lines.is_empty() {
                for fl in &failure_lines {
                    lines.push(format!("    {fl}"));
                }
                failure_lines.clear();
            }
        } else if in_failure_block && !trimmed.is_empty() && !trimmed.starts_with("test result:") {
            failure_lines.push(format_failure_line(trimmed));
        }
    }

    // Flush remaining failure lines
    if !failure_lines.is_empty() {
        for fl in &failure_lines {
            lines.push(format!("    {fl}"));
        }
    }

    let summary = TestSummary {
        passed,
        failed,
        lines,
    };

    if failed > 0 {
        Err(summary)
    } else {
        Ok(summary)
    }
}

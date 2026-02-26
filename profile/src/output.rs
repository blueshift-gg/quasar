use std::io::{BufWriter, Write};
use std::path::Path;

use crate::aggregate::ProfileResult;

pub fn write_svg(folded: &str, path: &Path, program_name: &str) {
    let mut opts = inferno::flamegraph::Options::default();
    opts.title = format!("{} — CU Profile", program_name);
    opts.count_name = "CUs".to_string();
    opts.flame_chart = false;
    opts.min_width = 2.1;

    let mut buf = Vec::new();
    inferno::flamegraph::from_reader(&mut opts, folded.as_bytes(), &mut buf).unwrap_or_else(|e| {
        eprintln!("Error: failed to generate flame graph: {}", e);
        std::process::exit(1);
    });

    let file = std::fs::File::create(path).unwrap_or_else(|e| {
        eprintln!("Error: failed to create {}: {}", path.display(), e);
        std::process::exit(1);
    });
    let mut writer = BufWriter::new(file);
    writer.write_all(&buf).unwrap();
    writer.flush().unwrap();
}

pub fn print_summary(result: &ProfileResult) {
    eprintln!("Total .text instructions: {} CUs", result.total_cus);
    eprintln!();
    eprintln!("Top functions by CU (leaf attribution):");

    let top_n = 20.min(result.function_cus.len());
    for (i, (name, cus)) in result.function_cus.iter().take(top_n).enumerate() {
        let pct = *cus as f64 / result.total_cus as f64 * 100.0;
        eprintln!("  {:>3}. {:>6} CUs ({:>5.1}%)  {}", i + 1, cus, pct, name);
    }

    if result.function_cus.len() > top_n {
        eprintln!(
            "  ... and {} more functions",
            result.function_cus.len() - top_n
        );
    }

    eprintln!();
    eprintln!("Note: Syscall CU costs (CPI, logging, etc.) are runtime-dependent and excluded.");
}

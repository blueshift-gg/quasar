//! Test-only CU recorder for the benchmark pipeline. Included by each
//! example's `tests.rs` via `#[path = "../../cu_bench.rs"]`.
//!
//! Each call appends one JSON line to `target/cu-bench/<package>.jsonl`;
//! `scripts/bench-tracked-programs.sh` reads the tracked values from there
//! instead of scraping test output, so benchmark tests stay silent on
//! success (see TESTING.md).

extern crate std;

use std::{format, fs, io::Write as _, path::PathBuf};

pub fn record_cu(instruction: &str, cu: u64) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/cu-bench");
    fs::create_dir_all(&dir).expect("create target/cu-bench");
    let line = format!("{{\"instruction\":\"{instruction}\",\"cu\":{cu}}}\n");
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(format!("{}.jsonl", env!("CARGO_PKG_NAME"))))
        .expect("open cu-bench record file")
        .write_all(line.as_bytes())
        .expect("append cu-bench record");
}

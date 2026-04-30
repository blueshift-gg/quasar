//! Deny test: no domain knowledge in the derive macro.
//!
//! The derive may know structural syntax (groups, fields, flags, trait consts)
//! but must NOT know domain concepts (program names, account type names,
//! CPI targets, layout semantics).

/// Domain strings that must never appear in derive/src/accounts/ source.
///
/// This list is the single source of truth.
const BANNED: &[&str] = &[
    // ATA domain knowledge (Phase 3 deleted)
    "AssociatedTokenProgram",
    "find_field_by_inner_name",
    // Expression-shape dispatch (Phase 1 deleted)
    "is_raw_slot_safe",
    // Migration type detection (Phase 4 deleted)
    "is_migration_type",
    // SPL param types
    "TokenParams",
    "MintParams",
    "SPL_TOKEN",
    "quasar_spl",
    // SPL trait names
    "HasTokenLayout",
    "HasMintLayout",
    "TokenClose",
    "TokenSweep",
    "CloseCtx",
    "SweepCtx",
    // SPL function names
    "realloc_account",
    "init_token",
    "validate_token",
    "close_token",
    "sweep_token",
    "init_ata",
    "validate_ata",
    // SPL type names
    "AccountClose",
    "SupportsRealloc",
    // Domain names in program type detection
    "SystemProgram",
    // Old PDA machinery (deleted in Task 8)
    "SeedNode",
    "classify_seed",
];

fn collect_rs_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_rs_files(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
}

#[test]
fn deny_domain_strings_in_derive() {
    let derive_src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/accounts");

    let mut files = Vec::new();
    collect_rs_files(&derive_src, &mut files);
    assert!(!files.is_empty(), "no .rs files found in {:?}", derive_src);

    let mut violations = Vec::new();

    for file in &files {
        let content = std::fs::read_to_string(file).unwrap();
        for (line_num, line) in content.lines().enumerate() {
            // Skip comments
            let trimmed = line.trim();
            if trimmed.starts_with("//") {
                continue;
            }
            for term in BANNED {
                if line.contains(term) {
                    violations.push(format!(
                        "  {}:{}: `{}`",
                        file.strip_prefix(env!("CARGO_MANIFEST_DIR"))
                            .unwrap_or(file)
                            .display(),
                        line_num + 1,
                        term,
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Domain strings found in derive/src/accounts/:\n{}",
        violations.join("\n"),
    );
}

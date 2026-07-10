//! Deny test: derive macro boundary enforcement.
//!
//! The derive is protocol-neutral. NO module in derive/src/accounts/ may
//! contain literal SPL domain strings. All SPL knowledge lives in
//! quasar-spl behavior modules. See `ARCHITECTURE.md` (section 3, the enforced
//! layer contract) for the rule this test encodes.

/// Domain strings that must never appear in derive/src/accounts/ source.
const BANNED: &[&str] = &[
    // SPL param types
    "TokenParams",
    "TokenInitKind",
    "MintParams",
    "MintInitParams",
    "SPL_TOKEN",
    "quasar_spl",
    // SPL trait names
    "TokenCheck",
    "MintCheck",
    "AssociatedTokenCheck",
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
    "TokenProgram",
    "Token2022Program",
    "AssociatedTokenProgram",
    "TokenInterface",
    // Old machinery
    "SeedNode",
    "classify_seed",
    "is_migration_type",
    "find_field_by_inner_name",
    "is_raw_slot_safe",
    // Group/op classification (deleted)
    "GroupKind",
    "OpKind",
    "ProgramCategory",
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

    // No exceptions. Every module in derive/src/accounts/ must be
    // protocol-neutral.

    let mut violations = Vec::new();

    for file in &files {
        let rel = file
            .strip_prefix(env!("CARGO_MANIFEST_DIR"))
            .unwrap_or(file);

        let content = std::fs::read_to_string(file).unwrap();
        for (line_num, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") {
                continue;
            }
            for term in BANNED {
                if line.contains(term) {
                    violations.push(format!("  {}:{}: `{}`", rel.display(), line_num + 1, term));
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

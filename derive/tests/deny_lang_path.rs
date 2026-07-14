//! Deny test: crate-identity boundary enforcement (Workstream H1).
//!
//! Generated code must reference the runtime crate through the resolved
//! dependency name, not a hard-coded `quasar_lang::` path. A literal
//! `quasar_lang::` inside a `quote!` body defeats consumer renames
//! (`ql = { package = "quasar-lang" }`). Emitters interpolate the resolved
//! path as `#krate` (see `derive/src/krate.rs`), which is the ONLY module
//! allowed to name `quasar_lang` in emitted tokens.
//!
//! Doc/line comments are exempt (rustdoc intra-doc links legitimately spell the
//! canonical name), as are the snapshot goldens under `src/snapshots/` (they
//! record the *resolved* path the emitters produce).

const BANNED: &str = "quasar_lang::";

fn collect_rs_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Goldens record the resolved path; they are not emitter source.
                if path.file_name().is_some_and(|n| n == "snapshots") {
                    continue;
                }
                collect_rs_files(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
}

#[test]
fn deny_lang_path_in_emitters() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");

    let mut files = Vec::new();
    collect_rs_files(&src, &mut files);
    assert!(!files.is_empty(), "no .rs files found in {src:?}");

    let mut violations = Vec::new();

    for file in &files {
        // `krate.rs` is the sanctioned home of the literal path.
        if file.file_name().is_some_and(|n| n == "krate.rs") {
            continue;
        }
        let rel = file
            .strip_prefix(env!("CARGO_MANIFEST_DIR"))
            .unwrap_or(file);
        let content = std::fs::read_to_string(file).unwrap();
        for (line_num, line) in content.lines().enumerate() {
            // Comments may name the canonical crate (rustdoc links, prose).
            if line.trim_start().starts_with("//") {
                continue;
            }
            if line.contains(BANNED) {
                violations.push(format!("  {}:{}", rel.display(), line_num + 1));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "hard-coded `quasar_lang::` found outside krate.rs (use `#krate`):\n{}",
        violations.join("\n"),
    );
}

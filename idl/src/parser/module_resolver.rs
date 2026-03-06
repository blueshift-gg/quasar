//! Resolves all Rust source files in a crate by walking `mod` declarations.

use std::path::{Path, PathBuf};

/// A resolved source file with its parsed AST.
pub struct ResolvedFile {
    pub path: PathBuf,
    pub file: syn::File,
}

/// Resolve all source files in a crate starting from `src/lib.rs`.
pub fn resolve_crate(crate_root: &Path) -> Vec<ResolvedFile> {
    let lib_path = crate_root.join("src").join("lib.rs");
    let mut files = Vec::new();
    resolve_file(&lib_path, &mut files);
    files
}

fn resolve_file(path: &Path, files: &mut Vec<ResolvedFile>) {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("warning: could not read {}: {}", path.display(), e);
            return;
        }
    };

    let file = match syn::parse_file(&source) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("warning: could not parse {}: {}", path.display(), e);
            return;
        }
    };

    // Find `mod foo;` declarations (external modules) and recurse
    let parent_dir = path.parent().unwrap();
    let stem = path.file_stem().unwrap().to_str().unwrap();

    for item in &file.items {
        if let syn::Item::Mod(item_mod) = item {
            // Skip #[cfg(test)] modules
            if has_cfg_test(&item_mod.attrs) {
                continue;
            }

            // Only follow external module declarations (no body)
            if item_mod.content.is_some() {
                continue;
            }

            let mod_name = item_mod.ident.to_string();

            // Resolve module file path:
            // If current file is mod.rs or lib.rs, look in the same directory
            // Otherwise look in a subdirectory named after the current file
            let search_dir = if stem == "mod" || stem == "lib" {
                parent_dir.to_path_buf()
            } else {
                parent_dir.join(stem)
            };

            // Try <dir>/<mod_name>.rs
            let candidate_file = search_dir.join(format!("{}.rs", mod_name));
            if candidate_file.exists() {
                resolve_file(&candidate_file, files);
                continue;
            }

            // Try <dir>/<mod_name>/mod.rs
            let candidate_dir = search_dir.join(&mod_name).join("mod.rs");
            if candidate_dir.exists() {
                resolve_file(&candidate_dir, files);
                continue;
            }

            eprintln!(
                "warning: could not resolve `mod {};` from {}",
                mod_name,
                path.display()
            );
        }
    }

    files.push(ResolvedFile {
        path: path.to_path_buf(),
        file,
    });
}

fn has_cfg_test(attrs: &[syn::Attribute]) -> bool {
    for attr in attrs {
        if attr.path().is_ident("cfg") {
            let tokens = attr.meta.require_list().ok().map(|l| l.tokens.to_string());
            if let Some(t) = tokens {
                if t.contains("test") {
                    return true;
                }
            }
        }
    }
    false
}

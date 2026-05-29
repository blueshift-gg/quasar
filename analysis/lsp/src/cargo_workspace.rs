//! Cargo-metadata-driven workspace discovery.
//!
//! On startup the server invokes `cargo metadata` rooted at the client's
//! first workspace folder. Every package whose resolved dependency graph
//! transitively includes `quasar-lang` is considered a Quasar crate; the LSP
//! only services files whose canonical path falls under one of those crate
//! roots.

use cargo_metadata::{Metadata, MetadataCommand, Package, PackageId};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// The Cargo crate the framework ships as. A package transitively depending
/// on this is a Quasar crate.
const QUASAR_FRAMEWORK_CRATE: &str = "quasar-lang";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceConfig {
    /// The client's workspace folders, as passed to `cargo metadata`.
    pub workspace_roots: Vec<PathBuf>,
    /// Manifest dirs of workspace-member Quasar crates. Gates *activation*:
    /// only files under these roots receive published diagnostics.
    pub quasar_crate_roots: Vec<PathBuf>,
    /// `#[account]` / `define_account!` type names found on disk across all
    /// Quasar crates (members + `quasar-lang`-depending dependencies). Lets the
    /// resolver tell a genuinely-unknown type from a legitimate one.
    pub known_account_types: Vec<String>,
    /// Every `.rs` file under *any* Quasar crate (members **and** dependencies).
    /// Registered as closed, `File`-backed entries in the symbol index so goto,
    /// completion, hover, has_one checks and references resolve an account type
    /// to its declaration wherever it lives. Deliberately broader than
    /// [`Self::quasar_crate_roots`]: these only feed the index, they don't gate
    /// activation — we resolve into a read-only dependency without linting it.
    pub indexed_source_files: Vec<PathBuf>,
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("cargo metadata failed: {0}")]
    Cargo(#[from] cargo_metadata::Error),
}

struct Loaded {
    crate_roots: Vec<PathBuf>,
    account_types: Vec<String>,
    source_files: Vec<PathBuf>,
}

/// Loads every workspace folder and merges their Quasar crate roots and the
/// account-type names discovered across all Quasar crate sources.
///
/// Per-folder `cargo metadata` failures are logged and skipped — one broken
/// folder doesn't sink the others. Returns `None` only when *every* folder
/// failed to load, which the server treats as "degrade to serving all open
/// files" rather than "gate everything out."
pub fn load_workspaces(roots: &[PathBuf]) -> Option<WorkspaceConfig> {
    let mut any_success = false;
    let mut crate_roots: Vec<PathBuf> = Vec::new();
    let mut account_types: Vec<String> = Vec::new();
    let mut source_files: Vec<PathBuf> = Vec::new();

    for root in roots {
        match load_single(root) {
            Ok(loaded) => {
                any_success = true;
                crate_roots.extend(loaded.crate_roots);
                account_types.extend(loaded.account_types);
                source_files.extend(loaded.source_files);
            }
            Err(err) => {
                tracing::warn!(root = %root.display(), error = %err, "cargo metadata failed");
            }
        }
    }

    if !any_success {
        return None;
    }

    crate_roots.sort();
    crate_roots.dedup();
    account_types.sort();
    account_types.dedup();
    source_files.sort();
    source_files.dedup();
    Some(WorkspaceConfig {
        workspace_roots: roots.to_vec(),
        quasar_crate_roots: crate_roots,
        known_account_types: account_types,
        indexed_source_files: source_files,
    })
}

/// Loads a single workspace folder.
pub fn load_workspace(root: &Path) -> Result<WorkspaceConfig, LoadError> {
    let loaded = load_single(root)?;
    let mut account_types = loaded.account_types;
    account_types.sort();
    account_types.dedup();
    let mut source_files = loaded.source_files;
    source_files.sort();
    source_files.dedup();
    Ok(WorkspaceConfig {
        workspace_roots: vec![root.to_path_buf()],
        quasar_crate_roots: loaded.crate_roots,
        known_account_types: account_types,
        indexed_source_files: source_files,
    })
}

/// Collects every `.rs` file path under the given Quasar-package `src/`
/// directories (members and dependencies alike). Used by the server to
/// register closed, disk-backed files in the symbol index.
fn collect_source_files(src_dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for dir in src_dirs {
        collect_rs_files(dir, &mut files);
    }
    files.sort();
    files.dedup();
    files
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

fn load_single(root: &Path) -> Result<Loaded, LoadError> {
    let metadata = MetadataCommand::new()
        .manifest_path(root.join("Cargo.toml"))
        .exec()?;
    let crate_roots = identify_quasar_crates(&metadata);
    // Both the name scan and the file collection cover *every* Quasar package
    // (members + dependencies) so account types resolve into dependency crates,
    // not just workspace members.
    let src_dirs = quasar_package_src_dirs(&metadata);
    let account_types = scan_account_type_names(&src_dirs);
    let source_files = collect_source_files(&src_dirs);
    Ok(Loaded {
        crate_roots,
        account_types,
        source_files,
    })
}

/// Source directories of every package — member or dependency — that
/// transitively depends on `quasar-lang`, and is therefore capable of
/// declaring `#[account]` types.
///
/// Dirs are derived from each package's `lib`/`bin`/`proc-macro` target
/// `src_path` (the crate root file) rather than assuming a `src/` layout, so
/// crates with a custom `[lib] path` are still scanned. Falls back to
/// `<manifest>/src` for packages whose targets expose no usable source path.
fn quasar_package_src_dirs(metadata: &Metadata) -> Vec<PathBuf> {
    let pkgs_by_id: HashMap<PackageId, &Package> =
        metadata.packages.iter().map(|p| (p.id.clone(), p)).collect();
    let Some(resolve) = &metadata.resolve else {
        return Vec::new();
    };
    let deps_by_id: HashMap<PackageId, Vec<PackageId>> = resolve
        .nodes
        .iter()
        .map(|n| (n.id.clone(), n.dependencies.clone()))
        .collect();

    let mut memo: HashMap<PackageId, bool> = HashMap::new();
    let mut dirs = Vec::new();
    for pkg in &metadata.packages {
        if !depends_on_quasar(&pkg.id, &pkgs_by_id, &deps_by_id, &mut memo, &mut HashSet::new()) {
            continue;
        }
        let before = dirs.len();
        for target in &pkg.targets {
            let is_code = target
                .kind
                .iter()
                .any(|k| k == "lib" || k == "bin" || k == "proc-macro" || k == "rlib");
            if is_code {
                if let Some(parent) = target.src_path.parent() {
                    dirs.push(PathBuf::from(parent));
                }
            }
        }
        // Fallback for packages whose targets had no usable source path.
        if dirs.len() == before {
            if let Some(parent) = pkg.manifest_path.parent() {
                dirs.push(PathBuf::from(parent).join("src"));
            }
        }
    }
    dirs.sort();
    dirs.dedup();
    dirs
}

/// Recursively scans `.rs` files under each directory for `#[account]` type
/// names. Cheap (~1ms/file) and bounded to Quasar packages.
fn scan_account_type_names(src_dirs: &[PathBuf]) -> Vec<String> {
    let mut names = Vec::new();
    for dir in src_dirs {
        scan_dir(dir, &mut names);
    }
    names
}

fn scan_dir(dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_dir(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            if let Ok(text) = std::fs::read_to_string(&path) {
                out.extend(quasar_hir::account_type_names(&text));
            }
        }
    }
}

/// Returns the manifest directory of every workspace member that
/// transitively depends on the Quasar framework crate.
pub fn identify_quasar_crates(metadata: &Metadata) -> Vec<PathBuf> {
    let pkgs_by_id: HashMap<PackageId, &Package> =
        metadata.packages.iter().map(|p| (p.id.clone(), p)).collect();

    let mut transitively_depends_on_quasar: HashMap<PackageId, bool> = HashMap::new();
    let resolve = match &metadata.resolve {
        Some(r) => r,
        None => return Vec::new(),
    };
    let deps_by_id: HashMap<PackageId, Vec<PackageId>> = resolve
        .nodes
        .iter()
        .map(|n| (n.id.clone(), n.dependencies.clone()))
        .collect();

    for pkg in &metadata.packages {
        depends_on_quasar(
            &pkg.id,
            &pkgs_by_id,
            &deps_by_id,
            &mut transitively_depends_on_quasar,
            &mut HashSet::new(),
        );
    }

    let mut roots: Vec<PathBuf> = metadata
        .workspace_members
        .iter()
        .filter_map(|id| {
            let pkg = pkgs_by_id.get(id)?;
            let depends = *transitively_depends_on_quasar.get(id).unwrap_or(&false);
            // The framework crate itself is reported as depending on quasar-lang
            // (it *is* quasar-lang), but we don't service it: it defines the
            // macros rather than consuming them. Other members that use it still
            // qualify.
            if depends && pkg.name != QUASAR_FRAMEWORK_CRATE {
                return pkg.manifest_path.parent().map(|p| p.into());
            }
            None
        })
        .collect();
    roots.sort();
    roots.dedup();
    roots
}

fn depends_on_quasar(
    id: &PackageId,
    pkgs_by_id: &HashMap<PackageId, &Package>,
    deps_by_id: &HashMap<PackageId, Vec<PackageId>>,
    memo: &mut HashMap<PackageId, bool>,
    visiting: &mut HashSet<PackageId>,
) -> bool {
    if let Some(&cached) = memo.get(id) {
        return cached;
    }
    if !visiting.insert(id.clone()) {
        // Cycle guard; resolve as false for the current recursion frame and
        // let the actual answer come from another path.
        return false;
    }

    let direct = pkgs_by_id
        .get(id)
        .map(|p| p.name.as_str() == QUASAR_FRAMEWORK_CRATE)
        .unwrap_or(false);

    let transitive = if direct {
        true
    } else if let Some(deps) = deps_by_id.get(id) {
        deps.iter()
            .any(|d| depends_on_quasar(d, pkgs_by_id, deps_by_id, memo, visiting))
    } else {
        false
    };

    visiting.remove(id);
    memo.insert(id.clone(), transitive);
    transitive
}

impl WorkspaceConfig {
    /// True when `path` is inside one of the discovered Quasar crate roots.
    ///
    /// Compares canonicalized paths so a symlinked checkout (the editor sends
    /// one path form, `cargo metadata` reports another) still matches, falling
    /// back to a raw prefix check when canonicalization fails — e.g. a path
    /// that doesn't exist on disk, as in unit tests.
    pub fn covers(&self, path: &Path) -> bool {
        let canon = std::fs::canonicalize(path).ok();
        self.quasar_crate_roots.iter().any(|root| {
            if path.starts_with(root) {
                return true;
            }
            match (&canon, std::fs::canonicalize(root).ok()) {
                (Some(p), Some(r)) => p.starts_with(&r),
                _ => false,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covers_matches_paths_inside_crate_roots() {
        let cfg = WorkspaceConfig {
            workspace_roots: vec![PathBuf::from("/ws")],
            quasar_crate_roots: vec![
                PathBuf::from("/ws/user-program"),
                PathBuf::from("/ws/other-program"),
            ],
            known_account_types: Vec::new(),
            indexed_source_files: Vec::new(),
        };
        assert!(cfg.covers(Path::new("/ws/user-program/src/lib.rs")));
        assert!(cfg.covers(Path::new("/ws/other-program/src/lib.rs")));
        assert!(!cfg.covers(Path::new("/ws/non-quasar/src/lib.rs")));
        assert!(!cfg.covers(Path::new("/elsewhere/file.rs")));
    }

    /// Smoke-test against the live Quasar workspace this LSP itself lives in.
    /// `examples/escrow` is a known workspace member that depends on
    /// quasar-lang via a path dependency, so it should be identified.
    /// `analysis/lsp` is also a workspace member, but does NOT depend on
    /// quasar-lang, so it should NOT be identified.
    #[test]
    fn load_workspace_identifies_real_quasar_crates() {
        let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let workspace_root = PathBuf::from(&crate_dir)
            .ancestors()
            .nth(2)
            .unwrap()
            .to_path_buf();

        let cfg = load_workspace(&workspace_root).expect("cargo metadata on quasar workspace");

        let escrow_present = cfg
            .quasar_crate_roots
            .iter()
            .any(|p| p.ends_with("examples/escrow") || p.ends_with("escrow"));
        assert!(
            escrow_present,
            "examples/escrow should be a Quasar crate, got {:?}",
            cfg.quasar_crate_roots
        );

        let lsp_present = cfg
            .quasar_crate_roots
            .iter()
            .any(|p| p.ends_with("analysis/lsp") || p.ends_with("lsp"));
        assert!(
            !lsp_present,
            "analysis/lsp must not be flagged as a Quasar crate, got {:?}",
            cfg.quasar_crate_roots
        );

        // The framework crate itself (`quasar-lang`, in `lang/`) defines the
        // macros rather than consuming them, so it must not be serviced.
        let framework_present = cfg.quasar_crate_roots.iter().any(|p| p.ends_with("lang"));
        assert!(
            !framework_present,
            "quasar-lang (lang/) must not be a serviced crate root, got {:?}",
            cfg.quasar_crate_roots
        );

        // The dependency scan should pull SPL account types (defined via
        // `define_account!`) out of quasar-spl's sources, plus the user's own
        // `#[account]` types from the examples.
        assert!(
            cfg.known_account_types.iter().any(|n| n == "Mint"),
            "Mint (define_account! in quasar-spl) should be indexed, got {:?}",
            cfg.known_account_types
        );
        assert!(
            cfg.known_account_types.iter().any(|n| n == "Token"),
            "Token (define_account! in quasar-spl) should be indexed"
        );
        assert!(
            cfg.known_account_types.iter().any(|n| n == "Escrow"),
            "Escrow (#[account] in examples/escrow) should be indexed"
        );

        // Indexed source files should include both the user's own crate
        // (escrow `state.rs`, holding `#[account] Escrow`) and the Quasar
        // crate that declares the SPL types (`spl/src/token.rs`, holding the
        // `define_account!` Mint/Token), so account types resolve into either.
        assert!(
            cfg.indexed_source_files
                .iter()
                .any(|p| p.ends_with("examples/escrow/src/state.rs")),
            "escrow state.rs should be an indexed source file, got {:?}",
            cfg.indexed_source_files
        );
        assert!(
            cfg.indexed_source_files
                .iter()
                .any(|p| p.ends_with("spl/src/token.rs")),
            "spl token.rs (define_account! Mint/Token) should be indexed, got {:?}",
            cfg.indexed_source_files
        );
    }
}

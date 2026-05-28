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
    pub workspace_root: PathBuf,
    pub quasar_crate_roots: Vec<PathBuf>,
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("cargo metadata failed: {0}")]
    Cargo(#[from] cargo_metadata::Error),
}

pub fn load_workspace(root: &Path) -> Result<WorkspaceConfig, LoadError> {
    let metadata = MetadataCommand::new()
        .manifest_path(root.join("Cargo.toml"))
        .exec()?;
    let quasar_crate_roots = identify_quasar_crates(&metadata);
    Ok(WorkspaceConfig {
        workspace_root: root.to_path_buf(),
        quasar_crate_roots,
    })
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
            if depends || pkg.name == QUASAR_FRAMEWORK_CRATE {
                // A package may directly _be_ quasar-lang (when running the LSP
                // inside the framework workspace). We do not service those
                // either since they define the macros rather than consume
                // them, but other workspace members that use them still
                // qualify above.
                if depends {
                    return pkg.manifest_path.parent().map(|p| p.into());
                }
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
    pub fn covers(&self, path: &Path) -> bool {
        self.quasar_crate_roots
            .iter()
            .any(|root| path.starts_with(root))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covers_matches_paths_inside_crate_roots() {
        let cfg = WorkspaceConfig {
            workspace_root: PathBuf::from("/ws"),
            quasar_crate_roots: vec![
                PathBuf::from("/ws/user-program"),
                PathBuf::from("/ws/other-program"),
            ],
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
    }
}

//! Workspace-level Salsa primitives: a multi-file collection and the
//! name -> defining-file index built from per-file scope scans.

use crate::db::Db;
use crate::input::File;
use crate::items::{ItemKind, Symbol};
use crate::scope::scope_items;
use std::collections::HashMap;
use std::sync::Arc;

/// All files visible to one cross-file analysis. The LSP layer rebuilds the
/// list when cargo metadata changes; tests construct it directly.
#[salsa::input]
pub struct Workspace {
    #[returns(ref)]
    pub files: Vec<File>,
}

/// Per-symbol entry in the workspace index: where the symbol is declared and
/// what kind of item it is.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SymbolEntry {
    pub file: File,
    pub kind: ItemKind,
}

/// Name -> defining-file index built by aggregating [`scope_items`] across
/// every file in the [`Workspace`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolIndex {
    entries: HashMap<String, SymbolEntry>,
}

impl SymbolIndex {
    pub fn lookup(&self, name: &str) -> Option<&SymbolEntry> {
        self.entries.get(name)
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }
}

#[salsa::tracked(returns(ref))]
pub fn workspace_symbol_index<'db>(
    db: &'db dyn Db,
    workspace: Workspace,
) -> Arc<SymbolIndex> {
    let mut entries: HashMap<String, SymbolEntry> = HashMap::new();
    for &file in workspace.files(db) {
        let symbols: &Arc<[Symbol]> = scope_items(db, file);
        for sym in symbols.iter() {
            entries
                .entry(sym.name.clone())
                .or_insert(SymbolEntry { file, kind: sym.kind });
        }
    }
    Arc::new(SymbolIndex { entries })
}

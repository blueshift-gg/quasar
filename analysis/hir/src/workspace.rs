//! Workspace-level Salsa primitives: a multi-file collection and the
//! name -> defining-file index built from per-file scope scans, augmented
//! with account-type names discovered in dependency-crate sources.

use {
    crate::{
        db::Db,
        input::File,
        items::{FieldDecl, ItemKind, Symbol},
        parse::parse_file,
        scope::scope_items,
    },
    std::{
        collections::{HashMap, HashSet},
        sync::Arc,
    },
};

/// All files visible to one cross-file analysis, plus the set of `#[account]`
/// type names found in dependency-crate sources (e.g. SPL `Mint`/`Token`).
/// The LSP layer rebuilds both when cargo metadata changes; tests construct
/// them directly.
#[salsa::input]
pub struct Workspace {
    #[returns(ref)]
    pub files: Vec<File>,
    /// Account-type names declared on disk in Quasar crates that aren't open
    /// in the editor (workspace members not yet opened + dependencies that
    /// depend on `quasar-lang`). Lets the resolver tell a genuinely-unknown
    /// type from a legitimate external one.
    #[returns(ref)]
    pub known_account_types: Vec<String>,
}

/// Per-symbol entry in the workspace index: where the symbol is declared and
/// what kind of item it is.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SymbolEntry {
    pub file: File,
    pub kind: ItemKind,
}

/// Name -> defining-file index built by aggregating [`scope_items`] across
/// every open file, plus an `external` set of account-type names known only
/// by name (from dependency sources).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolIndex {
    entries: HashMap<String, SymbolEntry>,
    external: HashSet<String>,
}

impl SymbolIndex {
    /// Lookup a live (open-file) symbol with its defining `File`.
    pub fn lookup(&self, name: &str) -> Option<&SymbolEntry> {
        self.entries.get(name)
    }

    /// True if `name` is a known account type — either a live workspace
    /// `#[account]` type or one indexed by name from dependency sources.
    pub fn is_known_account_type(&self, name: &str) -> bool {
        if let Some(entry) = self.entries.get(name) {
            if entry.kind == ItemKind::AccountType {
                return true;
            }
        }
        self.external.contains(name)
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }
}

/// A plain data struct located somewhere in the workspace: which file declares
/// it and its named fields. Used to recover a `define_account!` account type's
/// fields from its `: Data` clause, wherever that struct lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataStruct {
    pub file: File,
    pub fields: Vec<FieldDecl>,
}

/// Workspace-wide index of plain data structs by name. Built by aggregating
/// each file's [`ParsedFile::data_structs`](crate::parse::ParsedFile). The
/// first declaration of a given name wins (stable across files).
#[salsa::tracked(returns(ref))]
pub fn data_struct_index(db: &dyn Db, workspace: Workspace) -> Arc<HashMap<String, DataStruct>> {
    let mut map: HashMap<String, DataStruct> = HashMap::new();
    for &file in workspace.files(db) {
        let parsed = parse_file(db, file);
        for (name, fields) in parsed.data_structs(db).iter() {
            map.entry(name.clone()).or_insert_with(|| DataStruct {
                file,
                fields: fields.clone(),
            });
        }
    }
    Arc::new(map)
}

#[salsa::tracked(returns(ref))]
pub fn workspace_symbol_index(db: &dyn Db, workspace: Workspace) -> Arc<SymbolIndex> {
    let mut entries: HashMap<String, SymbolEntry> = HashMap::new();
    for &file in workspace.files(db) {
        let symbols: &Arc<[Symbol]> = scope_items(db, file);
        for sym in symbols.iter() {
            entries.entry(sym.name.clone()).or_insert(SymbolEntry {
                file,
                kind: sym.kind,
            });
        }
    }
    let external: HashSet<String> = workspace.known_account_types(db).iter().cloned().collect();
    Arc::new(SymbolIndex { entries, external })
}

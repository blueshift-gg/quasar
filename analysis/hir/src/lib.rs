//! Quasar HIR. Salsa-backed cross-file semantic analysis layered on top of
//! `quasar-syntax`.

pub mod accounts;
pub mod db;
pub mod diagnostic;
pub mod input;
pub mod items;
pub mod parse;
pub mod resolver;
pub mod scope;
pub mod workspace;

pub use db::{Database, Db};
pub use diagnostic::HirDiagnostic;
pub use input::File;
pub use items::{ItemHead, ItemKind, Symbol};
pub use parse::{line_index_for, parse_file, ParsedFile};
pub use resolver::{resolve_account_refs, AccountRef, AccountRefResolution};
pub use scope::scope_items;
pub use workspace::{workspace_symbol_index, SymbolEntry, SymbolIndex, Workspace};

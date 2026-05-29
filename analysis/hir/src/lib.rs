//! Quasar HIR. Salsa-backed cross-file semantic analysis layered on top of
//! `quasar-syntax`.

pub mod accounts;
pub mod constraints;
pub mod db;
pub mod diagnostic;
pub mod has_one;
pub mod input;
pub mod items;
pub mod parse;
pub mod resolver;
pub mod scope;
pub mod workspace;

pub use db::{Database, Db};
pub use diagnostic::HirDiagnostic;
pub use input::File;
pub use items::{FieldDecl, ItemHead, ItemKind, Symbol};
pub use parse::{line_index_for, parse_file, ParsedFile};
pub use constraints::{validate_accounts, AccountsValidation};
pub use has_one::{resolve_has_one, HasOneRef, HasOneResolution};
pub use resolver::{resolve_account_refs, AccountRef, AccountRefResolution};
pub use scope::{account_type_names, scope_items};
pub use workspace::{
    data_struct_index, workspace_symbol_index, DataStruct, SymbolEntry, SymbolIndex, Workspace,
};

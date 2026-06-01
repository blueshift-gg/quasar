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

pub use {
    constraints::{validate_accounts, AccountsValidation},
    db::{Database, Db},
    diagnostic::HirDiagnostic,
    has_one::{resolve_has_one, HasOneRef, HasOneResolution},
    input::File,
    items::{FieldDecl, ItemHead, ItemKind, Symbol},
    parse::{line_index_for, parse_file, ParsedFile},
    resolver::{resolve_account_refs, AccountRef, AccountRefResolution},
    scope::{account_type_names, scope_items},
    workspace::{
        data_struct_index, workspace_symbol_index, DataStruct, SymbolEntry, SymbolIndex, Workspace,
    },
};

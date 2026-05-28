//! `scope_items` — the cheap eager scan. Returns just the names and locations
//! of Quasar-relevant items in a file, without parsing attribute bodies.
//!
//! Value-stable across edits that don't add or remove top-level Quasar items,
//! so workspace-wide symbol indexes built on top of it get aggressive early
//! cutoff.

use crate::db::Db;
use crate::input::File;
use crate::items::{ItemKind, Symbol};
use std::sync::Arc;
use syn::{Attribute, Item};

#[salsa::tracked(returns(ref))]
pub fn scope_items<'db>(db: &'db dyn Db, file: File) -> Arc<[Symbol]> {
    let text = file.text(db);
    let Ok(tree) = syn::parse_file(text.as_ref()) else {
        return Arc::from([]);
    };

    let mut out: Vec<Symbol> = Vec::new();
    for item in &tree.items {
        let Item::Struct(item_struct) = item else {
            continue;
        };
        let kind = classify_struct(&item_struct.attrs);
        if let Some(kind) = kind {
            out.push(Symbol {
                name: item_struct.ident.to_string(),
                kind,
            });
        }
    }
    Arc::from(out.into_boxed_slice())
}

fn classify_struct(attrs: &[Attribute]) -> Option<ItemKind> {
    if attrs.iter().any(|a| a.path().is_ident("account")) {
        return Some(ItemKind::AccountType);
    }
    if attrs.iter().any(derives_accounts) {
        return Some(ItemKind::AccountsStruct);
    }
    None
}

/// True if the attribute is `#[derive(...)]` and one of its arguments is the
/// `Accounts` identifier.
pub(crate) fn derives_accounts(attr: &Attribute) -> bool {
    if !attr.path().is_ident("derive") {
        return false;
    }
    let mut found = false;
    let _ = attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("Accounts") {
            found = true;
        }
        Ok(())
    });
    found
}

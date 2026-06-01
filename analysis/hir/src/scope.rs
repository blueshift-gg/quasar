//! `scope_items` — the cheap eager scan. Returns just the names and locations
//! of Quasar-relevant items in a file, without parsing attribute bodies.
//!
//! Value-stable across edits that don't add or remove top-level Quasar items,
//! so workspace-wide symbol indexes built on top of it get aggressive early
//! cutoff.

use {
    crate::{
        db::Db,
        input::File,
        items::{ItemKind, Symbol},
    },
    std::sync::Arc,
    syn::{Attribute, Item},
};

#[salsa::tracked(returns(ref))]
pub fn scope_items(db: &dyn Db, file: File) -> Arc<[Symbol]> {
    let text = file.text(db);
    let Ok(tree) = syn::parse_file(text.as_ref()) else {
        return Arc::from([]);
    };

    let mut out: Vec<Symbol> = Vec::new();
    for item in &tree.items {
        match item {
            Item::Struct(item_struct) => {
                if let Some(kind) = classify_struct(&item_struct.attrs) {
                    out.push(Symbol {
                        name: item_struct.ident.to_string(),
                        kind,
                    });
                }
            }
            // `define_account!(pub struct Token => …)` declares an account type
            // just like `#[account]`, so it earns a symbol-index entry.
            Item::Macro(m) if is_define_account(m) => {
                for ident in struct_idents_in_tokens(m.mac.tokens.clone()) {
                    out.push(Symbol {
                        name: ident.to_string(),
                        kind: ItemKind::AccountType,
                    });
                }
            }
            _ => {}
        }
    }
    Arc::from(out.into_boxed_slice())
}

/// Names of account types declared in a source string. Pure (no Salsa) —
/// used to index dependency-crate sources that aren't open in the editor.
///
/// Recognises both forms:
///   - `#[account] pub struct Counter { … }` (user account types), and
///   - `define_account!(pub struct Token => [checks]: Data)` (framework / SPL
///     account types such as `Mint`, `Token`).
pub fn account_type_names(text: &str) -> Vec<String> {
    let Ok(tree) = syn::parse_file(text) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for item in &tree.items {
        match item {
            Item::Struct(s) if s.attrs.iter().any(|a| a.path().is_ident("account")) => {
                names.push(s.ident.to_string());
            }
            Item::Macro(m) if is_define_account(m) => {
                names.extend(
                    struct_idents_in_tokens(m.mac.tokens.clone())
                        .iter()
                        .map(|id| id.to_string()),
                );
            }
            _ => {}
        }
    }
    names
}

/// True when a macro invocation is `define_account!` (matched on the last path
/// segment, so both `define_account!` and `quasar_lang::define_account!`
/// count).
pub(crate) fn is_define_account(m: &syn::ItemMacro) -> bool {
    m.mac
        .path
        .segments
        .last()
        .is_some_and(|seg| seg.ident == "define_account")
}

/// Extracts every `struct <Name>` identifier from a token stream — used to
/// pull account-type names (with spans) out of `define_account!` macro bodies.
pub(crate) fn struct_idents_in_tokens(tokens: proc_macro2::TokenStream) -> Vec<proc_macro2::Ident> {
    let mut idents = Vec::new();
    let mut prev_was_struct = false;
    for tt in tokens {
        if let proc_macro2::TokenTree::Ident(id) = tt {
            if prev_was_struct {
                idents.push(id);
                prev_was_struct = false;
            } else {
                prev_was_struct = id == "struct";
            }
        } else {
            prev_was_struct = false;
        }
    }
    idents
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

#[cfg(test)]
mod tests {
    use super::account_type_names;

    #[test]
    fn finds_account_attr_and_define_account_forms() {
        let src = r#"
#[account(discriminator = 1)]
pub struct Counter { pub n: u64 }

quasar_lang::define_account!(
    /// doc
    pub struct Token => [checks::ZeroPod]: TokenData
);

quasar_lang::define_account!(pub struct TokenProgram => [checks::Executable]);

pub struct NotAnAccount { pub x: u8 }
"#;
        let mut names = account_type_names(src);
        names.sort();
        assert_eq!(names, vec!["Counter", "Token", "TokenProgram"]);
    }
}

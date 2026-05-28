//! Cross-file resolver. Walks `#[derive(Accounts)]` structs, finds
//! `Account<T>` field references, and resolves each `T` against the
//! workspace symbol index. Unresolved references become diagnostics.

use crate::db::Db;
use crate::diagnostic::HirDiagnostic;
use crate::input::File;
use crate::items::ByteRange;
use crate::scope::derives_accounts;
use crate::workspace::{workspace_symbol_index, Workspace};
use quasar_syntax::diagnostics::{DiagCode, Diagnostic, Diagnostics, Severity};
use quasar_syntax::types::extract_generic_inner_type;
use syn::{Item, Type};

/// One `Account<T>` (or `InterfaceAccount<T>`) reference detected in a file.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AccountRef {
    /// The referenced type's bare identifier (the last segment of its path).
    pub name: String,
    /// Byte range of the identifier in the source file.
    pub range: ByteRange,
}

/// Per-reference resolution outcome.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AccountRefResolution {
    Resolved { defining_file: File },
    Unknown,
}

#[salsa::tracked(debug)]
pub struct ResolvedFile<'db> {
    #[returns(ref)]
    pub refs: Vec<(AccountRef, AccountRefResolution)>,
    #[returns(ref)]
    pub diagnostics: Vec<HirDiagnostic>,
}

#[salsa::tracked]
pub fn resolve_account_refs<'db>(
    db: &'db dyn Db,
    workspace: Workspace,
    file: File,
) -> ResolvedFile<'db> {
    let text = file.text(db);
    let Ok(tree) = syn::parse_file(text.as_ref()) else {
        return ResolvedFile::new(db, vec![], vec![]);
    };

    let index = workspace_symbol_index(db, workspace);

    let mut refs: Vec<(AccountRef, AccountRefResolution)> = Vec::new();
    let mut sink = Diagnostics::new();

    for item in &tree.items {
        let Item::Struct(item_struct) = item else {
            continue;
        };
        if !item_struct.attrs.iter().any(derives_accounts) {
            continue;
        }

        for field in &item_struct.fields {
            if let Some(account_ref) = extract_account_ref(&field.ty) {
                let resolution = match index.lookup(&account_ref.name) {
                    Some(entry) if entry.kind == crate::items::ItemKind::AccountType => {
                        AccountRefResolution::Resolved {
                            defining_file: entry.file,
                        }
                    }
                    _ => {
                        sink.emit(Diagnostic {
                            severity: Severity::Error,
                            code: DiagCode::UnknownAccountType,
                            message: format!(
                                "unknown account type `{}`: not found in workspace",
                                account_ref.name
                            ),
                            primary: ident_span(&field.ty, &account_ref.name)
                                .unwrap_or_else(proc_macro2::Span::call_site),
                            labels: vec![],
                            fixes: vec![],
                        });
                        AccountRefResolution::Unknown
                    }
                };
                refs.push((account_ref, resolution));
            }
        }
    }

    let diagnostics = sink
        .into_items()
        .into_iter()
        .map(HirDiagnostic::lower)
        .collect();

    ResolvedFile::new(db, refs, diagnostics)
}

/// Returns the `T` from `Account<T>` / `InterfaceAccount<T>` / `Option<...>`
/// / `&'_ mut Account<T>` etc. by peeling reference, Option, and the typed
/// wrapper.
fn extract_account_ref(ty: &Type) -> Option<AccountRef> {
    let after_option = extract_generic_inner_type(ty, "Option")
        .cloned()
        .unwrap_or_else(|| ty.clone());

    let effective = match after_option {
        Type::Reference(r) => (*r.elem).clone(),
        other => other,
    };

    let inner = extract_generic_inner_type(&effective, "Account")
        .or_else(|| extract_generic_inner_type(&effective, "InterfaceAccount"))?;

    let Type::Path(type_path) = inner else {
        return None;
    };
    let last = type_path.path.segments.last()?;
    Some(AccountRef {
        name: last.ident.to_string(),
        range: ByteRange::from_span(last.ident.span()),
    })
}

/// Find the span of the named identifier inside the field's type tree, used
/// as the primary span for diagnostics.
fn ident_span(ty: &Type, name: &str) -> Option<proc_macro2::Span> {
    let after_option = extract_generic_inner_type(ty, "Option")
        .cloned()
        .unwrap_or_else(|| ty.clone());
    let effective = match after_option {
        Type::Reference(r) => (*r.elem).clone(),
        other => other,
    };
    let inner = extract_generic_inner_type(&effective, "Account")
        .or_else(|| extract_generic_inner_type(&effective, "InterfaceAccount"))?;
    let Type::Path(type_path) = inner else {
        return None;
    };
    for seg in &type_path.path.segments {
        if seg.ident == name {
            return Some(seg.ident.span());
        }
    }
    None
}

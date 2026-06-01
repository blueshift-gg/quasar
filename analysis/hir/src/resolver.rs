//! Cross-file resolver. Walks `#[derive(Accounts)]` structs, finds
//! `Account<T>` field references, and resolves each `T` against the workspace
//! symbol index. Resolution drives hover and goto-definition; unresolved
//! references are recorded as `Unknown` but not diagnosed, because we can't
//! distinguish a typo of a local type from a legitimate external account type
//! reached through a dependency.

use {
    crate::{
        db::Db,
        diagnostic::HirDiagnostic,
        input::File,
        items::ByteRange,
        scope::derives_accounts,
        workspace::{workspace_symbol_index, Workspace},
    },
    quasar_syntax::{
        diagnostics::{DiagCode, Diagnostic, Diagnostics, Severity},
        types::extract_generic_inner_type,
    },
    syn::{Item, Type},
};

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
    /// Resolved to an indexed account type — `#[account]` or `define_account!`,
    /// in any Quasar crate (member or dependency). Carries its declaring file
    /// for goto/hover.
    Resolved { defining_file: File },
    /// Known by name (in `known_account_types`) but with no indexed `File` to
    /// jump to — a fallback for a real type whose source wasn't indexed. Not
    /// diagnosed. Rare now that dependency sources are indexed.
    ResolvedExternal,
    /// Not found anywhere — genuinely unknown (typo or missing definition).
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
                    _ if index.is_known_account_type(&account_ref.name) => {
                        // Known by name but not indexed to a file — a real type
                        // we just can't jump to. Not diagnosed.
                        AccountRefResolution::ResolvedExternal
                    }
                    _ => {
                        // Indexed across every Quasar crate (members + deps), so
                        // a name found in neither the index nor the name set is
                        // genuinely unknown — worth flagging.
                        sink.emit(Diagnostic {
                            severity: Severity::Error,
                            code: DiagCode::UnknownAccountType,
                            message: format!(
                                "unknown account type `{}`: not found in the workspace or its \
                                 dependencies",
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

/// Find the span of the named identifier inside the field's type tree.
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
    type_path
        .path
        .segments
        .iter()
        .find(|seg| seg.ident == name)
        .map(|seg| seg.ident.span())
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

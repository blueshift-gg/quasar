//! `has_one` resolution for `#[derive(Accounts)]` fields.
//!
//! `#[account(has_one(authority))]` on a binding `vault: Account<Vault>`
//! asserts `vault.authority == authority.key()`. Each target therefore
//! references two things:
//!   - a sibling binding in the same Accounts struct (`authority`), and
//!   - a field of the binding's account type (`Vault.authority`).
//!
//! This query resolves both and produces diagnostics for the common
//! mistakes: referencing a non-existent binding, or an account type that
//! lacks the named field.

use crate::db::Db;
use crate::diagnostic::HirDiagnostic;
use crate::input::File;
use crate::items::{ByteRange, FieldDecl, ItemKind};
use crate::parse::parse_file;
use crate::workspace::{
    data_struct_index, workspace_symbol_index, DataStruct, SymbolIndex, Workspace,
};
use std::collections::HashMap;
use quasar_syntax::accounts::{parse_field_attrs_recoverable, Directive, UserCheck};
use quasar_syntax::diagnostics::{DiagCode, Diagnostic, Diagnostics, Severity};
use quasar_syntax::types::extract_generic_inner_type;
use std::collections::HashSet;
use syn::{Item, Type};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HasOneRef {
    /// The referenced target identifier (e.g. `authority`).
    pub target: String,
    /// Byte range of the target identifier in the source file.
    pub range: ByteRange,
    /// The binding the `has_one` is attached to (e.g. `vault`).
    pub binding: String,
    /// The account type of the binding (`Vault`), if it could be extracted.
    pub account_type: Option<String>,
    pub resolution: HasOneResolution,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HasOneResolution {
    Resolved,
    /// No sibling binding with this name in the Accounts struct.
    UnknownBinding,
    /// A sibling binding exists, but the account type lacks the named field.
    MissingAccountField { account_type: String },
}

#[salsa::tracked(debug)]
pub struct HasOneResolved<'db> {
    #[returns(ref)]
    pub refs: Vec<HasOneRef>,
    #[returns(ref)]
    pub diagnostics: Vec<HirDiagnostic>,
}

#[salsa::tracked]
pub fn resolve_has_one<'db>(
    db: &'db dyn Db,
    workspace: Workspace,
    file: File,
) -> HasOneResolved<'db> {
    let text = file.text(db);
    let Ok(tree) = syn::parse_file(text.as_ref()) else {
        return HasOneResolved::new(db, vec![], vec![]);
    };
    let index = workspace_symbol_index(db, workspace);
    let data_index = data_struct_index(db, workspace);

    let mut refs: Vec<HasOneRef> = Vec::new();
    let mut sink = Diagnostics::new();

    for item in &tree.items {
        let Item::Struct(item_struct) = item else {
            continue;
        };
        if !item_struct.attrs.iter().any(crate::scope::derives_accounts) {
            continue;
        }

        let binding_names: HashSet<String> = item_struct
            .fields
            .iter()
            .filter_map(|f| f.ident.as_ref().map(|i| i.to_string()))
            .collect();

        for field in &item_struct.fields {
            let Some(binding_ident) = field.ident.as_ref() else {
                continue;
            };
            let binding = binding_ident.to_string();
            let account_type = extract_account_type(&field.ty);

            let mut field_sink = Diagnostics::new();
            let directives = parse_field_attrs_recoverable(field, &mut field_sink);

            for directive in &directives {
                let Directive::Check(UserCheck::HasOne { targets, .. }) = directive else {
                    continue;
                };
                for target_ident in targets {
                    let target = target_ident.to_string();
                    let range = ByteRange::from_span(target_ident.span());
                    let resolution = resolve_target(
                        db,
                        index,
                        data_index,
                        &target,
                        &binding_names,
                        account_type.as_deref(),
                    );

                    match &resolution {
                        HasOneResolution::UnknownBinding => sink.emit(Diagnostic {
                            severity: Severity::Error,
                            code: DiagCode::HasOneUnknownBinding,
                            message: format!(
                                "`has_one({target})`: no binding `{target}` in this accounts struct"
                            ),
                            primary: target_ident.span(),
                            labels: vec![],
                            fixes: vec![],
                        }),
                        HasOneResolution::MissingAccountField { account_type } => {
                            sink.emit(Diagnostic {
                                severity: Severity::Error,
                                code: DiagCode::HasOneMissingAccountField,
                                message: format!(
                                    "account type `{account_type}` has no field `{target}` \
                                     referenced by `has_one`"
                                ),
                                primary: target_ident.span(),
                                labels: vec![],
                                fixes: vec![],
                            })
                        }
                        HasOneResolution::Resolved => {}
                    }

                    refs.push(HasOneRef {
                        target,
                        range,
                        binding: binding.clone(),
                        account_type: account_type.clone(),
                        resolution,
                    });
                }
            }
        }
    }

    let diagnostics = sink
        .into_items()
        .into_iter()
        .map(HirDiagnostic::lower)
        .collect();
    HasOneResolved::new(db, refs, diagnostics)
}

fn resolve_target(
    db: &dyn Db,
    index: &SymbolIndex,
    data_index: &HashMap<String, DataStruct>,
    target: &str,
    binding_names: &HashSet<String>,
    account_type: Option<&str>,
) -> HasOneResolution {
    if !binding_names.contains(target) {
        return HasOneResolution::UnknownBinding;
    }

    // A sibling binding exists; if we can resolve the account type, ensure the
    // field is present. If the account type is unknown, defer to the
    // unknown-account-type diagnostic from the account resolver and treat the
    // has_one target as resolved here.
    if let Some(at) = account_type {
        if let Some(entry) = index.lookup(at) {
            if entry.kind == ItemKind::AccountType {
                let parsed = parse_file(db, entry.file);
                if let Some(item) = parsed
                    .items(db)
                    .iter()
                    .find(|i| i.name == at && i.kind == ItemKind::AccountType)
                {
                    // Resolve the authoritative field list: directly-parsed
                    // structs carry it inline; a `define_account!` type's lives
                    // on its data struct, found workspace-wide via `data_type`.
                    // When neither is available the field set is unknown, so we
                    // leave the target resolved rather than false-flagging.
                    let fields: Option<&[FieldDecl]> = if item.fields_known {
                        Some(&item.fields)
                    } else {
                        item.data_type
                            .as_ref()
                            .and_then(|dt| data_index.get(dt))
                            .map(|ds| ds.fields.as_slice())
                    };
                    if let Some(fields) = fields {
                        if !fields.iter().any(|f| f.name == target) {
                            return HasOneResolution::MissingAccountField {
                                account_type: at.to_string(),
                            };
                        }
                    }
                }
            }
        }
    }

    HasOneResolution::Resolved
}

/// Peels `&`, `&mut`, and `Option<...>` then returns the inner type name of
/// an `Account<T>` / `InterfaceAccount<T>` wrapper.
fn extract_account_type(ty: &Type) -> Option<String> {
    let after_option = extract_generic_inner_type(ty, "Option")
        .cloned()
        .unwrap_or_else(|| ty.clone());
    let effective = match after_option {
        Type::Reference(r) => (*r.elem).clone(),
        other => other,
    };
    let inner = extract_generic_inner_type(&effective, "Account")
        .or_else(|| extract_generic_inner_type(&effective, "InterfaceAccount"))?;
    let Type::Path(tp) = inner else {
        return None;
    };
    Some(tp.path.segments.last()?.ident.to_string())
}

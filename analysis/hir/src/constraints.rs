//! Structural constraint validation for `#[derive(Accounts)]` fields.
//!
//! Runs the lifted `lower_semantics` (the same parse -> lower -> validate
//! pipeline the derive uses at compile time) over each Accounts struct and
//! surfaces any violation as an LSP diagnostic. Because it reuses the
//! derive's own logic, the LSP and the macro can never disagree about which
//! constraint combinations are legal (`init` requires `mut`, `realloc`
//! requires `mut`, `dup` requires a `/// CHECK:` doc, init/realloc are
//! mutually exclusive, and so on).

use {
    crate::{accounts::lower_semantics, db::Db, diagnostic::HirDiagnostic, input::File},
    quasar_syntax::diagnostics::{DiagCode, Diagnostic, Diagnostics, Severity},
    syn::{Fields, Item},
};

#[salsa::tracked(debug)]
pub struct AccountsValidation<'db> {
    #[returns(ref)]
    pub diagnostics: Vec<HirDiagnostic>,
}

#[salsa::tracked]
pub fn validate_accounts<'db>(db: &'db dyn Db, file: File) -> AccountsValidation<'db> {
    let text = file.text(db);
    let Ok(tree) = syn::parse_file(text.as_ref()) else {
        return AccountsValidation::new(db, vec![]);
    };

    let mut sink = Diagnostics::new();
    for item in &tree.items {
        let Item::Struct(item_struct) = item else {
            continue;
        };
        if !item_struct.attrs.iter().any(crate::scope::derives_accounts) {
            continue;
        }
        let Fields::Named(named) = &item_struct.fields else {
            continue;
        };
        if let Err(err) = lower_semantics(&named.named) {
            sink.emit(Diagnostic {
                severity: Severity::Error,
                code: DiagCode::AccountsConstraintViolation,
                message: err.to_string(),
                primary: err.span(),
                labels: vec![],
                fixes: vec![],
            });
        }
    }

    let diagnostics = sink
        .into_items()
        .into_iter()
        .map(HirDiagnostic::lower)
        .collect();
    AccountsValidation::new(db, diagnostics)
}

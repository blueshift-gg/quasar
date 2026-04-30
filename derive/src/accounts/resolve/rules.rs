//! Structural validation rules — no domain knowledge.

use super::FieldSemantics;

pub(super) fn validate_semantics(semantics: &[FieldSemantics]) -> syn::Result<()> {
    for sem in semantics {
        validate_field(sem)?;
    }
    Ok(())
}

fn validate_field(sem: &FieldSemantics) -> syn::Result<()> {
    let span = &sem.core.field;

    // init requires mut
    if sem.has_init() && !sem.core.is_mut {
        return Err(syn::Error::new_spanned(span, "`init(...)` requires `mut`"));
    }

    // dup requires /// CHECK: doc comment
    if sem.core.dup {
        let has_doc = sem
            .core
            .field
            .attrs
            .iter()
            .any(|a| a.path().is_ident("doc"));
        if !has_doc {
            return Err(syn::Error::new_spanned(
                span,
                "#[account(dup)] requires a /// CHECK: <reason> doc comment",
            ));
        }
    }

    // Optional accounts cannot have init or op groups
    if sem.core.optional {
        if sem.has_init() {
            return Err(syn::Error::new_spanned(
                span,
                "init(...) cannot be used on Option<T> fields",
            ));
        }
        if sem.realloc.is_some() {
            return Err(syn::Error::new_spanned(
                span,
                "`realloc = ...` cannot be used on Option<T> fields",
            ));
        }
        if !sem.groups.is_empty() {
            return Err(syn::Error::new_spanned(
                span,
                "op groups cannot be used on Option<T> fields (only has_one/address/constraints)",
            ));
        }
    }

    if sem.realloc.is_some() {
        if !sem.core.is_mut {
            return Err(syn::Error::new_spanned(
                span,
                "`realloc = ...` requires `mut`",
            ));
        }
        if sem.payer.is_none() {
            return Err(syn::Error::new_spanned(
                span,
                "`realloc = ...` requires `payer = ...` on the same field",
            ));
        }
    }

    // init(idempotent) requires a validation group or address constraint
    if let Some(init) = &sem.init {
        if init.idempotent {
            let has_validation = !sem.groups.is_empty();
            let has_address = sem.address.is_some();
            if !has_validation && !has_address {
                return Err(syn::Error::new_spanned(
                    span,
                    "`init(idempotent)` requires a validation group (e.g., token(...)) or address \
                     constraint",
                ));
            }
        }
    }

    Ok(())
}

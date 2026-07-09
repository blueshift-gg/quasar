//! Structural validation: invariants only, no protocol knowledge.
//!
//! Protocol-specific validation (required args, arg types, exit ordering)
//! is owned by behavior modules via builder errors and trait bounds.

use {
    super::{BehaviorArgValue, FieldSemantics},
    std::collections::HashSet,
    syn::Ident,
};

pub(super) fn validate_semantics(semantics: &[FieldSemantics]) -> syn::Result<()> {
    let field_names: HashSet<String> = semantics
        .iter()
        .map(|sem| sem.core.ident.to_string())
        .collect();
    for sem in semantics {
        validate_field(sem)?;
        validate_behavior_field_refs(sem, &field_names)?;
    }
    Ok(())
}

fn validate_field(sem: &FieldSemantics) -> syn::Result<()> {
    let span = &sem.core.field;

    // --- Migration exclusivity rules ---
    if sem.is_migration {
        if sem.core.optional {
            return Err(syn::Error::new_spanned(
                span,
                "`Option<Migration<...>>` is not supported: migration fields cannot be optional",
            ));
        }
        if sem.has_init() {
            return Err(syn::Error::new_spanned(
                span,
                "`init` cannot be used with `Migration<From, To>`",
            ));
        }
        if sem.realloc.is_some() {
            return Err(syn::Error::new_spanned(
                span,
                "`realloc` cannot be used with `Migration<From, To>`",
            ));
        }
        if !sem.groups.is_empty() {
            return Err(syn::Error::new_spanned(
                span,
                "behavior groups cannot be used with `Migration<From, To>`",
            ));
        }
    }

    // --- Deferred init exclusivity rules ---
    if sem.is_uninit {
        if sem.core.optional {
            return Err(syn::Error::new_spanned(
                span,
                "`Option<Uninit<...>>` is not supported: deferred init fields cannot be optional",
            ));
        }
        if sem.has_init() {
            return Err(syn::Error::new_spanned(
                span,
                "`init` cannot be used with `Uninit<T>`; call `.init(...)` in the handler",
            ));
        }
        if sem.realloc.is_some() {
            return Err(syn::Error::new_spanned(
                span,
                "`realloc` cannot be used with `Uninit<T>`",
            ));
        }
        if sem.close_dest.is_some() {
            return Err(syn::Error::new_spanned(
                span,
                "`close` cannot be used with `Uninit<T>`",
            ));
        }
        if !sem.groups.is_empty() {
            return Err(syn::Error::new_spanned(
                span,
                "behavior groups cannot be used with `Uninit<T>`; pass init params to `.init(...)`",
            ));
        }
    }

    // init requires mut
    if sem.has_init() && !sem.core.is_mut {
        return Err(syn::Error::new_spanned(span, "`init(...)` requires `mut`"));
    }

    // init + realloc mutual exclusion
    if sem.has_init() && sem.realloc.is_some() {
        return Err(syn::Error::new_spanned(
            span,
            "`realloc = ...` cannot be used with `init`",
        ));
    }

    // dup + mutation ops blocked (init, realloc, close, mut behavior groups)
    if sem.core.dup {
        if sem.has_init() {
            return Err(syn::Error::new_spanned(
                span,
                "`dup` cannot be used with `init`: mutation on aliased accounts is unsound",
            ));
        }
        if sem.realloc.is_some() {
            return Err(syn::Error::new_spanned(
                span,
                "`dup` cannot be used with `realloc`: mutation on aliased accounts is unsound",
            ));
        }
        if sem.close_dest.is_some() {
            return Err(syn::Error::new_spanned(
                span,
                "`dup` cannot be used with `close`: mutation on aliased accounts is unsound",
            ));
        }
        if sem.core.is_mut && !sem.groups.is_empty() {
            return Err(syn::Error::new_spanned(
                span,
                "`dup` with `mut` cannot have behavior groups: mutation on aliased accounts is \
                 unsound",
            ));
        }
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

    if sem.core.optional && sem.has_init() {
        return Err(syn::Error::new_spanned(
            span,
            "init(...) cannot be used on Option<T> fields",
        ));
    }

    // Optional realloc not supported
    if sem.core.optional && sem.realloc.is_some() {
        return Err(syn::Error::new_spanned(
            span,
            "`realloc = ...` cannot be used on Option<T> fields",
        ));
    }

    // realloc requires mut
    if sem.realloc.is_some() && !sem.core.is_mut {
        return Err(syn::Error::new_spanned(
            span,
            "`realloc = ...` requires `mut`",
        ));
    }

    // init(idempotent) requires a behavior group or address constraint
    if let Some(init) = &sem.init {
        if init.idempotent {
            let has_behavior = !sem.groups.is_empty();
            let has_address = sem.address.is_some();
            if !has_behavior && !has_address {
                return Err(syn::Error::new_spanned(
                    span,
                    "`init(idempotent)` requires a behavior group (e.g., token(...)) or address \
                     constraint",
                ));
            }
        }
    }

    Ok(())
}

/// Validate behavior arg field references: every `FieldRef` (bare lowercase
/// ident), at any nesting depth inside `Some(...)`, must name a real field.
/// The recursion closes the old one-level `Some(Some(typo))` hole.
fn validate_behavior_field_refs(
    sem: &FieldSemantics,
    field_names: &HashSet<String>,
) -> syn::Result<()> {
    for group in &sem.groups {
        for arg in &group.args {
            check_arg_value(&arg.value, &arg.key, field_names)?;
        }
    }
    Ok(())
}

/// Recursively reject `FieldRef` idents that aren't field names.
fn check_arg_value(
    value: &BehaviorArgValue,
    key: &Ident,
    field_names: &HashSet<String>,
) -> syn::Result<()> {
    match value {
        BehaviorArgValue::FieldRef(ident) => {
            let name = ident.to_string();
            if !field_names.contains(&name) {
                return Err(syn::Error::new_spanned(
                    ident,
                    format!("`{key} = {name}`: no field `{name}` in this accounts struct"),
                ));
            }
            Ok(())
        }
        BehaviorArgValue::Some(inner) => check_arg_value(inner, key, field_names),
        BehaviorArgValue::None | BehaviorArgValue::Expr(_) => Ok(()),
    }
}

//! Structural validation: invariants only, no protocol knowledge.
//!
//! Protocol-specific validation (required args, arg types, exit ordering)
//! is owned by behavior modules via builder errors and trait bounds.

use {super::model::FieldSemantics, std::collections::HashSet, syn::Expr};

pub fn validate_semantics(semantics: &[FieldSemantics]) -> syn::Result<()> {
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

    if sem.has_init() && !sem.core.is_mut {
        return Err(syn::Error::new_spanned(span, "`init(...)` requires `mut`"));
    }

    if sem.has_init() && sem.realloc.is_some() {
        return Err(syn::Error::new_spanned(
            span,
            "`realloc = ...` cannot be used with `init`",
        ));
    }

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

    if sem.core.optional && sem.realloc.is_some() {
        return Err(syn::Error::new_spanned(
            span,
            "`realloc = ...` cannot be used on Option<T> fields",
        ));
    }

    if sem.realloc.is_some() && !sem.core.is_mut {
        return Err(syn::Error::new_spanned(
            span,
            "`realloc = ...` requires `mut`",
        ));
    }

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

/// Validate behavior arg values: reject single-segment lowercase identifiers
/// that don't match any field name (likely typos or instruction args).
fn validate_behavior_field_refs(
    sem: &FieldSemantics,
    field_names: &HashSet<String>,
) -> syn::Result<()> {
    for group in &sem.groups {
        for arg in &group.args {
            validate_single_arg(&arg.value, &arg.key, field_names)?;
            if let Expr::Call(call) = &arg.value {
                if let Expr::Path(p) = &*call.func {
                    if p.path.segments.len() == 1
                        && p.path.segments[0].ident == "Some"
                        && call.args.len() == 1
                    {
                        validate_single_arg(&call.args[0], &arg.key, field_names)?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn validate_single_arg(
    expr: &Expr,
    key: &syn::Ident,
    field_names: &HashSet<String>,
) -> syn::Result<()> {
    if let Expr::Path(ep) = expr {
        if ep.qself.is_none() && ep.path.segments.len() == 1 {
            let name = ep.path.segments[0].ident.to_string();
            if name == "None" || name == "true" || name == "false" {
                return Ok(());
            }
            if name.starts_with(|c: char| c.is_uppercase()) {
                return Ok(());
            }
            if !field_names.contains(name.as_str()) {
                return Err(syn::Error::new_spanned(
                    expr,
                    format!("`{key} = {name}`: no field `{name}` in this accounts struct"),
                ));
            }
        }
    }
    Ok(())
}

//! Lowering: parsed directives to field semantics.

use {
    super::{
        model::{FieldCore, FieldKind, FieldSemantics},
        rules::validate_semantics,
    },
    quasar_syntax::{
        accounts::{
            parse_field_attrs, validate_behavior_arg, CoreDirective, Directive, InitDirective,
            UserCheck,
        },
        types::{extract_generic_inner_type, is_composite_type},
    },
    syn::Type,
};

pub fn lower_semantics(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
) -> syn::Result<Vec<FieldSemantics>> {
    let parsed: Vec<(syn::Field, Vec<Directive>)> = fields
        .iter()
        .map(|field| Ok((field.clone(), parse_field_attrs(field)?)))
        .collect::<syn::Result<_>>()?;

    let cores: Vec<FieldCore> = parsed
        .iter()
        .map(|(field, directives)| lower_core(field, directives))
        .collect();

    let semantics: Vec<FieldSemantics> = parsed
        .into_iter()
        .zip(cores)
        .map(|((_, directives), core)| {
            let is_migration = detect_wrapper(&core.effective_ty, "Migration");
            let is_uninit = detect_wrapper(&core.effective_ty, "Uninit");
            let mut sem = FieldSemantics {
                core,
                init: None,
                payer: None,
                address: None,
                realloc: None,
                close_dest: None,
                groups: Vec::new(),
                user_checks: Vec::new(),
                is_migration,
                is_uninit,
            };
            if sem.is_migration || sem.is_uninit {
                sem.core.is_mut = true;
            }
            lower_directives(&mut sem, directives)?;
            Ok(sem)
        })
        .collect::<syn::Result<_>>()?;

    validate_semantics(&semantics)?;

    Ok(semantics)
}

fn lower_core(field: &syn::Field, directives: &[Directive]) -> FieldCore {
    let ty = &field.ty;
    let option_inner = extract_generic_inner_type(ty, "Option").cloned();
    let optional = option_inner.is_some();
    let after_option = option_inner.unwrap_or_else(|| ty.clone());

    let effective_ty = match &after_option {
        Type::Reference(r) => (*r.elem).clone(),
        other => other.clone(),
    };

    let kind = classify_kind(
        ty,
        directives
            .iter()
            .any(|d| matches!(d, Directive::Core(CoreDirective::Group))),
    );

    let inner_ty = extract_inner_ty(&effective_ty);
    let dynamic = detect_dynamic(&effective_ty, inner_ty.as_ref());

    FieldCore {
        ident: field
            .ident
            .clone()
            .expect("account field must have an identifier"),
        field: field.clone(),
        effective_ty,
        kind,
        inner_ty,
        optional,
        dynamic,
        is_mut: directives
            .iter()
            .any(|d| matches!(d, Directive::Core(CoreDirective::Mut))),
        dup: directives
            .iter()
            .any(|d| matches!(d, Directive::Core(CoreDirective::Dup))),
    }
}

fn classify_kind(raw_ty: &Type, explicit_group: bool) -> FieldKind {
    if explicit_group || is_composite_type(raw_ty) {
        FieldKind::Composite
    } else {
        FieldKind::Single
    }
}

fn lower_directives(sem: &mut FieldSemantics, directives: Vec<Directive>) -> syn::Result<()> {
    let mut groups = Vec::new();

    for directive in directives {
        match directive {
            Directive::Core(core) => match core {
                CoreDirective::Mut | CoreDirective::Dup | CoreDirective::Group => {
                    /* handled in lower_core */
                }
                CoreDirective::Init { idempotent } => {
                    if sem.init.is_some() {
                        return Err(syn::Error::new_spanned(
                            &sem.core.field,
                            "duplicate `init` directive",
                        ));
                    }
                    sem.init = Some(InitDirective { idempotent });
                    sem.core.is_mut = true;
                }
                CoreDirective::Payer(ident) => {
                    if sem.payer.is_some() {
                        return Err(syn::Error::new_spanned(
                            &ident,
                            "duplicate `payer = ...` directive",
                        ));
                    }
                    sem.payer = Some(ident);
                }
                CoreDirective::Address(expr, error) => {
                    let has_address_check = sem
                        .user_checks
                        .iter()
                        .any(|check| matches!(check, UserCheck::Address { .. }));
                    if sem.address.is_some() || has_address_check {
                        return Err(syn::Error::new_spanned(
                            &expr,
                            "duplicate `address = ...` directive",
                        ));
                    }
                    if error.is_some() {
                        sem.user_checks.push(UserCheck::Address { expr, error });
                    } else {
                        sem.address = Some(expr);
                    }
                }
                CoreDirective::Realloc(expr) => {
                    if sem.realloc.is_some() {
                        return Err(syn::Error::new_spanned(
                            &expr,
                            "duplicate `realloc = ...` directive",
                        ));
                    }
                    sem.realloc = Some(expr);
                }
                CoreDirective::Close(dest) => {
                    if sem.close_dest.is_some() {
                        return Err(syn::Error::new_spanned(
                            &dest,
                            "duplicate `close(...)` directive",
                        ));
                    }
                    sem.close_dest = Some(dest);
                    sem.core.is_mut = true;
                }
            },
            Directive::Behavior(group) => {
                groups.push(group);
            }
            Directive::Check(check) => {
                sem.user_checks.push(check);
            }
        }
    }

    for group in &groups {
        for arg in &group.args {
            validate_behavior_arg(&arg.key, &arg.value)?;
        }
    }

    sem.groups = groups;

    Ok(())
}

fn extract_inner_ty(effective_ty: &Type) -> Option<Type> {
    for wrapper in &[
        "Account",
        "InterfaceAccount",
        "Migration",
        "Uninit",
        "Program",
        "Interface",
        "Sysvar",
    ] {
        if let Some(inner) = extract_generic_inner_type(effective_ty, wrapper) {
            return Some(inner.clone());
        }
    }
    None
}

fn detect_dynamic(effective_ty: &Type, inner_ty: Option<&Type>) -> bool {
    if extract_generic_inner_type(effective_ty, "Account").is_none() {
        return false;
    }
    let Some(inner) = inner_ty else { return false };
    if let Type::Path(tp) = inner {
        if let Some(last) = tp.path.segments.last() {
            if let syn::PathArguments::AngleBracketed(args) = &last.arguments {
                return args
                    .args
                    .iter()
                    .any(|arg| matches!(arg, syn::GenericArgument::Lifetime(_)));
            }
        }
    }
    false
}

/// Syntactic detection: last path segment matches `wrapper`.
fn detect_wrapper(ty: &Type, wrapper: &str) -> bool {
    match ty {
        Type::Path(tp) => tp
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == wrapper),
        _ => false,
    }
}

//! Lowering — parsed directives → FieldSemantics.
//! No inference, no classification, no validation — that's rules + planner.

use {
    super::{
        super::syntax::{
            attrs::{validate_behavior_arg, CoreDirective, Directive},
            parse_field_attrs,
        },
        rules::validate_semantics,
        FieldCore, FieldKind, FieldSemantics, InitDirective,
    },
    crate::helpers::{extract_generic_inner_type, is_composite_type},
    syn::Type,
};

pub(super) fn lower_semantics(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    _instruction_args: &Option<Vec<crate::accounts::InstructionArg>>,
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
    let optional = extract_generic_inner_type(ty, "Option").is_some();
    let after_option = extract_generic_inner_type(ty, "Option")
        .cloned()
        .unwrap_or_else(|| ty.clone());

    let effective_ty = match &after_option {
        Type::Reference(r) => (*r.elem).clone(),
        other => other.clone(),
    };

    let kind = classify_kind(ty);

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

fn classify_kind(raw_ty: &Type) -> FieldKind {
    if is_composite_type(raw_ty) {
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
                CoreDirective::Mut | CoreDirective::Dup => { /* handled in lower_core */ }
                CoreDirective::Init { idempotent } => {
                    sem.init = Some(InitDirective { idempotent });
                    sem.core.is_mut = true;
                }
                CoreDirective::Payer(ident) => {
                    sem.payer = Some(ident);
                }
                CoreDirective::Address(expr, error) => {
                    if error.is_some() {
                        sem.user_checks
                            .push(super::UserCheck::Address { expr, error });
                    } else {
                        sem.address = Some(expr);
                    }
                }
                CoreDirective::Realloc(expr) => {
                    sem.realloc = Some(expr);
                }
                CoreDirective::Close(dest) => {
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

    // Validate behavior arg grammar on behavior groups.
    for group in &groups {
        for arg in &group.args {
            validate_behavior_arg(&arg.key, &arg.value)?;
        }
    }

    sem.groups = groups;

    Ok(())
}

// --- Type classification helpers ---

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

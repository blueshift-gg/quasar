//! Planner: phase scheduling only.
//!
//! Reads FieldSemantics, produces phase-ordered BehaviorCall candidates.
//! No validation, no protocol knowledge. The planner should be boring.

use {
    super::{
        model::{BehaviorGroup, FieldKind, FieldSemantics, ValueKind},
        specs::*,
    },
    syn::{Expr, Ident, Type},
};

/// Build a typed execution plan from lowered field semantics.
pub(crate) fn build_plan(semantics: &[FieldSemantics]) -> syn::Result<AccountsPlanTyped> {
    let field_names: Vec<String> = semantics
        .iter()
        .map(|sem| sem.core.ident.to_string())
        .collect();
    let optional_fields: Vec<String> = semantics
        .iter()
        .filter(|sem| sem.core.optional)
        .map(|sem| sem.core.ident.to_string())
        .collect();

    let payer_field = find_payer_field(semantics);

    let fields: Vec<FieldPlan> = semantics
        .iter()
        .map(|sem| plan_field(sem, payer_field.as_ref(), &field_names, &optional_fields))
        .collect::<syn::Result<_>>()?;

    let rent = compute_rent_plan(semantics);

    Ok(AccountsPlanTyped { fields, rent })
}

/// Classify a behavior arg value into a ValueKind based on field names.
fn classify_value(expr: &Expr, field_names: &[String], optional_fields: &[String]) -> ValueKind {
    match expr {
        Expr::Path(ep)
            if ep.qself.is_none()
                && ep.path.segments.len() == 1
                && ep.path.segments[0].ident == "None" =>
        {
            ValueKind::NoneLiteral
        }
        Expr::Call(call)
            if matches!(&*call.func, Expr::Path(p)
                if p.path.segments.len() == 1 && p.path.segments[0].ident == "Some")
                && call.args.len() == 1 =>
        {
            let inner = &call.args[0];
            if let Some(name) = expr_as_ident(inner).map(|id| id.to_string()) {
                if field_names.contains(&name) {
                    return ValueKind::SomeFieldRef;
                }
            }
            ValueKind::SomeExpr
        }
        Expr::Path(ep) if ep.qself.is_none() && ep.path.segments.len() == 1 => {
            let name = ep.path.segments[0].ident.to_string();
            if field_names.contains(&name) {
                if optional_fields.contains(&name) {
                    ValueKind::OptionalFieldRef
                } else {
                    ValueKind::BareFieldRef
                }
            } else {
                ValueKind::Expr
            }
        }
        _ => ValueKind::Expr,
    }
}

fn plan_field(
    sem: &FieldSemantics,
    payer_field: Option<&Ident>,
    field_names: &[String],
    optional_fields: &[String],
) -> syn::Result<FieldPlan> {
    let mut pre_load = Vec::new();
    let mut post_load = Vec::new();
    let mut epilogue = Vec::new();

    let resolved_payer = resolve_field_payer(sem, payer_field);

    if sem.has_init() {
        if let Some(addr) = &sem.address {
            pre_load.push(PreLoadStep::VerifyAddress(AddressSpec {
                expr: addr.expr.clone(),
                error: addr.error.clone(),
            }));
        }
    }

    if let Some(init) = &sem.init {
        let Some(payer) = resolved_payer.as_ref() else {
            return Err(syn::Error::new_spanned(
                &sem.core.field,
                "init requires `payer = ...` (or add a field named `payer`)",
            ));
        };
        let init_plan = plan_init(sem, init.idempotent, payer, field_names, optional_fields);
        pre_load.push(PreLoadStep::Init(init_plan));
    }

    // Post-load: behavior phase candidates. Each group gets the phases
    // appropriate for this field's lifecycle. The emitter guards each call
    // behind its associated const.
    for group in &sem.groups {
        if sem.has_init() {
            post_load.push(PostLoadStep::Behavior(lower_behavior_call(
                group,
                BehaviorPhase::AfterInit,
                field_names,
                optional_fields,
            )));
        }
        post_load.push(PostLoadStep::Behavior(lower_behavior_call(
            group,
            BehaviorPhase::Check,
            field_names,
            optional_fields,
        )));
    }

    // Post-load: realloc.
    if let Some(realloc_expr) = &sem.realloc {
        let payer = match resolved_payer.as_ref() {
            Some(p) => p,
            None => {
                return Err(syn::Error::new_spanned(
                    &sem.core.field,
                    "`realloc = ...` requires `payer = ...` (or add a field named `payer`)",
                ));
            }
        };
        post_load.push(PostLoadStep::Realloc(ReallocSpec {
            new_space: realloc_expr.clone(),
            payer: payer.clone(),
        }));
    }

    // Post-load: address verification for non-init fields.
    if !sem.has_init() {
        if let Some(addr) = &sem.address {
            post_load.push(PostLoadStep::VerifyExistingAddress(AddressSpec {
                expr: addr.expr.clone(),
                error: addr.error.clone(),
            }));
        }
    }

    // Post-load + epilogue: update and exit candidates (mut fields only).
    if sem.core.is_mut {
        for group in &sem.groups {
            post_load.push(PostLoadStep::Behavior(lower_behavior_call(
                group,
                BehaviorPhase::Update,
                field_names,
                optional_fields,
            )));
            epilogue.push(EpilogueStep::Behavior(lower_behavior_call(
                group,
                BehaviorPhase::Exit,
                field_names,
                optional_fields,
            )));
        }
    }

    // Epilogue: core program close (lamport drain).
    if let Some(dest) = &sem.close_dest {
        epilogue.push(EpilogueStep::ProgramClose(ProgramCloseSpec {
            destination_field: dest.clone(),
        }));
    }

    Ok(FieldPlan {
        pre_load,
        post_load,
        epilogue,
    })
}

fn plan_init(
    sem: &FieldSemantics,
    idempotent: bool,
    payer: &FieldRef,
    field_names: &[String],
    optional_fields: &[String],
) -> InitPlan {
    // If there are behavior groups attached, this is a delegated init.
    if sem.groups.is_empty() {
        return InitPlan::Program(ProgramInitSpec {
            payer: payer.clone(),
            space_ty: sem.core.effective_ty.clone(),
            idempotent,
        });
    }

    // Delegated init: behavior groups contribute init params via
    // set_init_param. After_init and check run as post-load steps (planned
    // separately in plan_field).
    let mut init_param_calls = Vec::new();
    for group in &sem.groups {
        init_param_calls.push(lower_behavior_call(
            group,
            BehaviorPhase::SetInitParam,
            field_names,
            optional_fields,
        ));
    }

    InitPlan::Behavior(BehaviorInitSpec {
        payer: payer.clone(),
        idempotent,
        init_param_calls,
    })
}

/// Lower a BehaviorGroup directive into a BehaviorCall with classified values.
fn lower_behavior_call(
    group: &BehaviorGroup,
    phase: BehaviorPhase,
    field_names: &[String],
    optional_fields: &[String],
) -> BehaviorCall {
    let args = group
        .args
        .iter()
        .map(|arg| {
            let kind = classify_value(&arg.value, field_names, optional_fields);
            LoweredArg {
                key: arg.key.clone(),
                lowered: lower_value(&arg.value, kind),
            }
        })
        .collect();

    BehaviorCall {
        path: group.path.clone(),
        args,
        phase,
    }
}

/// Convert a classified value into a LoweredValue.
fn lower_value(expr: &Expr, kind: ValueKind) -> LoweredValue {
    match kind {
        ValueKind::BareFieldRef => {
            let ident =
                expr_as_ident(expr).expect("BareFieldRef is only assigned to bare identifiers");
            LoweredValue::FieldView(ident)
        }
        ValueKind::OptionalFieldRef => {
            let ident =
                expr_as_ident(expr).expect("OptionalFieldRef is only assigned to bare identifiers");
            LoweredValue::OptionalFieldView(ident)
        }
        ValueKind::Expr => LoweredValue::Expr(expr.clone()),
        ValueKind::NoneLiteral => LoweredValue::NoneLiteral,
        ValueKind::SomeFieldRef => match expr {
            Expr::Call(call) => LoweredValue::SomeFieldView(
                expr_as_ident(&call.args[0])
                    .expect("SomeFieldRef is only assigned to Some(field_ident)"),
            ),
            _ => LoweredValue::Expr(expr.clone()),
        },
        ValueKind::SomeExpr => match expr {
            Expr::Call(call) => LoweredValue::SomeExpr(call.args[0].clone()),
            _ => LoweredValue::Expr(expr.clone()),
        },
    }
}

/// Find the struct-wide payer field (by name convention).
fn find_payer_field(semantics: &[FieldSemantics]) -> Option<Ident> {
    semantics
        .iter()
        .find(|sem| sem.core.ident == "payer" && sem.core.kind == FieldKind::Single)
        .map(|sem| sem.core.ident.clone())
}

/// Resolve payer for a specific field: explicit > inferred by name.
fn resolve_field_payer(sem: &FieldSemantics, payer_field: Option<&Ident>) -> Option<FieldRef> {
    if let Some(explicit_payer) = &sem.payer {
        return Some(FieldRef {
            ident: explicit_payer.clone(),
        });
    }

    let needs_payer = sem.init.is_some() || sem.realloc.is_some();
    if needs_payer {
        if let Some(payer_ident) = payer_field {
            return Some(FieldRef {
                ident: payer_ident.clone(),
            });
        }
    }

    None
}

fn compute_rent_plan(semantics: &[FieldSemantics]) -> RentPlan {
    let needs_rent = semantics
        .iter()
        .any(|sem| sem.init.is_some() || sem.realloc.is_some());

    if !needs_rent {
        return RentPlan::NotNeeded;
    }

    for sem in semantics {
        if sem.core.optional {
            continue;
        }
        if let Type::Path(tp) = &sem.core.effective_ty {
            if let Some(last) = tp.path.segments.last() {
                if last.ident == "Sysvar" {
                    if let syn::PathArguments::AngleBracketed(args) = &last.arguments {
                        for arg in &args.args {
                            if let syn::GenericArgument::Type(Type::Path(inner)) = arg {
                                if inner
                                    .path
                                    .segments
                                    .last()
                                    .is_some_and(|s| s.ident == "Rent")
                                {
                                    return RentPlan::FromSysvarField {
                                        field: sem.core.ident.clone(),
                                    };
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    RentPlan::FetchOnce
}

fn expr_as_ident(expr: &Expr) -> Option<Ident> {
    if let Expr::Path(ep) = expr {
        if ep.qself.is_none() && ep.path.segments.len() == 1 {
            return Some(ep.path.segments[0].ident.clone());
        }
    }
    None
}

//! Planner — phase scheduling only.
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

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Build a typed execution plan from lowered field semantics.
pub(crate) fn build_plan(semantics: &[FieldSemantics]) -> syn::Result<AccountsPlanTyped> {
    // Collect field names and optional field names for value classification.
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

// ---------------------------------------------------------------------------
// Value classification
// ---------------------------------------------------------------------------

/// Classify a behavior arg value into a ValueKind based on field names.
fn classify_value(expr: &Expr, field_names: &[String], optional_fields: &[String]) -> ValueKind {
    match expr {
        // None literal
        Expr::Path(ep)
            if ep.qself.is_none()
                && ep.path.segments.len() == 1
                && ep.path.segments[0].ident == "None" =>
        {
            ValueKind::NoneLiteral
        }
        // Some(inner)
        Expr::Call(call)
            if matches!(&*call.func, Expr::Path(p)
                if p.path.segments.len() == 1 && p.path.segments[0].ident == "Some")
                && call.args.len() == 1 =>
        {
            let inner = &call.args[0];
            if let Some(name) = expr_as_ident_str(inner) {
                if field_names.contains(&name) {
                    return ValueKind::SomeFieldRef;
                }
            }
            ValueKind::SomeExpr
        }
        // Bare ident — check if it's a field
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
        // Everything else is an expression
        _ => ValueKind::Expr,
    }
}

// ---------------------------------------------------------------------------
// Per-field planning
// ---------------------------------------------------------------------------

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

    // Pre-load: address verification for init fields.
    if sem.has_init() {
        if let Some(addr_expr) = &sem.address {
            pre_load.push(PreLoadStep::VerifyAddress(AddressSpec {
                expr: addr_expr.clone(),
            }));
        }
    }

    // Pre-load: init plan.
    if let Some(init) = &sem.init {
        if resolved_payer.is_none() {
            return Err(syn::Error::new_spanned(
                &sem.core.field,
                "init requires `payer = ...` (or add a field named `payer`)",
            ));
        }
        let init_plan = plan_init(
            sem,
            init.idempotent,
            &resolved_payer,
            field_names,
            optional_fields,
        )?;
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
        if let Some(addr_expr) = &sem.address {
            post_load.push(PostLoadStep::VerifyExistingAddress(AddressSpec {
                expr: addr_expr.clone(),
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

// ---------------------------------------------------------------------------
// Init planning
// ---------------------------------------------------------------------------

fn plan_init(
    sem: &FieldSemantics,
    idempotent: bool,
    resolved_payer: &Option<FieldRef>,
    field_names: &[String],
    optional_fields: &[String],
) -> syn::Result<InitPlan> {
    let payer = resolved_payer
        .as_ref()
        .ok_or_else(|| {
            syn::Error::new_spanned(
                &sem.core.field,
                "init requires `payer = ...` (or add a field named `payer`)",
            )
        })?
        .clone();

    // If there are behavior groups attached, this is a delegated init.
    if sem.groups.is_empty() {
        return Ok(InitPlan::Program(ProgramInitSpec {
            payer,
            space: SpaceSpec::FromType(sem.core.effective_ty.clone()),
            idempotent,
        }));
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

    Ok(InitPlan::Behavior(BehaviorInitSpec {
        payer,
        idempotent,
        init_param_calls,
    }))
}

// ---------------------------------------------------------------------------
// Behavior call lowering
// ---------------------------------------------------------------------------

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
            let ident = expr_as_ident(expr).unwrap();
            LoweredValue::FieldView(ident)
        }
        ValueKind::OptionalFieldRef => {
            let ident = expr_as_ident(expr).unwrap();
            LoweredValue::OptionalFieldView(ident)
        }
        ValueKind::Expr => LoweredValue::Expr(expr.clone()),
        ValueKind::NoneLiteral => LoweredValue::NoneLiteral,
        ValueKind::SomeFieldRef => match expr {
            Expr::Call(call) => LoweredValue::SomeFieldView(expr_as_ident(&call.args[0]).unwrap()),
            _ => LoweredValue::Expr(expr.clone()),
        },
        ValueKind::SomeExpr => match expr {
            Expr::Call(call) => LoweredValue::SomeExpr(call.args[0].clone()),
            _ => LoweredValue::Expr(expr.clone()),
        },
    }
}

// ---------------------------------------------------------------------------
// Payer resolution
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Rent planning
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn expr_as_ident(expr: &Expr) -> Option<Ident> {
    if let Expr::Path(ep) = expr {
        if ep.qself.is_none() && ep.path.segments.len() == 1 {
            return Some(ep.path.segments[0].ident.clone());
        }
    }
    None
}

fn expr_as_ident_str(expr: &Expr) -> Option<String> {
    expr_as_ident(expr).map(|id| id.to_string())
}

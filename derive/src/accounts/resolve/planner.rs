//! Planner: phase scheduling only.
//!
//! Reads FieldSemantics, produces phase-ordered BehaviorCall candidates.
//! No validation, no protocol knowledge. The planner should be boring.

use {
    super::{
        model::{account_meta_flags, BehaviorArgValue, BehaviorGroup, FieldKind, FieldSemantics},
        reserved::PAYER_FIELD,
        specs::*,
        wrapper::{sysvar_inner, WrapperKind},
    },
    syn::{Ident, Type},
};

/// Build a typed execution plan from lowered field semantics.
pub(crate) fn build_plan(semantics: &[FieldSemantics]) -> syn::Result<AccountsPlanTyped> {
    let optional_fields: Vec<String> = semantics
        .iter()
        .filter(|sem| sem.core.optional)
        .map(|sem| sem.core.ident.to_string())
        .collect();

    let payer_field = find_payer_field(semantics);

    let fields: Vec<FieldPlan> = semantics
        .iter()
        .map(|sem| plan_field(sem, payer_field.as_ref(), semantics, &optional_fields))
        .collect::<syn::Result<_>>()?;

    let rent = compute_rent_plan(semantics);

    Ok(AccountsPlanTyped { fields, rent })
}

fn plan_field(
    sem: &FieldSemantics,
    payer_field: Option<&Ident>,
    semantics: &[FieldSemantics],
    optional_fields: &[String],
) -> syn::Result<FieldPlan> {
    let mut pre_load = Vec::new();
    let mut post_load = Vec::new();
    let mut epilogue = Vec::new();

    let resolved_payer = resolve_field_payer(sem, payer_field);

    // 019-D: a resolved payer must be writable (it funds rent) and a signer;
    // a `close` destination must be writable (it receives drained lamports).
    if sem.init.is_some() || sem.realloc.is_some() {
        if let Some(payer) = resolved_payer.as_ref() {
            validate_payer_field(payer, semantics)?;
        }
    }
    if let Some(dest) = &sem.close_dest {
        validate_close_dest(dest, semantics)?;
    }

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
        let init_plan = plan_init(sem, init.idempotent, payer, optional_fields);
        pre_load.push(PreLoadStep::Init(init_plan));
    }

    // Post-load: behavior phase candidates. Each group gets the phases
    // appropriate for this field's lifecycle. The emitter guards each call
    // behind its associated const.
    for group in &sem.groups {
        if sem.has_init() {
            post_load.push(PostLoadStep::Behavior {
                phase: PostLoadPhase::AfterInit,
                call: lower_behavior_call(group, optional_fields),
            });
        }
        post_load.push(PostLoadStep::Behavior {
            phase: PostLoadPhase::Check,
            call: lower_behavior_call(group, optional_fields),
        });
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

    // Post-load + epilogue: update and exit candidates (writable fields only).
    if sem.is_writable() {
        for group in &sem.groups {
            post_load.push(PostLoadStep::Behavior {
                phase: PostLoadPhase::Update,
                call: lower_behavior_call(group, optional_fields),
            });
            epilogue.push(EpilogueStep::Behavior(lower_behavior_call(
                group,
                optional_fields,
            )));
        }
    }

    // Post-load: structural user checks (has_one / constraints), emitted after
    // every other post-load step for the field.
    for check in &sem.user_checks {
        post_load.push(PostLoadStep::UserCheck(check.clone()));
    }

    // Epilogue: core program close (lamport drain).
    if let Some(dest) = &sem.close_dest {
        epilogue.push(EpilogueStep::ProgramClose(ProgramCloseSpec {
            destination_field: dest.clone(),
        }));
    }

    let flags = account_meta_flags(sem);
    let load = plan_load(sem);
    let bump = sem.address.is_some().then_some(BumpSlot);
    let behaviors: Vec<BehaviorGroupRef> = sem
        .groups
        .iter()
        .map(|g| BehaviorGroupRef {
            path: g.path.clone(),
            name: g.name(),
        })
        .collect();

    Ok(FieldPlan {
        ident: sem.core.ident.clone(),
        effective_ty: sem.core.effective_ty.clone(),
        wrapper: sem.core.wrapper,
        kind: sem.core.kind,
        optional: sem.core.optional,
        dup: sem.core.dup,
        writable: flags.writable,
        signer: flags.signer,
        load,
        bump,
        behaviors,
        pre_load,
        post_load,
        epilogue,
    })
}

/// Select the load mode for a field. Dynamic wrappers load via
/// `from_account_view`; fixed accounts load via `AccountLoad`, guarded by the
/// behavior groups' `VALIDATES_ACCOUNT_DATA`.
fn plan_load(sem: &FieldSemantics) -> LoadStep {
    if sem.core.dynamic {
        let base_ty = sem
            .core
            .inner_ty
            .clone()
            .unwrap_or_else(|| sem.core.effective_ty.clone());
        LoadStep::Dynamic { base_ty }
    } else {
        LoadStep::Fixed {
            validates_paths: sem.groups.iter().map(|g| g.path.clone()).collect(),
        }
    }
}

fn plan_init(
    sem: &FieldSemantics,
    idempotent: bool,
    payer: &FieldRef,
    optional_fields: &[String],
) -> InitPlan {
    // A preceding `VerifyAddress` step stored `__addr_{f}`/`__bumps_{f}` when
    // the field has an `address`; init signs with those seeds.
    let verified_address = sem.address.as_ref().map(|addr| AddressSpec {
        expr: addr.expr.clone(),
        error: addr.error.clone(),
    });

    // If there are behavior groups attached, this is a delegated init.
    if sem.groups.is_empty() {
        return InitPlan::Program(ProgramInitSpec {
            payer: payer.clone(),
            space_ty: sem.core.effective_ty.clone(),
            idempotent,
            verified_address,
        });
    }

    // Delegated init: behavior groups contribute init params via
    // set_init_param. After_init and check run as post-load steps (planned
    // separately in plan_field).
    let mut init_param_calls = Vec::new();
    for group in &sem.groups {
        init_param_calls.push(lower_behavior_call(group, optional_fields));
    }

    InitPlan::Behavior(BehaviorInitSpec {
        payer: payer.clone(),
        idempotent,
        init_param_calls,
        verified_address,
    })
}

/// Lower a BehaviorGroup directive into a BehaviorCall with lowered values.
/// The lifecycle phase is supplied by the owning step, not stored here.
fn lower_behavior_call(group: &BehaviorGroup, optional_fields: &[String]) -> BehaviorCall {
    let args = group
        .args
        .iter()
        .map(|arg| LoweredArg {
            key: arg.key.clone(),
            lowered: lower_behavior_arg_value(&arg.value, optional_fields),
        })
        .collect();

    BehaviorCall {
        path: group.path.clone(),
        args,
    }
}

/// Lower a parsed behavior-arg value into the emitter's `LoweredValue`. Total
/// match on `BehaviorArgValue`; every `FieldRef` was validated to name a real
/// field by rules, so there is no re-derivation and no fallible `.expect`.
fn lower_behavior_arg_value(value: &BehaviorArgValue, optional_fields: &[String]) -> LoweredValue {
    match value {
        BehaviorArgValue::None => LoweredValue::NoneLiteral,
        BehaviorArgValue::Expr(expr) => LoweredValue::Expr(expr.clone()),
        BehaviorArgValue::FieldRef(ident) => {
            if optional_fields.contains(&ident.to_string()) {
                LoweredValue::OptionalFieldView(ident.clone())
            } else {
                LoweredValue::FieldView(ident.clone())
            }
        }
        BehaviorArgValue::Some(inner) => match inner.as_ref() {
            BehaviorArgValue::FieldRef(ident) => LoweredValue::SomeFieldView(ident.clone()),
            other => LoweredValue::SomeExpr(other.as_expr()),
        },
    }
}

/// Find the struct-wide payer field (by name convention).
fn find_payer_field(semantics: &[FieldSemantics]) -> Option<Ident> {
    semantics
        .iter()
        .find(|sem| sem.core.ident == PAYER_FIELD && sem.core.kind == FieldKind::Single)
        .map(|sem| sem.core.ident.clone())
}

/// 019-D: the resolved payer for an `init`/`realloc` must exist, be writable
/// (it funds rent), and be a signer (it authorizes the create/realloc CPI).
fn validate_payer_field(payer: &FieldRef, semantics: &[FieldSemantics]) -> syn::Result<()> {
    let Some(payer_sem) = semantics.iter().find(|s| s.core.ident == payer.ident) else {
        return Err(syn::Error::new_spanned(
            &payer.ident,
            format!(
                "payer `{}` does not name a field in this accounts struct",
                payer.ident
            ),
        ));
    };
    let flags = account_meta_flags(payer_sem);
    if !flags.writable {
        return Err(syn::Error::new_spanned(
            &payer_sem.core.field,
            format!(
                "payer field `{}` must be writable (`#[account(mut)]`): it funds account rent",
                payer.ident
            ),
        ));
    }
    if !flags.signer {
        return Err(syn::Error::new_spanned(
            &payer_sem.core.field,
            format!("payer field `{}` must be a `Signer`", payer.ident),
        ));
    }
    Ok(())
}

/// 019-D: a `close` destination must exist and be writable (it receives the
/// drained lamports).
fn validate_close_dest(dest: &Ident, semantics: &[FieldSemantics]) -> syn::Result<()> {
    let Some(dest_sem) = semantics.iter().find(|s| s.core.ident == *dest) else {
        return Err(syn::Error::new_spanned(
            dest,
            format!("close destination `{dest}` does not name a field in this accounts struct"),
        ));
    };
    if !dest_sem.is_writable() {
        return Err(syn::Error::new_spanned(
            &dest_sem.core.field,
            format!(
                "close destination `{dest}` must be writable (`#[account(mut)]`): it receives the \
                 drained lamports"
            ),
        ));
    }
    Ok(())
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
        if sem.core.wrapper != WrapperKind::Sysvar {
            continue;
        }
        let is_rent = sysvar_inner(&sem.core.effective_ty).is_some_and(|inner| {
            matches!(inner, Type::Path(inner)
                if inner.path.segments.last().is_some_and(|s| s.ident == "Rent"))
        });
        if is_rent {
            return RentPlan::FromSysvarField {
                field: sem.core.ident.clone(),
            };
        }
    }

    RentPlan::FetchOnce
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::accounts::resolve::model::{FieldCore, InitDirective},
        quote::quote,
        syn::{parse::Parser, Expr},
    };

    fn expr(tokens: proc_macro2::TokenStream) -> Expr {
        syn::parse2(tokens).expect("expr parses")
    }

    fn ident(name: &str) -> Ident {
        syn::parse_str(name).expect("ident")
    }

    /// Lower a fixture struct and build its plan, returning the plan error
    /// text.
    fn plan_err(ts: proc_macro2::TokenStream) -> String {
        let item: syn::ItemStruct = syn::parse2(ts).expect("struct parses");
        let fields = match item.fields {
            syn::Fields::Named(named) => named.named,
            _ => Default::default(),
        };
        let sems = crate::accounts::resolve::lower_semantics(&fields, &[]).expect("fixture lowers");
        build_plan(&sems)
            .err()
            .expect("expected a plan error")
            .to_string()
    }

    #[test]
    fn payer_must_be_writable() {
        let err = plan_err(quote! {
            struct S {
                payer: Signer,
                #[account(init, payer = payer, address = Foo::seeds(payer.address()))]
                acct: Account<Foo>,
            }
        });
        assert!(err.contains("must be writable"), "{err}");
    }

    #[test]
    fn payer_must_be_signer() {
        let err = plan_err(quote! {
            struct S {
                #[account(mut)]
                payer: Account<Foo>,
                #[account(init, payer = payer, address = Bar::seeds(payer.address()))]
                acct: Account<Bar>,
            }
        });
        assert!(err.contains("must be a `Signer`"), "{err}");
    }

    #[test]
    fn close_destination_must_be_writable() {
        let err = plan_err(quote! {
            struct S {
                authority: Signer,
                #[account(mut, close(dest = authority))]
                acct: Account<Foo>,
            }
        });
        assert!(
            err.contains("close destination") && err.contains("must be writable"),
            "{err}"
        );
    }

    /// Minimal single-account semantics with the given name and effective type.
    fn make_sem(name: &str, ty: &str) -> FieldSemantics {
        let ident: Ident = syn::parse_str(name).expect("ident");
        let effective_ty: Type = syn::parse_str(ty).expect("type");
        let field = syn::Field::parse_named
            .parse2(quote!(#ident: #effective_ty))
            .expect("field");
        FieldSemantics {
            core: FieldCore {
                ident,
                field,
                wrapper: crate::accounts::resolve::wrapper::classify_wrapper(&effective_ty),
                effective_ty,
                kind: FieldKind::Single,
                inner_ty: None,
                optional: false,
                dynamic: false,
                declared_mut: false,
                dup: false,
            },
            init: None,
            payer: None,
            address: None,
            realloc: None,
            close_dest: None,
            groups: Vec::new(),
            user_checks: Vec::new(),
            is_migration: false,
            is_uninit: false,
        }
    }

    #[test]
    fn lower_bare_field_ref_uses_optional_flag() {
        let optional = vec!["maybe".to_string()];
        assert!(matches!(
            lower_behavior_arg_value(&BehaviorArgValue::FieldRef(ident("authority")), &optional),
            LoweredValue::FieldView(id) if id == "authority"
        ));
        assert!(matches!(
            lower_behavior_arg_value(&BehaviorArgValue::FieldRef(ident("maybe")), &optional),
            LoweredValue::OptionalFieldView(id) if id == "maybe"
        ));
    }

    #[test]
    fn lower_none_and_expr_passthrough() {
        assert!(matches!(
            lower_behavior_arg_value(&BehaviorArgValue::None, &[]),
            LoweredValue::NoneLiteral
        ));
        assert!(matches!(
            lower_behavior_arg_value(&BehaviorArgValue::Expr(expr(quote!(5u8))), &[]),
            LoweredValue::Expr(_)
        ));
    }

    #[test]
    fn lower_some_field_ref_unwraps_to_view() {
        assert!(matches!(
            lower_behavior_arg_value(
                &BehaviorArgValue::Some(Box::new(BehaviorArgValue::FieldRef(ident("authority")))),
                &[],
            ),
            LoweredValue::SomeFieldView(id) if id == "authority"
        ));
    }

    #[test]
    fn lower_some_non_field_collapses_to_some_expr() {
        // `Some(None)` and `Some(literal)` collapse to `SomeExpr` carrying the
        // reconstructed inner expression.
        assert!(matches!(
            lower_behavior_arg_value(
                &BehaviorArgValue::Some(Box::new(BehaviorArgValue::None)),
                &[],
            ),
            LoweredValue::SomeExpr(_)
        ));
        assert!(matches!(
            lower_behavior_arg_value(
                &BehaviorArgValue::Some(Box::new(BehaviorArgValue::Expr(expr(quote!(42u64))))),
                &[],
            ),
            LoweredValue::SomeExpr(_)
        ));
    }

    #[test]
    fn find_payer_field_by_name_convention() {
        let with_payer = vec![
            make_sem("payer", "Signer"),
            make_sem("config", "Account<C>"),
        ];
        assert_eq!(
            find_payer_field(&with_payer)
                .expect("payer found")
                .to_string(),
            "payer"
        );
        let without = vec![make_sem("authority", "Signer")];
        assert!(find_payer_field(&without).is_none());
    }

    #[test]
    fn rent_plan_not_needed_without_init_or_realloc() {
        let sems = vec![
            make_sem("authority", "Signer"),
            make_sem("config", "Account<C>"),
        ];
        assert!(matches!(compute_rent_plan(&sems), RentPlan::NotNeeded));
    }

    #[test]
    fn rent_plan_fetch_once_when_init_and_no_sysvar() {
        let mut escrow = make_sem("escrow", "Account<E>");
        escrow.init = Some(InitDirective { idempotent: false });
        let sems = vec![make_sem("payer", "Signer"), escrow];
        assert!(matches!(compute_rent_plan(&sems), RentPlan::FetchOnce));
    }

    #[test]
    fn rent_plan_from_sysvar_when_rent_field_present() {
        let mut escrow = make_sem("escrow", "Account<E>");
        escrow.init = Some(InitDirective { idempotent: false });
        let sems = vec![
            make_sem("payer", "Signer"),
            escrow,
            make_sem("rent", "Sysvar<Rent>"),
        ];
        assert!(matches!(
            compute_rent_plan(&sems),
            RentPlan::FromSysvarField { field } if field == "rent"
        ));
    }
}

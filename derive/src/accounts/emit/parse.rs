//! Parse/epilogue body assembly: wires phase snippets into the output.
//!
//! Generated parse body shape:
//!
//! ```text
//! // Rent source (only when init/realloc/migration may need it)
//! let __rent_ctx = OpCtx::new(&program_id, &__rent);
//!
//! // Phase 1: load non-init fields
//! let field_a = <Ty>::load(field_a)?;
//!
//! // Phase 2: address verify + init CPI for init fields (field-ordered)
//! // Phase 3: load init fields (inlined into behavior init sequence)
//!
//! // Phase 4: behavior checks, user checks, realloc
//! <path::Behavior as AccountBehavior<Ty>>::check(&field, &args)?;
//!
//! Ok((Self { field_a, field_b, field_c }, bumps))
//! ```

use {
    super::{
        super::resolve::{
            specs::{
                AccountsPlanTyped, EpilogueStep, FieldPlan, InitPlan, PostLoadStep, PreLoadStep,
                RentPlan,
            },
            FieldKind, FieldSemantics, UserCheck,
        },
        typed_emit,
    },
    crate::helpers::strip_generics,
    quote::{format_ident, quote},
};

pub(crate) fn emit_parse_body(
    semantics: &[FieldSemantics],
    plan: &AccountsPlanTyped,
    cx: &super::EmitCx,
) -> proc_macro2::TokenStream {
    emit_parse_body_inner(semantics, plan, cx, true)
}

pub(crate) fn emit_parse_body_without_behavior_assertions(
    semantics: &[FieldSemantics],
    plan: &AccountsPlanTyped,
    cx: &super::EmitCx,
) -> proc_macro2::TokenStream {
    emit_parse_body_inner(semantics, plan, cx, false)
}

fn emit_parse_body_inner(
    semantics: &[FieldSemantics],
    plan: &AccountsPlanTyped,
    cx: &super::EmitCx,
    include_behavior_assertions: bool,
) -> proc_macro2::TokenStream {
    let parse_sequence = emit_parse_sequence(semantics, plan);
    let bump_vars = emit_bump_vars(semantics);
    let init_state_vars = emit_init_state_vars(&plan.fields, semantics);

    let bump_init = emit_bump_init(semantics, &cx.bumps_name);

    // Behavior const assertions: REQUIRES_MUT and SETS_INIT_PARAMS.
    let behavior_asserts = if include_behavior_assertions {
        emit_behavior_assertions(semantics)
    } else {
        quote! {}
    };

    let construct_fields: Vec<proc_macro2::TokenStream> = semantics
        .iter()
        .map(|sem| {
            let ident = &sem.core.ident;
            quote! { #ident }
        })
        .collect();

    quote! {
        #behavior_asserts
        #bump_vars
        #(#init_state_vars)*
        #parse_sequence
        Ok((Self { #(#construct_fields,)* }, #bump_init))
    }
}

// Rent context.

fn emit_rent_context(rent_plan: &RentPlan) -> proc_macro2::TokenStream {
    match rent_plan {
        RentPlan::NotNeeded => quote! {},
        RentPlan::FromSysvarField { field } => {
            quote! {
                let __rent_ctx = quasar_lang::ops::OpCtx::new(
                    // SAFETY: `__program_id` is already a valid `&Address`;
                    // this reborrow preserves the same address while keeping
                    // generated SBF in its cheaper shape.
                    unsafe { &*(__program_id as *const quasar_lang::prelude::Address) },
                    #field.get(),
                );
            }
        }
        RentPlan::FetchOnce => {
            quote! {
                let __rent_ctx = quasar_lang::ops::OpCtx::new(
                    // SAFETY: `__program_id` is already a valid `&Address`;
                    // this reborrow preserves the same address while keeping
                    // generated SBF in its cheaper shape.
                    unsafe { &*(__program_id as *const quasar_lang::prelude::Address) },
                    quasar_lang::ops::RentResolver::fetch_once(),
                );
            }
        }
    }
}

fn emit_parse_sequence(
    semantics: &[FieldSemantics],
    plan: &AccountsPlanTyped,
) -> proc_macro2::TokenStream {
    let init_phase = emit_init_phase_typed(&plan.fields, semantics);
    let load_init = emit_load_filtered(semantics, true);
    let phase4 = emit_post_load_typed(&plan.fields, semantics);

    match &plan.rent {
        RentPlan::NotNeeded => {
            let load_non_init = emit_load_filtered(semantics, false);
            quote! {
                #(#load_non_init)*
                #(#init_phase)*
                #(#load_init)*
                #(#phase4)*
            }
        }
        RentPlan::FetchOnce => {
            let ctx_init = emit_rent_context(&plan.rent);
            let load_non_init = emit_load_filtered(semantics, false);
            quote! {
                #ctx_init
                #(#load_non_init)*
                #(#init_phase)*
                #(#load_init)*
                #(#phase4)*
            }
        }
        RentPlan::FromSysvarField { field } => {
            // The rent field must be loaded before `__rent_ctx` can borrow it;
            // all other non-init fields keep their normal phase position.
            let rent_load = emit_load_by_ident(semantics, field);
            let ctx_init = emit_rent_context(&plan.rent);
            let load_non_init = emit_load_filtered_excluding(semantics, false, Some(field));
            quote! {
                #rent_load
                #ctx_init
                #(#load_non_init)*
                #(#init_phase)*
                #(#load_init)*
                #(#phase4)*
            }
        }
    }
}

// Init phase from the typed plan.

fn emit_init_phase_typed(
    field_plans: &[super::super::resolve::specs::FieldPlan],
    semantics: &[FieldSemantics],
) -> Vec<proc_macro2::TokenStream> {
    let mut stmts = Vec::new();

    for (fp, sem) in field_plans.iter().zip(semantics.iter()) {
        let ident = &sem.core.ident;
        let ty = &sem.core.effective_ty;

        for step in &fp.pre_load {
            match step {
                PreLoadStep::VerifyAddress(addr_spec) => {
                    let bump_var = format_ident!("__bumps_{}", ident);
                    let addr_var = format_ident!("__addr_{}", ident);
                    let addr_expr = &addr_spec.expr;
                    let term = address_verify_terminator(&addr_spec.error);
                    stmts.push(quote! {
                        let #addr_var = #addr_expr;
                        #bump_var = quasar_lang::address::AddressVerify::verify(
                            &#addr_var, #ident.address(), __program_id,
                        )#term;
                    });
                }
                PreLoadStep::Init(init_plan) => {
                    let has_address = sem.address.is_some();
                    let did_init_var = needs_init_state_var(fp)
                        .then(|| format_ident!("__quasar_did_init_{}", ident));
                    let ts = match init_plan {
                        InitPlan::Program(spec) => {
                            typed_emit::emit_program_init(spec, ident, ty, has_address)
                        }
                        InitPlan::Behavior(spec) => typed_emit::emit_behavior_init(
                            spec,
                            ident,
                            ty,
                            has_address,
                            did_init_var.as_ref(),
                        ),
                    };
                    stmts.push(ts);
                }
            }
        }
    }

    stmts
}

fn emit_init_state_vars(
    field_plans: &[FieldPlan],
    semantics: &[FieldSemantics],
) -> Vec<proc_macro2::TokenStream> {
    field_plans
        .iter()
        .zip(semantics.iter())
        .filter(|(fp, _)| needs_init_state_var(fp))
        .map(|(_, sem)| {
            let ident = &sem.core.ident;
            let did_init_var = format_ident!("__quasar_did_init_{}", ident);
            quote! { let mut #did_init_var = false; }
        })
        .collect()
}

fn needs_init_state_var(field_plan: &FieldPlan) -> bool {
    let has_behavior_init = field_plan
        .pre_load
        .iter()
        .any(|step| matches!(step, PreLoadStep::Init(InitPlan::Behavior(_))));
    let has_behavior_check = field_plan.post_load.iter().any(|step| {
        matches!(
            step,
            PostLoadStep::Behavior {
                phase: super::super::resolve::specs::PostLoadPhase::Check,
                ..
            }
        )
    });

    has_behavior_init && has_behavior_check
}

// Post-load phase from the typed plan.

fn emit_post_load_typed(
    field_plans: &[super::super::resolve::specs::FieldPlan],
    semantics: &[FieldSemantics],
) -> Vec<proc_macro2::TokenStream> {
    let mut stmts = Vec::new();

    for (fp, sem) in field_plans.iter().zip(semantics.iter()) {
        let ident = &sem.core.ident;
        let ty = &sem.core.effective_ty;
        let is_optional = sem.core.optional;
        let did_init_var =
            needs_init_state_var(fp).then(|| format_ident!("__quasar_did_init_{}", ident));

        for step in &fp.post_load {
            let (call, needs_mut) = match step {
                PostLoadStep::Behavior { phase, call } => {
                    let needs = matches!(
                        phase,
                        super::super::resolve::specs::PostLoadPhase::AfterInit
                            | super::super::resolve::specs::PostLoadPhase::Update
                    );
                    (
                        typed_emit::emit_post_load_behavior(
                            *phase,
                            call,
                            ident,
                            ty,
                            did_init_var.as_ref(),
                        ),
                        needs,
                    )
                }
                PostLoadStep::Realloc(spec) => {
                    let payer_ident = &spec.payer.ident;
                    let realloc_expr = &spec.new_space;
                    (
                        quote! {
                            {
                                let __realloc_op = quasar_lang::ops::realloc::Op {
                                    space: (#realloc_expr) as usize,
                                    payer: #payer_ident.to_account_view(),
                                };
                                __realloc_op.apply::<#ty, _>(&mut #ident, &__rent_ctx)?;
                            }
                        },
                        true,
                    )
                }
                PostLoadStep::UserCheck(check) => {
                    let check_stmts = emit_user_check(sem, check);
                    (quote! { #(#check_stmts)* }, false)
                }
                PostLoadStep::VerifyExistingAddress(addr_spec) => {
                    let bump_var = format_ident!("__bumps_{}", ident);
                    let addr_expr = &addr_spec.expr;
                    let term = address_verify_terminator(&addr_spec.error);
                    let verify_existing = if is_validated_account_type(ty) {
                        quote! {
                            #bump_var = quasar_lang::address::AddressVerify::verify_existing(
                                &__addr, #ident.to_account_view().address(), __program_id,
                            )#term;
                        }
                    } else {
                        quote! {
                            #bump_var = quasar_lang::address::AddressVerify::verify(
                                &__addr, #ident.to_account_view().address(), __program_id,
                            )#term;
                        }
                    };
                    let verify = if let Some(bump_offset_expr) = stored_bump_offset_expr(ty) {
                        quote! {
                            if let Some(__bump_offset) = #bump_offset_expr {
                                let __view = #ident.to_account_view();
                                #bump_var = quasar_lang::address::AddressVerify::verify_existing_from_account(
                                    &__addr,
                                    __view.address(),
                                    __program_id,
                                    __view,
                                    __bump_offset,
                                )#term;
                            } else {
                                #verify_existing
                            }
                        }
                    } else {
                        verify_existing
                    };
                    (
                        quote! {
                            {
                                let __addr = #addr_expr;
                                #verify
                            }
                        },
                        false,
                    )
                }
            };

            stmts.push(wrap_optional(is_optional, ident, &call, needs_mut));
        }
    }

    stmts
}

// Epilogue from the typed plan.

pub(crate) fn emit_epilogue(
    semantics: &[FieldSemantics],
    plan: &AccountsPlanTyped,
) -> proc_macro2::TokenStream {
    let mut exit_stmts = Vec::new();

    for (fp, sem) in plan.fields.iter().zip(semantics.iter()) {
        let ident = &sem.core.ident;
        let ty = &sem.core.effective_ty;

        for step in &fp.epilogue {
            let stmt = match step {
                EpilogueStep::Behavior(call) => typed_emit::emit_epilogue_behavior(call, ident, ty),
                EpilogueStep::ProgramClose(spec) => typed_emit::emit_program_close(spec, ident, ty),
            };
            exit_stmts.push(stmt);
        }
    }

    if exit_stmts.is_empty() {
        return quote! {};
    }

    quote! {
        #[inline(always)]
        fn epilogue(&mut self) -> Result<(), ProgramError> {
            #(#exit_stmts)*
            Ok(())
        }
    }
}

pub(crate) fn emit_has_epilogue_typed(
    plan: &AccountsPlanTyped,
    semantics: &[FieldSemantics],
) -> proc_macro2::TokenStream {
    // Collect const-evaluable terms for HAS_EPILOGUE.
    let mut terms: Vec<proc_macro2::TokenStream> = vec![quote! { false }];

    for (fp, sem) in plan.fields.iter().zip(semantics.iter()) {
        let ty = &sem.core.effective_ty;
        for step in &fp.epilogue {
            match step {
                EpilogueStep::Behavior(call) => {
                    let path = &call.path;
                    terms.push(quote! {
                        <#path::Behavior as quasar_lang::account_behavior::AccountBehavior<#ty>>::RUN_EXIT
                    });
                }
                EpilogueStep::ProgramClose(_) => terms.push(quote! { true }),
            }
        }
    }

    quote! { #(#terms)||* }
}

// Load phase.

fn emit_load_filtered(
    semantics: &[FieldSemantics],
    init_only: bool,
) -> Vec<proc_macro2::TokenStream> {
    emit_load_filtered_excluding(semantics, init_only, None)
}

fn emit_load_filtered_excluding(
    semantics: &[FieldSemantics],
    init_only: bool,
    skip_ident: Option<&syn::Ident>,
) -> Vec<proc_macro2::TokenStream> {
    semantics
        .iter()
        .filter(|sem| sem.core.kind == FieldKind::Single)
        .filter(|sem| sem.has_init() == init_only)
        .filter(|sem| skip_ident.is_none_or(|skip| sem.core.ident != *skip))
        .map(emit_one_load)
        .collect()
}

fn emit_load_by_ident(
    semantics: &[FieldSemantics],
    field: &syn::Ident,
) -> proc_macro2::TokenStream {
    semantics
        .iter()
        .find(|sem| sem.core.kind == FieldKind::Single && sem.core.ident == *field)
        .map(emit_one_load)
        .expect("rent plan field should exist in account semantics")
}

fn emit_one_load(sem: &FieldSemantics) -> proc_macro2::TokenStream {
    let ident = &sem.core.ident;
    let ty = &sem.core.effective_ty;
    let writable = sem.is_writable();
    let behavior_validates_account_data = behavior_validates_account_data_expr(sem);

    if sem.core.dynamic {
        let inner_ty = sem.core.inner_ty.as_ref().unwrap_or(ty);
        let base = strip_generics(inner_ty).unwrap_or_else(|_| quote! { #inner_ty });
        return quote! { let #ident = #base::from_account_view(#ident)?; };
    }

    if sem.core.optional {
        let load = emit_load_expr(
            ident,
            ty,
            writable,
            sem.core.dup,
            behavior_validates_account_data.as_ref(),
        );
        return if writable {
            quote! {
                let mut #ident = if quasar_lang::keys_eq(#ident.address(), __program_id) {
                    None
                } else {
                    Some(#load)
                };
            }
        } else {
            quote! {
                let #ident = if quasar_lang::keys_eq(#ident.address(), __program_id) {
                    None
                } else {
                    Some(#load)
                };
            }
        };
    }

    let load = emit_load_expr(
        ident,
        ty,
        writable,
        sem.core.dup,
        behavior_validates_account_data.as_ref(),
    );
    if writable {
        quote! { let mut #ident = #load; }
    } else {
        quote! { let #ident = #load; }
    }
}

fn emit_load_expr(
    ident: &syn::Ident,
    ty: &syn::Type,
    writable: bool,
    checked: bool,
    behavior_validates_account_data: Option<&proc_macro2::TokenStream>,
) -> proc_macro2::TokenStream {
    match (writable, checked, behavior_validates_account_data) {
        (true, true, _) => {
            quote! { <#ty as quasar_lang::account_load::AccountLoad>::load_mut_checked(#ident)? }
        }
        (false, true, _) => {
            quote! { <#ty as quasar_lang::account_load::AccountLoad>::load_checked(#ident)? }
        }
        (true, false, Some(validates_account_data)) => quote! {
            if #validates_account_data {
                // SAFETY: at least one behavior declared that its check validates
                // the account data before this load path uses it.
                unsafe {
                    <#ty as quasar_lang::account_load::AccountLoad>::load_mut_intrinsic(#ident)?
                }
            } else {
                <#ty as quasar_lang::account_load::AccountLoad>::load_mut(#ident)?
            }
        },
        (false, false, Some(validates_account_data)) => quote! {
            if #validates_account_data {
                // SAFETY: at least one behavior declared that its check validates
                // the account data before this load path uses it.
                unsafe {
                    <#ty as quasar_lang::account_load::AccountLoad>::load_intrinsic(#ident)?
                }
            } else {
                <#ty as quasar_lang::account_load::AccountLoad>::load(#ident)?
            }
        },
        (true, false, None) => {
            quote! { <#ty as quasar_lang::account_load::AccountLoad>::load_mut(#ident)? }
        }
        (false, false, None) => {
            quote! { <#ty as quasar_lang::account_load::AccountLoad>::load(#ident)? }
        }
    }
}

fn behavior_validates_account_data_expr(sem: &FieldSemantics) -> Option<proc_macro2::TokenStream> {
    if sem.groups.is_empty() {
        return None;
    }

    let ty = &sem.core.effective_ty;
    let terms = sem.groups.iter().map(|group| {
        let path = &group.path;
        quote! {
            <#path::Behavior as quasar_lang::account_behavior::AccountBehavior<#ty>>::VALIDATES_ACCOUNT_DATA
        }
    });

    Some(quote! { false #(|| #terms)* })
}

// User checks, structural rather than behavior-group based.

fn emit_user_check(sem: &FieldSemantics, check: &UserCheck) -> Vec<proc_macro2::TokenStream> {
    let field_ident = &sem.core.ident;
    let mut stmts = Vec::new();

    match check {
        UserCheck::HasOne { targets, error } => {
            let err = match error {
                Some(e) => quote! { #e.into() },
                None => quote! { QuasarError::HasOneMismatch.into() },
            };
            for target in targets {
                stmts.push(quote! {
                    quasar_lang::validation::check_address_match(
                        &#field_ident.#target,
                        #target.to_account_view().address(),
                        #err,
                    )?;
                });
            }
        }
        UserCheck::Constraints { exprs, error } => {
            let err = match error {
                Some(e) => quote! { #e.into() },
                None => quote! { QuasarError::ConstraintViolation.into() },
            };
            for expr in exprs {
                stmts.push(quote! {
                    quasar_lang::validation::check_constraint(#expr, #err)?;
                });
            }
        }
    }

    stmts
}

// Behavior assertions.

/// Emit compile-time assertions for behavior groups:
/// - `REQUIRES_MUT`: if true, field must be `mut`
/// - `SETS_INIT_PARAMS`: at most one per init field
fn emit_behavior_assertions(semantics: &[FieldSemantics]) -> proc_macro2::TokenStream {
    let mut asserts = Vec::new();

    for sem in semantics {
        let ty = &sem.core.effective_ty;
        let field_name = sem.core.ident.to_string();

        for group in &sem.groups {
            let path = &group.path;

            // REQUIRES_MUT assertion: if behavior requires mut but the field is
            // not writable, emit a compile error.
            if !sem.is_writable() {
                let msg = format!(
                    "behavior `{}` requires `#[account(mut)]` on field `{}`",
                    group.name(),
                    field_name,
                );
                asserts.push(quote! {
                    const _: () = assert!(
                        !<#path::Behavior as quasar_lang::account_behavior::AccountBehavior<#ty>>::REQUIRES_MUT,
                        #msg,
                    );
                });
            }

            let validates_data_msg = format!(
                "behavior `{}` sets VALIDATES_ACCOUNT_DATA and must keep RUN_CHECK = true",
                group.name(),
            );
            asserts.push(quote! {
                const _: () = assert!(
                    !<#path::Behavior as quasar_lang::account_behavior::AccountBehavior<#ty>>::VALIDATES_ACCOUNT_DATA
                        || <#path::Behavior as quasar_lang::account_behavior::AccountBehavior<#ty>>::RUN_CHECK,
                    #validates_data_msg,
                );
            });
        }

        // Init field assertions.
        if sem.has_init() {
            let init_contributor_count: Vec<proc_macro2::TokenStream> = sem
                .groups
                .iter()
                .map(|g| {
                    let p = &g.path;
                    quote! {
                        <#p::Behavior as quasar_lang::account_behavior::AccountBehavior<#ty>>::SETS_INIT_PARAMS as usize
                    }
                })
                .collect();

            if !init_contributor_count.is_empty() {
                // At most one behavior may set init params.
                let at_most_one_msg = format!(
                    "at most one behavior group on field `{}` may set `SETS_INIT_PARAMS = true`",
                    field_name,
                );
                asserts.push(quote! {
                    const _: () = assert!(
                        #(#init_contributor_count)+* <= 1,
                        #at_most_one_msg,
                    );
                });
            }

            // If the account type requires init params (DEFAULT_INIT_PARAMS_VALID
            // = false), at least one behavior must provide them.
            // This fires even with zero behavior groups (count_expr = 0usize).
            let count_expr = if init_contributor_count.is_empty() {
                quote! { 0usize }
            } else {
                quote! { #(#init_contributor_count)+* }
            };
            let required_msg = format!(
                "field `{}` requires an init-param behavior (e.g., token(...) or mint(...))",
                field_name,
            );
            asserts.push(quote! {
                const _: () = assert!(
                    <#ty as quasar_lang::account_init::AccountInit>::DEFAULT_INIT_PARAMS_VALID
                        || #count_expr >= 1,
                    #required_msg,
                );
            });
        }
    }

    quote! { #(#asserts)* }
}

// Helpers.

fn wrap_optional(
    is_optional: bool,
    ident: &syn::Ident,
    body: &proc_macro2::TokenStream,
    needs_mut: bool,
) -> proc_macro2::TokenStream {
    if !is_optional {
        return body.clone();
    }
    if needs_mut {
        quote! {
            if let Some(ref mut #ident) = #ident {
                #body
            }
        }
    } else {
        quote! {
            if let Some(ref #ident) = #ident {
                #body
            }
        }
    }
}

fn emit_bump_vars(semantics: &[FieldSemantics]) -> proc_macro2::TokenStream {
    let vars: Vec<proc_macro2::TokenStream> = semantics
        .iter()
        .filter(|sem| sem.address.is_some())
        .map(|sem| {
            let var = format_ident!("__bumps_{}", sem.core.ident);
            if sem.core.optional {
                quote! { let mut #var: u8 = 0; }
            } else {
                quote! { let #var: u8; }
            }
        })
        .collect();

    quote! { #(#vars)* }
}

fn emit_bump_init(
    semantics: &[FieldSemantics],
    bumps_name: &syn::Ident,
) -> proc_macro2::TokenStream {
    let inits: Vec<proc_macro2::TokenStream> = semantics
        .iter()
        .filter(|sem| sem.address.is_some() || matches!(sem.core.kind, FieldKind::Composite))
        .map(|sem| {
            let name = &sem.core.ident;
            if matches!(sem.core.kind, FieldKind::Composite) {
                let var = format_ident!("__composite_bumps_{}", name);
                quote! { #name: #var }
            } else {
                let var = format_ident!("__bumps_{}", name);
                quote! { #name: #var }
            }
        })
        .collect();

    if inits.is_empty() {
        quote! { #bumps_name }
    } else {
        quote! { #bumps_name { #(#inits,)* } }
    }
}

pub(crate) fn emit_bump_struct_def(
    semantics: &[FieldSemantics],
    cx: &super::EmitCx,
) -> proc_macro2::TokenStream {
    let bumps_name = &cx.bumps_name;
    let fields: Vec<proc_macro2::TokenStream> = semantics
        .iter()
        .filter(|sem| sem.address.is_some() || matches!(sem.core.kind, FieldKind::Composite))
        .map(|sem| {
            let name = &sem.core.ident;
            if matches!(sem.core.kind, FieldKind::Composite) {
                let ty = composite_assoc_ty(&sem.core.effective_ty);
                quote! { pub #name: <#ty as quasar_lang::traits::AccountBumps>::Bumps }
            } else {
                quote! { pub #name: u8 }
            }
        })
        .collect();

    if fields.is_empty() {
        quote! { #[derive(Copy, Clone)] pub struct #bumps_name; }
    } else {
        quote! { #[derive(Copy, Clone)] pub struct #bumps_name { #(#fields,)* } }
    }
}

fn composite_assoc_ty(ty: &syn::Type) -> proc_macro2::TokenStream {
    use super::super::resolve::wrapper::{classify_wrapper, WrapperKind};
    if classify_wrapper(ty) == WrapperKind::AccountsArray {
        return quote! { #ty };
    }
    // Composite field types are path types; fall back to the whole type token
    // (a localized trait error, never a cascade) if that ever fails to hold.
    strip_generics(ty).unwrap_or_else(|_| quote! { #ty })
}

/// Trailing operator for an `AddressVerify::verify*` call.
///
/// With no custom error it is just `?`; with an `address = expr @ error` custom
/// error it becomes a `.map_err(..)?` that surfaces the user's error in place
/// of the verifier's default. All `AddressVerify` methods return
/// `Result<u8, ProgramError>`, so this works for plain and typed-seeds
/// addresses alike (hence the reroute branch, not a rejection).
fn address_verify_terminator(error: &Option<syn::Expr>) -> proc_macro2::TokenStream {
    match error {
        Some(e) => quote! {
            .map_err(|_| -> quasar_lang::prelude::ProgramError { (#e).into() })?
        },
        None => quote! { ? },
    }
}

/// Returns true for account types with owner + discriminator validation.
fn is_validated_account_type(ty: &syn::Type) -> bool {
    use crate::helpers::extract_generic_inner_type;
    extract_generic_inner_type(ty, "Account").is_some()
        || extract_generic_inner_type(ty, "InterfaceAccount").is_some()
        || extract_generic_inner_type(ty, "Migration").is_some()
}

/// Account<T> stores the discriminator-owned bump offset on T. Restrict this
/// fast path to Account<T> so SPL/interface wrappers that do not implement
/// Discriminator keep using the generic existing-account verifier.
fn stored_bump_offset_expr(ty: &syn::Type) -> Option<proc_macro2::TokenStream> {
    use crate::helpers::extract_generic_inner_type;
    let inner = extract_generic_inner_type(ty, "Account")?;
    Some(quote! {
        <#inner as quasar_lang::traits::Discriminator>::BUMP_OFFSET
    })
}

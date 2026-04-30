//! Phased UFCS codegen — the core of v3.
//!
//! Generated parse body shape:
//!
//! ```text
//! // Phase 1: pre-load
//! let __ctx = OpCtx::new(...);
//! // AddressVerify, init::Op::before_load, before_load groups
//!
//! // Phase 2: load all
//! let field_a = <Ty>::load(field_a, "field_a")?;
//! let mut field_b = <Ty>::load_mut(field_b, "field_b")?;
//!
//! // Phase 3a: after_load (shared ref on locals)
//! <token::Op<'_> as AccountOp<Ty>>::after_load(&op, &field_b, &__ctx)?;
//!
//! // Phase 3b: after_load_mut (mut ref on owned locals)
//! <realloc::Op<'_> as AccountOp<Ty>>::after_load_mut(&op, &mut field_b, &__ctx)?;
//!
//! // Phase 3c: user checks
//! check_address_match(...)?;
//!
//! Ok((Self { field_a, field_b }, bumps))
//! ```

use {
    super::{
        super::resolve::{FieldKind, FieldSemantics, UserCheck},
        ops::{
            emit_op_struct, emit_op_type, emit_op_type_static, exit_arg, raw_slot_arg, typed_arg,
            OpEmitCtx,
        },
    },
    crate::helpers::strip_generics,
    quote::{format_ident, quote},
};

pub(crate) fn emit_parse_body(
    semantics: &[FieldSemantics],
    cx: &super::EmitCx,
) -> syn::Result<proc_macro2::TokenStream> {
    let op_ctx = OpEmitCtx {
        field_names: semantics.iter().map(|s| s.core.ident.to_string()).collect(),
    };
    let ctx_init = quote! {
        let __ctx = quasar_lang::ops::OpCtx::new(unsafe {
            &*(__program_id as *const quasar_lang::prelude::Address)
        });
    };
    let bump_vars = emit_bump_vars(semantics);

    // Phase ordering:
    // 1. Load non-init fields (they exist on-chain, data available)
    // 2. Address verify + init CPI for init fields (can reference loaded non-init
    //    fields like config.namespace)
    // 3. Load init fields (now created by init CPI)
    // 4. Ops, validation, user checks
    // Non-init groups that have before_load (rare but possible)
    let non_init_before_load = emit_non_init_before_load(semantics, &op_ctx);
    let load_non_init = emit_phase2_filtered(semantics, false);
    let init_phase = emit_init_phase(semantics, &op_ctx)?;
    let load_init = emit_phase2_filtered(semantics, true);
    let phase3 = emit_phase3(semantics, &op_ctx);
    let bump_init = emit_bump_init(semantics, &cx.bumps_name);

    let construct_fields: Vec<proc_macro2::TokenStream> = semantics
        .iter()
        .map(|sem| {
            let ident = &sem.core.ident;
            quote! { #ident }
        })
        .collect();

    Ok(quote! {
        #bump_vars
        #ctx_init
        #(#non_init_before_load)*
        #(#load_non_init)*
        #(#init_phase)*
        #(#load_init)*
        #(#phase3)*
        Ok((Self { #(#construct_fields,)* }, #bump_init))
    })
}

// ==== Pre-load: before_load for non-init field groups ====

fn emit_non_init_before_load(
    semantics: &[FieldSemantics],
    op_ctx: &OpEmitCtx,
) -> Vec<proc_macro2::TokenStream> {
    let mut stmts = Vec::new();
    for sem in semantics {
        if sem.has_init() {
            continue;
        }
        let ident = &sem.core.ident;
        let ty = &sem.core.effective_ty;
        for group in &sem.groups {
            let op_static = emit_op_type_static(group);
            let op_live = emit_op_type(group);
            let op = emit_op_struct(group, raw_slot_arg, op_ctx);
            stmts.push(quote! {
                if <#op_static as quasar_lang::ops::AccountOp<#ty>>::HAS_BEFORE_LOAD {
                    <#op_live as quasar_lang::ops::AccountOp<
                        #ty,
                    >>::before_load(
                        &#op,
                        #ident, &__ctx,
                    )?;
                }
            });
        }
    }
    stmts
}

// ==== Init phase: address verify + init CPI (runs after non-init fields
// loaded) ====

fn emit_init_phase(
    semantics: &[FieldSemantics],
    op_ctx: &OpEmitCtx,
) -> syn::Result<Vec<proc_macro2::TokenStream>> {
    let mut stmts = Vec::new();

    for sem in semantics {
        let ident = &sem.core.ident;
        let ty = &sem.core.effective_ty;

        // Address verification for init fields.
        // Runs after non-init fields are loaded, so expressions like
        // config.namespace work via Deref on loaded Account<T> fields.
        if sem.has_init() {
            if let Some(address_expr) = &sem.address {
                let bump_var = format_ident!("__bumps_{}", ident);
                let addr_var = format_ident!("__addr_{}", ident);
                stmts.push(quote! {
                    let #addr_var = #address_expr;
                    #bump_var = quasar_lang::address::AddressVerify::verify(
                        &#addr_var, #ident.address(), __program_id,
                    )?;
                });
            }
        }

        // init → init::Op::before_load
        if let Some(init) = &sem.init {
            stmts.push(emit_init_before_load(sem, init, op_ctx)?);
        }

        // before_load for ALL groups on init fields.
        // Args use typed_arg because referenced non-init fields are loaded.
        // The init field itself (#ident) is still a raw AccountView slot.
        if sem.has_init() {
            for group in &sem.groups {
                let op_static = emit_op_type_static(group);
                let op_live = emit_op_type(group);
                let op = emit_op_struct(group, typed_arg, op_ctx);
                stmts.push(quote! {
                    if <#op_static as quasar_lang::ops::AccountOp<#ty>>::HAS_BEFORE_LOAD {
                        <#op_live as quasar_lang::ops::AccountOp<
                            #ty,
                        >>::before_load(
                            &#op,
                            #ident, &__ctx,
                        )?;
                    }
                });
            }
        }
    }

    Ok(stmts)
}

fn emit_init_before_load(
    sem: &FieldSemantics,
    init: &super::super::resolve::InitDirective,
    op_ctx: &OpEmitCtx,
) -> syn::Result<proc_macro2::TokenStream> {
    let ident = &sem.core.ident;
    let ty = &sem.core.effective_ty;

    let payer = sem
        .payer
        .as_ref()
        .ok_or_else(|| syn::Error::new_spanned(&sem.core.field, "init requires `payer`"))?;

    // Space from Space::SPACE for program-owned accounts.
    // When groups contribute init params (token, mint, ata_init), space is 0
    // because the SPL init CPI handles allocation.
    let has_param_contributors = !sem.groups.is_empty();
    let space = if has_param_contributors {
        quote! { 0u64 }
    } else {
        quote! {
            <
                <#ty as quasar_lang::account_load::AccountLoad>::BehaviorTarget
                as quasar_lang::traits::Space
            >::SPACE as u64
        }
    };
    let idempotent = init.idempotent;

    // Init params: accumulate from ALL groups via apply_init_params.
    let params_block = quote! {
        let mut __init_params = <
            <#ty as quasar_lang::account_load::AccountLoad>::BehaviorTarget
            as quasar_lang::account_init::AccountInit
        >::InitParams::default();
    };

    // Use typed_arg because non-init fields are loaded at this point.
    let (op_locals, contributor_calls): (Vec<_>, Vec<_>) = sem
        .groups
        .iter()
        .enumerate()
        .map(|(i, group)| {
            let op = emit_op_struct(group, typed_arg, op_ctx);
            let op_live = emit_op_type(group);
            let op_name = format_ident!("__init_op_{}", i);
            let local = quote! { let #op_name = #op; };
            let call = quote! {
                <#op_live as quasar_lang::ops::AccountOp<#ty>>::apply_init_params(
                    &#op_name,
                    &mut __init_params as *mut _ as *mut u8,
                )?;
            };
            (local, call)
        })
        .unzip();

    let init_call = quote! {
        let __init_op = quasar_lang::ops::init::Op {
            payer: #payer.to_account_view(),
            space: #space,
            signers: __signers,
            params: __init_params,
            idempotent: #idempotent,
        };
        quasar_lang::ops::AccountOp::<#ty>::before_load(&__init_op, #ident, &__ctx)?;
    };

    let body = quote! {
            #params_block
            #(#op_locals)*
            #(#contributor_calls)*
            #init_call
    };

    // Build signers from address spec if present (PDA init).
    // AddressVerify ran before this, so __bumps_<ident> is populated.
    if sem.address.is_some() {
        let bump_var = format_ident!("__bumps_{}", ident);
        let addr_var = format_ident!("__addr_{}", ident);
        Ok(quote! {
            {
                let __bump_ref: &[u8] = &[#bump_var];
                quasar_lang::address::AddressVerify::with_signer_seeds(
                    &#addr_var,
                    __bump_ref,
                    |__maybe_signer| -> Result<(), quasar_lang::prelude::ProgramError> {
                        let __signers = match &__maybe_signer {
                            Some(__signer) => core::slice::from_ref(__signer),
                            None => &[] as &[quasar_lang::cpi::Signer<'_, '_>],
                        };
                        #body
                        Ok(())
                    },
                )?;
            }
        })
    } else {
        Ok(quote! {
            {
                let __signers: &[quasar_lang::cpi::Signer<'_, '_>] = &[];
                #body
            }
        })
    }
}

// ==== Phase 2: load (split into non-init first, then init) ====

fn emit_phase2_filtered(
    semantics: &[FieldSemantics],
    init_only: bool,
) -> Vec<proc_macro2::TokenStream> {
    semantics
        .iter()
        .filter(|sem| sem.core.kind == FieldKind::Single)
        .filter(|sem| sem.has_init() == init_only)
        .map(emit_one_load)
        .collect()
}

fn emit_one_load(sem: &FieldSemantics) -> proc_macro2::TokenStream {
    let ident = &sem.core.ident;
    let ty = &sem.core.effective_ty;
    let field_name_str = ident.to_string();

    if sem.core.dynamic {
        let inner_ty = sem.core.inner_ty.as_ref().unwrap_or(ty);
        let base = strip_generics(inner_ty);
        return quote! { let #ident = #base::from_account_view(#ident)?; };
    }

    // Always use validated load — no unchecked bypass.
    // init::Op writes discriminator + data, so load() succeeds after valid init.

    if sem.core.optional {
        return if sem.core.is_mut {
            quote! {
                let mut #ident = if quasar_lang::keys_eq(#ident.address(), __program_id) {
                    None
                } else {
                    Some(<#ty as quasar_lang::account_load::AccountLoad>::load_mut(#ident, #field_name_str)?)
                };
            }
        } else {
            quote! {
                let #ident = if quasar_lang::keys_eq(#ident.address(), __program_id) {
                    None
                } else {
                    Some(<#ty as quasar_lang::account_load::AccountLoad>::load(#ident, #field_name_str)?)
                };
            }
        };
    }

    if sem.core.is_mut {
        quote! {
            let mut #ident = <#ty as quasar_lang::account_load::AccountLoad>::load_mut(#ident, #field_name_str)?;
        }
    } else {
        quote! {
            let #ident = <#ty as quasar_lang::account_load::AccountLoad>::load(#ident, #field_name_str)?;
        }
    }
}

// ==== Phase 3: validate + mutate + user checks ====

fn emit_phase3(semantics: &[FieldSemantics], op_ctx: &OpEmitCtx) -> Vec<proc_macro2::TokenStream> {
    let mut stmts = Vec::new();

    for sem in semantics {
        let ident = &sem.core.ident;
        let ty = &sem.core.effective_ty;
        let is_optional = sem.core.optional;

        // Phase 3a: after_load for ALL groups
        for group in &sem.groups {
            let op = emit_op_struct(group, typed_arg, op_ctx);
            let op_live = emit_op_type(group);
            let call = quote! {
                <#op_live as quasar_lang::ops::AccountOp<
                    #ty,
                >>::after_load(&#op, &#ident, &__ctx)?;
            };
            stmts.push(wrap_optional(is_optional, ident, &call, false));
        }

        // Phase 3b: after_load_mut on owned mut locals (ALL groups)
        if sem.core.is_mut && sem.core.kind == FieldKind::Single {
            for group in &sem.groups {
                let op = emit_op_struct(group, typed_arg, op_ctx);
                let op_static = emit_op_type_static(group);
                let op_live = emit_op_type(group);
                // Gate on REQUIRES_MUT at compile time
                let call = quote! {
                    {
                        const __REQ: bool =
                            <#op_static as quasar_lang::ops::AccountOp<#ty>>::REQUIRES_MUT;
                        if __REQ {
                            <#op_live as quasar_lang::ops::AccountOp<
                                #ty,
                            >>::after_load_mut(&#op, &mut #ident, &__ctx)?;
                        }
                    }
                };
                stmts.push(wrap_optional(is_optional, ident, &call, true));
            }

            // Realloc op (Phase 3b) — emitted when field has `realloc = expr`
            if let Some(realloc_expr) = &sem.realloc {
                let payer = sem.payer.as_ref().expect("realloc requires payer");
                // Build crate path via ident to keep domain strings out of derive source.
                let spl_crate = format_ident!("quasar_{}", "spl");
                let call = quote! {
                    {
                        let __realloc_op = #spl_crate::ops::realloc::Op {
                            space: (#realloc_expr) as usize,
                            payer: #payer.to_account_view(),
                        };
                        <#spl_crate::ops::realloc::Op<'_> as quasar_lang::ops::AccountOp<
                            #ty,
                        >>::after_load_mut(&__realloc_op, &mut #ident, &__ctx)?;
                    }
                };
                stmts.push(wrap_optional(is_optional, ident, &call, true));
            }

            if has_field_lifecycle(sem) {
                // Field lifecycle before-handler hook. Migration uses this to
                // grow the account before handler code writes the target
                // layout.
                let payer_option = match &sem.payer {
                    Some(p) => quote! { Some(#p.to_account_view()) },
                    None => quote! { None },
                };
                let lifecycle_call = quote! {
                    if <#ty as quasar_lang::traits::FieldLifecycle>::HAS_LIFECYCLE_BEFORE {
                        quasar_lang::traits::FieldLifecycle::before_lifecycle(
                            &mut #ident,
                            #payer_option,
                            &__ctx,
                        )?;
                    }
                };
                stmts.push(wrap_optional(is_optional, ident, &lifecycle_call, true));
            }
        }

        // Phase 3c: address verification for NON-init fields (after load).
        // Uses verify_existing (fast path) — skips on-curve check since the
        // account already exists on-chain. Safe because SHA256 collision is
        // computationally infeasible. Init fields use full verify() in Phase 1.
        if !sem.has_init() {
            if let Some(address_expr) = &sem.address {
                let bump_var = format_ident!("__bumps_{}", ident);
                let call = quote! {
                    {
                        let __addr = #address_expr;
                        #bump_var = quasar_lang::address::AddressVerify::verify_existing(
                            &__addr, #ident.to_account_view().address(), __program_id,
                        )?;
                    }
                };
                stmts.push(wrap_optional(is_optional, ident, &call, false));
            }
        }

        // Phase 3d: user checks
        for check in &sem.user_checks {
            let check_stmts = emit_user_check(sem, check);
            let combined = quote! { #(#check_stmts)* };
            stmts.push(wrap_optional(is_optional, ident, &combined, false));
        }

        // REQUIRES_MUT compile-time assertions
        for group in &sem.groups {
            let op_static = emit_op_type_static(group);
            let path = &group.path;
            let is_mut = sem.core.is_mut;
            stmts.push(quote! {
                const _: () = assert!(
                    !<#op_static as quasar_lang::ops::AccountOp<#ty>>::REQUIRES_MUT
                    || #is_mut,
                    concat!(
                        "op `", stringify!(#path), "` requires `mut` on field `",
                        stringify!(#ty), "`"
                    ),
                );
            });
        }
    }

    stmts
}

/// Wrap a code block in `if let Some(ref field) = field { ... }` for optional
/// accounts. `needs_mut` uses `ref mut` instead.
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
        UserCheck::Address { expr, error } => {
            let err = match error {
                Some(e) => quote! { #e.into() },
                None => quote! { QuasarError::AddressMismatch.into() },
            };
            stmts.push(quote! {
                quasar_lang::validation::check_address_match(
                    #field_ident.to_account_view().address(),
                    &#expr,
                    #err,
                )?;
            });
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

// ==== Epilogue (Phase 4) ====

pub(crate) fn emit_epilogue(
    semantics: &[FieldSemantics],
    op_ctx: &OpEmitCtx,
) -> syn::Result<proc_macro2::TokenStream> {
    let mut exit_stmts = Vec::new();

    for sem in semantics {
        let ty = &sem.core.effective_ty;
        let field = &sem.core.ident;

        // Phase 4: exit — unconditional emission, symmetric with before_load.
        // Op-arg grammar guarantees exit_arg always produces valid code.
        // Gated on HAS_EXIT — dead code eliminated by LLVM.
        for group in &sem.groups {
            let op_static = emit_op_type_static(group);
            let op_live = emit_op_type(group);
            let op = emit_op_struct(group, exit_arg, op_ctx);
            exit_stmts.push(quote! {
                if <#op_static as quasar_lang::ops::AccountOp<#ty>>::HAS_EXIT {
                    <#op_live as quasar_lang::ops::AccountOp<
                        #ty,
                    >>::exit(&#op, &mut self.#field, &__ctx)?;
                }
            });
        }

        // FieldLifecycle exit check for types with lifecycle behavior.
        if sem.core.is_mut && sem.core.kind == FieldKind::Single && has_field_lifecycle(sem) {
            let payer_option = match &sem.payer {
                Some(p) => quote! { Some(self.#p.to_account_view()) },
                None => quote! { None },
            };
            exit_stmts.push(quote! {
                if <#ty as quasar_lang::traits::FieldLifecycle>::HAS_LIFECYCLE_EXIT {
                    quasar_lang::traits::FieldLifecycle::exit_lifecycle(
                        &mut self.#field,
                        #payer_option,
                        &__ctx,
                    )?;
                }
            });
        }
    }

    if exit_stmts.is_empty() {
        return Ok(quote! {});
    }

    Ok(quote! {
        #[inline(always)]
        fn epilogue(&mut self) -> Result<(), ProgramError> {
            let __ctx = quasar_lang::ops::OpCtx::new(&crate::ID);
            #(#exit_stmts)*
            Ok(())
        }
    })
}

pub(crate) fn emit_has_epilogue(semantics: &[FieldSemantics]) -> proc_macro2::TokenStream {
    let mut exprs: Vec<proc_macro2::TokenStream> = Vec::new();

    // Op-level HAS_EXIT
    for sem in semantics {
        let ty = sem.core.effective_ty.clone();
        for group in &sem.groups {
            let op_static = emit_op_type_static(group);
            exprs.push(quote! {
                <#op_static as quasar_lang::ops::AccountOp<#ty>>::HAS_EXIT
            });
        }

        // FieldLifecycle exit for types with lifecycle behavior.
        if sem.core.is_mut && sem.core.kind == FieldKind::Single && has_field_lifecycle(sem) {
            exprs.push(quote! {
                <#ty as quasar_lang::traits::FieldLifecycle>::HAS_LIFECYCLE_EXIT
            });
        }
    }

    if exprs.is_empty() {
        quote! { false }
    } else {
        quote! { #(#exprs)||* }
    }
}

fn has_field_lifecycle(sem: &FieldSemantics) -> bool {
    match &sem.core.effective_ty {
        syn::Type::Path(tp) => tp
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "Migration"),
        _ => false,
    }
}

// ==== Helpers ====

fn emit_bump_vars(semantics: &[FieldSemantics]) -> proc_macro2::TokenStream {
    let vars: Vec<proc_macro2::TokenStream> = semantics
        .iter()
        .filter(|sem| sem.address.is_some())
        .map(|sem| {
            let var = format_ident!("__bumps_{}", sem.core.ident);
            quote! { let mut #var: u8 = 0; }
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
        .filter(|sem| sem.address.is_some())
        .flat_map(|sem| {
            let name = &sem.core.ident;
            let var = format_ident!("__bumps_{}", name);
            let arr_name = format_ident!("__{}_bump", name);
            vec![quote! { #name: #var }, quote! { #arr_name: [#var] }]
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
        .filter(|sem| sem.address.is_some())
        .flat_map(|sem| {
            let name = &sem.core.ident;
            let arr_name = format_ident!("__{}_bump", name);
            vec![quote! { pub #name: u8 }, quote! { pub #arr_name: [u8; 1] }]
        })
        .collect();

    if fields.is_empty() {
        quote! { #[derive(Copy, Clone)] pub struct #bumps_name; }
    } else {
        quote! { #[derive(Copy, Clone)] pub struct #bumps_name { #(#fields,)* } }
    }
}

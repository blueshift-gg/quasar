//! Behavior call snippets — one shape per phase, all const-guarded.
//!
//! Every behavior phase emits the same pattern: const guard → build args →
//! call trait method. Protocol crates own the trait impls and builders.

use {
    super::super::resolve::specs::*,
    quote::{format_ident, quote},
};

// ---------------------------------------------------------------------------
// Behavior call emit — one function per phase, all const-guarded
// ---------------------------------------------------------------------------

/// Emit a const-guarded behavior phase call for the post-load phase.
/// The `BehaviorPhase` on the call determines which const, which builder
/// method, and which trait method to emit.
pub(crate) fn emit_post_load_behavior(
    call: &BehaviorCall,
    field_ident: &syn::Ident,
    field_ty: &syn::Type,
    did_init_var: Option<&syn::Ident>,
) -> proc_macro2::TokenStream {
    let path = &call.path;
    let bhv =
        quote! { <#path::Behavior as quasar_lang::account_behavior::AccountBehavior<#field_ty>> };
    let args_block = emit_args_builder(call, field_ty);

    match call.phase {
        BehaviorPhase::AfterInit => quote! {
            if #bhv::RUN_AFTER_INIT {
                #args_block
                #bhv::after_init(&mut #field_ident, &__bhv_args)?;
            }
        },
        BehaviorPhase::Check => {
            let fresh_init_guard = if let Some(did_init_var) = did_init_var {
                quote! { !(#did_init_var && #bhv::INIT_SATISFIES_CHECK) }
            } else {
                quote! { true }
            };
            quote! {
                if #bhv::RUN_CHECK && #fresh_init_guard {
                    #args_block
                    #bhv::check(&#field_ident, &__bhv_args)?;
                }
            }
        }
        BehaviorPhase::Update => quote! {
            if #bhv::RUN_UPDATE {
                #args_block
                #bhv::update(&mut #field_ident, &__bhv_args)?;
            }
        },
        _ => quote! {},
    }
}

/// Emit a const-guarded behavior phase call for the epilogue phase.
/// Exit args use `self.field` references.
pub(crate) fn emit_epilogue_behavior(
    call: &BehaviorCall,
    field_ident: &syn::Ident,
    field_ty: &syn::Type,
) -> proc_macro2::TokenStream {
    let path = &call.path;
    let bhv =
        quote! { <#path::Behavior as quasar_lang::account_behavior::AccountBehavior<#field_ty>> };
    let args_block = emit_exit_args_builder(call, field_ty);

    quote! {
        if #bhv::RUN_EXIT {
            #args_block
            #bhv::exit(&mut self.#field_ident, &__bhv_args)?;
        }
    }
}

/// Emit behavior init CPI: set_init_param → AccountInit::init.
/// The account is loaded in the normal load phase. After_init and check
/// run as post-load steps.
pub(crate) fn emit_behavior_init(
    spec: &BehaviorInitSpec,
    field_ident: &syn::Ident,
    field_ty: &syn::Type,
    has_address: bool,
    did_init_var: Option<&syn::Ident>,
) -> proc_macro2::TokenStream {
    let payer_ident = &spec.payer.ident;
    let idempotent = spec.idempotent;

    let set_params: Vec<proc_macro2::TokenStream> = spec
        .init_param_calls
        .iter()
        .map(|call| {
            let path = &call.path;
            let args_block = emit_args_builder(call, field_ty);
            quote! {
                if <#path::Behavior as quasar_lang::account_behavior::AccountBehavior<#field_ty>>::SETS_INIT_PARAMS {
                    #args_block
                    <#path::Behavior as quasar_lang::account_behavior::AccountBehavior<#field_ty>>::set_init_param(
                        &mut __init_params,
                        &__bhv_args,
                    )?;
                }
            }
        })
        .collect();

    let did_init_assignment = did_init_var
        .map(|did_init_var| quote! { #did_init_var = true; })
        .unwrap_or_else(|| quote! {});

    let init_cpi = quote! {
        let mut __init_params = <#field_ty as quasar_lang::account_init::AccountInit>::InitParams::default();
        #(#set_params)*
        let __init_op = quasar_lang::ops::init::Op {
            payer: #payer_ident.to_account_view(),
            space: 0u64,
            signers: __signers,
            params: __init_params,
            idempotent: #idempotent,
        };
        __init_op.apply::<#field_ty, _>(#field_ident, &__rent_ctx)?;
        #did_init_assignment
    };

    let body = if has_address {
        let bump_var = format_ident!("__bumps_{}", field_ident);
        let addr_var = format_ident!("__addr_{}", field_ident);
        quote! {
            let __bump_ref: &[u8] = &[#bump_var];
            quasar_lang::address::AddressVerify::with_signer_seeds(
                &#addr_var,
                __bump_ref,
                |__signers| -> Result<(), quasar_lang::prelude::ProgramError> {
                    #init_cpi
                    Ok(())
                },
            )?;
        }
    } else {
        quote! {
            let __signers: &[quasar_lang::cpi::Signer<'_, '_>] = &[];
            #init_cpi
        }
    };

    if idempotent {
        quote! {
            if quasar_lang::is_system_program(#field_ident.owner()) {
                #body
            }
        }
    } else {
        quote! { { #body } }
    }
}

/// Emit plain program init (no behavior — system program create +
/// discriminator).
pub(crate) fn emit_program_init(
    spec: &ProgramInitSpec,
    field_ident: &syn::Ident,
    field_ty: &syn::Type,
    has_address: bool,
) -> proc_macro2::TokenStream {
    let payer_ident = &spec.payer.ident;
    let idempotent = spec.idempotent;
    let space_ty = &spec.space_ty;
    let space = quote! {
        <#space_ty as quasar_lang::traits::Space>::SPACE as u64
    };

    let inner_body = quote! {
        let __init_params = ();
        let __init_op = quasar_lang::ops::init::Op {
            payer: #payer_ident.to_account_view(),
            space: #space,
            signers: __signers,
            params: __init_params,
            idempotent: #idempotent,
        };
        __init_op.apply::<#field_ty, _>(#field_ident, &__rent_ctx)?;
    };

    let body = if has_address {
        let bump_var = format_ident!("__bumps_{}", field_ident);
        let addr_var = format_ident!("__addr_{}", field_ident);
        quote! {
            let __bump_ref: &[u8] = &[#bump_var];
            quasar_lang::address::AddressVerify::with_signer_seeds(
                &#addr_var,
                __bump_ref,
                |__signers| -> Result<(), quasar_lang::prelude::ProgramError> {
                    #inner_body
                    Ok(())
                },
            )?;
        }
    } else {
        quote! {
            let __signers: &[quasar_lang::cpi::Signer<'_, '_>] = &[];
            #inner_body
        }
    };

    if idempotent {
        quote! {
            if quasar_lang::is_system_program(#field_ident.owner()) {
                #body
            }
        }
    } else {
        quote! { { #body } }
    }
}

// ---------------------------------------------------------------------------
// Program close emit (core lifecycle, not behavior)
// ---------------------------------------------------------------------------

pub(crate) fn emit_program_close(
    spec: &ProgramCloseSpec,
    field_ident: &syn::Ident,
    field_ty: &syn::Type,
) -> proc_macro2::TokenStream {
    let dest_ident = &spec.destination_field;
    let disc_ty = crate::helpers::extract_generic_inner_type(field_ty, "Account")
        .or_else(|| crate::helpers::extract_generic_inner_type(field_ty, "InterfaceAccount"))
        .unwrap_or(field_ty);
    quote! {
        {
            let __view = unsafe {
                <#field_ty as quasar_lang::account_load::AccountLoad>::to_account_view_mut(
                    &mut self.#field_ident
                )
            };
            quasar_lang::ops::close::close_account(
                __view,
                self.#dest_ident.to_account_view(),
                <#disc_ty as quasar_lang::traits::Discriminator>::DISCRIMINATOR.len(),
            )?;
        }
    }
}

// ---------------------------------------------------------------------------
// Args builder codegen
// ---------------------------------------------------------------------------

/// Emit the builder chain for a behavior call (parse-time phase: uses local
/// vars).
fn emit_args_builder(call: &BehaviorCall, field_ty: &syn::Type) -> proc_macro2::TokenStream {
    let path = &call.path;
    let bhv =
        quote! { <#path::Behavior as quasar_lang::account_behavior::AccountBehavior<#field_ty>> };
    let phase = emit_arg_phase_const(call.phase);
    let setters: Vec<proc_macro2::TokenStream> = call
        .args
        .iter()
        .map(|arg| {
            let key = &arg.key;
            let key_lit = key.to_string();
            let val = emit_lowered_value(&arg.lowered);
            quote! {
                let __bhv_builder = if #bhv::uses_arg::<
                    { #phase },
                    { quasar_lang::account_behavior::behavior_arg_key_hash(#key_lit) },
                >() {
                    __bhv_builder.#key(#val)
                } else {
                    __bhv_builder
                };
            }
        })
        .collect();

    let build_method = match call.phase {
        BehaviorPhase::SetInitParam | BehaviorPhase::AfterInit => quote! { build_init },
        BehaviorPhase::Check | BehaviorPhase::Update => quote! { build_check },
        BehaviorPhase::Exit => quote! { build_exit },
    };

    quote! {
        let __bhv_builder = #path::Args::builder();
        #(#setters)*
        let __bhv_args = __bhv_builder.#build_method()?;
    }
}

/// Emit the builder chain for an exit behavior call (epilogue phase: uses
/// `self.field`).
fn emit_exit_args_builder(call: &BehaviorCall, field_ty: &syn::Type) -> proc_macro2::TokenStream {
    let path = &call.path;
    let bhv =
        quote! { <#path::Behavior as quasar_lang::account_behavior::AccountBehavior<#field_ty>> };
    let phase = emit_arg_phase_const(call.phase);
    let setters: Vec<proc_macro2::TokenStream> = call
        .args
        .iter()
        .map(|arg| {
            let key = &arg.key;
            let key_lit = key.to_string();
            let val = emit_exit_lowered_value(&arg.lowered);
            quote! {
                let __bhv_builder = if #bhv::uses_arg::<
                    { #phase },
                    { quasar_lang::account_behavior::behavior_arg_key_hash(#key_lit) },
                >() {
                    __bhv_builder.#key(#val)
                } else {
                    __bhv_builder
                };
            }
        })
        .collect();

    quote! {
        let __bhv_builder = #path::Args::builder();
        #(#setters)*
        let __bhv_args = __bhv_builder.build_exit()?;
    }
}

fn emit_arg_phase_const(phase: BehaviorPhase) -> proc_macro2::TokenStream {
    match phase {
        BehaviorPhase::SetInitParam => {
            quote! { quasar_lang::account_behavior::ARG_PHASE_SET_INIT_PARAM }
        }
        BehaviorPhase::AfterInit => {
            quote! { quasar_lang::account_behavior::ARG_PHASE_AFTER_INIT }
        }
        BehaviorPhase::Check => quote! { quasar_lang::account_behavior::ARG_PHASE_CHECK },
        BehaviorPhase::Update => quote! { quasar_lang::account_behavior::ARG_PHASE_UPDATE },
        BehaviorPhase::Exit => quote! { quasar_lang::account_behavior::ARG_PHASE_EXIT },
    }
}

/// Emit a lowered value in parse-time context (local variables).
fn emit_lowered_value(val: &LoweredValue) -> proc_macro2::TokenStream {
    match val {
        LoweredValue::FieldView(ident) => quote! { #ident.to_account_view() },
        LoweredValue::OptionalFieldView(ident) => {
            quote! { #ident.as_ref().map(|v| v.to_account_view()) }
        }
        LoweredValue::Expr(expr) => quote! { #expr },
        LoweredValue::NoneLiteral => quote! { None },
        LoweredValue::SomeFieldView(ident) => {
            quote! { Some(#ident.to_account_view()) }
        }
        LoweredValue::SomeExpr(expr) => quote! { Some(#expr) },
    }
}

/// Emit a lowered value in epilogue context (`self.field`).
fn emit_exit_lowered_value(val: &LoweredValue) -> proc_macro2::TokenStream {
    match val {
        LoweredValue::FieldView(ident) => quote! { self.#ident.to_account_view() },
        LoweredValue::OptionalFieldView(ident) => {
            quote! { self.#ident.as_ref().map(|v| v.to_account_view()) }
        }
        LoweredValue::Expr(expr) => quote! { #expr },
        LoweredValue::NoneLiteral => quote! { None },
        LoweredValue::SomeFieldView(ident) => {
            quote! { Some(self.#ident.to_account_view()) }
        }
        LoweredValue::SomeExpr(expr) => quote! { Some(#expr) },
    }
}

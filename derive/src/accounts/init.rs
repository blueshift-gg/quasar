//! Init codegen for `#[derive(Accounts)]`.
//!
//! Generates CPI calls for account initialization: token accounts, mints,
//! generic Account<T>, ATAs, metadata, and master editions.

use {
    super::{
        attrs::AccountFieldAttrs,
        field_kind::{strip_ref, FieldKind},
    },
    crate::helpers::{seed_slice_expr_for_parse, strip_generics},
    quote::{format_ident, quote},
    syn::{Ident, Type},
};

/// Context needed by init codegen, gathered from DetectedFields + per-field locals.
pub(super) struct InitContext<'a> {
    pub payer: &'a Ident,
    pub system_program: &'a Ident,
    pub token_program: Option<&'a Ident>,
    pub ata_program: Option<&'a Ident>,
    pub metadata_account: Option<&'a Ident>,
    pub master_edition_account: Option<&'a Ident>,
    pub metadata_program: Option<&'a Ident>,
    pub mint_authority: Option<&'a Ident>,
    pub update_authority: Option<&'a Ident>,
    pub rent: Option<&'a Ident>,
    pub field_name_strings: &'a [String],
}

/// Result of generating an init block.
pub(super) struct InitBlockResult {
    pub tokens: proc_macro2::TokenStream,
    /// True if this init path uses `__shared_rent` (false for ATA).
    pub uses_rent: bool,
}

/// Build PDA signer seeds setup and reference for init_account calls.
fn gen_signers(
    field_name: &Ident,
    attrs: &AccountFieldAttrs,
    field_name_strings: &[String],
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    if let Some(seed_exprs) = &attrs.seeds {
        let bump_var = format_ident!("__bumps_{}", field_name);
        let seed_slices: Vec<proc_macro2::TokenStream> = seed_exprs
            .iter()
            .map(|expr| seed_slice_expr_for_parse(expr, field_name_strings))
            .collect();
        (
            quote! {
                let __init_bump_ref: &[u8] = &[#bump_var];
                let __init_signer_seeds = [#(quasar_lang::cpi::Seed::from(#seed_slices),)* quasar_lang::cpi::Seed::from(__init_bump_ref)];
                let __init_signers = [quasar_lang::cpi::Signer::from(&__init_signer_seeds[..])];
            },
            quote! { &__init_signers },
        )
    } else {
        (quote! {}, quote! { &[] })
    }
}

/// Shared init CPI body: try_minimum_balance + init_account + post_init.
/// Used by token, mint, and Account<T> init (NOT ATA -- ATA uses its own CPI).
fn gen_init_cpi_body(
    pay_field: &Ident,
    field_name: &Ident,
    space_expr: proc_macro2::TokenStream,
    owner_expr: proc_macro2::TokenStream,
    signers_setup: &proc_macro2::TokenStream,
    signers_ref: &proc_macro2::TokenStream,
    post_init: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    quote! {
        let __init_lamports = __shared_rent.try_minimum_balance(#space_expr as usize)?;
        #signers_setup
        quasar_lang::cpi::system::init_account(
            #pay_field, #field_name, __init_lamports, #space_expr as u64,
            #owner_expr, #signers_ref,
        )?;
        #post_init
    }
}

/// Wrap a CPI body with the init/init_if_needed guard pattern.
///
/// - `init`: reject if already initialized, then run CPI body.
/// - `init_if_needed`: if uninitialized run CPI body, else run validation (if
///   provided).
pub(super) fn wrap_init_block(
    field_name: &Ident,
    init_if_needed: bool,
    cpi_body: proc_macro2::TokenStream,
    validate_existing: Option<proc_macro2::TokenStream>,
) -> proc_macro2::TokenStream {
    if init_if_needed {
        let validate = validate_existing.unwrap_or_default();
        quote! {
            {
                if quasar_lang::is_system_program(#field_name.owner()) {
                    #cpi_body
                } else {
                    #validate
                }
            }
        }
    } else {
        quote! {
            {
                if !quasar_lang::is_system_program(#field_name.owner()) {
                    return Err(ProgramError::AccountAlreadyInitialized);
                }
                #cpi_body
            }
        }
    }
}

/// Generate the init block for a field. Returns None if not an init field
/// or if the field type doesn't support init.
pub(super) fn gen_init_block(
    field_name: &Ident,
    attrs: &AccountFieldAttrs,
    effective_ty: &Type,
    ctx: &InitContext,
) -> Result<Option<InitBlockResult>, proc_macro::TokenStream> {
    if !attrs.is_init && !attrs.init_if_needed {
        return Ok(None);
    }

    let (signers_setup, signers_ref) = gen_signers(field_name, attrs, ctx.field_name_strings);
    let pay = ctx.payer;

    // --- ATA init ---
    if attrs.associated_token_mint.is_some() {
        let ata_prog = ctx.ata_program.unwrap();
        let mint_field = attrs.associated_token_mint.as_ref().unwrap();
        let auth_field = attrs.associated_token_authority.as_ref().unwrap();
        let sys_field = ctx.system_program;
        let tok_field = attrs
            .associated_token_token_program
            .as_ref()
            .unwrap_or_else(|| ctx.token_program.unwrap());
        let token_program_addr = if let Some(tp) = &attrs.associated_token_token_program {
            quote! { #tp.address() }
        } else {
            let tp = ctx.token_program.unwrap();
            quote! { #tp.address() }
        };

        let ata_cpi = |instruction_byte: u8| {
            quote! {
                quasar_lang::cpi::CpiCall::new(
                    #ata_prog.address(),
                    [
                        quasar_lang::cpi::InstructionAccount::writable_signer(#pay.address()),
                        quasar_lang::cpi::InstructionAccount::writable(#field_name.address()),
                        quasar_lang::cpi::InstructionAccount::readonly(#auth_field.address()),
                        quasar_lang::cpi::InstructionAccount::readonly(#mint_field.address()),
                        quasar_lang::cpi::InstructionAccount::readonly(#sys_field.address()),
                        quasar_lang::cpi::InstructionAccount::readonly(#tok_field.address()),
                    ],
                    [#pay, #field_name, #auth_field, #mint_field, #sys_field, #tok_field],
                    [#instruction_byte],
                ).invoke()?;
            }
        };

        let validate = quote! {
            quasar_spl::validate_ata(
                #field_name.to_account_view(),
                #auth_field.to_account_view().address(),
                #mint_field.to_account_view().address(),
                #token_program_addr,
            )?;
        };

        let block = wrap_init_block(
            field_name,
            attrs.init_if_needed,
            ata_cpi(if attrs.init_if_needed { 1 } else { 0 }),
            Some(validate),
        );
        return Ok(Some(InitBlockResult {
            tokens: block,
            uses_rent: false, // ATA program handles rent
        }));
    }

    // --- Token init ---
    if attrs.token_mint.is_some() {
        let tok_field = ctx.token_program.unwrap();
        let mint_field = attrs.token_mint.as_ref().unwrap();
        let auth_field = attrs.token_authority.as_ref().unwrap();

        let cpi_body = gen_init_cpi_body(
            pay,
            field_name,
            quote! { quasar_spl::TokenAccountState::LEN },
            quote! { #tok_field.address() },
            &signers_setup,
            &signers_ref,
            quote! {
                quasar_spl::initialize_account3(
                    #tok_field, #field_name, #mint_field, #auth_field.address(),
                ).invoke()?;
            },
        );
        let tok_addr = quote! { #tok_field.address() };
        let validate = quote! {
            quasar_spl::validate_token_account(
                #field_name.to_account_view(),
                #mint_field.to_account_view().address(),
                #auth_field.to_account_view().address(),
                #tok_addr,
            )?;
        };
        let block = wrap_init_block(field_name, attrs.init_if_needed, cpi_body, Some(validate));
        return Ok(Some(InitBlockResult {
            tokens: block,
            uses_rent: true,
        }));
    }

    // --- Mint init ---
    if let Some(decimals_expr) = attrs.mint_decimals.as_ref() {
        let tok_field = ctx.token_program.unwrap();
        let auth_field = attrs.mint_init_authority.as_ref().ok_or_else(|| -> proc_macro::TokenStream {
            syn::Error::new_spanned(
                field_name,
                "`mint::decimals` requires `mint::authority = <field>`",
            )
            .to_compile_error()
            .into()
        })?;
        let freeze_expr = if let Some(ff) = &attrs.mint_freeze_authority {
            quote! { Some(#ff.address()) }
        } else {
            quote! { None }
        };

        let cpi_body = gen_init_cpi_body(
            pay,
            field_name,
            quote! { quasar_spl::MintAccountState::LEN },
            quote! { #tok_field.address() },
            &signers_setup,
            &signers_ref,
            quote! {
                quasar_spl::initialize_mint2(
                    #tok_field, #field_name,
                    (#decimals_expr) as u8,
                    #auth_field.address(),
                    #freeze_expr,
                ).invoke()?;
            },
        );
        let tok_addr = quote! { #tok_field.address() };
        let freeze_validate = if let Some(ff) = &attrs.mint_freeze_authority {
            quote! { Some(#ff.to_account_view().address()) }
        } else {
            quote! { None }
        };
        let validate = quote! {
            quasar_spl::validate_mint(
                #field_name.to_account_view(),
                #auth_field.to_account_view().address(),
                (#decimals_expr) as u8,
                #freeze_validate,
                #tok_addr,
            )?;
        };
        let block = wrap_init_block(field_name, attrs.init_if_needed, cpi_body, Some(validate));
        return Ok(Some(InitBlockResult {
            tokens: block,
            uses_rent: true,
        }));
    }

    // --- Generic Account<T> init ---
    let underlying = strip_ref(effective_ty);
    if let FieldKind::Account { inner_ty } = FieldKind::classify(underlying) {
        let inner_type = strip_generics(inner_ty);
        let space_expr = if let Some(space) = &attrs.space {
            quote! { (#space) as u64 }
        } else {
            quote! { <#inner_type as quasar_lang::traits::Space>::SPACE as u64 }
        };

        let cpi_body = gen_init_cpi_body(
            pay,
            field_name,
            space_expr,
            quote! { &crate::ID },
            &signers_setup,
            &signers_ref,
            quote! {
                let __disc = <#inner_type as quasar_lang::traits::Discriminator>::DISCRIMINATOR;
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        __disc.as_ptr(), #field_name.data_mut_ptr(), __disc.len(),
                    );
                }
            },
        );
        let block = wrap_init_block(field_name, attrs.init_if_needed, cpi_body, None);
        return Ok(Some(InitBlockResult {
            tokens: block,
            uses_rent: true,
        }));
    }

    Err(syn::Error::new_spanned(
        field_name,
        "#[account(init)] on non-Account<T> type requires `token::mint` and \
         `token::authority`, `associated_token::mint` and \
         `associated_token::authority`, or `mint::decimals` and `mint::authority`",
    )
    .to_compile_error()
    .into())
}

/// Generate metadata CPI init block. Returns None if no metadata attrs.
pub(super) fn gen_metadata_init(
    field_name: &Ident,
    attrs: &AccountFieldAttrs,
    ctx: &InitContext,
) -> Option<proc_macro2::TokenStream> {
    let meta_name = attrs.metadata_name.as_ref()?;
    let meta_symbol = attrs.metadata_symbol.as_ref().unwrap();
    let meta_uri = attrs.metadata_uri.as_ref().unwrap();
    let seller_fee = attrs
        .metadata_seller_fee_basis_points
        .as_ref()
        .map(|e| quote! { (#e) as u16 })
        .unwrap_or(quote! { 0u16 });
    let is_mutable = attrs
        .metadata_is_mutable
        .as_ref()
        .map(|e| quote! { #e })
        .unwrap_or(quote! { false });

    let meta_field = ctx.metadata_account.unwrap();
    let meta_prog = ctx.metadata_program.unwrap();
    let mint_auth = ctx.mint_authority.unwrap();
    let update_auth = ctx.update_authority.unwrap();
    let pay = ctx.payer;
    let sys = ctx.system_program;
    let rent = ctx.rent.unwrap();

    Some(quote! {
        {
            quasar_spl::metadata::MetadataCpi::create_metadata_accounts_v3(
                #meta_prog, #meta_field, #field_name, #mint_auth,
                #pay, #update_auth, #sys, #rent,
                quasar_lang::borsh::BorshString::new(#meta_name),
                quasar_lang::borsh::BorshString::new(#meta_symbol),
                quasar_lang::borsh::BorshString::new(#meta_uri),
                #seller_fee, #is_mutable, true,
            ).invoke()?;
        }
    })
}

/// Generate master edition CPI init block. Returns None if no master_edition attrs.
pub(super) fn gen_master_edition_init(
    field_name: &Ident,
    attrs: &AccountFieldAttrs,
    ctx: &InitContext,
) -> Option<proc_macro2::TokenStream> {
    let max_supply = attrs.master_edition_max_supply.as_ref()?;

    let me_field = ctx.master_edition_account.unwrap();
    let meta_field = ctx.metadata_account.unwrap();
    let meta_prog = ctx.metadata_program.unwrap();
    let mint_auth = ctx.mint_authority.unwrap();
    let update_auth = ctx.update_authority.unwrap();
    let pay = ctx.payer;
    let tok = ctx.token_program.unwrap();
    let sys = ctx.system_program;
    let rent = ctx.rent.unwrap();

    Some(quote! {
        {
            quasar_spl::metadata::MetadataCpi::create_master_edition_v3(
                #meta_prog, #me_field, #field_name, #update_auth,
                #mint_auth, #pay, #meta_field, #tok, #sys, #rent,
                Some(#max_supply as u64),
            ).invoke()?;
        }
    })
}

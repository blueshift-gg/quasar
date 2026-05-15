//! AccountInit implementations for metadata account types.
//!
//! Defines init params enums and CPI dispatch for `create_metadata_accounts_v3`
//! and `create_master_edition_v3`. The derive calls these via `Op::apply` when
//! a field has `#[account(init)]` with a metadata/master_edition behavior.

use {
    crate::state::{MasterEditionAccount, MetadataAccount},
    quasar_lang::prelude::*,
};

/// Init params for metadata account creation via CPI.
///
/// The derive constructs `Default` (Unset) and the metadata behavior fills
/// the `Create` variant via `AccountBehavior::set_init_param`.
#[derive(Default)]
pub enum MetadataInitParams<'a> {
    /// No behavior has filled init params yet.
    #[default]
    Unset,
    /// Create metadata via `create_metadata_accounts_v3` CPI.
    Create {
        program: &'a AccountView,
        mint: &'a AccountView,
        mint_authority: &'a AccountView,
        update_authority: &'a AccountView,
        system_program: &'a AccountView,
        rent: &'a AccountView,
        name: &'a str,
        symbol: &'a str,
        uri: &'a str,
        seller_fee_basis_points: u16,
        is_mutable: bool,
    },
}

impl quasar_lang::account_init::AccountInit for MetadataAccount {
    type InitParams<'a> = MetadataInitParams<'a>;
    const DEFAULT_INIT_PARAMS_VALID: bool = false;

    #[inline(always)]
    fn init<'a, R: quasar_lang::ops::RentAccess>(
        ctx: quasar_lang::account_init::InitCtx<'a, R>,
        params: &Self::InitParams<'a>,
    ) -> quasar_lang::__solana_program_error::ProgramResult {
        match params {
            MetadataInitParams::Unset => Err(ProgramError::InvalidArgument),
            MetadataInitParams::Create {
                program,
                mint,
                mint_authority,
                update_authority,
                system_program,
                rent,
                name,
                symbol,
                uri,
                seller_fee_basis_points,
                is_mutable,
            } => {
                crate::validate::validate_metadata_program(program)?;
                crate::instructions::create_metadata::create_metadata_accounts_v3(
                    program,
                    ctx.target,
                    mint,
                    mint_authority,
                    ctx.payer,
                    update_authority,
                    system_program,
                    rent,
                    *name,
                    *symbol,
                    *uri,
                    *seller_fee_basis_points,
                    *is_mutable,
                    true, // update_authority_is_signer
                )?
                .invoke()
            }
        }
    }
}

/// Init params for master edition account creation via CPI.
#[derive(Default)]
pub enum MasterEditionInitParams<'a> {
    /// No behavior has filled init params yet.
    #[default]
    Unset,
    /// Create master edition via `create_master_edition_v3` CPI.
    Create {
        program: &'a AccountView,
        mint: &'a AccountView,
        update_authority: &'a AccountView,
        mint_authority: &'a AccountView,
        metadata: &'a AccountView,
        token_program: &'a AccountView,
        system_program: &'a AccountView,
        rent: &'a AccountView,
        max_supply: Option<u64>,
    },
}

impl quasar_lang::account_init::AccountInit for MasterEditionAccount {
    type InitParams<'a> = MasterEditionInitParams<'a>;
    const DEFAULT_INIT_PARAMS_VALID: bool = false;

    #[inline(always)]
    fn init<'a, R: quasar_lang::ops::RentAccess>(
        ctx: quasar_lang::account_init::InitCtx<'a, R>,
        params: &Self::InitParams<'a>,
    ) -> quasar_lang::__solana_program_error::ProgramResult {
        match params {
            MasterEditionInitParams::Unset => Err(ProgramError::InvalidArgument),
            MasterEditionInitParams::Create {
                program,
                mint,
                update_authority,
                mint_authority,
                metadata,
                token_program,
                system_program,
                rent,
                max_supply,
            } => {
                crate::validate::validate_metadata_program(program)?;
                crate::instructions::create_master_edition::create_master_edition_v3(
                    program,
                    ctx.target,
                    mint,
                    update_authority,
                    mint_authority,
                    ctx.payer,
                    metadata,
                    token_program,
                    system_program,
                    rent,
                    *max_supply,
                )
                .invoke()
            }
        }
    }
}

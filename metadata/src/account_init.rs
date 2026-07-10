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
        /// Token Metadata program account.
        program: &'a AccountView,
        /// Mint described by the metadata account.
        mint: &'a AccountView,
        /// Current mint authority signer.
        mint_authority: &'a AccountView,
        /// Authority permitted to update the metadata.
        update_authority: &'a AccountView,
        /// System Program account.
        system_program: &'a AccountView,
        /// Rent sysvar account required by the CPI interface.
        rent: &'a AccountView,
        /// Display name written to metadata.
        name: &'a str,
        /// Token symbol written to metadata.
        symbol: &'a str,
        /// Metadata JSON URI.
        uri: &'a str,
        /// Royalty fee in basis points.
        seller_fee_basis_points: u16,
        /// Whether future metadata updates are permitted.
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
        /// Token Metadata program account.
        program: &'a AccountView,
        /// Mint receiving the master edition.
        mint: &'a AccountView,
        /// Metadata update authority signer.
        update_authority: &'a AccountView,
        /// Mint authority signer.
        mint_authority: &'a AccountView,
        /// Existing metadata account for the mint.
        metadata: &'a AccountView,
        /// SPL Token program account.
        token_program: &'a AccountView,
        /// System Program account.
        system_program: &'a AccountView,
        /// Rent sysvar account required by the CPI interface.
        rent: &'a AccountView,
        /// Optional maximum number of printable editions.
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

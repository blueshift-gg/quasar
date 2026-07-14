//! SPL Token program integration for Quasar.
//!
//! Provides zero-copy account types and CPI methods for the SPL Token program
//! and Token-2022 (Token Extensions) program.
//!
//! # Account types
//!
//! | Type | Owner check | Deref target | Use when |
//! |------|-------------|--------------|----------|
//! | `Account<Token>` | SPL Token only | [`TokenDataZc`] | Token accounts (incl. ATAs) for SPL Token |
//! | `Account<Mint>` | SPL Token only | [`MintDataZc`] | Mint owned by Token |
//! | `InterfaceAccount<Token>` | SPL Token **or** Token-2022 | [`TokenDataZc`] | Token accounts (incl. ATAs) for either program |
//! | `InterfaceAccount<Mint>` | SPL Token **or** Token-2022 | [`MintDataZc`] | Mint from either program |
//!
//! # Program types
//!
//! | Type | Accepts | Use when |
//! |------|---------|----------|
//! | `Program<TokenProgram>` | SPL Token only | CPI to Token program |
//! | `Program<Token2022Program>` | Token-2022 only | CPI to Token-2022 program |
//! | `Interface<TokenInterface>` | SPL Token **or** Token-2022 | CPI to either program |
//!
//! # CPI methods
//!
//! `Program<TokenProgram>`, `Program<Token2022Program>`, and
//! `Interface<TokenInterface>` expose the same CPI methods. All methods return
//! a `CpiCall` that can be invoked with `.invoke()`
//! or `.invoke_signed()`:
//!
//! ```ignore
//! ctx.accounts.token_program
//!     .transfer(&from, &to, &authority, amount)
//!     .invoke();
//! ```
//!
//! # Token lifecycle
//!
//! Use `#[account(init)]` to auto-create token accounts, mints, and ATAs.
//! The derive macro handles `create_account` + `initialize_*` CPI calls.
//!
//! For closing, use `close_account` on the token program directly:
//!
//! ```ignore
//! self.token_program.close_account(&self.vault, &self.maker, &self.escrow)
//!     .invoke_signed(&seeds);
//! ```

#![no_std]
#![warn(missing_docs)]

/// Implements `AccountInit` for a token account type (Token / Token2022).
/// Both dispatch to the same init_token_account / init_ata helpers.
macro_rules! impl_token_account_init {
    ($ty:ty) => {
        impl quasar_lang::account_init::AccountInit for $ty {
            type InitParams<'a> = crate::token::TokenInitKind<'a>;
            const DEFAULT_INIT_PARAMS_VALID: bool = false;

            #[inline(always)]
            fn init<'a, R: quasar_lang::ops::RentAccess>(
                ctx: quasar_lang::account_init::InitCtx<'a, R>,
                params: &Self::InitParams<'a>,
            ) -> Result<(), ProgramError> {
                match params {
                    crate::token::TokenInitKind::Unset => Err(ProgramError::InvalidArgument),
                    crate::token::TokenInitKind::Token {
                        mint,
                        authority,
                        token_program,
                    } => crate::init::init_token_account(
                        ctx.payer,
                        ctx.target,
                        token_program,
                        mint,
                        authority,
                        ctx.signers,
                        ctx.rent.get()?,
                    ),
                    crate::token::TokenInitKind::AssociatedToken {
                        mint,
                        authority,
                        token_program,
                        system_program,
                        ata_program,
                        idempotent,
                    } => {
                        crate::validate::validate_ata_program_id(ata_program)?;
                        crate::validate::validate_token_program_id(token_program)?;
                        crate::validate::validate_system_program_id(system_program)?;
                        crate::init::init_ata(
                            ata_program,
                            ctx.payer,
                            ctx.target,
                            authority,
                            mint,
                            system_program,
                            token_program,
                            *idempotent,
                        )
                    }
                }
            }
        }
    };
}

/// Implements `AccountInit` for a mint account type (Mint / Mint2022).
/// Both dispatch to the same init_mint_account helper.
macro_rules! impl_mint_account_init {
    ($ty:ty) => {
        impl quasar_lang::account_init::AccountInit for $ty {
            type InitParams<'a> = crate::token::MintInitParams<'a>;
            const DEFAULT_INIT_PARAMS_VALID: bool = false;

            #[inline(always)]
            fn init<'a, R: quasar_lang::ops::RentAccess>(
                ctx: quasar_lang::account_init::InitCtx<'a, R>,
                params: &Self::InitParams<'a>,
            ) -> Result<(), ProgramError> {
                match params {
                    crate::token::MintInitParams::Unset => Err(ProgramError::InvalidArgument),
                    crate::token::MintInitParams::Mint {
                        decimals,
                        authority,
                        freeze_authority,
                        token_program,
                    } => crate::init::init_mint_account(
                        ctx.payer,
                        ctx.target,
                        token_program,
                        *decimals,
                        authority,
                        *freeze_authority,
                        ctx.signers,
                        ctx.rent.get()?,
                    ),
                }
            }
        }
    };
}

/// Behavior modules for `#[derive(Accounts)]` integration.
pub mod accounts;
mod associated_token;
mod constants;
mod exit;
mod init;
mod instructions;
mod interface;
/// Convenience re-exports for SPL programs.
pub mod prelude;
mod token;
mod token_2022;
mod validate;

use quasar_lang::{
    accounts::{Account, DeferredInit},
    prelude::{AccountView, ProgramError},
};
pub use {
    associated_token::{
        create as ata_create, create_idempotent as ata_create_idempotent,
        get_associated_token_address_const, get_associated_token_address_with_program_const,
        AssociatedTokenCpi, AssociatedTokenProgram,
    },
    constants::{ATA_PROGRAM_ID, SPL_TOKEN_ID, TOKEN_2022_ID},
    instructions::TokenCpi,
    interface::TokenInterface,
    quasar_lang::prelude::InterfaceAccount,
    token::{
        Mint, MintData, MintDataZc, MintInitParams, Token, TokenData, TokenDataZc, TokenInitKind,
        TokenProgram,
    },
    token_2022::{Mint2022, Token2022, Token2022Program},
    validate::{validate_ata, validate_mint_with_freeze, validate_token_account, FreezeCheck},
};

macro_rules! impl_deferred_init {
    ($wrapper:ty, $params:ty) => {
        impl<'a> DeferredInit<$wrapper> for $params {
            #[inline(always)]
            fn init_uninit<'target>(
                self,
                target: &'target mut AccountView,
                payer: &AccountView,
                signers: &[quasar_lang::cpi::Signer<'_, '_>],
            ) -> Result<&'target mut $wrapper, ProgramError> {
                if quasar_lang::utils::hint::unlikely(!quasar_lang::is_system_program(
                    target.owner(),
                )) {
                    return Err(ProgramError::AccountAlreadyInitialized);
                }

                let rent =
                    <quasar_lang::sysvars::rent::Rent as quasar_lang::sysvars::Sysvar>::get()?;
                <$wrapper as quasar_lang::account_init::AccountInit>::init(
                    quasar_lang::account_init::InitCtx {
                        payer,
                        target,
                        program_id: &quasar_lang::cpi::system::ID,
                        space: <$wrapper as quasar_lang::account_layout::AccountLayout>::DATA_SIZE
                            as u64,
                        signers,
                        rent: &rent,
                    },
                    &self,
                )?;
                <$wrapper as quasar_lang::account_load::AccountLoad>::check(target)?;
                // SAFETY: The account was just initialized and re-checked as the target
                // wrapper.
                Ok(unsafe { <$wrapper>::from_account_view_unchecked_mut(target) })
            }
        }
    };
}

impl_deferred_init!(Account<Token>, TokenInitKind<'a>);
impl_deferred_init!(Account<Token2022>, TokenInitKind<'a>);
impl_deferred_init!(InterfaceAccount<Token>, TokenInitKind<'a>);
impl_deferred_init!(Account<Mint>, MintInitParams<'a>);
impl_deferred_init!(Account<Mint2022>, MintInitParams<'a>);
impl_deferred_init!(InterfaceAccount<Mint>, MintInitParams<'a>);

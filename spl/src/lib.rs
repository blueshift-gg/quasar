//! SPL Token program integration for Quasar.
//!
//! Provides zero-copy account types and CPI methods for the SPL Token program
//! and Token-2022 (Token Extensions) program.
//!
//! # Account types
//!
//! | Type | Owner check | Deref target | Use when |
//! |------|-------------|--------------|----------|
//! | `Account<Token>` | SPL Token only | [`TokenAccountState`] | Program only supports Token |
//! | `Account<Mint>` | SPL Token only | [`MintAccountState`] | Mint owned by Token |
//! | `InterfaceAccount<Token>` | SPL Token **or** Token-2022 | [`TokenAccountState`] | Program supports both |
//! | `InterfaceAccount<Mint>` | SPL Token **or** Token-2022 | [`MintAccountState`] | Mint from either program |
//!
//! # Program types
//!
//! | Type | Accepts | Use when |
//! |------|---------|----------|
//! | `Program<Token>` | SPL Token only | CPI to Token program |
//! | [`TokenInterface`] | SPL Token **or** Token-2022 | CPI to either program |
//!
//! # CPI methods
//!
//! Both `Program<Token>` and [`TokenInterface`] expose the same CPI methods.
//! All methods return a `CpiCall` that can be invoked with `.invoke()` or
//! `.invoke_signed()`:
//!
//! ```ignore
//! ctx.accounts.token_program
//!     .transfer(&from, &to, &authority, amount)
//!     .invoke()?;
//! ```
//!
//! # Token lifecycle
//!
//! Use `#[account(init)]` to auto-create token accounts, mints, and ATAs:
//!
//! ```ignore
//! #[account(init, payer = payer, token::mint = mint, token::authority = authority)]
//! pub token_account: &'info mut Account<Token>,
//!
//! // Or skip if already initialized (checks owner == system_program)
//! self.new_token.init_if_needed(
//!     self.system_program,
//!     self.payer,
//!     self.token_program,
//!     self.mint,
//!     self.owner.address(),
//!     Some(&*self.rent), // or None to fetch via syscall
//! )?;
//! ```
//!
//! For closing, use the [`TokenClose`] trait on `Account<T>`:
//!
//! ```ignore
//! self.vault.close(&self.token_program, &self.maker, &self.escrow)
//!     .invoke_signed(&seeds)?;
//! ```

#![no_std]

/// Implements `CheckOwner` for a type that is owned by exactly one program.
///
/// These types intentionally do NOT implement `Owner` — that would expose
/// `Account<T>::close()` which performs a direct lamport drain. Token/mint
/// accounts are owned by the SPL Token program, not the calling program,
/// so the direct close would always fail at runtime. Instead, use the
/// CPI-based `TokenClose` trait.
macro_rules! impl_single_owner {
    ($ty:ty, $id:expr, $target:ty) => {
        unsafe impl StaticView for $ty {}

        impl AsAccountView for $ty {
            #[inline(always)]
            fn to_account_view(&self) -> &AccountView {
                &self.__view
            }
        }

        impl AccountCheck for $ty {
            #[inline(always)]
            fn check(view: &AccountView) -> Result<(), ProgramError> {
                if view.data_len() < <$target>::LEN {
                    return Err(ProgramError::AccountDataTooSmall);
                }
                Ok(())
            }
        }

        impl CheckOwner for $ty {
            #[inline(always)]
            fn check_owner(view: &AccountView) -> Result<(), ProgramError> {
                if !quasar_core::keys_eq(unsafe { view.owner() }, &$id) {
                    return Err(ProgramError::IllegalOwner);
                }
                Ok(())
            }
        }

        impl core::ops::Deref for $ty {
            type Target = $target;

            #[inline(always)]
            fn deref(&self) -> &Self::Target {
                unsafe { &*(self.__view.data_ptr() as *const $target) }
            }
        }

        impl core::ops::DerefMut for $ty {
            #[inline(always)]
            fn deref_mut(&mut self) -> &mut Self::Target {
                unsafe { &mut *(self.__view.data_ptr() as *mut $target) }
            }
        }

        impl ZeroCopyDeref for $ty {
            type Target = $target;

            #[inline(always)]
            fn deref_from(view: &AccountView) -> &Self::Target {
                unsafe { &*(view.data_ptr() as *const $target) }
            }

            #[inline(always)]
            fn deref_from_mut(view: &AccountView) -> &mut Self::Target {
                unsafe { &mut *(view.data_ptr() as *mut $target) }
            }
        }
    };
}

mod associated_token;
mod helpers;
mod instructions;
mod interface;
#[cfg(feature = "metadata")]
pub mod metadata;
mod state;
mod token;
mod token_2022;

pub use associated_token::{
    create as ata_create, create_idempotent as ata_create_idempotent, get_associated_token_address,
    get_associated_token_address_const, get_associated_token_address_with_program,
    get_associated_token_address_with_program_const, validate_ata, AssociatedToken,
    AssociatedTokenProgram, InitAssociatedToken,
};
pub use helpers::close::TokenClose;
pub use helpers::constants::{ATA_PROGRAM_ID, SPL_TOKEN_ID, TOKEN_2022_ID};
pub use helpers::init::{validate_mint, validate_token_account, InitMint, InitToken};
pub use instructions::{initialize_account3, initialize_mint2, TokenCpi};
pub use interface::{InterfaceAccount, TokenInterface};
pub use state::{MintAccountState, TokenAccountState};
pub use token::{Mint, Token};
pub use token_2022::{Mint2022, Token2022};

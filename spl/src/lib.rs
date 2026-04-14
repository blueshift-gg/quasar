//! SPL Token program integration for Quasar.
//!
//! Provides zero-copy account types and CPI methods for the SPL Token program
//! and Token-2022 (Token Extensions) program.
//!
//! # Account types
//!
//! | Type | Owner check | Deref target | Use when |
//! |------|-------------|--------------|----------|
//! | `Account<Token>` | SPL Token only | [`TokenAccountState`] | Token accounts (incl. ATAs) for SPL Token |
//! | `Account<Mint>` | SPL Token only | [`MintAccountState`] | Mint owned by Token |
//! | `InterfaceAccount<Token>` | SPL Token **or** Token-2022 | [`TokenAccountState`] | Token accounts (incl. ATAs) for either program |
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

/// Implements the full account type contract for a type owned by a single
/// program.
///
/// Generates five trait implementations:
///
/// - `StaticView` — marks the type as having a fixed layout
/// - `AsAccountView` — provides access to the underlying `AccountView`
/// - `AccountCheck` — validates `data_len >= T::LEN`
/// - `CheckOwner` — validates `owner == $id`
/// - `Deref` / `DerefMut` → `$target` — zero-copy access to account data
/// - `ZeroCopyDeref` — enables `InterfaceAccount<T>` to deref through this type
///
/// # Safety
///
/// The `Deref` / `DerefMut` impls perform `unsafe` pointer casts from the
/// raw account data to `$target`. This is sound because:
///
/// 1. `AccountCheck::check` validated `data_len >= $target::LEN`
/// 2. `$target` is `#[repr(C)]` with alignment 1 (any pointer is valid)
/// 3. The owner check guarantees the data was written by the expected program
macro_rules! impl_program_account {
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
                if quasar_lang::utils::hint::unlikely(view.data_len() < <$target>::LEN) {
                    return Err(ProgramError::AccountDataTooSmall);
                }
                Ok(())
            }
        }

        impl CheckOwner for $ty {
            #[inline(always)]
            fn check_owner(view: &AccountView) -> Result<(), ProgramError> {
                if quasar_lang::utils::hint::unlikely(!quasar_lang::keys_eq(view.owner(), &$id)) {
                    return Err(ProgramError::IllegalOwner);
                }
                Ok(())
            }
        }

        impl core::ops::Deref for $ty {
            type Target = $target;

            #[inline(always)]
            fn deref(&self) -> &Self::Target {
                // SAFETY: `AccountCheck::check` validated `data_len >= LEN`.
                // `$target` is `#[repr(C)]` with alignment 1 — any data
                // pointer is valid.
                unsafe { &*(self.__view.data_ptr() as *const $target) }
            }
        }

        impl core::ops::DerefMut for $ty {
            #[inline(always)]
            fn deref_mut(&mut self) -> &mut Self::Target {
                // SAFETY: Same as Deref — length validated, alignment 1.
                // Mutability checked by the writable constraint.
                unsafe { &mut *(self.__view.data_mut_ptr() as *mut $target) }
            }
        }

        impl ZeroCopyDeref for $ty {
            type Target = $target;

            #[inline(always)]
            unsafe fn deref_from(view: &AccountView) -> &Self::Target {
                // SAFETY: Caller ensures `view.data_len() >= LEN`.
                // `$target` is `#[repr(C)]` with alignment 1.
                &*(view.data_ptr() as *const $target)
            }

            #[inline(always)]
            unsafe fn deref_from_mut(view: &mut AccountView) -> &mut Self::Target {
                // SAFETY: Same as `deref_from` — caller ensures length
                // and writable.
                &mut *(view.data_mut_ptr() as *mut $target)
            }
        }
    };
}

mod associated_token;
mod constants;
mod instructions;
mod interface;
#[cfg(feature = "metadata")]
pub mod metadata;
mod state;
mod token;
mod token_2022;
mod validate;

pub use {
    associated_token::{
        create as ata_create, create_idempotent as ata_create_idempotent,
        get_associated_token_address_const, get_associated_token_address_with_program_const,
        AssociatedTokenProgram,
    },
    constants::{ATA_PROGRAM_ID, SPL_TOKEN_ID, TOKEN_2022_ID},
    instructions::{initialize_account3, initialize_mint2, TokenCpi},
    interface::TokenInterface,
    // Re-export from quasar_lang for backward compatibility.
    quasar_lang::accounts::interface_account::InterfaceAccount,
    state::{COption, MintAccountState, TokenAccountState},
    token::{Mint, Token},
    token_2022::{Mint2022, Token2022},
    validate::{validate_ata, validate_mint, validate_token_account},
};

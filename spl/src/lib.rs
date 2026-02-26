//! SPL Token program integration for Quasar.
//!
//! Provides zero-copy account types and CPI methods for the SPL Token program
//! and Token-2022 (Token Extensions) program.
//!
//! # Account types
//!
//! | Type | Owner check | Deref target | Use when |
//! |------|-------------|--------------|----------|
//! | `Account<TokenAccount>` | SPL Token only | [`TokenAccountState`] | Program only supports Token |
//! | `Account<InterfaceTokenAccount>` | SPL Token **or** Token-2022 | [`TokenAccountState`] | Program supports both |
//! | `Account<MintAccount>` | SPL Token only | [`MintAccountState`] | Mint owned by Token |
//! | `Account<InterfaceMintAccount>` | SPL Token **or** Token-2022 | [`MintAccountState`] | Mint from either program |
//!
//! # Program types
//!
//! | Type | Accepts | Use when |
//! |------|---------|----------|
//! | [`TokenProgram`] | SPL Token only | CPI to Token program |
//! | [`TokenInterface`] | SPL Token **or** Token-2022 | CPI to either program |
//!
//! # CPI methods
//!
//! Both [`TokenProgram`] and [`TokenInterface`] expose the same CPI methods.
//! All methods return a [`CpiCall`] that can be invoked with `.invoke()` or
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
//! Extension traits on `Initialize<T>` provide init helpers. Pass an optional
//! `&Rent` sysvar — when `None`, rent is fetched via the `Rent::get()` syscall:
//!
//! ```ignore
//! // Create + initialize a token account via InitToken trait
//! self.new_token.init(
//!     self.system_program,
//!     self.payer,
//!     self.token_program,
//!     self.mint,
//!     self.owner.address(),
//!     None, // fetches Rent sysvar via syscall
//! )?;
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
//!     .invoke_signed(&[seeds])?;
//! ```

#![no_std]

/// Implements `CheckOwner` for a type that is owned by exactly one program.
///
/// These types intentionally do NOT implement [`Owner`] — that would expose
/// `Account<T>::close()` which performs a direct lamport drain. Token/mint
/// accounts are owned by the SPL Token program, not the calling program,
/// so the direct close would always fail at runtime. Instead, use the
/// CPI-based [`TokenClose`] trait.
macro_rules! impl_single_owner {
    ($ty:ty, $id:expr, $target:ty) => {
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
                if !view.owned_by(&$id) {
                    return Err(ProgramError::IllegalOwner);
                }
                Ok(())
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

mod close;
mod constants;
mod cpi;
mod init;
mod interface;
mod state;
mod token;
mod token_2022;

pub use close::TokenClose;
pub use constants::{SPL_TOKEN_ID, TOKEN_2022_ID};
pub use cpi::TokenCpi;
pub use init::{InitMint, InitToken};
pub use interface::{InterfaceMintAccount, InterfaceTokenAccount, TokenInterface};
pub use state::{MintAccountState, TokenAccountState};
pub use token::{MintAccount, TokenAccount, TokenProgram};
pub use token_2022::{Mint2022Account, Token2022Account, Token2022Program};

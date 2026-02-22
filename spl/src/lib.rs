//! SPL Token program integration for Quasar.
//!
//! Provides zero-copy account types and CPI methods for the SPL Token program.
//!
//! # Account types
//!
//! - [`TokenProgram`] — program account type for CPI calls
//! - [`TokenAccount`] — token account with zero-copy [`Deref`](core::ops::Deref)
//!   to [`TokenAccountState`]
//! - [`TokenAccountState`] — `#[repr(C)]` layout of SPL token account data
//!
//! # CPI methods
//!
//! All methods return a [`CpiCall`] that can be invoked with `.invoke()` or
//! `.invoke_signed()`:
//!
//! ```ignore
//! ctx.accounts.token_program
//!     .transfer(&from, &to, &authority, amount)
//!     .invoke()?;
//! ```
//!
//! # Known limitations
//!
//! - Token-2022 is not yet supported. `TokenAccount` validates the SPL Token
//!   program as owner; Token-2022 accounts will fail the owner check.

#![no_std]

mod token;

use quasar_core::checks;
use quasar_core::cpi::{CpiCall, InstructionAccount};
use quasar_core::prelude::*;

pub use token::TokenAccountState;

/// SPL Token program address.
pub const SPL_TOKEN_ID: Address = Address::new_from_array([
    6, 221, 246, 225, 215, 101, 161, 147, 217, 203, 225, 70, 206, 235, 121, 172, 28, 180, 133, 237,
    95, 91, 55, 145, 58, 140, 245, 133, 126, 255, 0, 169,
]);

/// Token-2022 program address (reserved for future use).
pub const TOKEN_2022_ID: Address = Address::new_from_array([
    6, 221, 246, 225, 238, 130, 236, 193, 200, 168, 65, 2, 106, 93, 64, 59, 117, 155, 197, 130,
    200, 159, 250, 31, 239, 205, 35, 168, 238, 94, 220, 87,
]);

// TODO: Support Token-2022 — needs multi-address check in define_account! or custom from_account_view
quasar_core::define_account!(pub struct TokenProgram => [checks::Executable, checks::Address]);

impl Program for TokenProgram {
    const ID: Address = SPL_TOKEN_ID;
}

/// SPL Token account type with zero-copy access to [`TokenAccountState`].
pub struct TokenAccount;

impl AccountCheck for TokenAccount {}

impl Owner for TokenAccount {
    // TODO: Only validates SPL Token owner. Token-2022 accounts will fail owner check.
    const OWNER: Address = SPL_TOKEN_ID;
}

impl ZeroCopyDeref for TokenAccount {
    type Target = TokenAccountState;

    #[inline(always)]
    fn deref_from(view: &AccountView) -> &Self::Target {
        unsafe { &*(view.data_ptr() as *const TokenAccountState) }
    }

    #[inline(always)]
    fn deref_from_mut(view: &AccountView) -> &mut Self::Target {
        unsafe { &mut *(view.data_ptr() as *mut TokenAccountState) }
    }
}

// --- CPI Methods ---

impl TokenProgram {
    /// Transfer tokens between accounts.
    #[inline(always)]
    pub fn transfer<'a>(
        &'a self,
        from: &'a impl AsAccountView,
        to: &'a impl AsAccountView,
        authority: &'a impl AsAccountView,
        amount: impl Into<u64>,
    ) -> CpiCall<'a, 3, 9> {
        let from = from.to_account_view();
        let to = to.to_account_view();
        let authority = authority.to_account_view();
        let amount: u64 = amount.into();

        let mut data = [0u8; 9];
        data[0] = 3;
        data[1..9].copy_from_slice(&amount.to_le_bytes());

        CpiCall::new(
            self.address(),
            [
                InstructionAccount::writable(from.address()),
                InstructionAccount::writable(to.address()),
                InstructionAccount::readonly_signer(authority.address()),
            ],
            [from, to, authority],
            data,
        )
    }

    /// Transfer tokens with decimal verification.
    #[inline(always)]
    pub fn transfer_checked<'a>(
        &'a self,
        from: &'a impl AsAccountView,
        mint: &'a impl AsAccountView,
        to: &'a impl AsAccountView,
        authority: &'a impl AsAccountView,
        amount: impl Into<u64>,
        decimals: u8,
    ) -> CpiCall<'a, 4, 10> {
        let from = from.to_account_view();
        let mint = mint.to_account_view();
        let to = to.to_account_view();
        let authority = authority.to_account_view();
        let amount: u64 = amount.into();

        let mut data = [0u8; 10];
        data[0] = 12;
        data[1..9].copy_from_slice(&amount.to_le_bytes());
        data[9] = decimals;

        CpiCall::new(
            self.address(),
            [
                InstructionAccount::writable(from.address()),
                InstructionAccount::readonly(mint.address()),
                InstructionAccount::writable(to.address()),
                InstructionAccount::readonly_signer(authority.address()),
            ],
            [from, mint, to, authority],
            data,
        )
    }

    /// Mint tokens to an account.
    #[inline(always)]
    pub fn mint_to<'a>(
        &'a self,
        mint: &'a impl AsAccountView,
        to: &'a impl AsAccountView,
        authority: &'a impl AsAccountView,
        amount: impl Into<u64>,
    ) -> CpiCall<'a, 3, 9> {
        let mint = mint.to_account_view();
        let to = to.to_account_view();
        let authority = authority.to_account_view();
        let amount: u64 = amount.into();

        let mut data = [0u8; 9];
        data[0] = 7;
        data[1..9].copy_from_slice(&amount.to_le_bytes());

        CpiCall::new(
            self.address(),
            [
                InstructionAccount::writable(mint.address()),
                InstructionAccount::writable(to.address()),
                InstructionAccount::readonly_signer(authority.address()),
            ],
            [mint, to, authority],
            data,
        )
    }

    /// Burn tokens from an account.
    #[inline(always)]
    pub fn burn<'a>(
        &'a self,
        from: &'a impl AsAccountView,
        mint: &'a impl AsAccountView,
        authority: &'a impl AsAccountView,
        amount: impl Into<u64>,
    ) -> CpiCall<'a, 3, 9> {
        let from = from.to_account_view();
        let mint = mint.to_account_view();
        let authority = authority.to_account_view();
        let amount: u64 = amount.into();

        let mut data = [0u8; 9];
        data[0] = 8;
        data[1..9].copy_from_slice(&amount.to_le_bytes());

        CpiCall::new(
            self.address(),
            [
                InstructionAccount::writable(from.address()),
                InstructionAccount::writable(mint.address()),
                InstructionAccount::readonly_signer(authority.address()),
            ],
            [from, mint, authority],
            data,
        )
    }

    /// Approve a delegate to transfer tokens.
    #[inline(always)]
    pub fn approve<'a>(
        &'a self,
        source: &'a impl AsAccountView,
        delegate: &'a impl AsAccountView,
        authority: &'a impl AsAccountView,
        amount: impl Into<u64>,
    ) -> CpiCall<'a, 3, 9> {
        let source = source.to_account_view();
        let delegate = delegate.to_account_view();
        let authority = authority.to_account_view();
        let amount: u64 = amount.into();

        let mut data = [0u8; 9];
        data[0] = 4;
        data[1..9].copy_from_slice(&amount.to_le_bytes());

        CpiCall::new(
            self.address(),
            [
                InstructionAccount::writable(source.address()),
                InstructionAccount::readonly(delegate.address()),
                InstructionAccount::readonly_signer(authority.address()),
            ],
            [source, delegate, authority],
            data,
        )
    }

    /// Close a token account and reclaim its lamports.
    #[inline(always)]
    pub fn close_account<'a>(
        &'a self,
        account: &'a impl AsAccountView,
        destination: &'a impl AsAccountView,
        authority: &'a impl AsAccountView,
    ) -> CpiCall<'a, 3, 1> {
        let account = account.to_account_view();
        let destination = destination.to_account_view();
        let authority = authority.to_account_view();

        CpiCall::new(
            self.address(),
            [
                InstructionAccount::writable(account.address()),
                InstructionAccount::writable(destination.address()),
                InstructionAccount::readonly_signer(authority.address()),
            ],
            [account, destination, authority],
            [9],
        )
    }

    /// Revoke a delegate's authority.
    #[inline(always)]
    pub fn revoke<'a>(
        &'a self,
        source: &'a impl AsAccountView,
        authority: &'a impl AsAccountView,
    ) -> CpiCall<'a, 2, 1> {
        let source = source.to_account_view();
        let authority = authority.to_account_view();

        CpiCall::new(
            self.address(),
            [
                InstructionAccount::writable(source.address()),
                InstructionAccount::readonly_signer(authority.address()),
            ],
            [source, authority],
            [5],
        )
    }

    /// Sync the lamport balance of a native SOL token account.
    #[inline(always)]
    pub fn sync_native<'a>(&'a self, token_account: &'a impl AsAccountView) -> CpiCall<'a, 1, 1> {
        let token_account = token_account.to_account_view();

        CpiCall::new(
            self.address(),
            [InstructionAccount::writable(token_account.address())],
            [token_account],
            [17],
        )
    }
}

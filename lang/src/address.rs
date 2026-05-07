//! Unified address verification trait.
//!
//! The derive emits a single call for all `address = expr` directives:
//! ```text
//! let __addr = #expr;
//! AddressVerify::verify(&__addr, field.address(), program_id)?;
//! ```
//!
//! Implementors:
//! - `Address` / `&Address` — exact equality check
//! - `SeedSet` types (from `#[derive(Seeds)]`) — PDA derivation check

use {
    solana_account_view::AccountView, solana_address::Address, solana_program_error::ProgramError,
};

/// Unified address verification trait.
///
/// Both exact address matches (`Address`) and PDA seed specs (`SeedSet` types)
/// implement this trait, letting the derive emit a single verification call
/// regardless of the address source.
pub trait AddressVerify {
    /// Full PDA derivation check. Safe for all contexts including init.
    ///
    /// Uses `find_program_address` (on-curve check) to derive the expected
    /// address and compare. Returns the bump seed for PDAs, 0 for exact
    /// matches.
    fn verify(&self, actual: &Address, program_id: &Address) -> Result<u8, ProgramError>;

    /// Fast verification for existing, validated accounts. Skips the on-curve
    /// check and uses `keys_eq` instead of `sol_curve_validate_point`.
    ///
    /// ONLY safe when:
    /// - The account already exists on-chain (runtime verified PDA at creation)
    /// - The account type has owner + discriminator validation (Account<T>,
    ///   InterfaceAccount<T>, Migration<From,To>)
    ///
    /// NOT safe for: init fields, UncheckedAccount, SystemAccount, Signer.
    /// Default: delegates to `verify()` (full derivation).
    #[inline(always)]
    fn verify_existing(&self, actual: &Address, program_id: &Address) -> Result<u8, ProgramError> {
        self.verify(actual, program_id)
    }

    /// Existing-account verification with access to the account data.
    ///
    /// Seed-set implementations use `account` and `bump_offset` to read a
    /// stored bump. Exact-address implementations ignore them and compare
    /// directly.
    fn verify_existing_from_account(
        &self,
        actual: &Address,
        program_id: &Address,
        _account: &AccountView,
        _bump_offset: usize,
    ) -> Result<u8, ProgramError> {
        self.verify_existing(actual, program_id)
    }

    /// Run `f` with signer seeds for CPI signing.
    ///
    /// Seed arrays must live at least as long as the CPI call that consumes
    /// them. A callback lets seed-set implementations build those arrays on
    /// their own stack frame and keep them alive while `f` runs.
    fn with_signer_seeds<R>(
        &self,
        bump: &[u8],
        f: impl FnOnce(&[crate::cpi::Signer<'_, '_>]) -> R,
    ) -> R;
}

// -- Exact address match impls ------------------------------------------------

impl AddressVerify for Address {
    #[inline(always)]
    fn verify(&self, actual: &Address, _program_id: &Address) -> Result<u8, ProgramError> {
        if crate::keys_eq(self, actual) {
            Ok(0)
        } else {
            Err(ProgramError::from(
                crate::error::QuasarError::AddressMismatch,
            ))
        }
    }

    #[inline(always)]
    fn verify_existing_from_account(
        &self,
        actual: &Address,
        program_id: &Address,
        _account: &AccountView,
        _bump_offset: usize,
    ) -> Result<u8, ProgramError> {
        self.verify_existing(actual, program_id)
    }

    #[inline(always)]
    fn with_signer_seeds<R>(
        &self,
        _bump: &[u8],
        f: impl FnOnce(&[crate::cpi::Signer<'_, '_>]) -> R,
    ) -> R {
        f(&[])
    }
}

impl AddressVerify for &Address {
    #[inline(always)]
    fn verify(&self, actual: &Address, program_id: &Address) -> Result<u8, ProgramError> {
        (*self).verify(actual, program_id)
    }

    #[inline(always)]
    fn verify_existing_from_account(
        &self,
        actual: &Address,
        program_id: &Address,
        account: &AccountView,
        bump_offset: usize,
    ) -> Result<u8, ProgramError> {
        (*self).verify_existing_from_account(actual, program_id, account, bump_offset)
    }

    #[inline(always)]
    fn with_signer_seeds<R>(
        &self,
        bump: &[u8],
        f: impl FnOnce(&[crate::cpi::Signer<'_, '_>]) -> R,
    ) -> R {
        (*self).with_signer_seeds(bump, f)
    }
}

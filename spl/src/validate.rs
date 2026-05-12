//! Account validation helpers.
//!
//! Single source of truth for validating token accounts, mints, and ATAs.
//! Every error path includes an optional debug log gated behind
//! `#[cfg(feature = "debug")]` for on-chain diagnostics.

use {
    crate::token::{MintDataZc, TokenDataZc},
    quasar_lang::{prelude::*, utils::hint::unlikely},
};

#[inline(always)]
fn validate_token_program(token_program: &Address) -> Result<(), ProgramError> {
    if quasar_lang::utils::hint::unlikely(
        !quasar_lang::keys_eq(token_program, &crate::SPL_TOKEN_ID)
            && !quasar_lang::keys_eq(token_program, &crate::TOKEN_2022_ID),
    ) {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("Invalid token program");
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}

/// Validate that an existing token account has the expected mint, authority,
/// and token program ownership.
///
/// # Errors
///
/// - [`ProgramError::IllegalOwner`] — account is not owned by `token_program`.
/// - [`ProgramError::InvalidAccountData`] — data is too small, mint or
///   authority does not match.
/// - [`ProgramError::UninitializedAccount`] — the token account state is not
///   initialized.
///
/// # Safety
///
/// Performs an unchecked pointer cast to [`TokenDataZc`]. This is safe
/// because the owner and data-length checks above guarantee the account data
/// is at least `165` bytes and belongs to a token program.
/// `TokenDataZc` is `#[repr(C)]` with alignment 1.
#[inline(always)]
pub fn validate_token_account(
    view: &AccountView,
    mint: &Address,
    authority: &Address,
    token_program: Option<&Address>,
) -> Result<(), ProgramError> {
    match token_program {
        Some(tp) => validate_token_account_inner(view, mint, authority, tp, true, true),
        // No token_program means AccountLoad already verified the owner.
        // Skip BOTH program validation AND owner check (owner == owner is tautological).
        None => validate_token_account_inner(view, mint, authority, view.owner(), false, false),
    }
}

#[inline(always)]
fn validate_token_account_inner(
    view: &AccountView,
    mint: &Address,
    authority: &Address,
    token_program: &Address,
    check_program: bool,
    check_owner: bool,
) -> Result<(), ProgramError> {
    if check_program {
        validate_token_program(token_program)?;
    }
    if check_owner && unlikely(!quasar_lang::keys_eq(view.owner(), token_program)) {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("validate_token_account: wrong program owner");
        return Err(ProgramError::IllegalOwner);
    }
    if unlikely(view.data_len() < 165) {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("validate_token_account: data too small");
        return Err(ProgramError::InvalidAccountData);
    }
    // SAFETY: Owner is a token program and `data_len >= LEN` checked
    // above. `TokenDataZc` is `#[repr(C)]` with alignment 1.
    let state = unsafe { &*(view.data_ptr() as *const TokenDataZc) };
    if unlikely(!state.coption_tags_valid()) {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("validate_token_account: invalid option tag");
        return Err(ProgramError::InvalidAccountData);
    }
    if unlikely(!state.state_valid()) {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("validate_token_account: invalid state");
        return Err(ProgramError::InvalidAccountData);
    }
    if unlikely(!state.is_initialized()) {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("validate_token_account: not initialized");
        return Err(ProgramError::UninitializedAccount);
    }
    if unlikely(!quasar_lang::keys_eq(state.mint(), mint)) {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("validate_token_account: mint mismatch");
        return Err(ProgramError::InvalidAccountData);
    }
    if unlikely(!quasar_lang::keys_eq(state.owner(), authority)) {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("validate_token_account: authority mismatch");
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(())
}

/// Validate that an existing mint account matches the provided parameters.
///
/// # Errors
///
/// - [`ProgramError::IllegalOwner`] — account is not owned by `token_program`.
/// - [`ProgramError::InvalidAccountData`] — data is too small, mint authority
///   or decimals do not match, or freeze authority state is unexpected.
/// - [`ProgramError::UninitializedAccount`] — the mint state is not
///   initialized.
///
/// # Safety
/// Three-state freeze authority check for validate_mint_with_freeze.
pub enum FreezeCheck<'a> {
    /// Omitted by user — skip check entirely.
    Skip,
    /// Assert no freeze authority.
    AssertNone,
    /// Assert freeze authority matches.
    AssertEquals(&'a Address),
}

/// Validate a mint with explicit freeze_authority check semantics.
#[inline(always)]
pub fn validate_mint_with_freeze(
    view: &AccountView,
    mint_authority: &Address,
    decimals: Option<u8>,
    freeze: FreezeCheck<'_>,
    token_program: Option<&Address>,
) -> Result<(), ProgramError> {
    if let Some(tp) = token_program {
        validate_token_program(tp)?;
        if unlikely(!quasar_lang::keys_eq(view.owner(), tp)) {
            #[cfg(feature = "debug")]
            quasar_lang::prelude::log("validate_mint: wrong program owner");
            return Err(ProgramError::IllegalOwner);
        }
    }
    if unlikely(view.data_len() < 82) {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("validate_mint: data too small");
        return Err(ProgramError::InvalidAccountData);
    }
    let state = unsafe { &*(view.data_ptr() as *const MintDataZc) };
    if unlikely(!state.coption_tags_valid()) {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("validate_mint: invalid option tag");
        return Err(ProgramError::InvalidAccountData);
    }
    if unlikely(!state.initialized_flag_valid()) {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("validate_mint: invalid initialized flag");
        return Err(ProgramError::InvalidAccountData);
    }
    if unlikely(!state.is_initialized()) {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("validate_mint: not initialized");
        return Err(ProgramError::UninitializedAccount);
    }
    if unlikely(
        !state.has_mint_authority()
            || !quasar_lang::keys_eq(state.mint_authority_unchecked(), mint_authority),
    ) {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("validate_mint: authority mismatch");
        return Err(ProgramError::InvalidAccountData);
    }
    if let Some(expected_decimals) = decimals {
        if unlikely(state.decimals() != expected_decimals) {
            #[cfg(feature = "debug")]
            quasar_lang::prelude::log("validate_mint: decimals mismatch");
            return Err(ProgramError::InvalidAccountData);
        }
    }
    match freeze {
        FreezeCheck::Skip => {}
        FreezeCheck::AssertNone => {
            if unlikely(state.has_freeze_authority()) {
                #[cfg(feature = "debug")]
                quasar_lang::prelude::log("validate_mint: freeze authority mismatch");
                return Err(ProgramError::InvalidAccountData);
            }
        }
        FreezeCheck::AssertEquals(expected) => {
            if unlikely(
                !state.has_freeze_authority()
                    || !quasar_lang::keys_eq(state.freeze_authority_unchecked(), expected),
            ) {
                #[cfg(feature = "debug")]
                quasar_lang::prelude::log("validate_mint: freeze authority mismatch");
                return Err(ProgramError::InvalidAccountData);
            }
        }
    }
    Ok(())
}

/// Validate that an account is the correct associated token account (ATA) for
/// a wallet and mint.
///
/// 1. Derives the expected ATA address from `wallet` + `mint` +
///    `token_program`.
/// 2. Checks the derived address matches the account's address.
/// 3. Delegates to [`validate_token_account`] for data validation.
///
/// # Errors
///
/// - [`ProgramError::InvalidSeeds`] — derived address does not match.
/// - All errors from [`validate_token_account`].
#[inline(always)]
pub fn validate_ata(
    view: &AccountView,
    wallet: &Address,
    mint: &Address,
    token_program: &Address,
) -> Result<(), ProgramError> {
    // The ATA already exists in the transaction (non-init path), which means
    // the ATA program created it and the runtime verified it's off-curve.
    // Use find_bump_for_address (keys_eq) instead of based_try_find_program_address
    // (on-curve check) to save ~90 CU per attempt.
    let seeds = [wallet.as_ref(), token_program.as_ref(), mint.as_ref()];
    quasar_lang::pda::find_bump_for_address(
        &seeds,
        &crate::constants::ATA_PROGRAM_ID,
        view.address(),
    )
    .map_err(|_| {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("validate_ata: address mismatch");
        ProgramError::InvalidSeeds
    })?;
    // The PDA derivation above already proved token_program is correct
    // (it's a seed in the ATA address). Skip the redundant
    // validate_token_program check inside validate_token_account.
    validate_token_account_inner(view, mint, wallet, token_program, false, true)
}

// ---------------------------------------------------------------------------
// Program ID validation for ops
// ---------------------------------------------------------------------------

#[inline(always)]
pub(crate) fn validate_token_program_id(view: &AccountView) -> Result<(), ProgramError> {
    validate_token_program(view.address())
}

#[inline(always)]
pub(crate) fn validate_ata_program_id(view: &AccountView) -> Result<(), ProgramError> {
    if unlikely(!quasar_lang::keys_eq(
        view.address(),
        &crate::constants::ATA_PROGRAM_ID,
    )) {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("Invalid ATA program");
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}

#[inline(always)]
pub(crate) fn validate_system_program_id(view: &AccountView) -> Result<(), ProgramError> {
    if unlikely(!quasar_lang::is_system_program(view.address())) {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("Invalid system program");
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Kani model-checking proof harnesses
// ---------------------------------------------------------------------------

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Prove 165 equals the actual struct size.
    /// This is the constant used in the `data_len < LEN` guard (line 68)
    /// before the pointer cast at line 75.
    #[kani::proof]
    fn token_account_len_matches_sizeof() {
        assert!(165 == core::mem::size_of::<TokenDataZc>());
    }

    /// Prove 82 equals the actual struct size.
    /// This is the constant used in the `data_len < LEN` guard (line 128)
    /// before the pointer cast at line 135.
    #[kani::proof]
    fn mint_account_len_matches_sizeof() {
        assert!(82 == core::mem::size_of::<MintDataZc>());
    }

    /// Prove: for any `data_len >= 165`, the data
    /// covers the full struct — i.e. `data_len >=
    /// size_of::<TokenDataZc>()`. This verifies the runtime guard is
    /// sufficient for a safe pointer cast.
    #[kani::proof]
    fn token_account_data_len_guard_sufficient() {
        let data_len: usize = kani::any();
        kani::assume(data_len >= 165);
        assert!(data_len >= core::mem::size_of::<TokenDataZc>());
    }

    /// Prove: for any `data_len >= 82`, the data
    /// covers the full struct — i.e. `data_len >=
    /// size_of::<MintDataZc>()`. This verifies the runtime guard is
    /// sufficient for a safe pointer cast.
    #[kani::proof]
    fn mint_account_data_len_guard_sufficient() {
        let data_len: usize = kani::any();
        kani::assume(data_len >= 82);
        assert!(data_len >= core::mem::size_of::<MintDataZc>());
    }
}

//! Account validation helpers.
//!
//! Single source of truth for validating token accounts, mints, and ATAs.
//! Every error path includes an optional debug log gated behind
//! `#[cfg(feature = "debug")]` for on-chain diagnostics.

use {
    crate::state::{MintAccountState, TokenAccountState},
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
/// Performs an unchecked pointer cast to [`TokenAccountState`]. This is safe
/// because the owner and data-length checks above guarantee the account data
/// is at least `TokenAccountState::LEN` bytes and belongs to a token program.
/// `TokenAccountState` is `#[repr(C)]` with alignment 1.
#[inline(always)]
pub fn validate_token_account(
    view: &AccountView,
    mint: &Address,
    authority: &Address,
    token_program: &Address,
) -> Result<(), ProgramError> {
    validate_token_account_inner(view, mint, authority, token_program, true)
}

#[inline(always)]
fn validate_token_account_inner(
    view: &AccountView,
    mint: &Address,
    authority: &Address,
    token_program: &Address,
    check_program: bool,
) -> Result<(), ProgramError> {
    if check_program {
        validate_token_program(token_program)?;
    }
    if unlikely(!quasar_lang::keys_eq(view.owner(), token_program)) {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("validate_token_account: wrong program owner");
        return Err(ProgramError::IllegalOwner);
    }
    if unlikely(view.data_len() < TokenAccountState::LEN) {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("validate_token_account: data too small");
        return Err(ProgramError::InvalidAccountData);
    }
    // SAFETY: Owner is a token program and `data_len >= LEN` checked
    // above. `TokenAccountState` is `#[repr(C)]` with alignment 1.
    let state = unsafe { &*(view.data_ptr() as *const TokenAccountState) };
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
///
/// Performs an unchecked pointer cast to [`MintAccountState`]. This is safe
/// because the owner and data-length checks above guarantee the account data
/// is at least `MintAccountState::LEN` bytes and belongs to a token program.
/// `MintAccountState` is `#[repr(C)]` with alignment 1.
///
/// When `freeze_authority` is `None`, the function asserts that no freeze
/// authority is set on-chain (matching Anchor's behavior).
#[inline(always)]
pub fn validate_mint(
    view: &AccountView,
    mint_authority: &Address,
    decimals: u8,
    freeze_authority: Option<&Address>,
    token_program: &Address,
) -> Result<(), ProgramError> {
    // Verify the token program is a known SPL token program.
    validate_token_program(token_program)?;
    if unlikely(!quasar_lang::keys_eq(view.owner(), token_program)) {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("validate_mint: wrong program owner");
        return Err(ProgramError::IllegalOwner);
    }
    if unlikely(view.data_len() < MintAccountState::LEN) {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("validate_mint: data too small");
        return Err(ProgramError::InvalidAccountData);
    }
    // SAFETY: Owner is a token program and `data_len >= LEN` checked
    // above. `MintAccountState` is `#[repr(C)]` with alignment 1.
    let state = unsafe { &*(view.data_ptr() as *const MintAccountState) };
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
    if unlikely(state.decimals() != decimals) {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("validate_mint: decimals mismatch");
        return Err(ProgramError::InvalidAccountData);
    }
    match freeze_authority {
        Some(expected) => {
            if unlikely(
                !state.has_freeze_authority()
                    || !quasar_lang::keys_eq(state.freeze_authority_unchecked(), expected),
            ) {
                #[cfg(feature = "debug")]
                quasar_lang::prelude::log("validate_mint: freeze authority mismatch");
                return Err(ProgramError::InvalidAccountData);
            }
        }
        None => {
            if unlikely(state.has_freeze_authority()) {
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
    validate_token_account_inner(view, mint, wallet, token_program, false)
}

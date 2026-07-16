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
/// Passing `Some(token_program)` validates that the account owner matches that
/// program and that the program is SPL Token or Token-2022. Passing `None`
/// skips those owner checks; use it only after a typed account wrapper already
/// proved the owner.
///
/// # Errors
///
/// - [`ProgramError::IllegalOwner`]: account is not owned by `token_program`.
/// - [`ProgramError::InvalidAccountData`]: data is too small, mint or authority
///   does not match.
/// - [`ProgramError::UninitializedAccount`]: the token account state is not
///   initialized.
///
/// Internally uses a zero-copy cast after validating owner and data length.
#[inline(always)]
pub fn validate_token_account(
    view: &AccountView,
    mint: &Address,
    authority: &Address,
    token_program: Option<&Address>,
) -> Result<(), ProgramError> {
    match token_program {
        Some(tp) => validate_token_account_inner(view, mint, authority, tp, true, true),
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
    if unlikely(view.data_len() < core::mem::size_of::<TokenDataZc>()) {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("validate_token_account: data too small");
        return Err(ProgramError::InvalidAccountData);
    }
    // SAFETY: The length check above covers the full token layout, and
    // `TokenDataZc` has alignment 1.
    let state = unsafe { &*(view.data_ptr() as *const TokenDataZc) };
    if unlikely(
        !state.delegate.tag_valid()
            || !state.native.tag_valid()
            || !state.close_authority.tag_valid(),
    ) {
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

/// Three-state freeze authority check for validate_mint_with_freeze.
pub enum FreezeCheck<'a> {
    /// Omitted by user; skip check entirely.
    Skip,
    /// Assert no freeze authority.
    AssertNone,
    /// Assert freeze authority matches.
    AssertEquals(&'a Address),
}

/// Validate a mint with explicit freeze_authority check semantics.
///
/// Passing `Some(token_program)` validates that the mint owner matches that
/// program and that the program is SPL Token or Token-2022. Passing `None`
/// skips those owner checks; use it only after a typed account wrapper already
/// proved the owner.
///
/// # Errors
///
/// - [`ProgramError::IllegalOwner`]: account is not owned by `token_program`.
/// - [`ProgramError::InvalidAccountData`]: data is too small, mint authority or
///   decimals do not match, or freeze authority state is unexpected.
/// - [`ProgramError::UninitializedAccount`]: the mint state is not initialized.
#[inline(always)]
pub fn validate_mint_with_freeze(
    view: &AccountView,
    mint_authority: &Address,
    decimals: Option<u8>,
    freeze: FreezeCheck<'_>,
    token_program: Option<&Address>,
) -> Result<(), ProgramError> {
    validate_mint_constraints(view, Some(mint_authority), decimals, freeze, token_program)
}

/// Internal mint validator used by account behaviors whose constraints may
/// omit the mint authority while still checking owner, decimals, or freeze
/// authority.
#[inline(always)]
pub(crate) fn validate_mint_constraints(
    view: &AccountView,
    mint_authority: Option<&Address>,
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
    if unlikely(view.data_len() < core::mem::size_of::<MintDataZc>()) {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("validate_mint: data too small");
        return Err(ProgramError::InvalidAccountData);
    }
    // SAFETY: The length check above covers the full mint layout, and
    // `MintDataZc` has alignment 1.
    let state = unsafe { &*(view.data_ptr() as *const MintDataZc) };
    if unlikely(!state.mint_authority.tag_valid() || !state.freeze_authority.tag_valid()) {
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
    if let Some(expected_authority) = mint_authority {
        if unlikely(match state.mint_authority() {
            Some(authority) => !quasar_lang::keys_eq(authority, expected_authority),
            None => true,
        }) {
            #[cfg(feature = "debug")]
            quasar_lang::prelude::log("validate_mint: authority mismatch");
            return Err(ProgramError::InvalidAccountData);
        }
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
            if unlikely(state.freeze_authority().is_some()) {
                #[cfg(feature = "debug")]
                quasar_lang::prelude::log("validate_mint: freeze authority mismatch");
                return Err(ProgramError::InvalidAccountData);
            }
        }
        FreezeCheck::AssertEquals(expected) => {
            if unlikely(match state.freeze_authority() {
                Some(authority) => !quasar_lang::keys_eq(authority, expected),
                None => true,
            }) {
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
/// - [`ProgramError::InvalidSeeds`]: derived address does not match.
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
    // Use find_bump_for_address (keys_eq) instead of try_find_program_address
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

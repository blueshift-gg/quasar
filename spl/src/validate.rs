//! Account validation helpers.
//!
//! Single source of truth for validating token accounts, mints, and ATAs.
//! Every error path includes an optional debug log gated behind
//! `#[cfg(feature = "debug")]` for on-chain diagnostics.

use {
    crate::state::{MintAccountState, TokenAccountState},
    quasar_lang::{prelude::*, utils::hint::unlikely},
};

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
    // Verify the token program is a known SPL token program.
    if unlikely(
        !quasar_lang::keys_eq(token_program, &crate::SPL_TOKEN_ID)
            && !quasar_lang::keys_eq(token_program, &crate::TOKEN_2022_ID)
    ) {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("validate_ata: token_program is not SPL Token or Token-2022");
        return Err(ProgramError::IncorrectProgramId);
    }

    let (expected, _) = crate::associated_token::get_associated_token_address_with_program(
        wallet,
        mint,
        token_program,
    );
    if unlikely(!quasar_lang::keys_eq(view.address(), &expected)) {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("validate_ata: address mismatch");
        return Err(ProgramError::InvalidSeeds);
    }
    validate_token_account(view, mint, wallet, token_program)
}

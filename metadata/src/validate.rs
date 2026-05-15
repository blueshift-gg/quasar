//! Account validation helpers.
//!
//! Single source of truth for validating metadata accounts, master editions,
//! and the Metadata program. Every error path uses `unlikely()` hints for
//! expected-success paths.
//!
//! These functions perform full validation (owner, data_len, key byte, fields)
//! so behavior modules and direct callers share one validation path.

use {
    crate::state::{
        MasterEditionPrefixZc, MetadataPrefixZc, KEY_MASTER_EDITION_V2, KEY_METADATA_V1,
    },
    quasar_lang::{prelude::*, utils::hint::unlikely},
};

/// Validate a metadata account (standalone, full checks).
///
/// Performs owner, data length, key discriminant, and field validation.
/// `mint` is mandatory (always known from PDA derivation or behavior args).
/// `update_authority` is optional; omit it to skip that check.
#[inline(always)]
pub fn validate_metadata_account(
    view: &AccountView,
    mint: &Address,
    update_authority: Option<&Address>,
) -> Result<(), ProgramError> {
    if unlikely(!quasar_lang::keys_eq(
        view.owner(),
        &crate::METADATA_PROGRAM_ID,
    )) {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("validate_metadata_account: wrong program owner");
        return Err(ProgramError::IllegalOwner);
    }
    if unlikely(view.data_len() < core::mem::size_of::<MetadataPrefixZc>()) {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("validate_metadata_account: data too small");
        return Err(ProgramError::InvalidAccountData);
    }
    // SAFETY: Owner and data length are checked above. MetadataPrefixZc has
    // alignment 1.
    let prefix = unsafe { &*(view.data_ptr() as *const MetadataPrefixZc) };
    if unlikely(prefix.key() != KEY_METADATA_V1) {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("validate_metadata_account: wrong key discriminant");
        return Err(ProgramError::InvalidAccountData);
    }
    if unlikely(!quasar_lang::keys_eq(prefix.mint(), mint)) {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("validate_metadata_account: mint mismatch");
        return Err(ProgramError::InvalidAccountData);
    }
    if let Some(expected_ua) = update_authority {
        if unlikely(!quasar_lang::keys_eq(
            prefix.update_authority(),
            expected_ua,
        )) {
            #[cfg(feature = "debug")]
            quasar_lang::prelude::log("validate_metadata_account: update_authority mismatch");
            return Err(ProgramError::InvalidAccountData);
        }
    }
    Ok(())
}

/// Validate a master edition account (standalone, full checks).
///
/// Performs owner, data length, key discriminant, and optional PDA validation.
#[inline(always)]
pub fn validate_master_edition_account(
    view: &AccountView,
    mint: Option<&Address>,
) -> Result<(), ProgramError> {
    if unlikely(!quasar_lang::keys_eq(
        view.owner(),
        &crate::METADATA_PROGRAM_ID,
    )) {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("validate_master_edition: wrong program owner");
        return Err(ProgramError::IllegalOwner);
    }
    if unlikely(view.data_len() < core::mem::size_of::<MasterEditionPrefixZc>()) {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("validate_master_edition: data too small");
        return Err(ProgramError::InvalidAccountData);
    }
    // SAFETY: Owner and data length are checked above. MasterEditionPrefixZc has
    // alignment 1.
    let prefix = unsafe { &*(view.data_ptr() as *const MasterEditionPrefixZc) };
    if unlikely(prefix.key() != KEY_MASTER_EDITION_V2) {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("validate_master_edition: wrong key discriminant");
        return Err(ProgramError::InvalidAccountData);
    }
    if unlikely(!prefix.max_supply_tag_valid()) {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("validate_master_edition: invalid max_supply tag");
        return Err(ProgramError::InvalidAccountData);
    }
    if let Some(mint_addr) = mint {
        crate::pda::verify_master_edition_address(view.address(), mint_addr)?;
    }
    Ok(())
}

/// Validate that an `AccountView` is the Metadata program.
#[inline(always)]
pub fn validate_metadata_program(view: &AccountView) -> Result<(), ProgramError> {
    if unlikely(!quasar_lang::keys_eq(
        view.address(),
        &crate::METADATA_PROGRAM_ID,
    )) {
        #[cfg(feature = "debug")]
        quasar_lang::prelude::log("Invalid metadata program");
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}

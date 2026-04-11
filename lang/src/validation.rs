//! Runtime validation helpers for account constraint checks.
//!
//! Each function is `#[inline(always)]` and 5–15 lines — independently
//! auditable, independently testable. The derive macro generates calls
//! to these functions instead of inline `quote!` blocks, so an auditor
//! reads this file once and then verifies the macro just wires them.

use {
    crate::{
        prelude::AccountView,
        traits::{AccountCheck, CheckOwner, Id, ProgramInterface},
        utils::hint::unlikely,
    },
    solana_address::Address,
    solana_program_error::ProgramError,
};

// ---------------------------------------------------------------------------
// Account owner + discriminator
// ---------------------------------------------------------------------------

/// Validate owner and discriminator for `Account<T>`.
#[inline(always)]
pub fn check_account<T: CheckOwner + AccountCheck>(
    view: &AccountView,
) -> Result<(), ProgramError> {
    T::check_owner(view)?;
    T::check(view)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Program / Sysvar / Interface address checks
// ---------------------------------------------------------------------------

/// Validate a `Program<T>` field's address matches `T::ID`.
#[inline(always)]
pub fn check_program<T: Id>(view: &AccountView) -> Result<(), ProgramError> {
    if unlikely(!crate::keys_eq(view.address(), &T::ID)) {
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}

/// Validate a `Sysvar<T>` field's address matches `T::ID`.
#[inline(always)]
pub fn check_sysvar<T: crate::sysvars::Sysvar>(
    view: &AccountView,
) -> Result<(), ProgramError> {
    if unlikely(!crate::keys_eq(view.address(), &T::ID)) {
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}

/// Validate an `Interface<T>` field matches any allowed program.
#[inline(always)]
pub fn check_interface<T: ProgramInterface>(
    view: &AccountView,
) -> Result<(), ProgramError> {
    if unlikely(!T::matches(view.address())) {
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Constraint checks (has_one, address, user constraint)
// ---------------------------------------------------------------------------

/// Validate that two addresses match (used for `has_one` constraints).
#[inline(always)]
pub fn check_has_one(
    stored: &Address,
    expected: &Address,
    error: ProgramError,
) -> Result<(), ProgramError> {
    if unlikely(!crate::keys_eq(stored, expected)) {
        return Err(error);
    }
    Ok(())
}

/// Validate that an account's address matches an expected value.
#[inline(always)]
pub fn check_address_match(
    actual: &Address,
    expected: &Address,
    error: ProgramError,
) -> Result<(), ProgramError> {
    if unlikely(!crate::keys_eq(actual, expected)) {
        return Err(error);
    }
    Ok(())
}

/// Validate a user-defined boolean constraint.
#[inline(always)]
pub fn check_constraint(
    condition: bool,
    error: ProgramError,
) -> Result<(), ProgramError> {
    if unlikely(!condition) {
        return Err(error);
    }
    Ok(())
}

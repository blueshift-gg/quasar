//! Runtime validation helpers for account constraint checks.
//!
//! Each function is `#[inline(always)]` and 5–15 lines — independently
//! auditable, independently testable. The derive macro generates calls
//! to these functions instead of inline `quote!` blocks, so an auditor
//! reads this file once and then verifies the macro just wires them.
//!
//! Debug logging: every check accepts a `_field: &str` parameter carrying
//! the field name from the accounts struct. In release builds the
//! `#[cfg(feature = "debug")]` blocks are stripped and LLVM eliminates
//! the parameter entirely — zero CU cost.

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
    _field: &str,
) -> Result<(), ProgramError> {
    T::check_owner(view).inspect_err(|_e| {
        #[cfg(feature = "debug")]
        crate::prelude::log(&::alloc::format!(
            "Owner check failed for account '{}'",
            _field
        ));
    })?;
    T::check(view).inspect_err(|_e| {
        #[cfg(feature = "debug")]
        crate::prelude::log(&::alloc::format!(
            "Discriminator check failed for account '{}': data may be uninitialized or corrupted",
            _field
        ));
    })?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Program / Sysvar / Interface address checks
// ---------------------------------------------------------------------------

/// Validate a `Program<T>` field's address matches `T::ID`.
#[inline(always)]
pub fn check_program<T: Id>(view: &AccountView, _field: &str) -> Result<(), ProgramError> {
    if unlikely(!crate::keys_eq(view.address(), &T::ID)) {
        #[cfg(feature = "debug")]
        crate::prelude::log(&::alloc::format!(
            "Incorrect program ID for account '{}': expected {}, got {}",
            _field,
            T::ID,
            view.address()
        ));
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}

/// Validate a `Sysvar<T>` field's address matches `T::ID`.
#[inline(always)]
pub fn check_sysvar<T: crate::sysvars::Sysvar>(
    view: &AccountView,
    _field: &str,
) -> Result<(), ProgramError> {
    if unlikely(!crate::keys_eq(view.address(), &T::ID)) {
        #[cfg(feature = "debug")]
        crate::prelude::log(&::alloc::format!(
            "Incorrect sysvar address for account '{}': expected {}, got {}",
            _field,
            T::ID,
            view.address()
        ));
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}

/// Validate an `Interface<T>` field matches any allowed program.
#[inline(always)]
pub fn check_interface<T: ProgramInterface>(
    view: &AccountView,
    _field: &str,
) -> Result<(), ProgramError> {
    if unlikely(!T::matches(view.address())) {
        #[cfg(feature = "debug")]
        crate::prelude::log(&::alloc::format!(
            "Program interface mismatch for account '{}': address {} does not match any allowed \
             programs",
            _field,
            view.address()
        ));
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Constraint checks (has_one, address, user constraint)
// ---------------------------------------------------------------------------

/// Validate that two addresses match (used for `has_one` and `address`
/// constraints — the check is identical).
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
pub fn check_constraint(condition: bool, error: ProgramError) -> Result<(), ProgramError> {
    if unlikely(!condition) {
        return Err(error);
    }
    Ok(())
}

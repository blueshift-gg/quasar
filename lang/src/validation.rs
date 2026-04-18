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

// ---------------------------------------------------------------------------
// Kani model-checking proof harnesses
// ---------------------------------------------------------------------------

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Prove `check_address_match` returns `Ok(())` when addresses are equal.
    #[kani::proof]
    fn check_address_match_equal_returns_ok() {
        let bytes: [u8; 32] = kani::any();
        let a = Address::new_from_array(bytes);
        let b = Address::new_from_array(bytes);
        assert!(check_address_match(&a, &b, ProgramError::InvalidArgument) == Ok(()));
    }

    /// Prove `check_address_match` returns the caller's exact error when
    /// addresses differ.
    #[kani::proof]
    fn check_address_match_unequal_returns_exact_error() {
        let a_bytes: [u8; 32] = kani::any();
        let b_bytes: [u8; 32] = kani::any();
        kani::assume(a_bytes != b_bytes);
        let a = Address::new_from_array(a_bytes);
        let b = Address::new_from_array(b_bytes);
        let code: u32 = kani::any();
        let error = ProgramError::Custom(code);
        assert!(check_address_match(&a, &b, error) == Err(ProgramError::Custom(code)));
    }

    /// Prove `check_constraint` returns `Ok(())` when condition is true.
    #[kani::proof]
    fn check_constraint_true_returns_ok() {
        let code: u32 = kani::any();
        let error = ProgramError::Custom(code);
        assert!(check_constraint(true, error) == Ok(()));
    }

    /// Prove `check_constraint` returns the caller's exact error when condition
    /// is false.
    #[kani::proof]
    fn check_constraint_false_returns_exact_error() {
        let code: u32 = kani::any();
        let error = ProgramError::Custom(code);
        assert!(check_constraint(false, error) == Err(ProgramError::Custom(code)));
    }
}

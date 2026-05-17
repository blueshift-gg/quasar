//! Runtime validation helpers for generated account constraints.
//!
//! The derive emits calls to these helpers instead of inlining equivalent
//! `quote!` blocks into each generated parser.

use {crate::utils::hint::unlikely, solana_address::Address, solana_program_error::ProgramError};

/// Validate that two addresses match (used for `has_one` and `address`
/// constraints; the check is identical).
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

#[cfg(kani)]
#[path = "../kani/validation.rs"]
mod kani_proofs;

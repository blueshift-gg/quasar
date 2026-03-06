//! Framework-level error codes.
//!
//! `QuasarError` variants start at offset 3000 to avoid collision with
//! Solana's built-in `ProgramError` codes (0–29) and leave room for
//! program-specific `#[error_code]` enums which start at 6000 by default.

use quasar_derive::error_code;
use solana_program_error::ProgramError;

#[error_code]
pub enum QuasarError {
    AccountNotInitialized = 3000,
    AccountAlreadyInitialized,
    InvalidPda,
    InvalidSeeds,
    ConstraintViolation,
    HasOneMismatch,
    InvalidDiscriminator,
    InsufficientSpace,
    AccountNotRentExempt,
    AccountOwnedByWrongProgram,
    AccountNotMutable,
    AccountNotSigner,
    AddressMismatch,
    DynamicFieldTooLong,
    RemainingAccountsOverflow,
}

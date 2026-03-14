//! Framework-level error codes.
//!
//! `QuasarError` variants start at offset 3000 to avoid collision with
//! Solana's built-in `ProgramError` codes (0–29) and leave room for
//! program-specific `#[error_code]` enums which start at 6000 by default.

use {quasar_derive::error_code, solana_program_error::ProgramError};

#[error_code]
pub enum QuasarError {
    /// Account data is all zeros or has no discriminator.
    AccountNotInitialized = 3000,
    /// Account discriminator is already set (double-init attempt).
    AccountAlreadyInitialized,
    /// PDA derivation does not match the expected address.
    InvalidPda,
    /// Seeds provided for PDA verification are invalid.
    InvalidSeeds,
    /// A `#[account(constraint = ...)]` expression evaluated to false.
    ConstraintViolation,
    /// `#[account(has_one = ...)]` field does not match.
    HasOneMismatch,
    /// Account discriminator does not match the expected value.
    InvalidDiscriminator,
    /// Account data is too small for the declared layout.
    InsufficientSpace,
    /// Account balance is below the rent-exemption minimum.
    AccountNotRentExempt,
    /// Account owner does not match the expected program.
    AccountOwnedByWrongProgram,
    /// Account was not passed as writable.
    AccountNotMutable,
    /// Account was not passed as a signer.
    AccountNotSigner,
    /// Account address does not match the expected value.
    AddressMismatch,
    /// A dynamic-length field exceeds its maximum byte length.
    DynamicFieldTooLong,
    /// More remaining accounts than can fit in the buffer.
    RemainingAccountsOverflow,
}

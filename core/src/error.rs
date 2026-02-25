use {quasar_derive::error_code, solana_program_error::ProgramError};

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
}

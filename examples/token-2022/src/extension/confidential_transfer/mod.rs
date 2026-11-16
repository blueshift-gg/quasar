// Adapted from solana-program/token-2022 at e8ca680ca8aeb12f9598524b003c776c6a4168f2.
// Modified to execute on Quasar; see ../README.md for provenance and compatibility.
#[deprecated(
    since = "9.1.0",
    note = "Use spl_token_2022_interface instead and remove spl_token_2022 as a dependency"
)]
pub use spl_token_2022_interface::extension::confidential_transfer::{
    ConfidentialTransferAccount, ConfidentialTransferMint, DecryptableBalance, EncryptedBalance,
    DEFAULT_MAXIMUM_PENDING_BALANCE_CREDIT_COUNTER, MAXIMUM_DEPOSIT_TRANSFER_AMOUNT,
    PENDING_BALANCE_LO_BIT_LENGTH,
};

/// Confidential Transfer Extension instructions
pub mod instruction;

/// Confidential Transfer Extension processor
pub mod processor;

/// Helper functions to verify zero-knowledge proofs in the Confidential
/// Transfer Extension
pub mod verify_proof;

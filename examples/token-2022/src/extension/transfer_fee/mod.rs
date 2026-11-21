// Adapted from solana-program/token-2022 at e8ca680ca8aeb12f9598524b003c776c6a4168f2.
// Modified to execute on Quasar; see ../README.md for provenance and compatibility.
/// Transfer fee extension instructions
pub mod instruction;

/// Transfer fee extension processor
pub mod processor;

#[deprecated(
    since = "9.1.0",
    note = "Use spl_token_2022_interface instead and remove spl_token_2022 as a dependency"
)]
pub use spl_token_2022_interface::extension::transfer_fee::{
    TransferFee, TransferFeeAmount, TransferFeeConfig, MAX_FEE_BASIS_POINTS,
};

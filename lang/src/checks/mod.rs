//! Compile-time account validation traits.
//!
//! These marker traits are implemented by the `#[derive(Accounts)]` macro to
//! generate runtime checks on account fields. Each trait maps to a single
//! validation: address equality, owner match, signer status, mutability,
//! or executable flag.

/// Exact account-address validation.
pub mod address;
/// Account-data length validation.
pub mod data_len;
/// Account discriminator validation.
pub mod discriminator;
/// Executable-account validation.
pub mod executable;
/// Writable-account validation.
pub mod mutable;
/// Account-owner validation.
pub mod owner;
/// Transaction-signer validation.
pub mod signer;
/// ZeroPod schema validation.
pub mod zeropod;

pub use {
    address::Address, data_len::DataLen, discriminator::Discriminator, executable::Executable,
    mutable::Mutable, owner::Owner, signer::Signer, zeropod::ZeroPod,
};

#[cfg(test)]
mod tests;

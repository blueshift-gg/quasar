//! Compile-time account validation traits.
//!
//! These marker traits are implemented by the `#[derive(Accounts)]` macro to
//! generate runtime checks on account fields. Each trait maps to a single
//! validation: address equality, owner match, signer status, mutability,
//! or executable flag.

pub mod address;
pub mod executable;
pub mod mutable;
pub mod owner;
pub mod signer;

pub use address::Address;
pub use executable::Executable;
pub use mutable::Mutable;
pub use owner::Owner;
pub use signer::Signer;

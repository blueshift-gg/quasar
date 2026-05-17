//! Account behavior modules for SPL token wrappers.
//!
//! Each module exposes its `Args`, builder, and `AccountBehavior`
//! implementation for supported wrapper types.

pub mod associated_token;
pub mod mint;
pub mod token;
pub mod token_close;
pub mod token_sweep;

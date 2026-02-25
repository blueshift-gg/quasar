//! Zero-copy Solana program framework.
//!
//! Quasar provides Anchor-compatible ergonomics with minimal compute unit overhead.
//! See [`quasar_core`] for framework primitives and the
//! [repository README](https://github.com/blueshift-gg/quasar) for a full guide.

#![no_std]

pub use quasar_core::*;
#[cfg(feature = "spl")]
pub use quasar_spl;

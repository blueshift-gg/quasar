//! Zero-copy Solana program framework.
//!
//! Quasar provides Anchor-compatible ergonomics with minimal compute unit
//! overhead. Account data is accessed through pointer casts to `#[repr(C)]`
//! companion structs — no deserialization, no heap allocation.
//!
//! # Quick start
//!
//! ```ignore
//! use quasar::prelude::*;
//!
//! #[program]
//! mod my_program {
//!     #[instruction(discriminator = 1)]
//!     pub fn initialize(ctx: Ctx<Initialize>) -> ProgramResult {
//!         let counter = ctx.accounts.counter.as_mut();
//!         counter.count = PodU64::from(0u64);
//!         Ok(())
//!     }
//! }
//! ```
//!
//! # Feature flags
//!
//! - **`spl`** (default) — SPL Token and Token-2022 integration via
//!   [`quasar_spl`]
//! - **`alloc`** — Enables `alloc`-dependent features in `quasar-core`
//!
//! # Crate structure
//!
//! This is the facade crate. It re-exports everything from [`quasar_core`] and
//! optionally [`quasar_spl`]. For detailed documentation, see:
//!
//! - [`quasar_core`] — framework primitives, traits, CPI, events
//! - [`quasar_spl`] — SPL Token CPI, token account types, metadata integration
//! - [Repository README](https://github.com/nickkuk/quasar) — full guide and
//!   examples

#![no_std]
pub use quasar_core::*;
#[cfg(feature = "spl")]
pub use quasar_spl;

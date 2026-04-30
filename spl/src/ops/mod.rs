//! Op-dispatch implementations for SPL token operations.
//!
//! Each module provides an `Op` struct implementing `AccountOp<F>` from
//! `quasar_lang::ops`. The derive macro emits UFCS calls to these ops
//! based on the `#[account(...)]` attribute syntax.

pub mod associated_token;
pub mod ata_init;
pub mod close;
pub mod mint;
pub mod realloc;
pub mod sweep;
pub mod token;

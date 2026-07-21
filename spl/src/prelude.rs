//! Convenience re-exports for SPL programs.
//!
//! ```rust
//! use quasar_lang::prelude::*;
//! use quasar_spl::prelude::*;
//! ```

pub use crate::{
    accounts::{associated_token, mint, token, token_close, token_sweep},
    instructions::TokenCpi,
    AssociatedTokenProgram, Mint, Mint2022, Token, Token2022, Token2022Program, TokenInterface,
    TokenProgram,
};

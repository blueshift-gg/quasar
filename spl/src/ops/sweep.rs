//! Token sweep — transfer all tokens out before closing.
//!
//! Token sweep behavior modules call this trait during epilogue.

use quasar_lang::prelude::*;

/// Trait for token account types that support sweep (transfer all tokens out).
pub(crate) trait TokenSweep {
    fn sweep(
        view: &AccountView,
        receiver: &AccountView,
        mint: &AccountView,
        authority: &AccountView,
        token_program: &AccountView,
    ) -> Result<(), ProgramError>;
}

//! Token close — CPI to the token program.
//!
//! Token close behavior modules call this trait during epilogue.

use quasar_lang::prelude::*;

/// Trait for token account types that can be closed via CPI.
///
/// Implemented on the behavior target (`Token`, `Token2022`). The close
/// is performed by CPI to the token program, which atomically drains
/// lamports and invalidates the account.
pub(crate) trait TokenClose {
    fn close(
        view: &mut AccountView,
        dest: &AccountView,
        authority: &AccountView,
        token_program: &AccountView,
    ) -> Result<(), ProgramError>;
}

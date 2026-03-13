use quasar_core::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

/// SPL Token `Revoke` instruction discriminator.
const REVOKE: u8 = 5;

/// Revoke a delegate's authority via CPI.
///
/// ### Accounts:
///   0. `[WRITE]` Source token account
///   1. `[SIGNER]` Source account owner
///
/// ### Instruction data (1 byte):
/// ```text
/// [0] discriminator (5)
/// ```
#[inline(always)]
pub fn revoke<'a>(
    token_program: &'a AccountView,
    source: &'a AccountView,
    authority: &'a AccountView,
) -> CpiCall<'a, 2, 1> {
    CpiCall::new(
        token_program.address(),
        [
            InstructionAccount::writable(source.address()),
            InstructionAccount::readonly_signer(authority.address()),
        ],
        [source, authority],
        [REVOKE],
    )
}

use quasar_core::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

/// SPL Token `SyncNative` instruction discriminator.
const SYNC_NATIVE: u8 = 17;

/// Sync the lamport balance of a native SOL token account via CPI.
///
/// ### Accounts:
///   0. `[WRITE]` Native SOL token account
///
/// ### Instruction data (1 byte):
/// ```text
/// [0] discriminator (17)
/// ```
#[inline(always)]
pub fn sync_native<'a>(
    token_program: &'a AccountView,
    token_account: &'a AccountView,
) -> CpiCall<'a, 1, 1> {
    CpiCall::new(
        token_program.address(),
        [InstructionAccount::writable(token_account.address())],
        [token_account],
        [SYNC_NATIVE],
    )
}

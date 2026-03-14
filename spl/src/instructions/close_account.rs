use quasar_core::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

/// SPL Token `CloseAccount` instruction discriminator.
const CLOSE_ACCOUNT: u8 = 9;

/// Close a token account and reclaim its lamports via CPI.
///
/// ### Accounts:
///   0. `[WRITE]` Account to close
///   1. `[WRITE]` Destination for remaining lamports
///   2. `[SIGNER]` Account owner / close authority
///
/// ### Instruction data (1 byte):
/// ```text
/// [0] discriminator (9)
/// ```
#[inline(always)]
pub fn close_account<'a>(
    token_program: &'a AccountView,
    account: &'a AccountView,
    destination: &'a AccountView,
    authority: &'a AccountView,
) -> CpiCall<'a, 3, 1> {
    CpiCall::new(
        token_program.address(),
        [
            InstructionAccount::writable(account.address()),
            InstructionAccount::writable(destination.address()),
            InstructionAccount::readonly_signer(authority.address()),
        ],
        [account, destination, authority],
        [CLOSE_ACCOUNT],
    )
}

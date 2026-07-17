//! SPL Token `Approve` instruction (discriminator 4).

use quasar_lang::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

/// Approve a delegate to transfer tokens via CPI.
///
/// ### Accounts:
///   0. `[WRITE]` Source token account
///   1. `[]`      Delegate
///   2. `[SIGNER]` Source account owner
///
/// ### Instruction data (9 bytes):
/// ```text
/// [0  ] discriminator (4)
/// [1..9] amount        (u64 LE)
/// ```
#[inline(always)]
pub(super) fn approve<'a>(
    token_program: &'a AccountView,
    source: &'a AccountView,
    delegate: &'a AccountView,
    authority: &'a AccountView,
    amount: u64,
) -> CpiCall<'a, 3, 9> {
    let data = super::amount_data::<4>(amount);

    CpiCall::new(
        token_program.address(),
        [
            InstructionAccount::writable(source.address()),
            InstructionAccount::readonly(delegate.address()),
            InstructionAccount::readonly_signer(authority.address()),
        ],
        [source, delegate, authority],
        data,
    )
}

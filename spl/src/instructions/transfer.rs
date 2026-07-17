//! SPL Token `Transfer` instruction (discriminator 3).

use quasar_lang::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

/// Transfer tokens between accounts via CPI.
///
/// ### Accounts:
///   0. `[WRITE]` Source token account
///   1. `[WRITE]` Destination token account
///   2. `[SIGNER]` Source account owner / delegate
///
/// ### Instruction data (9 bytes):
/// ```text
/// [0  ] discriminator (3)
/// [1..9] amount        (u64 LE)
/// ```
#[inline(always)]
pub(super) fn transfer<'a>(
    token_program: &'a AccountView,
    from: &'a AccountView,
    to: &'a AccountView,
    authority: &'a AccountView,
    amount: u64,
) -> CpiCall<'a, 3, 9> {
    let data = super::amount_data::<3>(amount);

    CpiCall::new(
        token_program.address(),
        [
            InstructionAccount::writable(from.address()),
            InstructionAccount::writable(to.address()),
            InstructionAccount::readonly_signer(authority.address()),
        ],
        [from, to, authority],
        data,
    )
}

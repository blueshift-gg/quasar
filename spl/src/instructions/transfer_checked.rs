//! SPL Token `TransferChecked` instruction (discriminator 12).

use quasar_lang::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

/// Transfer tokens with mint decimal verification via CPI.
///
/// ### Accounts:
///   0. `[WRITE]` Source token account
///   1. `[]`      Token mint
///   2. `[WRITE]` Destination token account
///   3. `[SIGNER]` Source account owner / delegate
///
/// ### Instruction data (10 bytes):
/// ```text
/// [0  ] discriminator (12)
/// [1..9] amount        (u64 LE)
/// [9  ] decimals       (u8)
/// ```
#[inline(always)]
pub(crate) fn transfer_checked<'a>(
    token_program: &'a AccountView,
    from: &'a AccountView,
    mint: &'a AccountView,
    to: &'a AccountView,
    authority: &'a AccountView,
    amount: u64,
    decimals: u8,
) -> CpiCall<'a, 4, 10> {
    let data = super::checked_amount_data::<12>(amount, decimals);

    CpiCall::new(
        token_program.address(),
        [
            InstructionAccount::writable(from.address()),
            InstructionAccount::readonly(mint.address()),
            InstructionAccount::writable(to.address()),
            InstructionAccount::readonly_signer(authority.address()),
        ],
        [from, mint, to, authority],
        data,
    )
}

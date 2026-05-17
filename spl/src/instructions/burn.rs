use quasar_lang::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

/// Burn tokens from an account via CPI.
///
/// ### Accounts:
///   0. `[WRITE]` Source token account
///   1. `[WRITE]` Token mint
///   2. `[SIGNER]` Source account owner / delegate
///
/// ### Instruction data (9 bytes):
/// ```text
/// [0  ] discriminator (8)
/// [1..9] amount        (u64 LE)
/// ```
#[inline(always)]
pub(super) fn burn<'a>(
    token_program: &'a AccountView,
    from: &'a AccountView,
    mint: &'a AccountView,
    authority: &'a AccountView,
    amount: u64,
) -> CpiCall<'a, 3, 9> {
    let data = super::amount_data::<8>(amount);

    CpiCall::new(
        token_program.address(),
        [
            InstructionAccount::writable(from.address()),
            InstructionAccount::writable(mint.address()),
            InstructionAccount::readonly_signer(authority.address()),
        ],
        [from, mint, authority],
        data,
    )
}

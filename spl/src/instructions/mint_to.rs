use quasar_lang::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

/// Mint new tokens to an account via CPI.
///
/// ### Accounts:
///   0. `[WRITE]` Mint account
///   1. `[WRITE]` Destination token account
///   2. `[SIGNER]` Mint authority
///
/// ### Instruction data (9 bytes):
/// ```text
/// [0  ] discriminator (7)
/// [1..9] amount        (u64 LE)
/// ```
#[inline(always)]
pub(super) fn mint_to<'a>(
    token_program: &'a AccountView,
    mint: &'a AccountView,
    to: &'a AccountView,
    authority: &'a AccountView,
    amount: u64,
) -> CpiCall<'a, 3, 9> {
    let data = super::amount_data::<7>(amount);

    CpiCall::new(
        token_program.address(),
        [
            InstructionAccount::writable(mint.address()),
            InstructionAccount::writable(to.address()),
            InstructionAccount::readonly_signer(authority.address()),
        ],
        [mint, to, authority],
        data,
    )
}

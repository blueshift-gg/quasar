//! SPL Token `InitializeMint2` instruction (discriminator 20).

use quasar_lang::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

/// Initialize a mint (InitializeMint2, opcode 20).
///
/// Free function variant for generated code that works with raw `AccountView`
/// references during parse-time init. Equivalent to
/// [`super::TokenCpi::initialize_mint2`].
///
/// Unlike InitializeMint, this variant does not require the Rent sysvar
/// account, saving one account in the CPI. The account must already be
/// allocated with the correct size (82 bytes).
///
/// ### Accounts:
///   0. `[WRITE]` Mint account to initialize
///
/// ### Instruction data (67 bytes):
/// ```text
/// [0    ] discriminator    (20)
/// [1    ] decimals         (u8)
/// [2..34 ] mint_authority   (32-byte address)
/// [34   ] has_freeze_auth  (u8, 0 or 1)
/// [35..67] freeze_authority (32-byte address, zeroed if absent)
/// ```
#[inline(always)]
pub(crate) fn initialize_mint2<'a>(
    token_program: &'a AccountView,
    mint: &'a AccountView,
    decimals: u8,
    mint_authority: &Address,
    freeze_authority: Option<&Address>,
) -> CpiCall<'a, 1, 67> {
    let data = super::initialize_mint2_data(decimals, mint_authority, freeze_authority);

    CpiCall::new(
        token_program.address(),
        [InstructionAccount::writable(mint.address())],
        [mint],
        data,
    )
}

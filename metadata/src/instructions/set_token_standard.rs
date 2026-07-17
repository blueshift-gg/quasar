//! Builds the Metaplex `SetTokenStandard` instruction (discriminator 35).

use quasar_lang::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

const SET_TOKEN_STANDARD: u8 = 35;

/// Set the token standard field on a metadata account.
///
/// ### Accounts:
///   0. `[WRITE]` Metadata account
///   1. `[SIGNER]` Update authority
///   2. `[]`      Token mint
///
/// ### Instruction data (1 byte):
/// ```text
/// [0] discriminator (35)
/// ```
#[inline(always)]
pub(super) fn set_token_standard<'a>(
    program: &'a AccountView,
    metadata: &'a AccountView,
    update_authority: &'a AccountView,
    mint: &'a AccountView,
) -> CpiCall<'a, 3, 1> {
    CpiCall::new(
        program.address(),
        [
            InstructionAccount::writable(metadata.address()),
            InstructionAccount::readonly_signer(update_authority.address()),
            InstructionAccount::readonly(mint.address()),
        ],
        [metadata, update_authority, mint],
        [SET_TOKEN_STANDARD],
    )
}

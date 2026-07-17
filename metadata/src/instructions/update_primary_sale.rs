//! Builds the Metaplex `UpdatePrimarySaleHappenedViaToken` instruction
//! (discriminator 4).

use quasar_lang::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

const UPDATE_PRIMARY_SALE_HAPPENED_VIA_TOKEN: u8 = 4;

/// Flip the primary-sale-happened flag, authorized by a token holder.
///
/// ### Accounts:
///   0. `[WRITE]` Metadata account
///   1. `[SIGNER]` Token account owner
///   2. `[]`      Token account proving ownership
///
/// ### Instruction data (1 byte):
/// ```text
/// [0] discriminator (4)
/// ```
#[inline(always)]
pub(super) fn update_primary_sale_happened_via_token<'a>(
    program: &'a AccountView,
    metadata: &'a AccountView,
    owner: &'a AccountView,
    token: &'a AccountView,
) -> CpiCall<'a, 3, 1> {
    CpiCall::new(
        program.address(),
        [
            InstructionAccount::writable(metadata.address()),
            InstructionAccount::readonly_signer(owner.address()),
            InstructionAccount::readonly(token.address()),
        ],
        [metadata, owner, token],
        [UPDATE_PRIMARY_SALE_HAPPENED_VIA_TOKEN],
    )
}

//! Builds the Metaplex `RemoveCreatorVerification` instruction
//! (discriminator 28).

use quasar_lang::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

const REMOVE_CREATOR_VERIFICATION: u8 = 28;

/// Clear a creator's verified flag on a metadata account.
///
/// ### Accounts:
///   0. `[SIGNER]` Creator revoking their verification
///   1. `[WRITE]` Metadata account
///
/// ### Instruction data (1 byte):
/// ```text
/// [0] discriminator (28)
/// ```
#[inline(always)]
pub(super) fn remove_creator_verification<'a>(
    program: &'a AccountView,
    creator: &'a AccountView,
    metadata: &'a AccountView,
) -> CpiCall<'a, 2, 1> {
    CpiCall::new(
        program.address(),
        [
            InstructionAccount::readonly_signer(creator.address()),
            InstructionAccount::writable(metadata.address()),
        ],
        [creator, metadata],
        [REMOVE_CREATOR_VERIFICATION],
    )
}

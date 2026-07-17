//! Builds the Metaplex `SignMetadata` instruction (discriminator 7).

use quasar_lang::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

const SIGN_METADATA: u8 = 7;

/// Mark a creator as verified on a metadata account.
///
/// ### Accounts:
///   0. `[SIGNER]` Creator signing off on the metadata
///   1. `[WRITE]` Metadata account
///
/// ### Instruction data (1 byte):
/// ```text
/// [0] discriminator (7)
/// ```
#[inline(always)]
pub(super) fn sign_metadata<'a>(
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
        [SIGN_METADATA],
    )
}

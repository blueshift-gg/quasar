//! Builds the Metaplex `RevokeCollectionAuthority` instruction
//! (discriminator 24).

use quasar_lang::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

const REVOKE_COLLECTION_AUTHORITY: u8 = 24;

/// Revoke a previously delegated collection authority.
///
/// ### Accounts:
///   0. `[WRITE]` Collection authority record PDA (closed)
///   1. `[]`      Delegated authority being revoked
///   2. `[SIGNER]` Update authority or the delegate itself
///   3. `[]`      Collection metadata account
///   4. `[]`      Collection mint
///
/// ### Instruction data (1 byte):
/// ```text
/// [0] discriminator (24)
/// ```
#[inline(always)]
pub(super) fn revoke_collection_authority<'a>(
    program: &'a AccountView,
    collection_authority_record: &'a AccountView,
    delegate_authority: &'a AccountView,
    revoke_authority: &'a AccountView,
    metadata: &'a AccountView,
    mint: &'a AccountView,
) -> CpiCall<'a, 5, 1> {
    CpiCall::new(
        program.address(),
        [
            InstructionAccount::writable(collection_authority_record.address()),
            InstructionAccount::readonly(delegate_authority.address()),
            InstructionAccount::readonly_signer(revoke_authority.address()),
            InstructionAccount::readonly(metadata.address()),
            InstructionAccount::readonly(mint.address()),
        ],
        [
            collection_authority_record,
            delegate_authority,
            revoke_authority,
            metadata,
            mint,
        ],
        [REVOKE_COLLECTION_AUTHORITY],
    )
}

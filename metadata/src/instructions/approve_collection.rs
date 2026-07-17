//! Builds the Metaplex `ApproveCollectionAuthority` instruction
//! (discriminator 23).

use quasar_lang::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

const APPROVE_COLLECTION_AUTHORITY: u8 = 23;

/// Delegate a new authority allowed to verify the collection.
///
/// ### Accounts:
///   0. `[WRITE]` Collection authority record PDA (initialized)
///   1. `[]`      New collection authority to delegate
///   2. `[SIGNER]` Collection update authority
///   3. `[WRITE, SIGNER]` Payer funding the record account
///   4. `[]`      Collection metadata account
///   5. `[]`      Collection mint
///
/// ### Instruction data (1 byte):
/// ```text
/// [0] discriminator (23)
/// ```
#[inline(always)]
pub(super) fn approve_collection_authority<'a>(
    program: &'a AccountView,
    collection_authority_record: &'a AccountView,
    new_collection_authority: &'a AccountView,
    update_authority: &'a AccountView,
    payer: &'a AccountView,
    metadata: &'a AccountView,
    mint: &'a AccountView,
) -> CpiCall<'a, 6, 1> {
    CpiCall::new(
        program.address(),
        [
            InstructionAccount::writable(collection_authority_record.address()),
            InstructionAccount::readonly(new_collection_authority.address()),
            InstructionAccount::readonly_signer(update_authority.address()),
            InstructionAccount::writable_signer(payer.address()),
            InstructionAccount::readonly(metadata.address()),
            InstructionAccount::readonly(mint.address()),
        ],
        [
            collection_authority_record,
            new_collection_authority,
            update_authority,
            payer,
            metadata,
            mint,
        ],
        [APPROVE_COLLECTION_AUTHORITY],
    )
}

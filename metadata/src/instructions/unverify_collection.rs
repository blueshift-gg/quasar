//! Builds the Metaplex `UnverifyCollection` (discriminator 22) and
//! `UnverifySizedCollectionItem` (discriminator 31) instructions.

use quasar_lang::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

const UNVERIFY_COLLECTION: u8 = 22;
const UNVERIFY_SIZED_COLLECTION_ITEM: u8 = 31;

/// Remove an item's verified membership in an unsized collection.
///
/// ### Accounts:
///   0. `[WRITE]` Item metadata account
///   1. `[SIGNER]` Collection update authority or delegate
///   2. `[]`      Collection mint
///   3. `[]`      Collection metadata account
///   4. `[]`      Collection master edition account
///
/// ### Instruction data (1 byte):
/// ```text
/// [0] discriminator (22)
/// ```
#[inline(always)]
pub(super) fn unverify_collection<'a>(
    program: &'a AccountView,
    metadata: &'a AccountView,
    collection_authority: &'a AccountView,
    collection_mint: &'a AccountView,
    collection_metadata: &'a AccountView,
    collection_master_edition: &'a AccountView,
) -> CpiCall<'a, 5, 1> {
    CpiCall::new(
        program.address(),
        [
            InstructionAccount::writable(metadata.address()),
            InstructionAccount::readonly_signer(collection_authority.address()),
            InstructionAccount::readonly(collection_mint.address()),
            InstructionAccount::readonly(collection_metadata.address()),
            InstructionAccount::readonly(collection_master_edition.address()),
        ],
        [
            metadata,
            collection_authority,
            collection_mint,
            collection_metadata,
            collection_master_edition,
        ],
        [UNVERIFY_COLLECTION],
    )
}

/// Remove an item's verified membership in a sized collection.
///
/// ### Accounts:
///   0. `[WRITE]` Item metadata account
///   1. `[SIGNER]` Collection update authority or delegate
///   2. `[WRITE, SIGNER]` Payer
///   3. `[]`      Collection mint
///   4. `[]`      Collection metadata account
///   5. `[]`      Collection master edition account
///
/// ### Instruction data (1 byte):
/// ```text
/// [0] discriminator (31)
/// ```
#[inline(always)]
pub(super) fn unverify_sized_collection_item<'a>(
    program: &'a AccountView,
    metadata: &'a AccountView,
    collection_authority: &'a AccountView,
    payer: &'a AccountView,
    collection_mint: &'a AccountView,
    collection_metadata: &'a AccountView,
    collection_master_edition: &'a AccountView,
) -> CpiCall<'a, 6, 1> {
    CpiCall::new(
        program.address(),
        [
            InstructionAccount::writable(metadata.address()),
            InstructionAccount::readonly_signer(collection_authority.address()),
            InstructionAccount::writable_signer(payer.address()),
            InstructionAccount::readonly(collection_mint.address()),
            InstructionAccount::readonly(collection_metadata.address()),
            InstructionAccount::readonly(collection_master_edition.address()),
        ],
        [
            metadata,
            collection_authority,
            payer,
            collection_mint,
            collection_metadata,
            collection_master_edition,
        ],
        [UNVERIFY_SIZED_COLLECTION_ITEM],
    )
}

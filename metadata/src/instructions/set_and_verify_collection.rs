//! Builds the Metaplex `SetAndVerifyCollection` (discriminator 25) and
//! `SetAndVerifySizedCollectionItem` (discriminator 32) instructions.

use quasar_lang::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

const SET_AND_VERIFY_COLLECTION: u8 = 25;
const SET_AND_VERIFY_SIZED_COLLECTION_ITEM: u8 = 32;

/// Set an item's collection and verify it in one unsized-collection call.
///
/// ### Accounts:
///   0. `[WRITE]` Item metadata account
///   1. `[SIGNER]` Collection update authority or delegate
///   2. `[WRITE, SIGNER]` Payer
///   3. `[]`      Update authority of the item
///   4. `[]`      Collection mint
///   5. `[]`      Collection metadata account
///   6. `[]`      Collection master edition account
///
/// ### Instruction data (1 byte):
/// ```text
/// [0] discriminator (25)
/// ```
#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub(super) fn set_and_verify_collection<'a>(
    program: &'a AccountView,
    metadata: &'a AccountView,
    collection_authority: &'a AccountView,
    payer: &'a AccountView,
    update_authority: &'a AccountView,
    collection_mint: &'a AccountView,
    collection_metadata: &'a AccountView,
    collection_master_edition: &'a AccountView,
) -> CpiCall<'a, 7, 1> {
    CpiCall::new(
        program.address(),
        [
            InstructionAccount::writable(metadata.address()),
            InstructionAccount::readonly_signer(collection_authority.address()),
            InstructionAccount::writable_signer(payer.address()),
            InstructionAccount::readonly(update_authority.address()),
            InstructionAccount::readonly(collection_mint.address()),
            InstructionAccount::readonly(collection_metadata.address()),
            InstructionAccount::readonly(collection_master_edition.address()),
        ],
        [
            metadata,
            collection_authority,
            payer,
            update_authority,
            collection_mint,
            collection_metadata,
            collection_master_edition,
        ],
        [SET_AND_VERIFY_COLLECTION],
    )
}

/// Set an item's collection and verify it in one sized-collection call.
///
/// ### Accounts:
///   0. `[WRITE]` Item metadata account
///   1. `[SIGNER]` Collection update authority or delegate
///   2. `[WRITE, SIGNER]` Payer
///   3. `[]`      Update authority of the item
///   4. `[]`      Collection mint
///   5. `[]`      Collection metadata account
///   6. `[]`      Collection master edition account
///
/// ### Instruction data (1 byte):
/// ```text
/// [0] discriminator (32)
/// ```
#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub(super) fn set_and_verify_sized_collection_item<'a>(
    program: &'a AccountView,
    metadata: &'a AccountView,
    collection_authority: &'a AccountView,
    payer: &'a AccountView,
    update_authority: &'a AccountView,
    collection_mint: &'a AccountView,
    collection_metadata: &'a AccountView,
    collection_master_edition: &'a AccountView,
) -> CpiCall<'a, 7, 1> {
    CpiCall::new(
        program.address(),
        [
            InstructionAccount::writable(metadata.address()),
            InstructionAccount::readonly_signer(collection_authority.address()),
            InstructionAccount::writable_signer(payer.address()),
            InstructionAccount::readonly(update_authority.address()),
            InstructionAccount::readonly(collection_mint.address()),
            InstructionAccount::readonly(collection_metadata.address()),
            InstructionAccount::readonly(collection_master_edition.address()),
        ],
        [
            metadata,
            collection_authority,
            payer,
            update_authority,
            collection_mint,
            collection_metadata,
            collection_master_edition,
        ],
        [SET_AND_VERIFY_SIZED_COLLECTION_ITEM],
    )
}

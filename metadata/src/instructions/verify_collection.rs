//! Builds the Metaplex `VerifyCollection` (discriminator 18) and
//! `VerifySizedCollectionItem` (discriminator 30) instructions.

use quasar_lang::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

const VERIFY_COLLECTION: u8 = 18;
const VERIFY_SIZED_COLLECTION_ITEM: u8 = 30;

/// Mark an item as a verified member of an unsized collection.
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
/// [0] discriminator (18)
/// ```
#[inline(always)]
pub(super) fn verify_collection<'a>(
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
        [VERIFY_COLLECTION],
    )
}

/// Mark an item as a verified member of a sized collection.
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
/// [0] discriminator (30)
/// ```
#[inline(always)]
pub(super) fn verify_sized_collection_item<'a>(
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
        [VERIFY_SIZED_COLLECTION_ITEM],
    )
}

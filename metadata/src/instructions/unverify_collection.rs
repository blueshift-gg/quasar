use quasar_lang::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

const UNVERIFY_COLLECTION: u8 = 22;
const UNVERIFY_SIZED_COLLECTION_ITEM: u8 = 31;

#[inline(always)]
pub fn unverify_collection<'a>(
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

#[inline(always)]
pub fn unverify_sized_collection_item<'a>(
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

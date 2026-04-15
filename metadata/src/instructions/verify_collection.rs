use quasar_lang::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

const VERIFY_COLLECTION: u8 = 18;
const VERIFY_SIZED_COLLECTION_ITEM: u8 = 30;

#[inline(always)]
pub fn verify_collection<'a>(
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

#[inline(always)]
pub fn verify_sized_collection_item<'a>(
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

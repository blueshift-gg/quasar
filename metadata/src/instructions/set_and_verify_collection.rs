use quasar_lang::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

const SET_AND_VERIFY_COLLECTION: u8 = 25;
const SET_AND_VERIFY_SIZED_COLLECTION_ITEM: u8 = 32;

#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub fn set_and_verify_collection<'a>(
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

#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub fn set_and_verify_sized_collection_item<'a>(
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

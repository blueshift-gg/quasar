use quasar_lang::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

const SET_COLLECTION_SIZE: u8 = 34;
const BUBBLEGUM_SET_COLLECTION_SIZE: u8 = 36;

#[inline(always)]
pub(super) fn set_collection_size<'a>(
    program: &'a AccountView,
    metadata: &'a AccountView,
    update_authority: &'a AccountView,
    mint: &'a AccountView,
    size: u64,
) -> CpiCall<'a, 3, 9> {
    let data = super::u64_data::<SET_COLLECTION_SIZE>(size);

    CpiCall::new(
        program.address(),
        [
            InstructionAccount::writable(metadata.address()),
            InstructionAccount::readonly_signer(update_authority.address()),
            InstructionAccount::readonly(mint.address()),
        ],
        [metadata, update_authority, mint],
        data,
    )
}

#[inline(always)]
pub(super) fn bubblegum_set_collection_size<'a>(
    program: &'a AccountView,
    metadata: &'a AccountView,
    update_authority: &'a AccountView,
    mint: &'a AccountView,
    bubblegum_signer: &'a AccountView,
    size: u64,
) -> CpiCall<'a, 4, 9> {
    let data = super::u64_data::<BUBBLEGUM_SET_COLLECTION_SIZE>(size);

    CpiCall::new(
        program.address(),
        [
            InstructionAccount::writable(metadata.address()),
            InstructionAccount::readonly_signer(update_authority.address()),
            InstructionAccount::readonly(mint.address()),
            InstructionAccount::readonly_signer(bubblegum_signer.address()),
        ],
        [metadata, update_authority, mint, bubblegum_signer],
        data,
    )
}

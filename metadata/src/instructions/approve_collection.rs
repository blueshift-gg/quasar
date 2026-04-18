use quasar_lang::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

const APPROVE_COLLECTION_AUTHORITY: u8 = 23;

#[inline(always)]
pub fn approve_collection_authority<'a>(
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

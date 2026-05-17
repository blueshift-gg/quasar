use quasar_lang::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

const CREATE_MASTER_EDITION_V3: u8 = 17;

#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn create_master_edition_v3<'a>(
    program: &'a AccountView,
    edition: &'a AccountView,
    mint: &'a AccountView,
    update_authority: &'a AccountView,
    mint_authority: &'a AccountView,
    payer: &'a AccountView,
    metadata: &'a AccountView,
    token_program: &'a AccountView,
    system_program: &'a AccountView,
    rent: &'a AccountView,
    max_supply: Option<u64>,
) -> CpiCall<'a, 9, 10> {
    let data = super::option_u64_data::<CREATE_MASTER_EDITION_V3>(max_supply);

    CpiCall::new(
        program.address(),
        [
            InstructionAccount::writable(edition.address()),
            InstructionAccount::writable(mint.address()),
            InstructionAccount::readonly_signer(update_authority.address()),
            InstructionAccount::readonly_signer(mint_authority.address()),
            InstructionAccount::writable_signer(payer.address()),
            InstructionAccount::writable(metadata.address()),
            InstructionAccount::readonly(token_program.address()),
            InstructionAccount::readonly(system_program.address()),
            InstructionAccount::readonly(rent.address()),
        ],
        [
            edition,
            mint,
            update_authority,
            mint_authority,
            payer,
            metadata,
            token_program,
            system_program,
            rent,
        ],
        data,
    )
}

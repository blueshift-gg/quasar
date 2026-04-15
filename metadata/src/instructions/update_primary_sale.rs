use quasar_lang::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

const UPDATE_PRIMARY_SALE_HAPPENED_VIA_TOKEN: u8 = 4;

#[inline(always)]
pub fn update_primary_sale_happened_via_token<'a>(
    program: &'a AccountView,
    metadata: &'a AccountView,
    owner: &'a AccountView,
    token: &'a AccountView,
) -> CpiCall<'a, 3, 1> {
    CpiCall::new(
        program.address(),
        [
            InstructionAccount::writable(metadata.address()),
            InstructionAccount::readonly_signer(owner.address()),
            InstructionAccount::readonly(token.address()),
        ],
        [metadata, owner, token],
        [UPDATE_PRIMARY_SALE_HAPPENED_VIA_TOKEN],
    )
}

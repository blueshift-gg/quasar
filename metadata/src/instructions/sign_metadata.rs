use quasar_lang::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

const SIGN_METADATA: u8 = 7;

#[inline(always)]
pub fn sign_metadata<'a>(
    program: &'a AccountView,
    creator: &'a AccountView,
    metadata: &'a AccountView,
) -> CpiCall<'a, 2, 1> {
    CpiCall::new(
        program.address(),
        [
            InstructionAccount::readonly_signer(creator.address()),
            InstructionAccount::writable(metadata.address()),
        ],
        [creator, metadata],
        [SIGN_METADATA],
    )
}

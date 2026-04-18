use quasar_lang::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

const REMOVE_CREATOR_VERIFICATION: u8 = 28;

#[inline(always)]
pub fn remove_creator_verification<'a>(
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
        [REMOVE_CREATOR_VERIFICATION],
    )
}

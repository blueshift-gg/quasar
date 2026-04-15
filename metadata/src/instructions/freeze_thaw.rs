use quasar_lang::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

const FREEZE_DELEGATED_ACCOUNT: u8 = 26;
const THAW_DELEGATED_ACCOUNT: u8 = 27;

#[inline(always)]
pub fn freeze_delegated_account<'a>(
    program: &'a AccountView,
    delegate: &'a AccountView,
    token_account: &'a AccountView,
    edition: &'a AccountView,
    mint: &'a AccountView,
    token_program: &'a AccountView,
) -> CpiCall<'a, 5, 1> {
    CpiCall::new(
        program.address(),
        [
            InstructionAccount::readonly_signer(delegate.address()),
            InstructionAccount::writable(token_account.address()),
            InstructionAccount::readonly(edition.address()),
            InstructionAccount::readonly(mint.address()),
            InstructionAccount::readonly(token_program.address()),
        ],
        [delegate, token_account, edition, mint, token_program],
        [FREEZE_DELEGATED_ACCOUNT],
    )
}

#[inline(always)]
pub fn thaw_delegated_account<'a>(
    program: &'a AccountView,
    delegate: &'a AccountView,
    token_account: &'a AccountView,
    edition: &'a AccountView,
    mint: &'a AccountView,
    token_program: &'a AccountView,
) -> CpiCall<'a, 5, 1> {
    CpiCall::new(
        program.address(),
        [
            InstructionAccount::readonly_signer(delegate.address()),
            InstructionAccount::writable(token_account.address()),
            InstructionAccount::readonly(edition.address()),
            InstructionAccount::readonly(mint.address()),
            InstructionAccount::readonly(token_program.address()),
        ],
        [delegate, token_account, edition, mint, token_program],
        [THAW_DELEGATED_ACCOUNT],
    )
}

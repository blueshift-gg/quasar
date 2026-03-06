use quasar_core::cpi::{CpiCall, InstructionAccount};
use quasar_core::prelude::*;

const CLOSE_ACCOUNT: u8 = 9;

#[inline(always)]
pub fn close_account<'a>(
    token_program: &'a AccountView,
    account: &'a AccountView,
    destination: &'a AccountView,
    authority: &'a AccountView,
) -> CpiCall<'a, 3, 1> {
    CpiCall::new(
        token_program.address(),
        [
            InstructionAccount::writable(account.address()),
            InstructionAccount::writable(destination.address()),
            InstructionAccount::readonly_signer(authority.address()),
        ],
        [account, destination, authority],
        [CLOSE_ACCOUNT],
    )
}

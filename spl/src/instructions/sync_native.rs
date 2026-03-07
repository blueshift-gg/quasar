use quasar_core::cpi::{CpiCall, InstructionAccount};
use quasar_core::prelude::*;

const SYNC_NATIVE: u8 = 17;

#[inline(always)]
pub fn sync_native<'a>(
    token_program: &'a AccountView,
    token_account: &'a AccountView,
) -> CpiCall<'a, 1, 1> {
    CpiCall::new(
        token_program.address(),
        [InstructionAccount::writable(token_account.address())],
        [token_account],
        [SYNC_NATIVE],
    )
}

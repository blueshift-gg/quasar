use quasar_core::cpi::{CpiCall, InstructionAccount};
use quasar_core::prelude::*;

const REVOKE: u8 = 5;

#[inline(always)]
pub fn revoke<'a>(
    token_program: &'a AccountView,
    source: &'a AccountView,
    authority: &'a AccountView,
) -> CpiCall<'a, 2, 1> {
    CpiCall::new(
        token_program.address(),
        [
            InstructionAccount::writable(source.address()),
            InstructionAccount::readonly_signer(authority.address()),
        ],
        [source, authority],
        [REVOKE],
    )
}

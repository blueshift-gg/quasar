use quasar_core::cpi::{CpiCall, InstructionAccount};
use quasar_core::prelude::*;

const MINT_TO: u8 = 7;

#[inline(always)]
pub fn mint_to<'a>(
    token_program: &'a AccountView,
    mint: &'a AccountView,
    to: &'a AccountView,
    authority: &'a AccountView,
    amount: u64,
) -> CpiCall<'a, 3, 9> {
    // SAFETY: All 9 bytes are written before assume_init.
    let data = unsafe {
        let mut buf = core::mem::MaybeUninit::<[u8; 9]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, MINT_TO);
        core::ptr::copy_nonoverlapping(amount.to_le_bytes().as_ptr(), ptr.add(1), 8);
        buf.assume_init()
    };

    CpiCall::new(
        token_program.address(),
        [
            InstructionAccount::writable(mint.address()),
            InstructionAccount::writable(to.address()),
            InstructionAccount::readonly_signer(authority.address()),
        ],
        [mint, to, authority],
        data,
    )
}

use quasar_core::cpi::{CpiCall, InstructionAccount};
use quasar_core::prelude::*;

const BURN: u8 = 8;

#[inline(always)]
pub fn burn<'a>(
    token_program: &'a AccountView,
    from: &'a AccountView,
    mint: &'a AccountView,
    authority: &'a AccountView,
    amount: u64,
) -> CpiCall<'a, 3, 9> {
    // SAFETY: All 9 bytes are written before assume_init.
    let data = unsafe {
        let mut buf = core::mem::MaybeUninit::<[u8; 9]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, BURN);
        core::ptr::copy_nonoverlapping(amount.to_le_bytes().as_ptr(), ptr.add(1), 8);
        buf.assume_init()
    };

    CpiCall::new(
        token_program.address(),
        [
            InstructionAccount::writable(from.address()),
            InstructionAccount::writable(mint.address()),
            InstructionAccount::readonly_signer(authority.address()),
        ],
        [from, mint, authority],
        data,
    )
}

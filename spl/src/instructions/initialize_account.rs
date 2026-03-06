use quasar_core::cpi::{CpiCall, InstructionAccount};
use quasar_core::prelude::*;

const INITIALIZE_ACCOUNT3: u8 = 18;

/// Initialize a token account (InitializeAccount3 — opcode 18).
///
/// Free function variant for generated code that works with raw `AccountView`
/// references during parse-time init. Equivalent to [`super::TokenCpi::initialize_account3`].
#[inline(always)]
#[allow(dead_code)]
pub fn initialize_account3<'a>(
    token_program: &'a AccountView,
    account: &'a AccountView,
    mint: &'a AccountView,
    owner: &Address,
) -> CpiCall<'a, 2, 33> {
    // SAFETY: All 33 bytes are written before assume_init.
    let data = unsafe {
        let mut buf = core::mem::MaybeUninit::<[u8; 33]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, INITIALIZE_ACCOUNT3);
        core::ptr::copy_nonoverlapping(owner.as_ref().as_ptr(), ptr.add(1), 32);
        buf.assume_init()
    };

    CpiCall::new(
        token_program.address(),
        [
            InstructionAccount::writable(account.address()),
            InstructionAccount::readonly(mint.address()),
        ],
        [account, mint],
        data,
    )
}

use quasar_core::cpi::{CpiCall, InstructionAccount};
use quasar_core::prelude::*;

const INITIALIZE_MINT2: u8 = 20;

/// Initialize a mint (InitializeMint2 — opcode 20).
///
/// Free function variant for generated code that works with raw `AccountView`
/// references during parse-time init. Equivalent to [`super::TokenCpi::initialize_mint2`].
#[inline(always)]
#[allow(dead_code)]
pub fn initialize_mint2<'a>(
    token_program: &'a AccountView,
    mint: &'a AccountView,
    decimals: u8,
    mint_authority: &Address,
    freeze_authority: Option<&Address>,
) -> CpiCall<'a, 1, 67> {
    let data = unsafe {
        let mut buf = core::mem::MaybeUninit::<[u8; 67]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, INITIALIZE_MINT2);
        core::ptr::write(ptr.add(1), decimals);
        core::ptr::copy_nonoverlapping(mint_authority.as_ref().as_ptr(), ptr.add(2), 32);
        match freeze_authority {
            Some(fa) => {
                core::ptr::write(ptr.add(34), 1u8);
                core::ptr::copy_nonoverlapping(fa.as_ref().as_ptr(), ptr.add(35), 32);
            }
            None => {
                core::ptr::write_bytes(ptr.add(34), 0, 33);
            }
        }
        buf.assume_init()
    };

    CpiCall::new(
        token_program.address(),
        [InstructionAccount::writable(mint.address())],
        [mint],
        data,
    )
}

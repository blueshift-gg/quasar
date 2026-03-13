use quasar_core::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

/// Burn tokens from an account via CPI.
///
/// ### Accounts:
///   0. `[WRITE]` Source token account
///   1. `[WRITE]` Token mint
///   2. `[SIGNER]` Source account owner / delegate
///
/// ### Instruction data (9 bytes):
/// ```text
/// [0  ] discriminator (8)
/// [1..9] amount        (u64 LE)
/// ```
#[inline(always)]
pub fn burn<'a>(
    token_program: &'a AccountView,
    from: &'a AccountView,
    mint: &'a AccountView,
    authority: &'a AccountView,
    amount: u64,
) -> CpiCall<'a, 3, 9> {
    // SAFETY: All 9 bytes written before `assume_init`.
    let data = unsafe {
        let mut buf = core::mem::MaybeUninit::<[u8; 9]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, 8);
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

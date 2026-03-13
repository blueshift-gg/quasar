use quasar_core::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

/// Transfer tokens with mint decimal verification via CPI.
///
/// ### Accounts:
///   0. `[WRITE]` Source token account
///   1. `[]`      Token mint
///   2. `[WRITE]` Destination token account
///   3. `[SIGNER]` Source account owner / delegate
///
/// ### Instruction data (10 bytes):
/// ```text
/// [0  ] discriminator (12)
/// [1..9] amount        (u64 LE)
/// [9  ] decimals       (u8)
/// ```
#[inline(always)]
pub fn transfer_checked<'a>(
    token_program: &'a AccountView,
    from: &'a AccountView,
    mint: &'a AccountView,
    to: &'a AccountView,
    authority: &'a AccountView,
    amount: u64,
    decimals: u8,
) -> CpiCall<'a, 4, 10> {
    // SAFETY: All 10 bytes written before `assume_init`.
    let data = unsafe {
        let mut buf = core::mem::MaybeUninit::<[u8; 10]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, 12);
        core::ptr::copy_nonoverlapping(amount.to_le_bytes().as_ptr(), ptr.add(1), 8);
        core::ptr::write(ptr.add(9), decimals);
        buf.assume_init()
    };

    CpiCall::new(
        token_program.address(),
        [
            InstructionAccount::writable(from.address()),
            InstructionAccount::readonly(mint.address()),
            InstructionAccount::writable(to.address()),
            InstructionAccount::readonly_signer(authority.address()),
        ],
        [from, mint, to, authority],
        data,
    )
}

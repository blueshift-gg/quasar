use quasar_core::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

/// Initialize a token account (InitializeAccount3 — opcode 18).
///
/// Free function variant for generated code that works with raw `AccountView`
/// references during parse-time init. Equivalent to
/// [`super::TokenCpi::initialize_account3`].
///
/// Unlike InitializeAccount/InitializeAccount2, this variant does not
/// require the Rent sysvar account, saving one account in the CPI.
/// The account must already be allocated with the correct size (165 bytes).
///
/// ### Accounts:
///   0. `[WRITE]` Token account to initialize
///   1. `[]`      Token mint
///
/// ### Instruction data (33 bytes):
/// ```text
/// [0   ] discriminator (18)
/// [1..33] owner          (32-byte address)
/// ```
#[inline(always)]
#[allow(dead_code)]
pub fn initialize_account3<'a>(
    token_program: &'a AccountView,
    account: &'a AccountView,
    mint: &'a AccountView,
    owner: &Address,
) -> CpiCall<'a, 2, 33> {
    // SAFETY: All 33 bytes written before `assume_init`.
    let data = unsafe {
        let mut buf = core::mem::MaybeUninit::<[u8; 33]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, 18);
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

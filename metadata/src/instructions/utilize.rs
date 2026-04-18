use quasar_lang::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

const UTILIZE: u8 = 19;

#[inline(always)]
pub fn utilize<'a>(
    program: &'a AccountView,
    metadata: &'a AccountView,
    token_account: &'a AccountView,
    mint: &'a AccountView,
    use_authority: &'a AccountView,
    owner: &'a AccountView,
    number_of_uses: u64,
) -> CpiCall<'a, 5, 9> {
    // SAFETY: All 9 bytes are written before assume_init.
    let data = unsafe {
        let mut buf = core::mem::MaybeUninit::<[u8; 9]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, UTILIZE);
        core::ptr::copy_nonoverlapping(number_of_uses.to_le_bytes().as_ptr(), ptr.add(1), 8);
        buf.assume_init()
    };

    CpiCall::new(
        program.address(),
        [
            InstructionAccount::writable(metadata.address()),
            InstructionAccount::writable(token_account.address()),
            InstructionAccount::writable(mint.address()),
            InstructionAccount::readonly_signer(use_authority.address()),
            InstructionAccount::readonly(owner.address()),
        ],
        [metadata, token_account, mint, use_authority, owner],
        data,
    )
}

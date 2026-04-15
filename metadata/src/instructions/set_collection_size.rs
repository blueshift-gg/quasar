use quasar_lang::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

const SET_COLLECTION_SIZE: u8 = 34;
const BUBBLEGUM_SET_COLLECTION_SIZE: u8 = 36;

#[inline(always)]
pub fn set_collection_size<'a>(
    program: &'a AccountView,
    metadata: &'a AccountView,
    update_authority: &'a AccountView,
    mint: &'a AccountView,
    size: u64,
) -> CpiCall<'a, 3, 9> {
    // SAFETY: All 9 bytes are written before assume_init.
    let data = unsafe {
        let mut buf = core::mem::MaybeUninit::<[u8; 9]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, SET_COLLECTION_SIZE);
        core::ptr::copy_nonoverlapping(size.to_le_bytes().as_ptr(), ptr.add(1), 8);
        buf.assume_init()
    };

    CpiCall::new(
        program.address(),
        [
            InstructionAccount::writable(metadata.address()),
            InstructionAccount::readonly_signer(update_authority.address()),
            InstructionAccount::readonly(mint.address()),
        ],
        [metadata, update_authority, mint],
        data,
    )
}

#[inline(always)]
pub fn bubblegum_set_collection_size<'a>(
    program: &'a AccountView,
    metadata: &'a AccountView,
    update_authority: &'a AccountView,
    mint: &'a AccountView,
    bubblegum_signer: &'a AccountView,
    size: u64,
) -> CpiCall<'a, 4, 9> {
    // SAFETY: All 9 bytes are written before assume_init.
    let data = unsafe {
        let mut buf = core::mem::MaybeUninit::<[u8; 9]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, BUBBLEGUM_SET_COLLECTION_SIZE);
        core::ptr::copy_nonoverlapping(size.to_le_bytes().as_ptr(), ptr.add(1), 8);
        buf.assume_init()
    };

    CpiCall::new(
        program.address(),
        [
            InstructionAccount::writable(metadata.address()),
            InstructionAccount::readonly_signer(update_authority.address()),
            InstructionAccount::readonly(mint.address()),
            InstructionAccount::readonly_signer(bubblegum_signer.address()),
        ],
        [metadata, update_authority, mint, bubblegum_signer],
        data,
    )
}

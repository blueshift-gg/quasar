use quasar_core::cpi::{CpiCall, InstructionAccount};
use quasar_core::prelude::*;

const CREATE_MASTER_EDITION_V3: u8 = 17;

#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub fn create_master_edition_v3<'a>(
    program: &'a AccountView,
    edition: &'a AccountView,
    mint: &'a AccountView,
    update_authority: &'a AccountView,
    mint_authority: &'a AccountView,
    payer: &'a AccountView,
    metadata: &'a AccountView,
    token_program: &'a AccountView,
    system_program: &'a AccountView,
    rent: &'a AccountView,
    max_supply: Option<u64>,
) -> CpiCall<'a, 9, 10> {
    let data = unsafe {
        let mut buf = core::mem::MaybeUninit::<[u8; 10]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, CREATE_MASTER_EDITION_V3);
        match max_supply {
            Some(v) => {
                core::ptr::write(ptr.add(1), 1u8);
                core::ptr::copy_nonoverlapping(v.to_le_bytes().as_ptr(), ptr.add(2), 8);
            }
            None => {
                core::ptr::write(ptr.add(1), 0u8);
                core::ptr::write_bytes(ptr.add(2), 0, 8);
            }
        }
        buf.assume_init()
    };

    CpiCall::new(
        program.address(),
        [
            InstructionAccount::writable(edition.address()),
            InstructionAccount::writable(mint.address()),
            InstructionAccount::readonly_signer(update_authority.address()),
            InstructionAccount::readonly_signer(mint_authority.address()),
            InstructionAccount::writable_signer(payer.address()),
            InstructionAccount::writable(metadata.address()),
            InstructionAccount::readonly(token_program.address()),
            InstructionAccount::readonly(system_program.address()),
            InstructionAccount::readonly(&super::RENT_SYSVAR),
        ],
        [
            edition,
            mint,
            update_authority,
            mint_authority,
            payer,
            metadata,
            token_program,
            system_program,
            rent,
        ],
        data,
    )
}

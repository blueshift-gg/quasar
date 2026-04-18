use quasar_lang::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

const MINT_NEW_EDITION_FROM_MASTER_EDITION_VIA_TOKEN: u8 = 11;

#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub fn mint_new_edition_from_master_edition_via_token<'a>(
    program: &'a AccountView,
    new_metadata: &'a AccountView,
    new_edition: &'a AccountView,
    master_edition: &'a AccountView,
    new_mint: &'a AccountView,
    edition_mark_pda: &'a AccountView,
    new_mint_authority: &'a AccountView,
    payer: &'a AccountView,
    token_account_owner: &'a AccountView,
    token_account: &'a AccountView,
    new_metadata_update_authority: &'a AccountView,
    metadata: &'a AccountView,
    token_program: &'a AccountView,
    system_program: &'a AccountView,
    rent: &'a AccountView,
    edition: u64,
) -> CpiCall<'a, 14, 9> {
    // SAFETY: All 9 bytes are written before assume_init.
    let data = unsafe {
        let mut buf = core::mem::MaybeUninit::<[u8; 9]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, MINT_NEW_EDITION_FROM_MASTER_EDITION_VIA_TOKEN);
        core::ptr::copy_nonoverlapping(edition.to_le_bytes().as_ptr(), ptr.add(1), 8);
        buf.assume_init()
    };

    CpiCall::new(
        program.address(),
        [
            InstructionAccount::writable(new_metadata.address()),
            InstructionAccount::writable(new_edition.address()),
            InstructionAccount::writable(master_edition.address()),
            InstructionAccount::writable(new_mint.address()),
            InstructionAccount::writable(edition_mark_pda.address()),
            InstructionAccount::readonly_signer(new_mint_authority.address()),
            InstructionAccount::writable_signer(payer.address()),
            InstructionAccount::readonly_signer(token_account_owner.address()),
            InstructionAccount::readonly(token_account.address()),
            InstructionAccount::readonly(new_metadata_update_authority.address()),
            InstructionAccount::readonly(metadata.address()),
            InstructionAccount::readonly(token_program.address()),
            InstructionAccount::readonly(system_program.address()),
            InstructionAccount::readonly(rent.address()),
        ],
        [
            new_metadata,
            new_edition,
            master_edition,
            new_mint,
            edition_mark_pda,
            new_mint_authority,
            payer,
            token_account_owner,
            token_account,
            new_metadata_update_authority,
            metadata,
            token_program,
            system_program,
            rent,
        ],
        data,
    )
}

//! Builds the Metaplex `MintNewEditionFromMasterEditionViaToken`
//! instruction (discriminator 11).

use quasar_lang::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

const MINT_NEW_EDITION_FROM_MASTER_EDITION_VIA_TOKEN: u8 = 11;

/// Print a new numbered edition from a master edition NFT.
///
/// ### Accounts:
///   0. `[WRITE]` New print edition metadata PDA
///   1. `[WRITE]` New print edition PDA
///   2. `[WRITE]` Master edition account
///   3. `[WRITE]` New edition mint
///   4. `[WRITE]` Edition marker PDA
///   5. `[SIGNER]` New mint authority
///   6. `[WRITE, SIGNER]` Payer
///   7. `[SIGNER]` Owner of the master edition token
///   8. `[]`      Token account holding the master edition
///   9. `[]`      Update authority for the new metadata
///   10. `[]`      Master edition metadata account
///   11. `[]`      SPL Token program
///   12. `[]`      System program
///   13. `[]`      Rent sysvar
///
/// ### Instruction data (9 bytes):
/// ```text
/// [0]    discriminator (11)
/// [1..9] edition (u64 LE)
/// ```
#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub(super) fn mint_new_edition_from_master_edition_via_token<'a>(
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
    let data = super::u64_data::<MINT_NEW_EDITION_FROM_MASTER_EDITION_VIA_TOKEN>(edition);

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

//! Builds the Metaplex `CreateMasterEditionV3` instruction
//! (discriminator 17).

use quasar_lang::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

const CREATE_MASTER_EDITION_V3: u8 = 17;

/// Create a master edition, marking the mint a verified 1/1 NFT.
///
/// ### Accounts:
///   0. `[WRITE]` Master edition PDA (initialized)
///   1. `[WRITE]` NFT mint (0 decimals, supply 1)
///   2. `[SIGNER]` Update authority
///   3. `[SIGNER]` Mint authority
///   4. `[WRITE, SIGNER]` Payer funding the edition account
///   5. `[WRITE]` Metadata account of the mint
///   6. `[]`      SPL Token program
///   7. `[]`      System program
///   8. `[]`      Rent sysvar
///
/// ### Instruction data (10 bytes):
/// ```text
/// [0]     discriminator (17)
/// [1]     max_supply Option tag (0 = None, 1 = Some)
/// [2..10] max_supply (u64 LE, 0 when None)
/// ```
#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn create_master_edition_v3<'a>(
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
    let data = super::option_u64_data::<CREATE_MASTER_EDITION_V3>(max_supply);

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
            InstructionAccount::readonly(rent.address()),
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

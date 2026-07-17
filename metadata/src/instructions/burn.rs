//! Builds the Metaplex `BurnNft` (discriminator 29) and `BurnEditionNft`
//! (discriminator 37) instructions.

use quasar_lang::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

const BURN_NFT: u8 = 29;
const BURN_EDITION_NFT: u8 = 37;

/// Burn an NFT and close its metadata, edition, token, and mint.
///
/// ### Accounts:
///   0. `[WRITE]` Metadata account
///   1. `[WRITE, SIGNER]` NFT owner
///   2. `[WRITE]` NFT mint
///   3. `[WRITE]` Token account holding the NFT
///   4. `[WRITE]` Master edition account
///   5. `[]`      SPL Token program
///
/// ### Instruction data (1 byte):
/// ```text
/// [0] discriminator (29)
/// ```
#[inline(always)]
pub(super) fn burn_nft<'a>(
    program: &'a AccountView,
    metadata: &'a AccountView,
    owner: &'a AccountView,
    mint: &'a AccountView,
    token: &'a AccountView,
    edition: &'a AccountView,
    spl_token: &'a AccountView,
) -> CpiCall<'a, 6, 1> {
    CpiCall::new(
        program.address(),
        [
            InstructionAccount::writable(metadata.address()),
            InstructionAccount::writable_signer(owner.address()),
            InstructionAccount::writable(mint.address()),
            InstructionAccount::writable(token.address()),
            InstructionAccount::writable(edition.address()),
            InstructionAccount::readonly(spl_token.address()),
        ],
        [metadata, owner, mint, token, edition, spl_token],
        [BURN_NFT],
    )
}

/// Burn a printed edition NFT and update its master edition.
///
/// ### Accounts:
///   0. `[WRITE]` Print edition metadata account
///   1. `[WRITE, SIGNER]` Owner of the print edition NFT
///   2. `[WRITE]` Print edition mint
///   3. `[]`      Master edition mint
///   4. `[WRITE]` Print edition token account
///   5. `[WRITE]` Master edition token account
///   6. `[WRITE]` Master edition account
///   7. `[WRITE]` Print edition account
///   8. `[WRITE]` Edition marker PDA
///   9. `[]`      SPL Token program
///
/// ### Instruction data (1 byte):
/// ```text
/// [0] discriminator (37)
/// ```
#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub(super) fn burn_edition_nft<'a>(
    program: &'a AccountView,
    metadata: &'a AccountView,
    owner: &'a AccountView,
    print_edition_mint: &'a AccountView,
    master_edition_mint: &'a AccountView,
    print_edition_token: &'a AccountView,
    master_edition_token: &'a AccountView,
    master_edition: &'a AccountView,
    print_edition: &'a AccountView,
    edition_marker: &'a AccountView,
    spl_token: &'a AccountView,
) -> CpiCall<'a, 10, 1> {
    CpiCall::new(
        program.address(),
        [
            InstructionAccount::writable(metadata.address()),
            InstructionAccount::writable_signer(owner.address()),
            InstructionAccount::writable(print_edition_mint.address()),
            InstructionAccount::readonly(master_edition_mint.address()),
            InstructionAccount::writable(print_edition_token.address()),
            InstructionAccount::writable(master_edition_token.address()),
            InstructionAccount::writable(master_edition.address()),
            InstructionAccount::writable(print_edition.address()),
            InstructionAccount::writable(edition_marker.address()),
            InstructionAccount::readonly(spl_token.address()),
        ],
        [
            metadata,
            owner,
            print_edition_mint,
            master_edition_mint,
            print_edition_token,
            master_edition_token,
            master_edition,
            print_edition,
            edition_marker,
            spl_token,
        ],
        [BURN_EDITION_NFT],
    )
}

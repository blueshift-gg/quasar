//! Builds the Metaplex `Utilize` instruction (discriminator 19).

use quasar_lang::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

const UTILIZE: u8 = 19;

/// Consume one or more uses from a utility NFT.
///
/// ### Accounts:
///   0. `[WRITE]` Metadata account
///   1. `[WRITE]` Token account holding the NFT
///   2. `[WRITE]` NFT mint
///   3. `[SIGNER]` Use authority
///   4. `[]`      NFT owner
///
/// ### Instruction data (9 bytes):
/// ```text
/// [0]    discriminator (19)
/// [1..9] number_of_uses (u64 LE)
/// ```
#[inline(always)]
pub(super) fn utilize<'a>(
    program: &'a AccountView,
    metadata: &'a AccountView,
    token_account: &'a AccountView,
    mint: &'a AccountView,
    use_authority: &'a AccountView,
    owner: &'a AccountView,
    number_of_uses: u64,
) -> CpiCall<'a, 5, 9> {
    let data = super::u64_data::<UTILIZE>(number_of_uses);

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

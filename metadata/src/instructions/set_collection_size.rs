//! Builds the Metaplex `SetCollectionSize` (discriminator 34) and
//! `BubblegumSetCollectionSize` (discriminator 36) instructions.

use quasar_lang::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

const SET_COLLECTION_SIZE: u8 = 34;
const BUBBLEGUM_SET_COLLECTION_SIZE: u8 = 36;

/// Set the recorded size of a sized collection.
///
/// ### Accounts:
///   0. `[WRITE]` Collection metadata account
///   1. `[SIGNER]` Update authority
///   2. `[]`      Collection mint
///
/// ### Instruction data (9 bytes):
/// ```text
/// [0]    discriminator (34)
/// [1..9] size (u64 LE)
/// ```
#[inline(always)]
pub(super) fn set_collection_size<'a>(
    program: &'a AccountView,
    metadata: &'a AccountView,
    update_authority: &'a AccountView,
    mint: &'a AccountView,
    size: u64,
) -> CpiCall<'a, 3, 9> {
    let data = super::u64_data::<SET_COLLECTION_SIZE>(size);

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

/// Set the collection size on behalf of the Bubblegum program.
///
/// ### Accounts:
///   0. `[WRITE]` Collection metadata account
///   1. `[SIGNER]` Update authority
///   2. `[]`      Collection mint
///   3. `[SIGNER]` Bubblegum program PDA signer
///
/// ### Instruction data (9 bytes):
/// ```text
/// [0]    discriminator (36)
/// [1..9] size (u64 LE)
/// ```
#[inline(always)]
pub(super) fn bubblegum_set_collection_size<'a>(
    program: &'a AccountView,
    metadata: &'a AccountView,
    update_authority: &'a AccountView,
    mint: &'a AccountView,
    bubblegum_signer: &'a AccountView,
    size: u64,
) -> CpiCall<'a, 4, 9> {
    let data = super::u64_data::<BUBBLEGUM_SET_COLLECTION_SIZE>(size);

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

//! PDA derivation and verification helpers for Metaplex Token Metadata.
//!
//! Canonical seeds:
//! - Metadata: `["metadata", metadata_program_id, mint]`
//! - Master Edition: `["metadata", metadata_program_id, mint, "edition"]`

use {
    crate::constants::{METADATA_PROGRAM_BYTES, METADATA_PROGRAM_ID},
    quasar_lang::__solana_program_error::ProgramError,
    solana_address::Address,
};

const METADATA_SEED: &[u8] = b"metadata";
const EDITION_SEED: &[u8] = b"edition";

#[inline(always)]
fn find_metadata_pda(seeds: &[&[u8]]) -> (Address, u8) {
    quasar_lang::pda::try_find_program_address(seeds, &METADATA_PROGRAM_ID)
        .expect("metadata PDA must be derivable")
}

#[inline(always)]
fn verify_metadata_pda(address: &Address, seeds: &[&[u8]]) -> Result<(), ProgramError> {
    quasar_lang::pda::find_bump_for_address(seeds, &METADATA_PROGRAM_ID, address)
        .map(|_| ())
        .map_err(|_| ProgramError::InvalidSeeds)
}

/// Derive the metadata PDA address from a mint.
///
/// Prefer [`verify_metadata_address`] when the caller already has an address.
#[inline(always)]
pub fn metadata_address(mint: &Address) -> (Address, u8) {
    find_metadata_pda(&[METADATA_SEED, &METADATA_PROGRAM_BYTES, mint.as_ref()])
}

/// Derive the master edition PDA address from a mint.
///
/// Prefer [`verify_master_edition_address`] when the caller already has an
/// address.
#[inline(always)]
pub fn master_edition_address(mint: &Address) -> (Address, u8) {
    find_metadata_pda(&[
        METADATA_SEED,
        &METADATA_PROGRAM_BYTES,
        mint.as_ref(),
        EDITION_SEED,
    ])
}

/// Verify a metadata address matches the expected PDA for a mint.
#[inline(always)]
pub fn verify_metadata_address(address: &Address, mint: &Address) -> Result<(), ProgramError> {
    verify_metadata_pda(
        address,
        &[METADATA_SEED, &METADATA_PROGRAM_BYTES, mint.as_ref()],
    )
}

/// Verify a master edition address matches the expected PDA for a mint.
#[inline(always)]
pub fn verify_master_edition_address(
    address: &Address,
    mint: &Address,
) -> Result<(), ProgramError> {
    verify_metadata_pda(
        address,
        &[
            METADATA_SEED,
            &METADATA_PROGRAM_BYTES,
            mint.as_ref(),
            EDITION_SEED,
        ],
    )
}

//! Canonical Metaplex PDAs, using the runtime's SHA256 syscall on chain.
use crate::METADATA_PROGRAM_ID;
use quasar_lang::{Address, ProgramError, Result, pda::Seeds};

pub fn metadata_address(mint: &Address) -> Result<(Address, u8)> {
    Seeds::new([b"metadata".as_slice(), METADATA_PROGRAM_ID.as_ref(), mint.as_ref()])
        .canonical(&METADATA_PROGRAM_ID)
}

pub fn master_edition_address(mint: &Address) -> Result<(Address, u8)> {
    Seeds::new([b"metadata".as_slice(), METADATA_PROGRAM_ID.as_ref(), mint.as_ref(), b"edition"])
        .canonical(&METADATA_PROGRAM_ID)
}

pub fn verify_metadata_address(address: &Address, mint: &Address) -> Result {
    if *address == metadata_address(mint)?.0 { Ok(()) } else { Err(ProgramError::InvalidSeeds) }
}

pub fn verify_master_edition_address(address: &Address, mint: &Address) -> Result {
    if *address == master_edition_address(mint)?.0 {
        Ok(())
    } else {
        Err(ProgramError::InvalidSeeds)
    }
}

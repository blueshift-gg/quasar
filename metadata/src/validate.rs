use crate::{METADATA_PROGRAM_ID, MasterEdition, Metadata};
use quasar_lang::{
    Address, ProgramError, Result, account::InterfaceKind, input::Entry, zeropod::Layout,
};

pub fn validate_metadata_account(
    entry: &Entry,
    mint: &Address,
    authority: Option<&Address>,
) -> Result {
    if *entry.owner() != METADATA_PROGRAM_ID {
        return Err(ProgramError::IllegalOwner);
    }
    if !Metadata::validate(entry.data()) {
        return Err(ProgramError::InvalidAccountData);
    }
    let view = Metadata::view(entry.data()).map_err(|_| ProgramError::InvalidAccountData)?;
    if view.mint() != mint
        || authority.is_some_and(|authority| view.update_authority() != authority)
    {
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(())
}

pub fn validate_master_edition_account(entry: &Entry, mint: Option<&Address>) -> Result {
    if *entry.owner() != METADATA_PROGRAM_ID {
        return Err(ProgramError::IllegalOwner);
    }
    if !MasterEdition::validate(entry.data()) {
        return Err(ProgramError::InvalidAccountData);
    }
    if let Some(mint) = mint {
        crate::pda::verify_master_edition_address(entry.key(), mint)?;
    }
    Ok(())
}

pub fn validate_metadata_program(entry: &Entry) -> Result {
    if *entry.key() != METADATA_PROGRAM_ID || !entry.executable() {
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}

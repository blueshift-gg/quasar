//! Creation helpers. Metaplex derives and allocates these PDAs itself.
//! The returned account is fully checked; mutable input wrappers may need
//! `reload()` after the call. The payer may also be a readonly authority.
use crate::{
    MasterEditionAccount, MetadataAccount, MetadataProgram,
    instructions::{self, Data},
};
use quasar_lang::{
    ProgramError, Result, input,
    pda::SignerSeeds,
    wrapper::{Program, UncheckedAccount, Wrapper},
};

/// Creates and checks a `MetadataAccount` at this unallocated PDA.
pub trait InitMetadata: Wrapper {
    #[allow(clippy::too_many_arguments)]
    fn init_metadata(
        mut self,
        program: &Program<MetadataProgram>,
        mint: &impl Wrapper,
        mint_authority: &impl Wrapper,
        payer: &impl Wrapper,
        update_authority: &impl Wrapper,
        system_program: &impl Wrapper,
        rent: &impl Wrapper,
        data: Data<'_>,
        is_mutable: bool,
        update_authority_is_signer: bool,
    ) -> Result<MetadataAccount> {
        crate::pda::verify_metadata_address(self.address(), mint.address())?;
        instructions::create_metadata_accounts_v3(
            program,
            &mut self,
            mint,
            mint_authority,
            payer,
            update_authority,
            system_program,
            rent,
            data,
            is_mutable,
            update_authority_is_signer,
        )?
        .invoke()?;
        input::existing::<MetadataAccount, _>(self).map_err(|_| ProgramError::InvalidAccountData)
    }
    #[allow(clippy::too_many_arguments)]
    fn init_metadata_signed<const S: usize>(
        mut self,
        program: &Program<MetadataProgram>,
        mint: &impl Wrapper,
        mint_authority: &impl Wrapper,
        payer: &impl Wrapper,
        update_authority: &impl Wrapper,
        system_program: &impl Wrapper,
        rent: &impl Wrapper,
        data: Data<'_>,
        is_mutable: bool,
        update_authority_is_signer: bool,
        signer: &SignerSeeds<'_, S>,
    ) -> Result<MetadataAccount> {
        crate::pda::verify_metadata_address(self.address(), mint.address())?;
        instructions::create_metadata_accounts_v3(
            program,
            &mut self,
            mint,
            mint_authority,
            payer,
            update_authority,
            system_program,
            rent,
            data,
            is_mutable,
            update_authority_is_signer,
        )?
        .invoke_signed(signer)?;
        input::existing::<MetadataAccount, _>(self).map_err(|_| ProgramError::InvalidAccountData)
    }
}

impl InitMetadata for UncheckedAccount {}

/// Creates and checks a `MasterEditionAccount` at this unallocated PDA.
pub trait InitMasterEdition: Wrapper {
    #[allow(clippy::too_many_arguments)]
    fn init_master_edition(
        mut self,
        program: &Program<MetadataProgram>,
        mint: &mut impl Wrapper,
        update_authority: &impl Wrapper,
        mint_authority: &impl Wrapper,
        payer: &impl Wrapper,
        metadata: &mut impl Wrapper,
        token_program: &impl Wrapper,
        system_program: &impl Wrapper,
        rent: &impl Wrapper,
        max_supply: Option<u64>,
    ) -> Result<MasterEditionAccount> {
        crate::pda::verify_master_edition_address(self.address(), mint.address())?;
        instructions::create_master_edition_v3(
            program,
            &mut self,
            mint,
            update_authority,
            mint_authority,
            payer,
            metadata,
            token_program,
            system_program,
            rent,
            max_supply,
        )?
        .invoke()?;
        input::existing::<MasterEditionAccount, _>(self)
            .map_err(|_| ProgramError::InvalidAccountData)
    }
    #[allow(clippy::too_many_arguments)]
    fn init_master_edition_signed<const S: usize>(
        mut self,
        program: &Program<MetadataProgram>,
        mint: &mut impl Wrapper,
        update_authority: &impl Wrapper,
        mint_authority: &impl Wrapper,
        payer: &impl Wrapper,
        metadata: &mut impl Wrapper,
        token_program: &impl Wrapper,
        system_program: &impl Wrapper,
        rent: &impl Wrapper,
        max_supply: Option<u64>,
        signer: &SignerSeeds<'_, S>,
    ) -> Result<MasterEditionAccount> {
        crate::pda::verify_master_edition_address(self.address(), mint.address())?;
        instructions::create_master_edition_v3(
            program,
            &mut self,
            mint,
            update_authority,
            mint_authority,
            payer,
            metadata,
            token_program,
            system_program,
            rent,
            max_supply,
        )?
        .invoke_signed(signer)?;
        input::existing::<MasterEditionAccount, _>(self)
            .map_err(|_| ProgramError::InvalidAccountData)
    }
}

impl InitMasterEdition for UncheckedAccount {}

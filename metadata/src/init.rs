use {super::instructions::MetadataCpi, crate::codec::BorshCpiEncode, quasar_lang::prelude::*};

/// Extension trait for metadata account initialization.
///
/// Invokes `create_metadata_accounts_v3` via CPI. The Metaplex program
/// derives and allocates the metadata PDA internally — no
/// `SystemProgram::create_account` needed from the caller.
///
/// ```ignore
/// self.metadata.init(
///     &self.metadata_program,
///     &self.mint,
///     &self.mint_authority,
///     &self.payer,
///     &self.update_authority,
///     &self.system_program,
///     "My Token",
///     "TKN",
///     "https://example.com/meta.json",
///     0,    // seller_fee_basis_points
///     true, // is_mutable
/// )?;
/// ```
pub trait InitMetadata: AsAccountView + Sized {
    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    fn init(
        &self,
        metadata_program: &impl MetadataCpi,
        mint: &impl AsAccountView,
        mint_authority: &impl AsAccountView,
        payer: &impl AsAccountView,
        update_authority: &impl AsAccountView,
        system_program: &Program<System>,
        rent: &impl AsAccountView,
        name: impl BorshCpiEncode,
        symbol: impl BorshCpiEncode,
        uri: impl BorshCpiEncode,
        seller_fee_basis_points: u16,
        is_mutable: bool,
    ) -> Result<(), ProgramError> {
        metadata_program
            .create_metadata_accounts_v3(
                self,
                mint,
                mint_authority,
                payer,
                update_authority,
                system_program,
                rent,
                name,
                symbol,
                uri,
                seller_fee_basis_points,
                is_mutable,
                true, // update_authority_is_signer
            )?
            .invoke()
    }

    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    fn init_signed(
        &self,
        metadata_program: &impl MetadataCpi,
        mint: &impl AsAccountView,
        mint_authority: &impl AsAccountView,
        payer: &impl AsAccountView,
        update_authority: &impl AsAccountView,
        system_program: &Program<System>,
        rent: &impl AsAccountView,
        name: impl BorshCpiEncode,
        symbol: impl BorshCpiEncode,
        uri: impl BorshCpiEncode,
        seller_fee_basis_points: u16,
        is_mutable: bool,
        seeds: &[Seed],
    ) -> Result<(), ProgramError> {
        metadata_program
            .create_metadata_accounts_v3(
                self,
                mint,
                mint_authority,
                payer,
                update_authority,
                system_program,
                rent,
                name,
                symbol,
                uri,
                seller_fee_basis_points,
                is_mutable,
                true,
            )?
            .invoke_signed(seeds)
    }
}

/// Extension trait for master edition account initialization.
///
/// Invokes `create_master_edition_v3` via CPI. The Metaplex program
/// derives and allocates the master edition PDA internally.
///
/// ```ignore
/// self.master_edition.init(
///     &self.metadata_program,
///     &self.mint,
///     &self.update_authority,
///     &self.mint_authority,
///     &self.payer,
///     &self.metadata,
///     &self.token_program,
///     &self.system_program,
///     Some(0), // max_supply: 0 = unique 1/1 NFT
/// )?;
/// ```
pub trait InitMasterEdition: AsAccountView + Sized {
    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    fn init(
        &self,
        metadata_program: &impl MetadataCpi,
        mint: &impl AsAccountView,
        update_authority: &impl AsAccountView,
        mint_authority: &impl AsAccountView,
        payer: &impl AsAccountView,
        metadata: &impl AsAccountView,
        token_program: &impl AsAccountView,
        system_program: &Program<System>,
        rent: &impl AsAccountView,
        max_supply: Option<u64>,
    ) -> Result<(), ProgramError> {
        metadata_program
            .create_master_edition_v3(
                self,
                mint,
                update_authority,
                mint_authority,
                payer,
                metadata,
                token_program,
                system_program,
                rent,
                max_supply,
            )
            .invoke()
    }

    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    fn init_signed(
        &self,
        metadata_program: &impl MetadataCpi,
        mint: &impl AsAccountView,
        update_authority: &impl AsAccountView,
        mint_authority: &impl AsAccountView,
        payer: &impl AsAccountView,
        metadata: &impl AsAccountView,
        token_program: &impl AsAccountView,
        system_program: &Program<System>,
        rent: &impl AsAccountView,
        max_supply: Option<u64>,
        seeds: &[Seed],
    ) -> Result<(), ProgramError> {
        metadata_program
            .create_master_edition_v3(
                self,
                mint,
                update_authority,
                mint_authority,
                payer,
                metadata,
                token_program,
                system_program,
                rent,
                max_supply,
            )
            .invoke_signed(seeds)
    }
}

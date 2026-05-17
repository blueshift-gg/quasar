use {
    quasar_lang::prelude::*,
    quasar_metadata::{accounts::metadata, prelude::*},
};

/// Initializes metadata through Metaplex CPI and verifies prefix fields.
#[derive(Accounts)]
pub struct InitMetadataTest {
    #[account(mut)]
    pub payer: Signer,
    pub metadata_program: Program<MetadataProgram>,
    pub system_program: Program<SystemProgram>,
    pub rent: Sysvar<Rent>,
    pub mint: UncheckedAccount,
    pub mint_authority: Signer,
    pub update_authority: Signer,

    #[account(
        init,
        payer = payer,
        metadata(
            program = metadata_program,
            mint = mint,
            mint_authority = mint_authority,
            update_authority = update_authority,
            system_program = system_program,
            rent = rent,
            name = "Test NFT",
            symbol = "TNFT",
            uri = "https://example.com/nft.json",
            seller_fee_basis_points = 500,
            is_mutable = true,
        )
    )]
    pub metadata: Account<MetadataAccount>,
}

impl InitMetadataTest {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        let meta = &*self.metadata;

        require!(meta.key == 4, ProgramError::InvalidAccountData);

        require_keys_eq!(
            meta.update_authority,
            *self.update_authority.to_account_view().address(),
            ProgramError::InvalidAccountData
        );

        require_keys_eq!(
            meta.mint,
            *self.mint.to_account_view().address(),
            ProgramError::InvalidAccountData
        );

        Ok(())
    }
}

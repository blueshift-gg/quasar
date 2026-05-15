use {
    quasar_lang::prelude::*,
    quasar_metadata::{
        accounts::{master_edition, metadata},
        prelude::*,
    },
    quasar_spl::prelude::*,
};

/// Initializes metadata and master edition through derive behaviors, then
/// verifies prefix fields on both accounts.
///
/// Metadata is declared before master_edition because init runs in field order.
#[derive(Accounts)]
pub struct InitMasterEditionTest {
    #[account(mut)]
    pub payer: Signer,
    pub metadata_program: Program<MetadataProgram>,
    pub system_program: Program<SystemProgram>,
    pub rent: Sysvar<Rent>,
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub update_authority: Signer,
    pub mint_authority: Signer,
    pub token_program: Program<TokenProgram>,

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
    pub metadata_account: Account<MetadataAccount>,

    #[account(
        init,
        payer = payer,
        master_edition(
            program = metadata_program,
            mint = mint,
            update_authority = update_authority,
            mint_authority = mint_authority,
            metadata = metadata_account,
            token_program = token_program,
            system_program = system_program,
            rent = rent,
            max_supply = Some(0),
        )
    )]
    pub master_edition: Account<MasterEditionAccount>,
}

impl InitMasterEditionTest {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        let meta = &*self.metadata_account;
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

        let me = &*self.master_edition;
        require!(me.key == 6, ProgramError::InvalidAccountData);
        require!(me.supply_value() == 0, ProgramError::InvalidAccountData);
        match me.max_supply_value() {
            Some(0) => {}
            _ => return Err(ProgramError::InvalidAccountData),
        }

        Ok(())
    }
}

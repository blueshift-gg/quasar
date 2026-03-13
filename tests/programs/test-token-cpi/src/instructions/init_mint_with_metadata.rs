use {
    quasar_core::prelude::*,
    quasar_spl::{metadata::MetadataProgram, Mint, Token},
};

#[derive(Accounts)]
pub struct InitMintWithMetadata<'info> {
    pub payer: &'info mut Signer,
    pub mint_authority: &'info Signer,
    #[account(
        init,
        mint::decimals = 0,
        mint::authority = mint_authority,
        metadata::name = b"Test NFT",
        metadata::symbol = b"TNFT",
        metadata::uri = b"https://example.com/nft.json",
        metadata::seller_fee_basis_points = 500,
        metadata::is_mutable = true,
    )]
    pub mint: &'info mut Account<Mint>,
    pub metadata: &'info mut UncheckedAccount,
    pub metadata_program: &'info Program<MetadataProgram>,
    pub token_program: &'info Program<Token>,
    pub system_program: &'info Program<System>,
    pub rent: &'info UncheckedAccount,
}

impl<'info> InitMintWithMetadata<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}

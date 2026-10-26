//! SBF consumer of the Metaplex account constraints and creation helpers.
#![no_std]
use quasar_lang::prelude::*;
use quasar_metadata::prelude::*;

declare_id!("11111111111111111111111111111113");

#[derive(Accounts)]
pub struct Read {
    pub mint: UncheckedAccount,
    pub authority: Signer,
    #[account(metadata(mint = mint, update_authority = authority))]
    pub metadata: MetadataAccount,
    #[account(master_edition(mint = mint, metadata = metadata))]
    pub edition: MasterEditionAccount,
}

#[derive(Accounts)]
pub struct Create {
    pub metadata_program: Program<MetadataProgram>,
    pub system_program: Program<SystemProgram>,
    pub token_program: UncheckedAccount,
    pub rent: Sysvar<Rent>,
    #[account(mut)]
    pub authority: Signer,
    #[account(mut)]
    pub mint: UncheckedAccount,
    #[account(mut)]
    pub metadata: UncheckedAccount,
    #[account(mut)]
    pub edition: UncheckedAccount,
}

#[program]
mod metaplex {
    use super::*;

    #[instruction(0)]
    pub fn read(ctx: Ctx<Read>) -> Result<u64> {
        Ok(ctx.accounts.edition.supply())
    }

    #[instruction(1)]
    pub fn create(ctx: Ctx<Create>) -> Result {
        let Create {
            metadata_program,
            system_program,
            token_program,
            rent,
            authority,
            mut mint,
            metadata,
            edition,
        } = ctx.accounts;
        let data =
            instructions::Data { name: "Quasar", symbol: "Q", uri: "", seller_fee_basis_points: 0 };
        let mut metadata = metadata.init_metadata(
            &metadata_program,
            &mint,
            &authority,
            &authority,
            &authority,
            &system_program,
            &rent,
            data,
            true,
            true,
        )?;
        let _edition = edition.init_master_edition(
            &metadata_program,
            &mut mint,
            &authority,
            &authority,
            &authority,
            &mut metadata,
            &token_program,
            &system_program,
            &rent,
            Some(0),
        )?;
        metadata.reload()?;
        Ok(())
    }
}

use quasar_lang::prelude::*;
use quasar_metadata::prelude::*;

declare_id!("11111111111111111111111111111112");

#[derive(Accounts)]
pub struct Read {
    pub mint: UncheckedAccount,
    pub authority: Signer,
    pub metadata_program: Program<MetadataProgram>,
    #[account(metadata(program = metadata_program, mint = mint, update_authority = authority))]
    pub metadata: MetadataAccount,
    #[account(master_edition(program = metadata_program, mint = mint, metadata = metadata))]
    pub edition: MasterEditionAccount,
}

#[allow(dead_code, clippy::too_many_arguments)]
fn create(
    metadata: UncheckedAccount,
    edition: UncheckedAccount,
    program: &Program<MetadataProgram>,
    mint: &mut UncheckedAccount,
    authority: &Signer,
    system: &Program<SystemProgram>,
    token: &UncheckedAccount,
    rent: &UncheckedAccount,
) -> Result {
    let data = instructions::Data { name: "NFT", symbol: "N", uri: "", seller_fee_basis_points: 0 };
    // One signer can pay and serve as both authorities.
    let mut metadata = metadata.init_metadata(program, mint, authority, authority, authority, system, rent, data, true, true)?;
    let _edition = edition.init_master_edition(program, mint, authority, authority, authority, &mut metadata, token, system, rent, Some(0))?;
    metadata.reload()?;
    Ok(())
}

fn main() {}

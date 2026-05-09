#![allow(unexpected_cfgs)]

use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 1)]
pub struct Vault {
    pub authority: Address,
    pub count: PodU64,
}

#[derive(Accounts)]
pub struct InitVault {
    #[account(mut)]
    pub payer: Signer,
    pub vault: Uninit<Account<Vault>>,
}

#[program]
pub mod test_uninit {
    use super::*;

    #[instruction(discriminator = 1)]
    pub fn init_vault(ctx: Ctx<InitVault>) -> Result<(), ProgramError> {
        let vault = ctx.accounts.vault.init(
            &ctx.accounts.payer,
            VaultData {
                authority: *ctx.accounts.payer.address(),
                count: PodU64::from(1),
            },
        )?;
        vault.count = PodU64::from(2);
        Ok(())
    }
}

fn main() {}

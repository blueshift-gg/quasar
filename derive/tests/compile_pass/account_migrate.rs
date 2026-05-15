#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;
solana_address::declare_id!("11111111111111111111111111111112");
#[account(discriminator = 1)]
pub struct ConfigV1 {
    pub authority: Address,
    pub value: PodU64,
}
#[account(discriminator = 2)]
pub struct ConfigV2 {
    pub authority: Address,
    pub value: PodU64,
    pub new_field: PodU32,
}
#[account(discriminator = 3)]
pub struct ConfigV2Slim {
    pub authority: Address,
    pub value: PodU64,
}
#[account(discriminator = 4)]
pub struct ConfigV1Big {
    pub authority: Address,
    pub value: PodU64,
    pub obsolete: PodU32,
}
#[account(discriminator = 10)]
#[seeds(b"vault", authority: Address)]
pub struct VaultV1 {
    pub authority: Address,
    pub balance: PodU64,
    pub bump: u8,
}
#[account(discriminator = 11)]
pub struct VaultV2 {
    pub authority: Address,
    pub balance: PodU64,
    pub fee_bps: PodU16,
    pub bump: u8,
}
/// Basic grow migration (V1 to V2).
#[derive(Accounts)]
pub struct MigrateGrow {
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<SystemProgram>,
    #[account(constraints(config.authority == *authority.address()))]
    pub config: Migration<ConfigV1, ConfigV2>,
    pub authority: Signer,
}
/// Same-size migration (V1 to V2Slim).
#[derive(Accounts)]
pub struct MigrateSameSize {
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<SystemProgram>,
    pub config: Migration<ConfigV1, ConfigV2Slim>,
}
/// Shrink migration (V1Big to V2Slim).
#[derive(Accounts)]
pub struct MigrateShrink {
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<SystemProgram>,
    pub config: Migration<ConfigV1Big, ConfigV2Slim>,
}
/// PDA migration with seeds and bump.
#[derive(Accounts)]
pub struct MigrateVault {
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<SystemProgram>,
    #[account(
        constraints(vault.authority == *authority.address()),
        address = VaultV1::seeds(authority.address()),
    )]
    pub vault: Migration<VaultV1, VaultV2>,
    pub authority: Signer,
}
/// Non-default payer name.
#[derive(Accounts)]
pub struct MigrateWithFunder {
    #[account(mut)]
    pub funder: Signer,
    pub system_program: Program<SystemProgram>,
    pub config: Migration<ConfigV1, ConfigV2>,
}
#[program]
pub mod test_migrate {
    use super::*;
    #[instruction(discriminator = 1)]
    pub fn migrate_grow(ctx: Ctx<MigrateGrow>) -> Result<(), ProgramError> {
        let val = ctx.accounts.config.value;
        let auth = ctx.accounts.config.authority;
        ctx.accounts.config.migrate(&ctx.accounts.payer, ConfigV2Data {
            authority: auth, value: val, new_field: PodU32::from(0),
        })?;
        Ok(())
    }
    #[instruction(discriminator = 2)]
    pub fn migrate_same_size(ctx: Ctx<MigrateSameSize>) -> Result<(), ProgramError> {
        let val = ctx.accounts.config.value;
        let auth = ctx.accounts.config.authority;
        ctx.accounts.config.migrate(&ctx.accounts.payer, ConfigV2SlimData {
            authority: auth, value: val,
        })?;
        Ok(())
    }
    #[instruction(discriminator = 3)]
    pub fn migrate_shrink(ctx: Ctx<MigrateShrink>) -> Result<(), ProgramError> {
        let val = ctx.accounts.config.value;
        let auth = ctx.accounts.config.authority;
        ctx.accounts.config.migrate(&ctx.accounts.payer, ConfigV2SlimData {
            authority: auth, value: val,
        })?;
        Ok(())
    }
    #[instruction(discriminator = 4)]
    pub fn migrate_vault(ctx: Ctx<MigrateVault>) -> Result<(), ProgramError> {
        let bal = ctx.accounts.vault.balance;
        let auth = ctx.accounts.vault.authority;
        let bump = ctx.accounts.vault.bump;
        ctx.accounts.vault.migrate(&ctx.accounts.payer, VaultV2Data {
            authority: auth, balance: bal, fee_bps: PodU16::from(30), bump,
        })?;
        Ok(())
    }
    #[instruction(discriminator = 5)]
    pub fn migrate_with_funder(ctx: Ctx<MigrateWithFunder>) -> Result<(), ProgramError> {
        let val = ctx.accounts.config.value;
        let auth = ctx.accounts.config.authority;
        ctx.accounts.config.migrate(&ctx.accounts.funder, ConfigV2Data {
            authority: auth, value: val, new_field: PodU32::from(0),
        })?;
        Ok(())
    }
}
fn main() {}

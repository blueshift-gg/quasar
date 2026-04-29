#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

// ---------------------------------------------------------------------------
// V1: original layout (41 bytes: 1 disc + 32 addr + 8 u64)
// ---------------------------------------------------------------------------

#[account(discriminator = 1)]
pub struct ConfigV1 {
    pub authority: Address,
    pub value: PodU64,
}

// ---------------------------------------------------------------------------
// V2: larger (45 bytes: 1 disc + 32 addr + 8 u64 + 4 u32)
// ---------------------------------------------------------------------------

#[account(discriminator = 2)]
pub struct ConfigV2 {
    pub authority: Address,
    pub value: PodU64,
    pub new_field: PodU32,
}

impl Migrate<ConfigV2Data> for ConfigV1Data {
    fn migrate(&self) -> ConfigV2Data {
        ConfigV2Data {
            authority: self.authority,
            value: self.value,
            new_field: PodU32::from(0),
        }
    }
}

// ---------------------------------------------------------------------------
// V2slim: same size as V1 (41 bytes: 1 disc + 32 addr + 8 u64)
// Different disc, same layout — pure disc swap migration.
// ---------------------------------------------------------------------------

#[account(discriminator = 3)]
pub struct ConfigV2Slim {
    pub authority: Address,
    pub value: PodU64,
}

impl Migrate<ConfigV2SlimData> for ConfigV1Data {
    fn migrate(&self) -> ConfigV2SlimData {
        ConfigV2SlimData {
            authority: self.authority,
            value: self.value,
        }
    }
}

// ---------------------------------------------------------------------------
// V1Big: larger than V2Slim (shrink migration)
// 45 bytes: 1 disc + 32 addr + 8 u64 + 4 u32
// ---------------------------------------------------------------------------

#[account(discriminator = 4)]
pub struct ConfigV1Big {
    pub authority: Address,
    pub value: PodU64,
    pub obsolete: PodU32,
}

impl Migrate<ConfigV2SlimData> for ConfigV1BigData {
    fn migrate(&self) -> ConfigV2SlimData {
        ConfigV2SlimData {
            authority: self.authority,
            value: self.value,
        }
    }
}

// ---------------------------------------------------------------------------
// PDA account with seeds + bump
// ---------------------------------------------------------------------------

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

impl Migrate<VaultV2Data> for VaultV1Data {
    fn migrate(&self) -> VaultV2Data {
        VaultV2Data {
            authority: self.authority,
            balance: self.balance,
            fee_bps: PodU16::from(30),
            bump: self.bump,
        }
    }
}

// ---------------------------------------------------------------------------
// Accounts structs — using Migration<From, To> field type
// ---------------------------------------------------------------------------

/// Basic grow migration (V1 → V2)
#[derive(Accounts)]
pub struct MigrateGrow {
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<System>,

    #[account(payer = payer, has_one = authority)]
    pub config: Migration<ConfigV1, ConfigV2>,

    /// CHECK: authority
    pub authority: Signer,
}

/// Same-size migration (V1 → V2Slim, disc swap only)
#[derive(Accounts)]
pub struct MigrateSameSize {
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<System>,

    #[account(payer = payer)]
    pub config: Migration<ConfigV1, ConfigV2Slim>,
}

/// Shrink migration (V1Big → V2Slim, target smaller)
#[derive(Accounts)]
pub struct MigrateShrink {
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<System>,

    #[account(payer = payer)]
    pub config: Migration<ConfigV1Big, ConfigV2Slim>,
}

/// PDA migration with seeds + bump
#[derive(Accounts)]
pub struct MigrateVault {
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<System>,

    #[account(
        payer = payer,
        has_one = authority,
        seeds = VaultV1::seeds(authority),
        bump,
    )]
    pub vault: Migration<VaultV1, VaultV2>,

    /// CHECK: authority
    pub authority: Signer,
}

/// Non-default payer name (payer = funder, not payer = payer)
#[derive(Accounts)]
pub struct MigrateWithFunder {
    #[account(mut)]
    pub funder: Signer,
    pub system_program: Program<System>,

    #[account(payer = funder)]
    pub config: Migration<ConfigV1, ConfigV2>,
}

// ---------------------------------------------------------------------------
// Program
// ---------------------------------------------------------------------------

#[program]
pub mod test_migrate {
    use super::*;

    #[instruction(discriminator = 1)]
    pub fn migrate_grow(ctx: Ctx<MigrateGrow>) -> Result<(), ProgramError> {
        let _val: u64 = ctx.accounts.config.source().unwrap().value.into();
        Ok(())
    }

    #[instruction(discriminator = 2)]
    pub fn migrate_same_size(ctx: Ctx<MigrateSameSize>) -> Result<(), ProgramError> {
        let _val: u64 = ctx.accounts.config.source().unwrap().value.into();
        Ok(())
    }

    #[instruction(discriminator = 3)]
    pub fn migrate_shrink(ctx: Ctx<MigrateShrink>) -> Result<(), ProgramError> {
        let _val: u64 = ctx.accounts.config.source().unwrap().value.into();
        Ok(())
    }

    #[instruction(discriminator = 4)]
    pub fn migrate_vault(ctx: Ctx<MigrateVault>) -> Result<(), ProgramError> {
        let _val: u64 = ctx.accounts.vault.source().unwrap().balance.into();
        Ok(())
    }

    #[instruction(discriminator = 5)]
    pub fn migrate_with_funder(ctx: Ctx<MigrateWithFunder>) -> Result<(), ProgramError> {
        let _val: u64 = ctx.accounts.config.source().unwrap().value.into();
        Ok(())
    }
}

fn main() {}

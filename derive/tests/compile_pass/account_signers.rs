//! Generated context signer seeds.
#![allow(unexpected_cfgs)]
extern crate alloc;

use quasar_derive::Accounts;
use quasar_lang::{cpi::CpiSignerSeeds, prelude::*};

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 8)]
#[seeds(b"vault", authority: Address)]
pub struct Vault {
    pub authority: Address,
    pub bump: u8,
}

#[derive(Accounts)]
pub struct VaultBundle {
    pub authority: Signer,

    #[account(mut, address = Vault::seeds(authority.address()))]
    pub vault: Account<Vault>,
}

#[derive(Accounts)]
pub struct VaultAccounts {
    #[account(group)]
    pub vault_bundle: VaultBundle,
}

#[derive(Accounts)]
pub struct UsesNestedVault {
    #[account(group)]
    pub vault_bundle: VaultBundle,
}

fn assert_cpi_signer<S: CpiSignerSeeds + ?Sized>(_seeds: &S) {}

fn use_flat(ctx: Ctx<VaultAccounts>) {
    let vault_signer = ctx
        .accounts
        .vault_bundle
        .vault_signer(&ctx.bumps.vault_bundle);
    assert_cpi_signer(&vault_signer);
}

fn use_nested(ctx: Ctx<UsesNestedVault>) {
    let vault_signer = ctx
        .accounts
        .vault_bundle
        .vault_signer(&ctx.bumps.vault_bundle);
    assert_cpi_signer(&vault_signer);
}

fn main() {}

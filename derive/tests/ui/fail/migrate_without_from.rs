use quasar_lang::prelude::*;

declare_id!("FaNXqQYPePivF3hVJ3ZdM74zzMa9P4QU7HX7t6RrDygg");

#[account(discriminator = 1)]
pub struct VaultV1 {
    pub amount: u64,
}

#[account(discriminator = 2)]
pub struct VaultV2 {
    pub amount: u64,
    pub locked: bool,
}

#[derive(Accounts)]
pub struct Read {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut, migrate(payer = payer))]
    pub vault: Account<VaultV2>,
    pub system_program: Program<SystemProgram>,
}

fn main() {}

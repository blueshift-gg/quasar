use quasar_lang::prelude::*;

declare_id!("FaNXqQYPePivF3hVJ3ZdM74zzMa9P4QU7HX7t6RrDygg");

#[account(discriminator = 1)]
pub struct Vault {
    pub amount: u64,
}

#[derive(Accounts)]
pub struct Withdraw {
    #[account(check(vault.amount() >= data.amount))]
    pub vault: Account<Vault>,
}

fn main() {}

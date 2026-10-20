use quasar_lang::prelude::*;

declare_id!("FaNXqQYPePivF3hVJ3ZdM74zzMa9P4QU7HX7t6RrDygg");

#[account(discriminator = 1)]
#[seeds(b"vault", depositor: Address)]
pub struct Vault {
    pub depositor: Address,
}

#[derive(Accounts)]
pub struct Withdraw {
    pub depositor: Signer,
    #[account(seeds(depositor))]
    pub vault: Account<Vault>,
}

fn main() {}

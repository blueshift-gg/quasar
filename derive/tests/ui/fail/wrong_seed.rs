use quasar_lang::prelude::*;

declare_id!("FaNXqQYPePivF3hVJ3ZdM74zzMa9P4QU7HX7t6RrDygg");

#[account(discriminator = 1)]
#[seeds(b"vault", depositor: Address)]
pub struct Vault {
    pub depositor: Address,
}

#[derive(InstructionData)]
pub struct Amount {
    pub amount: u64,
}

#[derive(Accounts)]
#[data(Amount)]
pub struct Withdraw {
    pub depositor: Signer,
    #[account(seeds(data.amount))]
    pub vault: Pda<Account<Vault>>,
}

fn main() {}

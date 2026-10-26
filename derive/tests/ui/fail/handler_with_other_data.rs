use quasar_lang::prelude::*;

declare_id!("FaNXqQYPePivF3hVJ3ZdM74zzMa9P4QU7HX7t6RrDygg");

#[derive(InstructionData)]
pub struct Amount {
    pub amount: u64,
}

#[derive(InstructionData)]
pub struct Other {
    pub other: u64,
}

#[account(discriminator = 1)]
pub struct Vault {
    pub amount: u64,
}

#[derive(Accounts)]
#[data(Amount)]
pub struct Withdraw {
    #[account(check(vault.amount() >= data.amount))]
    pub vault: Account<Vault>,
}

#[instruction(0)]
pub fn withdraw(_ctx: Ctx<Withdraw, Other>) -> Result {
    Ok(())
}

fn main() {}

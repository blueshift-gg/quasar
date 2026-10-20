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
    pub vault: Pda<Account<Vault>>,
}
#[program]
mod vaults {
    use super::*;

    #[instruction(0)]
    pub fn withdraw(_ctx: Ctx<Withdraw>) -> Result {
        Ok(())
    }
}

fn main() {}

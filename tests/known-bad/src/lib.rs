//! A vault with its `has_one(depositor)` removed, and the property that
//! catches it: once property proofs run in `make kani`, they must fail here,
//! on that property, or they prove nothing.
#![no_std]

use quasar_lang::prelude::*;

declare_id!("KnownBad11111111111111111111111111111111111");

#[account(discriminator = 1)]
#[seeds(b"vault", depositor: Address)]
pub struct Vault {
    pub depositor: Address,
    pub bump: u8,
    pub amount: u64,
}

#[derive(Accounts)]
pub struct Withdraw {
    #[account(mut)]
    pub depositor: Signer,
    #[account(mut)]
    pub vault: Account<Vault>,
}

#[derive(InstructionData)]
pub struct Amount {
    pub amount: u64,
}

/// Only the depositor can take lamports out of their vault.
#[property(known_bad::withdraw)]
fn only_the_depositor_withdraws(before: &Withdraw, after: &Withdraw, _data: &Amount) -> bool {
    after.vault.lamports() >= before.vault.lamports()
        || before.vault.depositor() == before.depositor.address()
}
#[program]
mod known_bad {
    use super::*;

    #[instruction(0)]
    pub fn withdraw(mut ctx: Ctx<Withdraw, Amount>) -> Result<()> {
        let Withdraw { depositor, vault } = &mut ctx.accounts;
        vault.sub_lamports(ctx.data.amount)?;
        depositor.add_lamports(ctx.data.amount)
    }
}

#![allow(unexpected_cfgs)]

use quasar_lang::prelude::*;

declare_id!("22222222222222222222222222222222222222222222");

#[account(discriminator = 1)]
pub struct Counter {
    pub authority: Address,
    pub count: u64,
}

#[derive(Accounts)]
pub struct Increment {
    #[account(mut, has_one(authority))]
    pub counter: Account<Counter>,
    pub authority: Signer,
}

#[program]
mod counter_program {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn increment(ctx: Ctx<Increment>) -> Result<(), ProgramError> {
        ctx.accounts.counter.count += 1;
        Ok(())
    }
}
fn main() {}

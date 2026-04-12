#![no_std]

use quasar_lang::prelude::*;

mod errors;
mod instructions;
use instructions::*;
pub mod pyth;
#[cfg(test)]
mod tests;

declare_id!("55555555555555555555555555555555555555555555");

#[program]
mod quasar_pyth_oracle {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn check_price(ctx: Ctx<CheckPrice>) -> Result<(), ProgramError> {
        ctx.accounts.check_price()
    }
}

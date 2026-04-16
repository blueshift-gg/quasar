#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[derive(Accounts)]
pub struct Update {
    pub authority: Signer,
}

#[program]
mod test_program {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn update(
        _ctx: Ctx<Update>,
        amount: u64,
        label: String<8>,
        flag: u8,
    ) -> Result<(), ProgramError> {
        let _ = (amount, label, flag);
        Ok(())
    }
}

fn main() {}

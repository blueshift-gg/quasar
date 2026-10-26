#![deny(unused_must_use)]

use quasar_lang::prelude::*;

declare_id!("FaNXqQYPePivF3hVJ3ZdM74zzMa9P4QU7HX7t6RrDygg");

#[derive(InstructionData)]
pub struct Amount {
    pub lamports: u64,
}

#[derive(Accounts)]
pub struct Fund {
    #[account(mut)]
    pub from: Signer,
    #[account(mut)]
    pub to: UncheckedAccount,
    pub system_program: Program<SystemProgram>,
}
#[program]
mod funding {
    use super::*;

    #[instruction(0)]
    pub fn fund(mut ctx: Ctx<Fund, Amount>) -> Result<()> {
        let Fund { from, to, system_program } = &mut ctx.accounts;
        system_program.transfer(from, to, ctx.data.lamports);
        Ok(())
    }
}

fn main() {}

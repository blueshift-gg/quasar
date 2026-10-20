//! The expansion must not depend on names a program can shadow.
#![no_std]
use quasar_lang::prelude::*;

#[allow(dead_code)]
struct Ok;
#[allow(dead_code)]
struct Option;
#[allow(dead_code)]
struct Vec;
#[allow(dead_code)]
mod core {}

declare_id!("FaNXqQYPePivF3hVJ3ZdM74zzMa9P4QU7HX7t6RrDygg");

#[account(discriminator = 1)]
#[seeds(b"state", user: Address, seed: u64)]
pub struct State {
    pub value: u64,
    pub bump: u8,
}

#[derive(InstructionData)]
pub struct Value {
    pub value: u64,
    pub seed: u64,
}

#[derive(Accounts)]
#[data(Value)]
pub struct Greet {
    #[account(mut)]
    pub user: Signer,
    #[account(init(payer = user), seeds(user, data.seed))]
    pub state: Pda<Account<State>>,
    pub rent: Sysvar<Rent>,
    pub system_program: Program<SystemProgram>,
}
#[program]
mod hostile {
    use super::*;

    #[instruction(0)]
    pub fn greet(mut ctx: Ctx<Greet, Value>) -> Result<()> {
        let state = &mut ctx.accounts.state;
        state.set_inner(&State { value: ctx.data.value, bump: state.bump() })?;
        Result::Ok(())
    }
}

fn main() {}
